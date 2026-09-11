use crate::{DiscordApi, Failure};
use discord_protocol::guild_folders::{self, Decoded, MAX_SETTINGS_RESPONSE};
use model::guild_folders::Settings;
use reqwest::Method;

impl DiscordApi {
	async fn read_guild_folders(&self) -> Result<Decoded, Failure> {
		let bytes = self
			.request_limited(
				Method::GET,
				"/users/@me/settings-proto/1",
				None,
				MAX_SETTINGS_RESPONSE,
			)
			.await?;
		guild_folders::decode_response(&bytes).map_err(|_| {
			Failure::ProtocolAt("Server folder settings are unavailable or unsupported")
		})
	}

	pub async fn guild_folders(&self) -> Result<Settings, Failure> {
		self.read_guild_folders()
			.await
			.map(|response| response.settings)
	}

	pub async fn save_guild_folders(&self, settings: Settings) -> Result<Settings, Failure> {
		if !settings.valid() {
			return Err(Failure::Protocol);
		}
		let current = self.read_guild_folders().await?;
		if current.settings.version != settings.version {
			return Err(Failure::ProtocolAt(
				"Server folders changed elsewhere; refresh folders and try again",
			));
		}
		let patch =
			guild_folders::encode_patch(&current, &settings).map_err(|_| Failure::Protocol)?;
		let bytes = self
			.request_limited(
				Method::PATCH,
				"/users/@me/settings-proto/1",
				Some(
					serde_json::json!({"settings": patch, "required_data_version": settings.version}),
				),
				MAX_SETTINGS_RESPONSE,
			)
			.await?;
		let saved = guild_folders::decode_response(&bytes)
			.map_err(|_| {
				Failure::ProtocolAt(
					"Server folder save was not confirmed; refresh folders before retrying",
				)
			})?
			.settings;
		if saved.folders != settings.folders {
			return Err(Failure::ProtocolAt(
				"Server folder save was not confirmed; refresh folders before retrying",
			));
		}
		Ok(saved)
	}
}
