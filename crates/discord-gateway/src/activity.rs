use client_core::auth::Failure;
use std::time::Duration;
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message;

/// A single replaceable game name, at most 128 UTF-8 bytes, plus the last sent value.
/// Five seconds between attempts is stricter than the documented 5 updates/20 seconds.
pub(super) struct Pending {
	current: Option<String>,
	sent: Option<Option<String>>,
	next_send: Instant,
}
impl Default for Pending {
	fn default() -> Self {
		Self {
			current: None,
			sent: None,
			next_send: Instant::now(),
		}
	}
}
impl Pending {
	pub fn update(&mut self, name: &Option<String>) -> Result<(), Failure> {
		if name.as_ref().is_some_and(|name| {
			name.len() > 128 || name.trim().is_empty() || name.chars().any(char::is_control)
		}) {
			return Err(Failure::Protocol);
		}
		if self.current != *name {
			self.current = name.clone();
		}
		Ok(())
	}
	pub fn reconnect(&mut self) {
		self.sent = None;
	}
	pub fn deadline(&self) -> Option<Instant> {
		(self.sent.as_ref() != Some(&self.current)).then_some(self.next_send)
	}
	pub fn packet(&mut self, now: Instant) -> Option<Message> {
		if self.deadline().is_none_or(|deadline| now < deadline) {
			return None;
		}
		let activities: Vec<_> = self
			.current
			.iter()
			.map(|name| serde_json::json!({"name": name, "type": 0}))
			.collect();
		self.sent = Some(self.current.clone());
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

	#[test]
	fn activity_coalesces_clears_reconnects_and_bounds_utf8() {
		let mut pending = Pending::default();
		let now = Instant::now();
		let packet = |frame: Message| -> serde_json::Value {
			serde_json::from_str(frame.to_text().unwrap()).unwrap()
		};
		pending.update(&Some("osu!".into())).unwrap();
		assert_eq!(
			packet(pending.packet(now).unwrap()),
			serde_json::json!({"op":3,"d":{
				"since":null,"activities":[{"name":"osu!","type":0}],"status":"online","afk":false
			}})
		);
		assert!(pending.deadline().is_none());
		pending.update(&Some("Intermediate".into())).unwrap();
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
			assert_eq!(pending.update(&Some(name)), Err(Failure::Protocol));
		}
		pending.update(&Some("é".repeat(64))).unwrap();
		assert!(pending.packet(now + Duration::from_secs(14)).is_none());
		assert!(pending.packet(now + Duration::from_secs(15)).is_some());
		assert!(pending.deadline().is_none());
	}
}
