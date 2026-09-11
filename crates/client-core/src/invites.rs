//! Session-only invite metadata: 32 entries, at most 4 KiB each, five-minute lifetime.
use crate::{Command, State, auth::Failure};
use std::{
	collections::BTreeMap,
	time::{Duration, Instant},
};
pub type Cache = BTreeMap<String, (Instant, Option<Result<model::Embed, Failure>>)>;
pub fn valid_code(code: &str) -> bool {
	!code.is_empty()
		&& code.len() <= 100
		&& code
			.bytes()
			.all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl State {
	pub fn request_invite(&mut self, code: String) -> Option<Command> {
		if self.demo
			|| !valid_code(&code)
			|| self.auth != crate::auth::AuthState::Authenticated
			|| !self.gateway_connected
			|| !self.selected.is_some_and(|c| self.can_read_history(c))
		{
			return None;
		}
		self.invites
			.retain(|_, (at, _)| at.elapsed() < Duration::from_secs(300));
		if self.invites.contains_key(&code) || self.invites.values().any(|(_, v)| v.is_none()) {
			return None;
		}
		if self.invites.len() >= 32 {
			let oldest = self
				.invites
				.iter()
				.min_by_key(|(_, (at, _))| *at)
				.map(|(code, _)| code.clone());
			if let Some(oldest) = oldest {
				self.invites.remove(&oldest);
			}
		}
		self.invites.insert(code.clone(), (Instant::now(), None));
		Some(Command::Invite { code })
	}
	pub fn apply_invite(&mut self, code: String, result: Result<model::Embed, Failure>) {
		let result = result.and_then(|embed| {
			if model::valid_embeds(std::slice::from_ref(&embed)) && embed.bytes() <= 4096 {
				Ok(embed)
			} else {
				Err(Failure::Capacity)
			}
		});
		if let Some((at, value)) = self.invites.get_mut(&code) {
			*at = Instant::now();
			*value = Some(result);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Envelope, Event};

	#[test]
	fn boxed_invite_charges_payload_and_reaches_the_pending_preview() {
		let mut title = String::with_capacity(1024);
		title.push_str("Synthetic invite");
		let embed = model::Embed {
			title: Some(title),
			..Default::default()
		};
		let event = Event::Invite {
			code: "synthetic".into(),
			result: Ok(Box::new(embed)),
		};
		assert!(event.bytes() >= size_of::<Event>() + size_of::<model::Embed>() + 1024);
		let mut state = State::default();
		state
			.invites
			.insert("synthetic".into(), (Instant::now(), None));
		state.apply(Envelope {
			generation: state.generation,
			event,
		});
		assert_eq!(
			state.invites["synthetic"]
				.1
				.as_ref()
				.unwrap()
				.as_ref()
				.unwrap()
				.title
				.as_deref(),
			Some("Synthetic invite")
		);
	}
}
