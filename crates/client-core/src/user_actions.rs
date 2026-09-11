//! Explicit account actions; one pending write, never an automatic retry.
use crate::{
	Command, State,
	auth::{AuthState, Failure},
};
use model::Id;
use std::collections::BTreeMap;

pub const MAX_RELATIONSHIPS: usize = 4000;
pub const MAX_RELATIONSHIP_BYTES: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
	CloseDm(Id),
	Block { user: Id, blocked: bool },
	Mute { channel: Id, muted: bool },
}

pub enum Event {
	Relationships(Option<Vec<(Id, bool)>>),
	Relationship {
		user: Id,
		blocked: bool,
	},
	Written {
		action: Action,
		request: u64,
		result: Result<(), Failure>,
	},
}
#[derive(Default)]
pub struct Actions {
	relationships: BTreeMap<Id, bool>,
	known: bool,
	sequence: u64,
	pending: Option<(Action, u64, bool)>,
	status: Option<&'static str>,
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
	pub fn user_blocked(&self, user: Id) -> Option<bool> {
		self.user_actions
			.relationships
			.get(&user)
			.copied()
			.or_else(|| (self.demo || self.user_actions.known).then_some(false))
	}
	pub fn user_action_pending(&self) -> bool {
		self.user_actions.pending.is_some()
	}
	pub fn user_action_status(&self) -> Option<&'static str> {
		self.user_actions.status
	}
	pub fn close_dm(&mut self, channel: Id) -> Option<Command> {
		if !self.is_one_to_one_dm(channel) {
			return None;
		}
		if self.pending.iter().any(|p| p.channel == channel)
			|| self
				.voice
				.active
				.as_ref()
				.is_some_and(|call| call.channel == channel)
		{
			self.user_actions.status =
				Some("Finish pending messages and leave the call before closing this DM");
			self.status = self.user_actions.status.unwrap();
			return None;
		}
		self.request_user_action(Action::CloseDm(channel))
	}
	pub fn set_user_blocked(&mut self, user: Id, blocked: bool) -> Option<Command> {
		if user.0 == 0
			|| self.user.as_ref().is_none_or(|owner| owner.id == user)
			|| (!blocked && self.user_blocked(user) != Some(true))
		{
			return None;
		}
		self.request_user_action(Action::Block { user, blocked })
	}
	pub fn set_dm_muted(&mut self, channel: Id, muted: bool) -> Option<Command> {
		if !self.is_one_to_one_dm(channel) {
			return None;
		}
		self.request_user_action(Action::Mute { channel, muted })
	}
	fn is_one_to_one_dm(&self, channel: Id) -> bool {
		self.channel(channel)
			.is_some_and(|c| c.guild.is_none() && c.kind == 1 && c.recipients.len() == 1)
	}
	fn request_user_action(&mut self, action: Action) -> Option<Command> {
		if self.user_action_pending() {
			return None;
		}
		if !self.demo && (self.auth != AuthState::Authenticated || !self.gateway_connected) {
			self.user_actions.status = Some("User actions unavailable while disconnected");
			self.status = self.user_actions.status.unwrap();
			return None;
		}
		self.user_actions.sequence = self.user_actions.sequence.wrapping_add(1);
		let request = self.user_actions.sequence;
		self.user_actions.pending = Some((action, request, false));
		self.user_actions.status = Some("Updating user settings…");
		self.status = self.user_actions.status.unwrap();
		Some(Command::UserAction { action, request })
	}
	pub(crate) fn cancel_user_action(&mut self) {
		if self.user_actions.pending.take().is_some() {
			self.user_actions.status =
				Some("Outcome unknown · check the official client before retrying");
		}
	}
	pub(crate) fn observe_dm_reopened(&mut self, channel: Id) {
		if let Some((Action::CloseDm(target), _, observed)) = &mut self.user_actions.pending
			&& *target == channel
		{
			*observed = true;
		}
	}
	pub(crate) fn observe_dm_settings(&mut self, event: &crate::notifications::Event) {
		if let Some((Action::Mute { channel, .. }, _, observed)) = &mut self.user_actions.pending {
			*observed |= match event {
				crate::notifications::Event::Invalidate => true,
				crate::notifications::Event::Settings { entries, replace } => {
					*replace
						|| entries.iter().any(|s| {
							s.guild.is_none() && s.channels.iter().any(|(id, ..)| id == channel)
						})
				}
				_ => false,
			};
		}
	}
	pub(crate) fn apply_user_action(&mut self, event: Event) -> Result<(), &'static str> {
		match event {
			Event::Relationships(entries) => {
				self.user_actions.relationships.clear();
				self.user_actions.known = false;
				if let Some(entries) = entries {
					if entries.len() > MAX_RELATIONSHIPS
						|| entries.capacity() * size_of::<(Id, bool)>() > MAX_RELATIONSHIP_BYTES
					{
						return Err("Relationships exceed safe capacity");
					}
					for (user, blocked) in entries {
						if user.0 == 0
							|| self
								.user_actions
								.relationships
								.insert(user, blocked)
								.is_some()
						{
							self.user_actions.relationships.clear();
							return Err("Relationships contain invalid or duplicate users");
						}
					}
					self.user_actions.known = true;
				}
			}
			Event::Relationship { user, blocked } => {
				self.store_relationship(user, blocked)?;
				if let Some((Action::Block { user: target, .. }, _, observed)) =
					&mut self.user_actions.pending
					&& *target == user
				{
					*observed = true;
				}
			}
			Event::Written {
				action,
				request,
				result,
			} => {
				let Some((pending, sequence, observed)) = self.user_actions.pending else {
					return Ok(());
				};
				if pending != action || sequence != request {
					return Ok(());
				}
				self.user_actions.pending = None;
				if let Err(failure) = result {
					self.user_actions.status = Some(failure.label());
					self.status = failure.label();
					if failure.ends_session() {
						self.fail(failure);
					}
					return Ok(());
				}
				if !observed {
					match action {
						Action::CloseDm(channel) => {
							self.remove_channels(&std::collections::BTreeSet::from([channel]));
							if self.selected == Some(channel) {
								self.selected = None;
							}
						}
						Action::Block { user, blocked } => {
							self.store_relationship(user, blocked)?
						}
						Action::Mute { channel, muted } => self.confirm_dm_muted(channel, muted)?,
					}
				}
				self.user_actions.status = Some(if observed {
					"Request completed · latest service settings shown"
				} else {
					match action {
						Action::CloseDm(_) => "DM closed · messages and drafts were not deleted",
						Action::Block { blocked: true, .. } => "User blocked",
						Action::Block { blocked: false, .. } => "User unblocked",
						Action::Mute { muted: true, .. } => {
							"DM notifications muted until you turn them back on"
						}
						Action::Mute { muted: false, .. } => "DM notifications unmuted",
					}
				});
				self.status = self.user_actions.status.unwrap();
			}
		}
		Ok(())
	}
	fn store_relationship(&mut self, user: Id, blocked: bool) -> Result<(), &'static str> {
		let entries = &mut self.user_actions.relationships;
		// Fixed-size IDs and booleans: <= 4000 entries and a conservative 32-byte entry estimate.
		if user.0 == 0
			|| (!entries.contains_key(&user)
				&& (entries.len() >= MAX_RELATIONSHIPS
					|| (entries.len() + 1) * 32 > MAX_RELATIONSHIP_BYTES))
		{
			return Err("Relationships exceed safe capacity or contain an invalid user");
		}
		entries.insert(user, blocked);
		self.read_state.activity.clear_notifications();
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Envelope, Event as CoreEvent};
	fn state() -> State {
		let user = |id| model::User {
			id: Id(id),
			name: "Synthetic".into(),
			avatar: None,
			discriminator: 0,
		};
		State {
			user: Some(user(1)),
			auth: AuthState::Authenticated,
			gateway_connected: true,
			selected: Some(Id(10)),
			channels: vec![model::Channel {
				id: Id(10),
				guild: None,
				kind: 1,
				recipients: vec![user(2)],
				name: "Synthetic DM".into(),
				parent_id: None,
				position: 0,
				last_message: None,
				member_list_id: None,
				message_count: None,
			}],
			..State::default()
		}
	}
	fn finish(state: &mut State, command: Command, result: Result<(), Failure>) {
		let Command::UserAction { action, request } = command else {
			panic!("wrong command")
		};
		state.apply(Envelope {
			generation: state.generation,
			event: CoreEvent::UserAction(Event::Written {
				action,
				request,
				result,
			}),
		});
	}
	#[test]
	fn user_actions_wait_for_success_preserve_drafts_and_reject_stale_results() {
		let mut state = state();
		assert_eq!(state.user_blocked(Id(2)), None);
		assert!(state.set_user_blocked(Id(1), true).is_none());
		assert!(state.set_user_blocked(Id(2), false).is_none());
		let block = state.set_user_blocked(Id(2), true).unwrap();
		assert_eq!(state.user_blocked(Id(2)), None);
		assert!(state.close_dm(Id(10)).is_none());
		finish(&mut state, block, Err(Failure::Forbidden));
		assert!(!state.user_action_pending());
		assert_eq!(state.user_blocked(Id(2)), None);
		assert_eq!(state.status, Failure::Forbidden.label());
		let block = state.set_user_blocked(Id(2), true).unwrap();
		finish(&mut state, block, Ok(()));
		assert_eq!(state.user_blocked(Id(2)), Some(true));
		let unblock = state.set_user_blocked(Id(2), false).unwrap();
		finish(&mut state, unblock, Ok(()));
		assert_eq!(state.user_blocked(Id(2)), Some(false));
		state.drafts.insert(Id(10), "Keep my draft".into());
		let close = state.close_dm(Id(10)).unwrap();
		finish(&mut state, close, Err(Failure::Ambiguous));
		assert!(state.channel(Id(10)).is_some());
		let close = state.close_dm(Id(10)).unwrap();
		finish(&mut state, close, Ok(()));
		assert!(state.channel(Id(10)).is_none());
		assert_eq!(state.selected, None);
		assert_eq!(state.drafts[&Id(10)], "Keep my draft");
		let block = state.set_user_blocked(Id(2), true).unwrap();
		state.cancel_user_action();
		finish(&mut state, block, Ok(()));
		assert_eq!(state.user_blocked(Id(2)), Some(false));
		state.gateway_connected = false;
		assert!(state.set_user_blocked(Id(2), true).is_none());
		state.demo = true;
		let command = state.set_user_blocked(Id(2), true).unwrap();
		state.command_rejected(command);
		assert!(!state.user_action_pending());
		assert_eq!(state.user_blocked(Id(2)), Some(false));
	}
	#[test]
	fn fresh_ready_does_not_reuse_an_outstanding_write_request() {
		let mut state = state();
		let old = state.set_user_blocked(Id(2), true).unwrap();
		let user = state.user.clone().unwrap();
		let channels = state.channels.clone();
		state.apply(Envelope {
			generation: state.generation,
			event: CoreEvent::Ready {
				permissions: Default::default(),
				user,
				guilds: vec![],
				channels,
			},
		});
		let new = state.set_user_blocked(Id(2), true).unwrap();
		finish(&mut state, old, Ok(()));
		assert!(state.user_action_pending());
		assert_eq!(state.user_blocked(Id(2)), None);
		finish(&mut state, new, Ok(()));
		assert_eq!(state.user_blocked(Id(2)), Some(true));
	}
	#[test]
	fn gateway_updates_win_over_late_writes_and_settings_stay_bounded() {
		let mut state = state();
		state
			.apply_user_action(Event::Relationships(Some(vec![(Id(2), false)])))
			.unwrap();
		let block = state.set_user_blocked(Id(2), true).unwrap();
		state
			.apply_user_action(Event::Relationship {
				user: Id(2),
				blocked: true,
			})
			.unwrap();
		state
			.apply_user_action(Event::Relationship {
				user: Id(2),
				blocked: false,
			})
			.unwrap();
		finish(&mut state, block, Ok(()));
		assert_eq!(state.user_blocked(Id(2)), Some(false));
		let mute = state.set_dm_muted(Id(10), true).unwrap();
		assert_eq!(state.dm_muted(Id(10)), None);
		finish(&mut state, mute, Ok(()));
		assert_eq!(state.dm_muted(Id(10)), Some(true));
		let unmute = state.set_dm_muted(Id(10), false).unwrap();
		state
			.apply_notification_preferences(crate::notifications::Event::Settings {
				entries: vec![crate::notifications::Setting {
					channels: vec![(Id(10), Some(true), Some(2))],
					..Default::default()
				}],
				replace: false,
			})
			.unwrap();
		finish(&mut state, unmute, Ok(()));
		assert_eq!(state.dm_muted(Id(10)), Some(true));
		assert_eq!(
			state.status,
			"Request completed · latest service settings shown"
		);
		let unmute = state.set_dm_muted(Id(10), false).unwrap();
		state
			.apply_notification_preferences(crate::notifications::Event::Settings {
				entries: vec![crate::notifications::Setting {
					guild: Some(Id(999)),
					..Default::default()
				}],
				replace: false,
			})
			.unwrap();
		finish(&mut state, unmute, Ok(()));
		assert_eq!(state.dm_muted(Id(10)), Some(false));
		let close = state.close_dm(Id(10)).unwrap();
		let channel = state.channel(Id(10)).unwrap().clone();
		state.apply(Envelope {
			generation: state.generation,
			event: CoreEvent::Unavailable(Id(10)),
		});
		state.apply(Envelope {
			generation: state.generation,
			event: CoreEvent::ChannelCreated(channel),
		});
		finish(&mut state, close, Ok(()));
		assert!(
			state.channel(Id(10)).is_some(),
			"late close must not remove a DM reopened by a later event"
		);
		assert!(
			state
				.apply_user_action(Event::Relationships(Some(vec![
					(Id(2), true);
					MAX_RELATIONSHIPS + 1
				])))
				.is_err()
		);
		assert_eq!(state.user_blocked(Id(2)), None);
		assert!(
			state
				.apply_user_action(Event::Relationships(Some(vec![
					(Id(2), true),
					(Id(2), false)
				])))
				.is_err()
		);
		state.logout();
		assert_eq!(state.dm_muted(Id(10)), None);
		assert_eq!(state.user_blocked(Id(2)), None);
	}
}
