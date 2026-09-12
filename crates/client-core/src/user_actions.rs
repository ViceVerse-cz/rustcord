//! Explicit account actions; one pending write, never an automatic retry.
use crate::{
	Command, State,
	auth::{AuthState, Failure},
};
use model::Id;
use std::collections::BTreeMap;

pub const MAX_RELATIONSHIPS: usize = 4000;
pub const MAX_RELATIONSHIP_BYTES: usize = 128 * 1024;
pub const MAX_FRIEND_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
	AddFriend { username: String },
	ResolveFriend { user: Id, accept: bool },
	CloseDm(Id),
	Block { user: Id, blocked: bool },
	Mute { channel: Id, muted: bool },
}

pub enum Event {
	Requests(Option<Vec<(model::User, String, bool)>>),
	Request {
		user: Id,
		incoming: Option<bool>,
		profile: Option<(model::User, String)>,
	},
	FriendProfile((model::User, String)),
	Friends(Option<Vec<(model::User, String)>>),
	Friend {
		user: Id,
		friend: bool,
		profile: Option<(model::User, String)>,
	},
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
	requests: BTreeMap<Id, (model::User, String, bool)>,
	requests_known: bool,
	last_requested: Option<String>,
	friends: BTreeMap<Id, (model::User, String)>,
	friends_known: bool,
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
	pub fn pending_friends(&self) -> impl Iterator<Item = &(model::User, String, bool)> {
		self.user_actions
			.requests
			.values()
			.filter(|(u, _, _)| self.user_blocked(u.id) == Some(false))
	}
	pub fn friend_requests_known(&self) -> bool {
		self.user_actions.requests_known
	}
	pub fn add_friend(&mut self, username: &str) -> Option<Command> {
		if username.len() > 132 {
			self.user_actions.status = Some("Enter a Discord username of at most 32 characters.");
			return None;
		}
		let username = username.trim().trim_start_matches('@').to_ascii_lowercase();
		if !valid_username(&username) {
			self.user_actions.status =
				Some("Enter a Discord username: 2–32 letters, numbers, underscores or periods.");
			return None;
		}
		if self.user_actions.last_requested.as_deref() == Some(&username)
			|| self
				.user_actions
				.friends
				.values()
				.any(|(_, n)| n == &username)
			|| self
				.user_actions
				.requests
				.values()
				.any(|(_, n, _)| n == &username)
		{
			self.user_actions.status =
				Some("Already friends or a request is pending. Check the Pending tab.");
			return None;
		}
		self.request_user_action(Action::AddFriend { username })
	}
	pub fn resolve_friend_request(&mut self, user: Id, accept: bool) -> Option<Command> {
		let (_, _, incoming) = self.user_actions.requests.get(&user)?;
		if (accept && !incoming) || self.user_blocked(user) != Some(false) {
			return None;
		}
		self.request_user_action(Action::ResolveFriend { user, accept })
	}
	pub fn friends(&self) -> impl Iterator<Item = &model::User> {
		self.user_actions
			.friends
			.values()
			.map(|(user, _)| user)
			.filter(|u| self.user_blocked(u.id) == Some(false))
	}
	pub fn friends_known(&self) -> bool {
		self.user_actions.friends_known
	}
	pub fn friend_username(&self, user: Id) -> Option<&str> {
		self.user_actions
			.friends
			.get(&user)
			.map(|(_, name)| name.as_str())
	}
	pub fn user_blocked(&self, user: Id) -> Option<bool> {
		if let Some((
			Action::Block {
				user: target,
				blocked,
			},
			_,
			false,
		)) = &self.user_actions.pending
			&& *target == user
		{
			return Some(*blocked);
		}
		self.user_actions
			.relationships
			.get(&user)
			.copied()
			.or_else(|| (self.demo || self.user_actions.known).then_some(false))
	}
	pub(crate) fn pending_dm_muted(&self, channel: Id) -> Option<bool> {
		match &self.user_actions.pending {
			Some((
				Action::Mute {
					channel: target,
					muted,
				},
				_,
				false,
			)) if *target == channel => Some(*muted),
			_ => None,
		}
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
		if self.server_invite_pending()
			|| self.pending.iter().any(|p| p.channel == channel)
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
		if !self.is_one_to_one_dm(channel) && !self.is_group_dm(channel) {
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
		self.user_actions.pending = Some((action.clone(), request, false));
		self.user_actions.status = None;
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
		if let Some((Action::Mute { .. }, _, observed)) = &mut self.user_actions.pending {
			*observed |= match event {
				crate::notifications::Event::Invalidate => true,
				crate::notifications::Event::Settings { entries, replace } => {
					*replace || entries.iter().any(|s| s.guild.is_none())
				}
				_ => false,
			};
		}
	}
	pub(crate) fn apply_user_action(&mut self, event: Event) -> Result<(), &'static str> {
		match event {
			Event::Requests(entries) => {
				self.user_actions.requests.clear();
				self.user_actions.requests_known = false;
				if let Some(entries) = entries {
					if entries.len() > MAX_RELATIONSHIPS
						|| entries.capacity() * size_of::<(model::User, String, bool)>()
							+ entries
								.iter()
								.map(|(u, n, _)| u.heap_bytes() + n.capacity() + 64)
								.sum::<usize>() > MAX_FRIEND_BYTES
					{
						return Err("Friend requests exceed safe capacity");
					}
					for (user, name, incoming) in entries {
						if self.user_actions.requests.contains_key(&user.id) {
							return Err("Duplicate friend request");
						}
						self.apply_user_action(Event::Request {
							user: user.id,
							incoming: Some(incoming),
							profile: Some((user, name)),
						})?;
					}
					self.user_actions.requests_known = true;
				}
			}
			Event::Request {
				user,
				incoming,
				profile,
			} => {
				if user.0 == 0 {
					return Err("Invalid friend request");
				}
				if let Some((Action::ResolveFriend { user: target, .. }, _, observed)) =
					&mut self.user_actions.pending
					&& *target == user
				{
					*observed = true;
				}
				if let Some(incoming) = incoming {
					let profile = profile.or_else(|| {
						(!self.user_actions.requests.contains_key(&user)).then(|| {
							(
								model::User {
									id: user,
									name: "Unknown user".into(),
									avatar: None,
									discriminator: 0,
									webhook: false,
								},
								format!("User ID: {user}"),
							)
						})
					});
					if let Some((record, name)) = profile {
						if user != record.id || !valid_friend(&record, &name) {
							return Err("Invalid friend request profile");
						}
						let entries = &mut self.user_actions.requests;
						let bytes = entries
							.iter()
							.filter(|(id, _)| **id != user)
							.map(|(_, (u, n, _))| {
								u.heap_bytes()
									+ n.capacity() + size_of::<(Id, model::User, String, bool)>()
									+ 64
							})
							.sum::<usize>();
						if (!entries.contains_key(&user) && entries.len() >= MAX_RELATIONSHIPS)
							|| bytes
								+ record.heap_bytes() + name.capacity()
								+ size_of::<(Id, model::User, String, bool)>()
								+ 64 > MAX_FRIEND_BYTES
						{
							return Err("Friend requests exceed safe capacity");
						}
						entries.insert(user, (record, name, incoming));
					} else if let Some(entry) = self.user_actions.requests.get_mut(&user) {
						entry.2 = incoming;
					}
				} else if let Some((_, name, _)) = self.user_actions.requests.remove(&user)
					&& self.user_actions.last_requested.as_deref() == Some(&name)
				{
					self.user_actions.last_requested = None;
				}
			}
			Event::FriendProfile(profile) => {
				if self.user_actions.friends.contains_key(&profile.0.id) {
					return self.apply_user_action(Event::Friend {
						user: profile.0.id,
						friend: true,
						profile: Some(profile),
					});
				}
			}
			Event::Friends(entries) => {
				self.user_actions.friends.clear();
				self.user_actions.friends_known = false;
				if let Some(entries) = entries {
					if entries.len() > MAX_RELATIONSHIPS
						|| entries.capacity() * size_of::<(model::User, String)>()
							+ entries
								.iter()
								.map(|(u, n)| u.heap_bytes() + n.capacity() + 64)
								.sum::<usize>() > MAX_FRIEND_BYTES
					{
						return Err("Friends exceed safe capacity");
					}
					for (user, name) in entries {
						if !valid_friend(&user, &name)
							|| self
								.user_actions
								.friends
								.insert(user.id, (user, name))
								.is_some()
						{
							self.user_actions.friends.clear();
							return Err("Friends contain invalid or duplicate users");
						}
					}
					self.user_actions.friends_known = true;
				}
			}
			Event::Friend {
				user,
				friend,
				profile,
			} => {
				let profile = profile.or_else(|| {
					self.user_actions
						.requests
						.get(&user)
						.map(|(u, n, _)| (u.clone(), n.clone()))
				});
				if !friend {
					self.user_actions.friends.remove(&user);
				} else if let Some((record, name)) = profile {
					let entries = &mut self.user_actions.friends;
					if user != record.id || !valid_friend(&record, &name) {
						return Err("Invalid friend update");
					}
					let old = entries.get(&user).map_or(0, |(u, n)| {
						u.heap_bytes() + n.capacity() + size_of::<(Id, model::User, String)>() + 64
					});
					let bytes: usize = entries
						.values()
						.map(|(u, n)| {
							u.heap_bytes()
								+ n.capacity() + size_of::<(Id, model::User, String)>()
								+ 64
						})
						.sum();
					if (!entries.contains_key(&user) && entries.len() >= MAX_RELATIONSHIPS)
						|| bytes - old
							+ record.heap_bytes() + name.capacity()
							+ size_of::<(Id, model::User, String)>()
							+ 64 > MAX_FRIEND_BYTES
					{
						return Err("Friends exceed safe capacity");
					}
					entries.insert(user, (record, name));
				}
			}
			Event::Relationships(entries) => {
				if let Some((Action::Block { .. }, _, observed)) = &mut self.user_actions.pending {
					*observed = true;
				}
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
				let Some((pending, sequence, observed)) = &self.user_actions.pending else {
					return Ok(());
				};
				if *pending != action || *sequence != request {
					return Ok(());
				}
				let observed = *observed;
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
						Action::AddFriend { ref username } => {
							self.user_actions.last_requested = Some(username.clone())
						}
						Action::ResolveFriend { user, accept } => {
							if let Some((record, name, _)) =
								self.user_actions.requests.remove(&user)
							{
								self.user_actions.last_requested = None;
								if accept {
									self.apply_user_action(Event::Friend {
										user,
										friend: true,
										profile: Some((record, name)),
									})?;
								}
							}
						}
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
						Action::AddFriend { .. } => {
							"Friend request sent · waiting for service update"
						}
						Action::ResolveFriend { accept: true, .. } => "Friend request accepted",
						Action::ResolveFriend { accept: false, .. } => "Friend request removed",
						Action::CloseDm(_) => "DM closed · messages and drafts were not deleted",
						Action::Block { blocked: true, .. } => "User blocked",
						Action::Block { blocked: false, .. } => "User unblocked",
						Action::Mute { muted: true, .. } => {
							"Conversation notifications muted until you turn them back on"
						}
						Action::Mute { muted: false, .. } => "Conversation notifications unmuted",
					}
				});
				self.status = self.user_actions.status.unwrap();
			}
		}
		Ok(())
	}
	fn store_relationship(&mut self, user: Id, blocked: bool) -> Result<(), &'static str> {
		if blocked {
			self.user_actions.friends.remove(&user);
			self.user_actions.requests.remove(&user);
		}
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

