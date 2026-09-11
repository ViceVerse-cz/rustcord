//! Deliberate server writes: one pending request and one bounded, session-only result.
use crate::{
	Command, State,
	auth::{AuthState, Failure},
};
use model::{Id, permissions::VIEW_CHANNEL};
use std::time::{Duration, Instant};

const CREATE_INSTANT_INVITE: u128 = 1;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
	CreateInvite { guild: Id, channel: Id },
	Leave(Id),
}
impl Action {
	pub fn guild(self) -> Id {
		match self {
			Self::CreateInvite { guild, .. } | Self::Leave(guild) => guild,
		}
	}
}
pub enum Event {
	Written {
		action: Action,
		request: u64,
		result: Result<Option<String>, Failure>,
	},
}
#[derive(Default)]
pub struct Actions {
	sequence: u64,
	pending: Option<(Action, u64, bool)>,
	status: Option<(Id, &'static str)>,
	invite: Option<(Id, Id, String, Instant)>,
}
impl Actions {
	pub(crate) fn reset(&mut self) {
		*self = Self {
			sequence: self.sequence,
			..Self::default()
		};
	}
}
impl State {
	pub fn server_action_pending(&self) -> bool {
		self.server_actions.pending.is_some()
	}
	pub fn server_action_status(&self, guild: Id) -> Option<&'static str> {
		self.server_actions
			.status
			.filter(|(id, _)| *id == guild)
			.map(|(_, text)| text)
	}
	pub fn created_invite(&self, guild: Id) -> Option<&str> {
		self.server_actions
			.invite
			.as_ref()
			.filter(|(id, channel, _, at)| {
				*id == guild
					&& self.can_create_server_invite(guild, *channel)
					&& at.elapsed() < Duration::from_secs(86400)
			})
			.map(|(_, _, url, _)| url.as_str())
	}
	pub fn clear_server_action_result(&mut self, guild: Id) {
		if self
			.server_actions
			.status
			.is_some_and(|(id, _)| id == guild)
		{
			self.server_actions.status = None;
		}
		if self
			.server_actions
			.invite
			.as_ref()
			.is_some_and(|(id, _, _, _)| *id == guild)
		{
			self.server_actions.invite = None;
		}
	}
	pub fn can_create_server_invite(&self, guild: Id, channel: Id) -> bool {
		self.channel(channel)
			.is_some_and(|c| c.guild == Some(guild) && matches!(c.kind, 0 | 2 | 5 | 13 | 15 | 16))
			&& self.permission(channel, VIEW_CHANNEL | CREATE_INSTANT_INVITE) == Some(true)
	}
	pub fn invite_channel(&self, guild: Id) -> Option<Id> {
		self.selected
			.filter(|id| self.can_create_server_invite(guild, *id))
			.or_else(|| {
				self.channels
					.iter()
					.find(|c| self.can_create_server_invite(guild, c.id))
					.map(|c| c.id)
			})
	}
	pub fn leave_server_reason(&self, guild: Id) -> Option<&'static str> {
		if self.guild(guild).is_none() {
			return Some("Server is no longer available");
		}
		let Some(owner) = self.permissions.guilds.get(&guild).and_then(|g| g.owner) else {
			return Some("Server ownership is not available yet");
		};
		let Some(user) = self.user.as_ref() else {
			return Some("Sign in before leaving a server");
		};
		if owner == user.id {
			return Some("Transfer ownership in Discord before leaving this server");
		}
		if self.pending.iter().any(|p| {
			self.channel(p.channel)
				.is_some_and(|c| c.guild == Some(guild))
		}) || self
			.voice
			.active
			.as_ref()
			.is_some_and(|call| call.guild == Some(guild))
		{
			return Some("Finish pending messages and leave the call before leaving this server");
		}
		None
	}
	pub fn create_server_invite(&mut self, guild: Id, channel: Id) -> Option<Command> {
		if !self.can_create_server_invite(guild, channel) {
			self.server_actions.status = Some((
				guild,
				"You need Create Invite permission in a visible channel",
			));
			return None;
		}
		self.request_server_action(Action::CreateInvite { guild, channel })
	}
	pub fn leave_server(&mut self, guild: Id) -> Option<Command> {
		if let Some(reason) = self.leave_server_reason(guild) {
			self.server_actions.status = Some((guild, reason));
			return None;
		}
		self.request_server_action(Action::Leave(guild))
	}
	fn request_server_action(&mut self, action: Action) -> Option<Command> {
		if self.server_action_pending() {
			return None;
		}
		if !self.demo && (self.auth != AuthState::Authenticated || !self.gateway_connected) {
			self.server_actions.status = Some((
				action.guild(),
				"Server actions unavailable while disconnected",
			));
			return None;
		}
		self.server_actions.sequence = self.server_actions.sequence.wrapping_add(1);
		let request = self.server_actions.sequence;
		self.server_actions.pending = Some((action, request, false));
		self.clear_server_action_result(action.guild());
		Some(Command::ServerAction { action, request })
	}
	pub(crate) fn cancel_server_action(&mut self) {
		if let Some((action, _, _)) = self.server_actions.pending.take() {
			self.server_actions.status = Some((
				action.guild(),
				"Outcome unknown; check Discord before retrying",
			));
		}
	}
	pub(crate) fn observe_server_joined(&mut self, guild: Id) {
		if let Some((Action::Leave(id), _, observed)) = &mut self.server_actions.pending
			&& *id == guild
		{
			*observed = true;
		}
	}
	pub(crate) fn apply_server_action(&mut self, event: Event) -> Result<(), &'static str> {
		let Event::Written {
			action,
			request,
			result,
		} = event;
		let Some((pending, sequence, observed)) = self.server_actions.pending else {
			return Ok(());
		};
		if pending != action || sequence != request {
			return Ok(());
		}
		self.server_actions.pending = None;
		let result = result.and_then(|code| match action {
			Action::CreateInvite { .. }
				if code.as_ref().is_some_and(|code| {
					crate::invites::valid_code(code) && code.capacity() <= 1024
				}) =>
			{
				Ok(code)
			}
			Action::Leave(_) if code.is_none() => Ok(None),
			_ => Err(Failure::Ambiguous),
		});
		let status = match result {
			Err(failure) => {
				if failure.ends_session() {
					self.fail(failure);
				}
				failure.label()
			}
			Ok(code) => match action {
				Action::CreateInvite { guild, channel } => {
					if self.can_create_server_invite(guild, channel) {
						self.server_actions.invite = Some((
							guild,
							channel,
							format!("https://discord.gg/{}", code.unwrap()),
							Instant::now(),
						));
						"Invite created"
					} else {
						"Invite created, but channel access changed; check Discord"
					}
				}
				Action::Leave(guild) => {
					if observed {
						"Request completed; latest server membership shown"
					} else {
						self.remove_server(guild);
						"Left server"
					}
				}
			},
		};
		self.server_actions.status = Some((action.guild(), status));
		self.status = status;
		Ok(())
	}
	fn remove_server(&mut self, guild: Id) {
		let removed = self
			.channels
			.iter()
			.filter(|c| c.guild == Some(guild))
			.map(|c| c.id)
			.collect();
		self.remove_channels(&removed);
		if self.selected.is_some_and(|id| removed.contains(&id)) {
			self.selected = None;
		}
		self.guilds.retain(|g| g.id != guild);
		self.permissions.guilds.remove(&guild);
		self.permissions.clear_cache();
		self.invalidate_navigation();
		self.clear_server_action_result(guild);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Envelope, Event as CoreEvent};
	fn state() -> State {
		let mut state = State {
			user: Some(model::User {
				id: Id(1),
				name: "Synthetic".into(),
				avatar: None,
				discriminator: 0,
			}),
			guilds: vec![model::Guild {
				id: Id(2),
				name: "Synthetic server".into(),
				icon: None,
				emojis: None,
			}],
			channels: vec![model::Channel {
				id: Id(3),
				guild: Some(Id(2)),
				name: "general".into(),
				kind: 0,
				parent_id: None,
				position: 0,
				recipients: vec![],
				last_message: None,
				member_list_id: None,
				message_count: None,
			}],
			selected: Some(Id(3)),
			auth: AuthState::Authenticated,
			gateway_connected: true,
			..State::default()
		};
		state
			.permissions
			.replace(model::permissions::Snapshot {
				guilds: vec![model::permissions::Guild {
					id: Id(2),
					owner: Some(Id(9)),
					roles: Some(vec![model::permissions::Role {
						id: Id(2),
						bits: VIEW_CHANNEL | CREATE_INSTANT_INVITE,
						name: String::new(),
						color: 0,
						position: 0,
						hoist: false,
					}]),
					member: Some(model::permissions::Member {
						roles: vec![],
						timeout_until: None,
					}),
				}],
				channels: vec![model::permissions::Channel {
					id: Id(3),
					guild: Id(2),
					overwrites: Some(vec![]),
				}],
			})
			.unwrap();
		state
	}
	fn finish(state: &mut State, command: Command, result: Result<Option<String>, Failure>) {
		let Command::ServerAction { action, request } = command else {
			panic!("wrong command");
		};
		let event = CoreEvent::ServerAction(Event::Written {
			action,
			request,
			result,
		});
		if matches!(
			&event,
			CoreEvent::ServerAction(Event::Written {
				action: Action::Leave(_),
				result: Ok(None),
				..
			})
		) {
			assert!(
				event.changes_access(),
				"confirmed leave must invalidate cached access"
			);
		}
		state.apply(Envelope {
			generation: state.generation,
			event,
		});
	}
	#[test]
	fn server_actions_scope_permissions_and_confirmed_removal() {
		let mut state = state();
		assert_eq!(state.invite_channel(Id(2)), Some(Id(3)));
		assert_eq!(state.invite_channel(Id(8)), None);
		assert!(state.create_server_invite(Id(8), Id(3)).is_none());
		let invite = state.create_server_invite(Id(2), Id(3)).unwrap();
		assert!(state.leave_server(Id(2)).is_none());
		finish(&mut state, invite, Ok(Some("safe-code_1".into())));
		assert_eq!(
			state.created_invite(Id(2)),
			Some("https://discord.gg/safe-code_1")
		);
		assert_eq!(state.created_invite(Id(8)), None);
		let invite = state.create_server_invite(Id(2), Id(3)).unwrap();
		finish(&mut state, invite, Ok(Some("../malicious".into())));
		assert_eq!(state.created_invite(Id(2)), None);
		assert_eq!(
			state.server_action_status(Id(2)),
			Some(Failure::Ambiguous.label())
		);
		state.permissions.guilds.get_mut(&Id(2)).unwrap().owner = Some(Id(1));
		assert!(state.leave_server(Id(2)).is_none());
		state.permissions.guilds.get_mut(&Id(2)).unwrap().owner = None;
		assert!(state.leave_server(Id(2)).is_none());
		state.permissions.guilds.get_mut(&Id(2)).unwrap().owner = Some(Id(9));
		state.drafts.insert(Id(3), "Keep draft".into());
		let leave = state.leave_server(Id(2)).unwrap();
		finish(&mut state, leave, Err(Failure::Ambiguous));
		assert!(state.guild(Id(2)).is_some());
		let old = state.leave_server(Id(2)).unwrap();
		state.cancel_server_action();
		let new = state.leave_server(Id(2)).unwrap();
		finish(&mut state, old, Ok(None));
		assert!(state.server_action_pending());
		assert!(state.guild(Id(2)).is_some());
		finish(&mut state, new, Ok(None));
		assert!(state.guild(Id(2)).is_none());
		assert!(state.channel(Id(3)).is_none());
		assert_eq!(state.selected, None);
		assert_eq!(state.drafts[&Id(3)], "Keep draft");
	}
	#[test]
	fn server_actions_reject_stale_sessions_and_keep_rejoined_guilds() {
		let mut state = state();
		let leave = state.leave_server(Id(2)).unwrap();
		state.observe_server_joined(Id(2));
		finish(&mut state, leave, Ok(None));
		assert!(state.guild(Id(2)).is_some());
		let invite = state.create_server_invite(Id(2), Id(3)).unwrap();
		state.command_rejected(invite);
		assert!(!state.server_action_pending());
		state.gateway_connected = false;
		assert!(state.create_server_invite(Id(2), Id(3)).is_none());
		state.gateway_connected = true;
		let old = state.create_server_invite(Id(2), Id(3)).unwrap();
		let Command::ServerAction { action, request } = old else {
			unreachable!()
		};
		let generation = state.generation;
		state.logout();
		state.apply(Envelope {
			generation,
			event: CoreEvent::ServerAction(Event::Written {
				action,
				request,
				result: Ok(Some("old".into())),
			}),
		});
		assert_eq!(state.created_invite(Id(2)), None);
	}
}
