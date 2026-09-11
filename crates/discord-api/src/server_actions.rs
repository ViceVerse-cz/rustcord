use crate::{DiscordApi, Failure};
use client_core::server_actions::Action;
use reqwest::Method;

impl DiscordApi {
	// Documented developer API routes; normal-user interoperability remains unverified.
	pub(super) async fn server_action(&self, action: Action) -> Result<Option<String>, Failure> {
		if action.guild().0 == 0 {
			return Err(Failure::Protocol);
		}
		match action {
			Action::Leave(guild) => self
				.request_limited(
					Method::DELETE,
					&format!("/users/@me/guilds/{guild}"),
					None,
					64 * 1024,
				)
				.await
				.map_err(write_failure)
				.and_then(|body| {
					if body.is_empty() {
						Ok(None)
					} else {
						Err(Failure::Ambiguous)
					}
				}),
			Action::CreateInvite { guild, channel } => {
				if channel.0 == 0 {
					return Err(Failure::Protocol);
				}
				let bytes = self
					.request_limited(
						Method::POST,
						&format!("/channels/{channel}/invites"),
						Some(
							serde_json::json!({"max_age":86400,"max_uses":0,"temporary":false,"unique":true}),
						),
						64 * 1024,
					)
					.await
					.map_err(write_failure)?;
				discord_protocol::invites::created_code(&bytes, guild, channel)
					.map(Some)
					.map_err(|_| Failure::Ambiguous)
			}
		}
	}
}
fn write_failure(failure: Failure) -> Failure {
	// A response exceeding admission bounds can still belong to a completed write.
	if failure == Failure::Capacity {
		Failure::Ambiguous
	} else {
		failure
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use client_core::{Command, Event, auth::SessionSecret};
	use model::Id;
	use std::sync::Arc;
	use tokio::{
		io::{AsyncReadExt, AsyncWriteExt},
		net::TcpListener,
	};
	#[tokio::test]
	async fn server_actions_routes_scope_and_uncertain_writes() {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let mut api = DiscordApi::new(Arc::new(
			SessionSecret::from_owner_input("SYNTHETIC_SERVER_TOKEN".into()).unwrap(),
		))
		.unwrap();
		api.base = format!("http://{}", listener.local_addr().unwrap());
		let invite = Action::CreateInvite {
			guild: Id(2),
			channel: Id(3),
		};
		for (action, status, body, expected) in [
			(
				invite,
				200,
				r#"{"code":"safe_1","guild":{"id":"2"},"channel":{"id":"3"}}"#,
				Ok(Some("safe_1".to_owned())),
			),
			(
				invite,
				200,
				r#"{"code":"safe_1","guild":{"id":"8"},"channel":{"id":"3"}}"#,
				Err(Failure::Ambiguous),
			),
			(
				invite,
				200,
				r#"{"code":"../bad","guild":{"id":"2"},"channel":{"id":"3"}}"#,
				Err(Failure::Ambiguous),
			),
			(invite, 403, "{}", Err(Failure::Forbidden)),
			(invite, 500, "{}", Err(Failure::Ambiguous)),
			(Action::Leave(Id(2)), 204, "", Ok(None)),
			(Action::Leave(Id(2)), 200, "{}", Err(Failure::Ambiguous)),
			(
				Action::Leave(Id(2)),
				429,
				r#"{"retry_after":0.01}"#,
				Err(Failure::RateLimited),
			),
		] {
			let server = async {
				let (mut socket, _) = listener.accept().await.unwrap();
				let mut bytes = Vec::new();
				loop {
					let mut chunk = [0; 1024];
					let n = socket.read(&mut chunk).await.unwrap();
					assert!(n > 0);
					bytes.extend_from_slice(&chunk[..n]);
					assert!(bytes.len() <= 4096);
					if let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") {
						let headers = std::str::from_utf8(&bytes[..end]).unwrap();
						let length: usize = headers
							.lines()
							.find_map(|h| {
								h.to_ascii_lowercase()
									.strip_prefix("content-length: ")
									.map(str::to_owned)
							})
							.map_or(0, |n| n.parse().unwrap());
						if bytes.len() < end + 4 + length {
							continue;
						}
						assert!(headers.contains("SYNTHETIC_SERVER_TOKEN"));
						match action {
							Action::CreateInvite { .. } => {
								assert!(headers.starts_with("POST /channels/3/invites HTTP/1.1"));
								assert_eq!(
									serde_json::from_slice::<serde_json::Value>(&bytes[end + 4..])
										.unwrap(),
									serde_json::json!({"max_age":86400,"max_uses":0,"temporary":false,"unique":true})
								);
							}
							Action::Leave(_) => {
								assert!(headers.starts_with("DELETE /users/@me/guilds/2 HTTP/1.1"));
								assert_eq!(length, 0);
							}
						}
						break;
					}
				}
				socket
					.write_all(
						format!(
							"HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
							body.len()
						)
						.as_bytes(),
					)
					.await
					.unwrap();
			};
			let (event, ()) = tokio::join!(
				api.execute(Command::ServerAction { action, request: 1 }),
				server
			);
			let Event::ServerAction(client_core::server_actions::Event::Written { result, .. }) =
				event
			else {
				panic!("wrong response");
			};
			assert_eq!(result, expected);
		}
		assert_eq!(
			api.server_action(Action::Leave(Id(0))).await,
			Err(Failure::Protocol)
		);
		assert!(
			tokio::time::timeout(std::time::Duration::from_millis(10), listener.accept())
				.await
				.is_err(),
			"writes must never retry automatically"
		);
	}
}
