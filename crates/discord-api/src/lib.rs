// Direct, origin-fixed REST adapter. No cookies, redirects, logging, persistence or bot SDK.
use client_core::{
    Command, Event,
    auth::{AuthProvider, Failure, SessionSecret},
};
use discord_protocol::*;
use model::User;
use reqwest::{
    Client, Method, StatusCode,
    header::{AUTHORIZATION, HeaderValue},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::Mutex,
    time::{Instant, sleep_until},
};

pub struct DiscordApi {
    client: Client,
    secret: Arc<SessionSecret>,
    cooldown: Mutex<Instant>,
    stopped: AtomicBool,
    #[cfg(test)]
    base: String,
}
impl DiscordApi {
    pub fn new(secret: Arc<SessionSecret>) -> Result<Self, Failure> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(10))
            .user_agent("Serein/0.1 (unofficial native client)")
            .build()
            .map_err(|_| Failure::Network)?;
        Ok(Self {
            client,
            secret,
            cooldown: Mutex::new(Instant::now()),
            stopped: AtomicBool::new(false),
            #[cfg(test)]
            base: "https://discord.com/api/v10".into(),
        })
    }
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }
    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }
    async fn request(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<Vec<u8>, Failure> {
        // Only typed adapter methods construct paths. Never accept a URL or route from UI/content.
        if !path.starts_with('/')
            || path.contains("://")
            || path.contains('\\')
            || path.contains("..")
        {
            return Err(Failure::Protocol);
        }
        // ponytail: serialize REST and conservatively share every service cooldown; split by bucket only if measured latency requires it.
        let mut next = self.cooldown.lock().await;
        sleep_until(*next).await;
        if self.stopped() {
            return Err(Failure::Expired);
        }
        let mut authorization =
            HeaderValue::from_str(self.secret.expose()).map_err(|_| Failure::InvalidCredential)?;
        authorization.set_sensitive(true);
        #[cfg(not(test))]
        let base = "https://discord.com/api/v10";
        #[cfg(test)]
        let base = &self.base;
        let write = method != Method::GET;
        let mut request = self
            .client
            .request(method, format!("{base}{path}"))
            .header(AUTHORIZATION, authorization);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let mut response = request.send().await.map_err(|_| {
            if write {
                Failure::Ambiguous
            } else {
                Failure::Network
            }
        })?;
        let status = response.status();
        let exhausted = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            == Some("0");
        let reset = response
            .headers()
            .get("x-ratelimit-reset-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<f64>().ok());
        let retry_header = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<f64>().ok());
        if exhausted || status == StatusCode::TOO_MANY_REQUESTS {
            *next = Instant::now() + safe_delay(reset.or(retry_header))?;
        }
        if status == StatusCode::UNAUTHORIZED {
            self.stop();
            return Err(Failure::Expired);
        }
        if response
            .content_length()
            .is_some_and(|n| n > MAX_WIRE as u64)
        {
            return Err(Failure::Capacity);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| {
            if write {
                Failure::Ambiguous
            } else {
                Failure::Network
            }
        })? {
            if bytes.len() + chunk.len() > MAX_WIRE {
                return Err(Failure::Capacity);
            }
            bytes.extend_from_slice(&chunk);
        }
        if !status.is_success() {
            let error = decode::<ErrorBody>(&bytes).unwrap_or_default();
            if error.captcha_key.is_some() || matches!(error.code, Some(60003 | 50014)) {
                self.stop();
                return Err(Failure::Challenged);
            }
            if status == StatusCode::TOO_MANY_REQUESTS {
                *next =
                    (*next).max(Instant::now() + safe_delay(error.retry_after.or(retry_header))?);
                return Err(Failure::RateLimited);
            }
            return Err(if status == StatusCode::FORBIDDEN {
                Failure::Forbidden
            } else if status.is_server_error() && write {
                Failure::Ambiguous
            } else {
                Failure::Protocol
            });
        }
        Ok(bytes)
    }
    pub async fn gateway_url(&self) -> Result<String, Failure> {
        let bytes = self.request(Method::GET, "/gateway", None).await?;
        decode::<GatewayLocation>(&bytes)
            .map(|g| g.url)
            .map_err(|_| Failure::Protocol)
    }
    pub async fn current_user(&self) -> Result<User, Failure> {
        let bytes = self.request(Method::GET, "/users/@me", None).await?;
        let user = decode::<UserDto>(&bytes).map_err(|_| Failure::Protocol)?;
        if user.bot {
            self.stop();
            return Err(Failure::InvalidCredential);
        }
        Ok(user.into_model())
    }
    /// Unofficial normal-user DM endpoint; no retry of ambiguous ringing writes.
    pub async fn ring_call(
        &self,
        channel: model::Id,
        recipient: Option<model::Id>,
        stop: bool,
    ) -> Result<(), Failure> {
        let route = if stop { "stop-ringing" } else { "ring" };
        let body = match recipient {
            Some(recipient) => serde_json::json!({"recipients":[recipient]}),
            None if stop => serde_json::json!({}),
            None => serde_json::json!({"recipients":null}),
        };
        self.request(
            Method::POST,
            &format!("/channels/{channel}/call/{route}"),
            Some(body),
        )
        .await
        .map(|_| ())
    }
    pub async fn execute(&self, command: Command) -> Event {
        match command {
            Command::Voice(_) | Command::Members { .. } => Event::Failure(Failure::Protocol),
            Command::History {
                channel,
                before,
                request,
            } => {
                let mut path = format!("/channels/{channel}/messages?limit=50");
                if let Some(before) = before {
                    path.push_str(&format!("&before={before}"));
                }
                match self
                    .request(Method::GET, &path, None)
                    .await
                    .and_then(|bytes| {
                        decode::<Vec<MessageDto>>(&bytes).map_err(|_| Failure::Protocol)
                    }) {
                    Ok(messages) if messages.len() <= 50 => Event::History {
                        channel,
                        request,
                        older: before.is_some(),
                        messages: messages.into_iter().map(MessageDto::into_model).collect(),
                    },
                    Ok(_) => Event::Failure(Failure::Capacity),
                    Err(Failure::Forbidden) => Event::Unavailable(channel),
                    Err(f) => Event::Failure(f),
                }
            }
            Command::Send {
                channel,
                content,
                nonce,
                reply,
            } => {
                if content.trim().is_empty() || content.chars().count() > client_core::MAX_CONTENT {
                    return Event::SendResult {
                        nonce,
                        result: Err(Failure::Capacity),
                    };
                }
                let mut body = serde_json::json!({ "content": content, "nonce": nonce, "allowed_mentions": {"parse": [], "replied_user": false} });
                if let Some(reply) = reply {
                    body["message_reference"] =
                        serde_json::json!({"message_id": reply, "channel_id": channel});
                }
                // No enforce_nonce claim until normal-user semantics are live verified. Never auto-retry writes.
                let result = self
                    .request(
                        Method::POST,
                        &format!("/channels/{channel}/messages"),
                        Some(body),
                    )
                    .await
                    .and_then(|bytes| {
                        decode::<MessageDto>(&bytes)
                            .map(MessageDto::into_model)
                            .map_err(|_| Failure::Ambiguous)
                    });
                Event::SendResult { nonce, result }
            }
            Command::Edit {
                channel,
                message,
                content,
            } => {
                if content.trim().is_empty() || content.chars().count() > client_core::MAX_CONTENT {
                    return Event::Failure(Failure::Capacity);
                }
                let body = serde_json::json!({"content": content, "allowed_mentions": {"parse": [], "replied_user": false}});
                match self
                    .request(
                        Method::PATCH,
                        &format!("/channels/{channel}/messages/{message}"),
                        Some(body),
                    )
                    .await
                    .and_then(|bytes| {
                        decode::<MessageDto>(&bytes)
                            .map(MessageDto::into_model)
                            .map_err(|_| Failure::Ambiguous)
                    }) {
                    Ok(m) => Event::Message(m),
                    Err(Failure::Forbidden) => Event::Unavailable(channel),
                    Err(f) => Event::Failure(f),
                }
            }
            Command::Delete { channel, message } => {
                match self
                    .request(
                        Method::DELETE,
                        &format!("/channels/{channel}/messages/{message}"),
                        None,
                    )
                    .await
                {
                    Ok(_) => Event::Delete {
                        channel,
                        id: message,
                    },
                    Err(Failure::Forbidden) => Event::Unavailable(channel),
                    Err(f) => Event::Failure(f),
                }
            }
        }
    }
}
impl AuthProvider for DiscordApi {
    async fn authenticate(&mut self) -> Result<User, Failure> {
        self.current_user().await
    }
}
fn safe_delay(seconds: Option<f64>) -> Result<Duration, Failure> {
    let seconds = seconds.unwrap_or(1.0);
    if !seconds.is_finite() || !(0.0..=86400.0).contains(&seconds) {
        return Err(Failure::Protocol);
    }
    Ok(Duration::from_secs_f64(seconds.max(0.05)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    #[tokio::test]
    async fn explicit_dm_ring_and_decline_use_only_scoped_routes() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut api = DiscordApi::new(Arc::new(
            SessionSecret::from_owner_input("SYNTHETIC_OWNER_TOKEN".into()).unwrap(),
        ))
        .unwrap();
        api.base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for (route, body) in [
                ("ring", serde_json::json!({"recipients":null})),
                ("stop-ringing", serde_json::json!({"recipients":["1"]})),
                ("stop-ringing", serde_json::json!({})),
            ] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = vec![0; 4096];
                let n = stream.read(&mut bytes).await.unwrap();
                let text = std::str::from_utf8(&bytes[..n]).unwrap();
                assert!(text.starts_with(&format!("POST /channels/2/call/{route} HTTP/1.1")));
                let actual: serde_json::Value =
                    serde_json::from_str(text.split_once("\r\n\r\n").unwrap().1).unwrap();
                assert_eq!(actual, body);
                stream.write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
            }
        });
        api.ring_call(model::Id(2), None, false).await.unwrap();
        api.ring_call(model::Id(2), Some(model::Id(1)), true)
            .await
            .unwrap();
        api.ring_call(model::Id(2), None, true).await.unwrap();
        server.await.unwrap();
    }
    #[tokio::test]
    async fn local_http_checks_redirect_expiry_rate_limits_and_response_cap() {
        for (status, body, expected) in [
            ("302 Found", "", Failure::Protocol),
            ("401 Unauthorized", "{}", Failure::Expired),
            (
                "429 Too Many Requests",
                "{\"retry_after\":0.1,\"global\":true}",
                Failure::RateLimited,
            ),
            (
                "403 Forbidden",
                "{\"captcha_key\":[\"challenge\"]}",
                Failure::Challenged,
            ),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let task = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buffer = [0; 4096];
                let n = socket.read(&mut buffer).await.unwrap();
                let request = String::from_utf8_lossy(&buffer[..n]);
                assert!(request.contains("SYNTHETIC_SECRET_MARKER"));
                let response = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nLocation: http://127.0.0.1:1/never\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            });
            let mut api = DiscordApi::new(Arc::new(
                SessionSecret::from_owner_input("SYNTHETIC_SECRET_MARKER".into()).unwrap(),
            ))
            .unwrap();
            api.base = format!("http://{address}");
            assert_eq!(
                api.request(Method::GET, "/users/@me", None)
                    .await
                    .unwrap_err(),
                expected
            );
            if expected.ends_session() {
                assert!(api.stopped());
            }
            task.await.unwrap();
        }
        assert!(safe_delay(Some(f64::NAN)).is_err());
        assert!(safe_delay(Some(-1.0)).is_err());
    }
}
