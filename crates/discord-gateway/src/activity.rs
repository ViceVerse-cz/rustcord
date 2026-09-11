use client_core::auth::Failure;
use discord_protocol::activity_sessions::{self, Observation};
use discord_protocol::rpc::Activity;
use std::time::Duration;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;

/// A single replaceable bounded rich activity, plus the last sent value.
/// Five seconds between attempts is stricter than the documented 5 updates/20 seconds.
pub(super) struct Pending {
	current: Option<Activity>,
	sent: Option<Option<Activity>>,
	next_send: Instant,
	pub observation: Observation,
}
impl Default for Pending {
	fn default() -> Self {
		Self {
			current: None,
			sent: None,
			next_send: Instant::now(),
			observation: Observation::Unconfirmed,
		}
	}
}
impl Pending {
	pub fn update(&mut self, activity: &Option<Activity>) -> Result<(), Failure> {
		if let Some(activity) = activity {
			activity.validate().map_err(|_| Failure::Protocol)?;
		}
		if self.current != *activity {
			self.current = activity.clone();
			self.observation = Observation::Unconfirmed;
		}
		Ok(())
	}
	pub fn reconnect(&mut self) {
		self.sent = None;
		self.observation = Observation::Unconfirmed;
	}
	pub fn observe(&mut self, bytes: &[u8], session: &str) {
		if self.sent.as_ref() != Some(&self.current) {
			self.observation = Observation::Unconfirmed;
			return;
		}
		self.observation = self
			.current
			.as_ref()
			.map_or(Observation::Unconfirmed, |current| {
				activity_sessions::observe(bytes, session, current)
					.unwrap_or(Observation::Unconfirmed)
			});
	}
	pub fn deadline(&self) -> Option<Instant> {
		(self.sent.as_ref() != Some(&self.current)).then_some(self.next_send)
	}
	pub fn packet(&mut self, now: Instant) -> Option<Message> {
		if self.deadline().is_none_or(|deadline| now < deadline) {
			return None;
		}
		let activities: Vec<_> = self.current.iter().collect();
		self.sent = Some(self.current.clone());
		self.observation = Observation::Unconfirmed;
		// Charge attempts too: a failed write may have reached Discord.
		self.next_send = now + Duration::from_secs(5);
		Some(Message::Text(
			serde_json::json!({"op": 3, "d": {
				"since": null, "activities": activities, "status": "online", "afk": false
			}})
			.to_string()
			.into(),
		))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use discord_protocol::rpc::{ActivityFields, Assets, Timestamps};
	use model::Id;

	fn game(name: &str) -> Activity {
		ActivityFields {
			details: Some("Ranked match".into()),
			state: Some("Round 2".into()),
			timestamps: Some(Timestamps {
				start: Some(1_700_000_000),
				end: None,
			}),
			assets: Some(Assets {
				large_image: Some("123".into()),
				..Default::default()
			}),
			..Default::default()
		}
		.into_activity(Id(42), name.into())
		.unwrap()
	}

	#[test]
	fn server_observations_do_not_confirm_unsent_changed_or_cleared_games() {
		let mut pending = Pending::default();
		let listed = br#"[{"session_id":"all","activities":[{"application_id":"42","type":0}]}]"#;
		pending.update(&Some(game("osu!"))).unwrap();
		pending.observe(listed, "own");
		assert_eq!(pending.observation, Observation::Unconfirmed);
		pending.packet(Instant::now()).unwrap();
		pending.observe(listed, "own");
		assert_eq!(pending.observation, Observation::ServerListed);
		let mut changed = game("osu!");
		changed.details = Some("Next map".into());
		pending.update(&Some(changed)).unwrap();
		pending.observe(listed, "own");
		assert_eq!(pending.observation, Observation::Unconfirmed);
		pending.update(&None).unwrap();
		pending.observe(listed, "own");
		assert_eq!(pending.observation, Observation::Unconfirmed);
	}

	#[test]
	fn activity_coalesces_clears_reconnects_and_bounds_utf8() {
		let mut pending = Pending::default();
		let now = Instant::now();
		let packet = |frame: Message| -> serde_json::Value {
			serde_json::from_str(frame.to_text().unwrap()).unwrap()
		};
		pending.update(&Some(game("osu!"))).unwrap();
		assert_eq!(
			packet(pending.packet(now).unwrap()),
			serde_json::json!({"op":3,"d":{
				"since":null,"activities":[{"name":"osu!","type":0,"application_id":"42",
					"details":"Ranked match","state":"Round 2","timestamps":{"start":1_700_000_000_000_u64},
					"assets":{"large_image":"123"}}],"status":"online","afk":false
			}})
		);
		assert!(pending.deadline().is_none());
		pending.update(&Some(game("Intermediate"))).unwrap();
		pending.update(&None).unwrap();
		assert!(pending.packet(now + Duration::from_millis(4999)).is_none());
		assert_eq!(
			packet(pending.packet(now + Duration::from_secs(5)).unwrap())["d"]["activities"],
			serde_json::json!([])
		);
		assert!(pending.deadline().is_none());
		pending.reconnect();
		assert!(pending.packet(now + Duration::from_secs(9)).is_none());
		assert_eq!(
			packet(pending.packet(now + Duration::from_secs(10)).unwrap())["d"]["activities"],
			serde_json::json!([])
		);
		for name in [
			"x".repeat(129),
			"é".repeat(65),
			"game\n".into(),
			"\0".into(),
			" ".into(),
		] {
			let mut activity = game("Valid");
			activity.name = name;
			assert_eq!(pending.update(&Some(activity)), Err(Failure::Protocol));
		}
		pending.update(&Some(game(&"é".repeat(64)))).unwrap();
		assert!(pending.packet(now + Duration::from_secs(14)).is_none());
		assert!(pending.packet(now + Duration::from_secs(15)).is_some());
		assert!(pending.deadline().is_none());
		let mut changed = game(&"é".repeat(64));
		changed.details = Some("Next match".into());
		pending.update(&Some(changed.clone())).unwrap();
		changed.state = Some("x".repeat(129));
		assert_eq!(pending.update(&Some(changed)), Err(Failure::Protocol));
		assert_eq!(
			packet(pending.packet(now + Duration::from_secs(20)).unwrap())["d"]["activities"][0]["details"],
			"Next match"
		);
	}
}
