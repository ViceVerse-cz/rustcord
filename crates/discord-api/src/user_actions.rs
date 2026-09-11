use crate::{DiscordApi, Failure};
use client_core::user_actions::Action;
use reqwest::Method;
use serde_json::json;

impl DiscordApi {
	pub(super) async fn user_action(&self, action: Action) -> Result<(), Failure> {
		let id = match action {
			Action::CloseDm(id)
			| Action::Block { user: id, .. }
			| Action::Mute { channel: id, .. } => id,
		};
		if id.0 == 0 {
			return Err(Failure::Protocol);
		}
		match action {
			Action::CloseDm(channel) => self
				.request(Method::DELETE, &format!("/channels/{channel}"), None)
				.await
				.map(|_| ()),
			Action::Block { user, blocked } => self
				.request(
					if blocked { Method::PUT } else { Method::DELETE },
					&format!("/users/@me/relationships/{user}"),
					blocked.then(|| json!({"type": 2})),
				)
				.await
				.map(|_| ()),
			Action::Mute { channel, muted } => {
				let bytes = self.request(Method::PATCH, "/users/@me/guilds/@me/settings",
					Some(json!({"channel_overrides": {channel.to_string(): {"muted": muted, "mute_config": {"end_time": null, "selected_time_window": -1}}}}))).await?;
				let setting: discord_protocol::notifications::Setting =
					discord_protocol::decode(&bytes).map_err(|_| Failure::Protocol)?;
				if setting.guild_id.is_some()
					|| !setting.channel_overrides.is_some_and(|o| {
						o.0.iter()
							.any(|c| c.channel_id == channel && c.muted == Some(muted))
					}) {
					return Err(Failure::ProtocolAt(
						"DM mute response did not confirm the requested setting",
					));
				}
				Ok(())
			}
		}
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
	async fn account_actions_use_scoped_routes_and_confirm_remote_outcomes() {
		let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
		let mut api = DiscordApi::new(Arc::new(
			SessionSecret::from_owner_input("SYNTHETIC_USER_ACTION_TOKEN".into()).unwrap(),
		))
		.unwrap();
		api.base = format!("http://{}", listener.local_addr().unwrap());
		let cases = [
			(
				Action::CloseDm(Id(10)),
				"DELETE /channels/10",
				None,
				204,
				"",
				Ok(()),
			),
			(
				Action::Block {
					user: Id(2),
					blocked: true,
				},
				"PUT /users/@me/relationships/2",
				Some(json!({"type":2})),
				204,
				"",
				Ok(()),
			),
			(
				Action::Block {
					user: Id(2),
					blocked: false,
				},
				"DELETE /users/@me/relationships/2",
				None,
				204,
				"",
				Ok(()),
			),
			(
				Action::Mute {
					channel: Id(10),
					muted: true,
				},
				"PATCH /users/@me/guilds/@me/settings",
				Some(
					json!({"channel_overrides":{"10":{"muted":true,"mute_config":{"end_time":null,"selected_time_window":-1}}}}),
				),
				200,
				r#"{"guild_id":null,"channel_overrides":[{"channel_id":"10","muted":true}]}"#,
				Ok(()),
			),
			(
				Action::Mute {
					channel: Id(10),
					muted: false,
				},
				"PATCH /users/@me/guilds/@me/settings",
				Some(
					json!({"channel_overrides":{"10":{"muted":false,"mute_config":{"end_time":null,"selected_time_window":-1}}}}),
				),
				200,
				r#"{"guild_id":null,"channel_overrides":[{"channel_id":"11","muted":false}]}"#,
				Err(Failure::ProtocolAt(
					"DM mute response did not confirm the requested setting",
				)),
			),
			(
				Action::CloseDm(Id(10)),
				"DELETE /channels/10",
				None,
				403,
				"{}",
				Err(Failure::Forbidden),
			),
			(
				Action::Block {
					user: Id(2),
					blocked: true,
				},
				"PUT /users/@me/relationships/2",
				Some(json!({"type":2})),
				500,
				"{}",
				Err(Failure::Ambiguous),
			),
			(
				Action::Block {
					user: Id(2),
					blocked: true,
				},
				"PUT /users/@me/relationships/2",
				Some(json!({"type":2})),
				429,
				r#"{"retry_after":0.01}"#,
				Err(Failure::RateLimited),
			),
		];
		for (action, route, payload, status, body, expected) in cases {
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
						let len: usize = headers
							.lines()
							.find_map(|h| {
								h.to_ascii_lowercase()
									.strip_prefix("content-length: ")
									.map(str::to_owned)
							})
							.map_or(0, |n| n.parse().unwrap());
						if bytes.len() < end + 4 + len {
							continue;
						}
						assert!(headers.starts_with(&format!("{route} HTTP/1.1\r\n")));
						assert!(headers.contains("SYNTHETIC_USER_ACTION_TOKEN"));
						assert_eq!(
							if len == 0 {
								None
							} else {
								Some(
									serde_json::from_slice::<serde_json::Value>(&bytes[end + 4..])
										.unwrap(),
								)
							},
							payload
						);
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
				api.execute(Command::UserAction { action, request: 1 }),
				server
			);
			let Event::UserAction(client_core::user_actions::Event::Written { result, .. }) = event
			else {
				panic!("wrong result")
			};
			assert_eq!(result, expected);
		}
		assert_eq!(
			api.user_action(Action::CloseDm(Id(0))).await,
			Err(Failure::Protocol)
		);
		assert!(
			tokio::time::timeout(std::time::Duration::from_millis(10), listener.accept())
				.await
				.is_err(),
			"writes must not retry automatically"
		);
	}
}