pub fn valid_username(name: &str) -> bool {
	(2..=32).contains(&name.len())
		&& !name.contains("..")
		&& name
			.bytes()
			.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.'))
}
fn valid_friend(user: &model::User, username: &str) -> bool {
	user.id.0 != 0
		&& !user.name.is_empty()
		&& user.name.len() <= 512
		&& !user.name.chars().any(char::is_control)
		&& !username.is_empty()
		&& username.len() <= 128
		&& !username.chars().any(char::is_control)
		&& user
			.avatar
			.as_ref()
			.is_none_or(|hash| model::valid_avatar_hash(hash))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Envelope, Event as CoreEvent};
	#[test]
	fn friend_requests_validate_bound_and_reconcile_without_duplicate_writes() {
		let mut state = state();
		let user = state.channels[0].recipients[0].clone();
		state
			.apply_user_action(Event::Relationships(Some(vec![])))
			.unwrap();
		state
			.apply_user_action(Event::Requests(Some(vec![(
				user.clone(),
				"synthetic".into(),
				true,
			)])))
			.unwrap();
		assert_eq!(state.pending_friends().count(), 1);
		assert!(state.add_friend("../invalid").is_none());
		let send = state.add_friend(" @new_friend ").unwrap();
		assert!(state.add_friend("new_friend").is_none());
		assert!(state.resolve_friend_request(user.id, true).is_none());
		finish(&mut state, send, Ok(()));
		assert!(state.add_friend("new_friend").is_none());
		assert_eq!(
			state.pending_friends().count(),
			1,
			"no invented outgoing identity"
		);
		let accept = state.resolve_friend_request(user.id, true).unwrap();
		finish(&mut state, accept, Err(Failure::Forbidden));
		assert_eq!(state.pending_friends().count(), 1);
		let accept = state.resolve_friend_request(user.id, true).unwrap();
		state
			.apply_user_action(Event::Request {
				user: user.id,
				incoming: None,
				profile: None,
			})
			.unwrap();
		finish(&mut state, accept, Ok(()));
		assert_eq!(
			state.friends().count(),
			0,
			"newer Gateway removal wins over acceptance"
		);
		state
			.apply_user_action(Event::Request {
				user: user.id,
				incoming: Some(false),
				profile: Some((user.clone(), "synthetic".into())),
			})
			.unwrap();
		assert!(state.resolve_friend_request(user.id, true).is_none());
		let cancel = state.resolve_friend_request(user.id, false).unwrap();
		finish(&mut state, cancel, Ok(()));
		assert_eq!(state.pending_friends().count(), 0);
		state
			.apply_user_action(Event::Request {
				user: user.id,
				incoming: Some(true),
				profile: Some((user.clone(), "synthetic".into())),
			})
			.unwrap();
		let accept = state.resolve_friend_request(user.id, true).unwrap();
		state
			.apply_user_action(Event::Friend {
				user: user.id,
				friend: true,
				profile: None,
			})
			.unwrap();
		state
			.apply_user_action(Event::Request {
				user: user.id,
				incoming: None,
				profile: None,
			})
			.unwrap();
		assert_eq!(
			state.friends().count(),
			1,
			"acceptance without repeated metadata retains the request profile"
		);
		finish(&mut state, accept, Ok(()));
		assert_eq!(state.friends().count(), 1);
		assert_eq!(state.pending_friends().count(), 0);
		assert!(
			state
				.apply_user_action(Event::Requests(Some(vec![(
					user,
					"huge".repeat(MAX_FRIEND_BYTES),
					true
				)])))
				.is_err()
		);
		state.gateway_connected = false;
		assert!(state.add_friend("another").is_none());
		state.logout();
		assert!(!state.friend_requests_known());
		assert_eq!(state.pending_friends().count(), 0);
	}
	fn state() -> State {
		let user = |id| model::User {
			id: Id(id),
			name: "Synthetic".into(),
			avatar: None,
			webhook: false,
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
				icon: None,
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
	fn friends_are_explicit_bounded_and_removed_by_relationship_changes() {
		let mut state = state();
		let user = state.channels[0].recipients[0].clone();
		state
			.apply_user_action(Event::Relationships(Some(vec![(user.id, false)])))
			.unwrap();
		state
			.apply_user_action(Event::Friends(Some(vec![(
				user.clone(),
				"synthetic_friend".into(),
			)])))
			.unwrap();
		assert!(state.friends_known());
		assert_eq!(state.friends().count(), 1);
		assert_eq!(state.friend_username(user.id), Some("synthetic_friend"));
		let mut changed = user.clone();
		changed.name = "New display".into();
		state
			.apply_user_action(Event::FriendProfile((changed, "new_username".into())))
			.unwrap();
		assert_eq!(state.friends().next().unwrap().name, "New display");
		state
			.apply_user_action(Event::Friend {
				user: user.id,
				friend: false,
				profile: None,
			})
			.unwrap();
		assert_eq!(state.friends().count(), 0);
		state
			.apply_user_action(Event::Friend {
				user: user.id,
				friend: true,
				profile: Some((user.clone(), "user".into())),
			})
			.unwrap();
		let command = state.set_user_blocked(user.id, true).unwrap();
		assert_eq!(state.friends().count(), 0);
		finish(&mut state, command, Err(Failure::Forbidden));
		assert_eq!(state.friends().count(), 1);
		state
			.apply_user_action(Event::Relationship {
				user: user.id,
				blocked: true,
			})
			.unwrap();
		assert_eq!(state.friends().count(), 0);
		assert!(
			state
				.apply_user_action(Event::Friends(Some(vec![(
					user,
					"x".repeat(MAX_FRIEND_BYTES)
				)])))
				.is_err()
		);
		assert!(!state.friends_known());
		state.logout();
		assert_eq!(state.friends().count(), 0);
	}
	#[test]
	fn user_actions_update_immediately_rollback_and_reject_stale_results() {
		let mut state = state();
		assert_eq!(state.user_blocked(Id(2)), None);
		assert!(state.set_user_blocked(Id(1), true).is_none());
		assert!(state.set_user_blocked(Id(2), false).is_none());
		let block = state.set_user_blocked(Id(2), true).unwrap();
		assert_eq!(state.user_blocked(Id(2)), Some(true));
		assert_eq!(state.user_action_status(), None);
		assert!(state.close_dm(Id(10)).is_none());
		finish(&mut state, block, Err(Failure::Forbidden));
		assert!(!state.user_action_pending());
		assert_eq!(state.user_blocked(Id(2)), None);
		assert_eq!(state.status, Failure::Forbidden.label());
		let block = state.set_user_blocked(Id(2), true).unwrap();
		finish(&mut state, block, Ok(()));
		assert_eq!(state.user_blocked(Id(2)), Some(true));
		let unblock = state.set_user_blocked(Id(2), false).unwrap();
		assert_eq!(state.user_blocked(Id(2)), Some(false));
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
		assert_eq!(state.user_blocked(Id(2)), Some(true));
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
		assert_eq!(state.dm_muted(Id(10)), Some(true));
		finish(&mut state, mute, Err(Failure::Forbidden));
		assert_eq!(state.dm_muted(Id(10)), None);
		let mute = state.set_dm_muted(Id(10), true).unwrap();
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
