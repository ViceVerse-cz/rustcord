//! Guild voice signaling and unofficial DM calls. Never joins on incoming events or reconnect.
use client_core::{
	Event,
	auth::Failure,
	screen,
	voice::{self, Command, Participant, RosterEntry, Secret},
};
use discord_protocol::{
	CallDto, GuildDto, UserDto, VoiceMemberDto, VoiceServerDto, VoiceStateDto, decode, stream,
};
use model::{Id, Member, User};
use serde_json::json;
use std::{collections::BTreeMap, time::Duration};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message as Frame;
use zeroize::Zeroizing;

struct StreamAttempt {
	channel: Id,
	request: u64,
	stream_request: u64,
	key: String,
	created: Option<(Id, Id)>,
	departing: bool,
}

#[derive(Default)]
pub(super) struct Calls {
	pub(super) allowed: BTreeMap<Id, Option<Id>>,
	pub(super) active: Option<(Id, u64)>,
	active_guild: Option<Id>,
	muted: bool,
	deafened: bool,
	camera: bool,
	departing: Option<(Id, u64)>,
	departing_guild: Option<Id>,
	pub(super) departure_deadline: Option<Instant>,
	stream: Option<StreamAttempt>,
	// Only retained between READY and READY_SUPPLEMENTAL, with the roster's item/byte budget.
	pub(super) users: BTreeMap<Id, User>,
}
impl Calls {
	pub(super) fn disconnected(&mut self) {
		self.active = None;
		self.active_guild = None;
		self.muted = false;
		self.deafened = false;
		self.camera = false;
		self.departing = None;
		self.departure_deadline = None;
		self.stream = None;
		self.users.clear();
	}
	pub(super) fn invalidate(&mut self, channel: Id) {
		self.allowed.remove(&channel);
		if self.active.is_some_and(|(active, _)| active == channel) {
			self.active = None;
			self.active_guild = None;
			self.stream = None;
			self.camera = false;
		}
	}
	pub(super) fn departure_expired(&mut self) -> Option<Event> {
		self.departure_deadline = None;
		self.departing.map(|(channel, request)| {
			Event::Voice(voice::Event::Failed {
				channel,
				request,
				message: "Discord did not acknowledge hangup; reconnect before calling again",
			})
		})
	}
	pub(super) fn remember_users(&mut self, users: Vec<UserDto>) -> Result<(), Failure> {
		if users.len() > voice::MAX_ROSTER {
			return Err(Failure::Capacity);
		}
		self.users = users
			.into_iter()
			.map(|u| {
				let u = u.into_model();
				(u.id, u)
			})
			.collect();
		if self
			.users
			.values()
			.map(|u| size_of::<User>() + u.heap_bytes())
			.sum::<usize>()
			> voice::MAX_ROSTER_BYTES
		{
			return Err(Failure::Capacity);
		}
		Ok(())
	}
	fn members(&self, members: Vec<VoiceMemberDto>) -> Result<BTreeMap<Id, Member>, Failure> {
		if members.len() > voice::MAX_ROSTER {
			return Err(Failure::Capacity);
		}
		let members: BTreeMap<_, _> = members
			.into_iter()
			.filter_map(|m| self.member(m).map(|m| (m.user.id, m)))
			.collect();
		if members.values().map(Member::bytes).sum::<usize>() > voice::MAX_ROSTER_BYTES {
			return Err(Failure::Capacity);
		}
		Ok(members)
	}
	fn member(&self, member: VoiceMemberDto) -> Option<Member> {
		Some(Member {
			roles: vec![],
			user: member
				.user
				.map(UserDto::into_model)
				.or_else(|| member.user_id.and_then(|id| self.users.get(&id).cloned()))?,
			nick: member.nick.map(|n| n.chars().take(128).collect()),
			status: None,
			custom_status: None,
			activities: vec![],
		})
	}
	pub(super) fn snapshot(&self, guild: &mut GuildDto, partial: bool) -> Result<Event, Failure> {
		if guild.voice_states.len() > voice::MAX_ROSTER {
			return Err(Failure::Capacity);
		}
		let members = self.members(std::mem::take(&mut guild.members))?;
		let mut participants = Vec::new();
		let mut bytes = 0;
		for mut state in std::mem::take(&mut guild.voice_states) {
			let _secret = state.session_id.take().map(Zeroizing::new);
			let Some(channel) = state.channel_id else {
				continue;
			};
			if self.allowed.get(&channel) != Some(&Some(guild.id)) {
				continue;
			}
			let participant = participant(&state);
			let member = state
				.member
				.and_then(|m| self.member(m))
				.or_else(|| members.get(&state.user_id).cloned());
			let entry = RosterEntry {
				guild: guild.id,
				channel,
				participant,
				member,
			};
			bytes += entry.bytes();
			if bytes > voice::MAX_ROSTER_BYTES {
				return Err(Failure::Capacity);
			}
			participants.push(entry);
		}
		Ok(Event::Voice(voice::Event::Snapshot {
			partial,
			guild: Some(guild.id),
			participants,
		}))
	}
	pub(super) fn passive(
		&mut self,
		mut update: discord_protocol::PassiveVoiceUpdate,
		owner: Option<Id>,
		emit: &impl Fn(Event) -> Result<(), Failure>,
	) -> Result<(), Failure> {
		if update.updated_voice_states.len() + update.removed_voice_states.len() > voice::MAX_ROSTER
		{
			return Err(Failure::Capacity);
		}
		let Some(guild) = update.guild_id else {
			return if update.updated_voice_states.is_empty()
				&& update.removed_voice_states.is_empty()
			{
				Ok(())
			} else {
				Err(Failure::Protocol)
			};
		};
		let members = self.members(update.updated_members)?;
		for mut state in update.updated_voice_states.drain(..) {
			state.guild_id = Some(guild);
			let member = members.get(&state.user_id).cloned();
			self.state(state, member, owner, emit)?;
		}
		for user in update.removed_voice_states {
			self.state(
				VoiceStateDto {
					guild_id: Some(guild),
					channel_id: None,
					user_id: user,
					member: None,
					session_id: None,
					self_mute: false,
					self_deaf: false,
					mute: false,
					deaf: false,
					suppress: false,
				},
				None,
				owner,
				emit,
			)?;
		}
		Ok(())
	}
	pub(super) fn packet(&mut self, command: Command) -> Result<Option<Frame>, Failure> {
		let (channel, guild) = match command {
			Command::Join {
				channel, request, ..
			} => {
				let guild = *self.allowed.get(&channel).ok_or(Failure::Protocol)?;
				if self.active.is_some() || self.departing.is_some() {
					return Err(Failure::Protocol);
				}
				self.active = Some((channel, request));
				self.active_guild = guild;
				self.muted = false;
				self.deafened = false;
				self.camera = false;
				(Some(channel), guild)
			}
			Command::Leave { channel, request } => {
				if self.active != Some((channel, request)) {
					return Ok(None);
				}
				self.departing = self.active.take();
				self.departing_guild = self.active_guild;
				self.departure_deadline = Some(Instant::now() + Duration::from_secs(10));
				self.stream = None;
				self.muted = true;
				self.deafened = true;
				self.camera = false;
				(None, self.active_guild)
			}
			Command::SetMute {
				channel,
				request,
				mute,
				deaf,
			} => {
				if self.active != Some((channel, request)) {
					return Ok(None);
				}
				if self.allowed.get(&channel) != Some(&self.active_guild) {
					return Err(Failure::Forbidden);
				}
				self.muted = mute || deaf;
				self.deafened = deaf;
				(Some(channel), self.active_guild)
			}
			Command::SetCamera {
				channel,
				request,
				enabled,
			} => {
				if self.active != Some((channel, request)) {
					return Ok(None);
				}
				if enabled && self.allowed.get(&channel) != Some(&self.active_guild) {
					return Err(Failure::Forbidden);
				}
				self.camera = enabled;
				(Some(channel), self.active_guild)
			}
			Command::StartStream { .. }
			| Command::StopStream { .. }
			| Command::Decline { .. }
			| Command::Ring { .. } => return Ok(None),
		};
		Ok(Some(Frame::Text(json!({"op":4,"d":{"guild_id":guild,"channel_id":channel,"self_mute":self.muted,"self_deaf":self.deafened,"self_video":self.camera}}).to_string().into())))
	}
	pub(super) fn stream_packet(
		&mut self,
		command: Command,
		owner: Option<Id>,
	) -> Result<Option<Frame>, Failure> {
		match command {
			Command::StartStream {
				channel,
				request,
				stream_request,
			} => {
				let owner = owner.filter(|id| id.0 != 0).ok_or(Failure::Protocol)?;
				if self.stream.is_some()
					|| self.active != Some((channel, request))
					|| self.allowed.get(&channel) != Some(&self.active_guild)
				{
					return Err(Failure::Forbidden);
				}
				let (kind, key) = self.active_guild.map_or_else(
					|| ("call", format!("call:{channel}:{owner}")),
					|guild| ("guild", format!("guild:{guild}:{channel}:{owner}")),
				);
				self.stream = Some(StreamAttempt {
					channel,
					request,
					stream_request,
					key,
					created: None,
					departing: false,
				});
				Ok(Some(Frame::Text(
					json!({"op":18,"d":{"type":kind,"guild_id":self.active_guild,"channel_id":channel,"preferred_region":null}})
						.to_string()
						.into(),
				)))
			}
			Command::StopStream {
				channel,
				request,
				stream_request,
			} => {
				let Some(stream) = &mut self.stream else {
					return Ok(None);
				};
				if (stream.channel, stream.request, stream.stream_request)
					!= (channel, request, stream_request)
					|| stream.departing
				{
					return Ok(None);
				}
				stream.departing = true;
				Ok(Some(Frame::Text(
					json!({"op":19,"d":{"stream_key":stream.key}})
						.to_string()
						.into(),
				)))
			}
			_ => Ok(None),
		}
	}
	fn emit_stream(
		&self,
		event: screen::Event,
		emit: &impl Fn(Event) -> Result<(), Failure>,
	) -> Result<(), Failure> {
		if let Some(stream) = &self.stream {
			emit(Event::Voice(voice::Event::Stream {
				channel: stream.channel,
				request: stream.request,
				stream_request: stream.stream_request,
				event,
			}))?;
		}
		Ok(())
	}
	fn stream_dispatch(
		&mut self,
		kind: &str,
		data: &[u8],
		emit: &impl Fn(Event) -> Result<(), Failure>,
	) -> Result<(), Failure> {
		let Ok(key) = decode::<stream::Key>(data) else {
			return Ok(());
		};
		if let Some(stream) = &self.stream
			&& (self.active != Some((stream.channel, stream.request))
				|| self.allowed.get(&stream.channel) != Some(&self.active_guild))
		{
			self.stream = None;
			return Ok(());
		}
		if key.stream_key.len() > 128
			|| self
				.stream
				.as_ref()
				.is_none_or(|stream| stream.key != key.stream_key)
		{
			return Ok(());
		}
		if kind != "STREAM_DELETE" && self.stream.as_ref().is_some_and(|stream| stream.departing) {
			return Ok(());
		}
		match kind {
			"STREAM_CREATE" => {
				let Ok(created) = decode::<stream::Created>(data) else {
					return self.emit_stream(
						screen::Event::Failed("Discord sent invalid screen-share setup"),
						emit,
					);
				};
				let ids = (created.rtc_server_id, created.rtc_channel_id);
				match self.stream.as_ref().and_then(|stream| stream.created) {
					Some(previous) if previous != ids => self.emit_stream(
						screen::Event::Failed("Discord changed screen-share connection identity"),
						emit,
					)?,
					Some(_) => {}
					None => {
						self.stream.as_mut().unwrap().created = Some(ids);
						self.emit_stream(
							screen::Event::Created {
								rtc_server: ids.0,
								rtc_channel: ids.1,
							},
							emit,
						)?;
					}
				}
			}
			"STREAM_SERVER_UPDATE" => {
				let Ok(mut server) = decode::<stream::ServerUpdate>(data) else {
					return self.emit_stream(
						screen::Event::Failed("Discord sent invalid screen-share server data"),
						emit,
					);
				};
				if server.token.len() > 2048
					|| server
						.endpoint
						.as_ref()
						.is_some_and(|endpoint| endpoint.len() > 512)
				{
					return self.emit_stream(
						screen::Event::Failed("Discord sent oversized screen-share server data"),
						emit,
					);
				}
				let token = Zeroizing::new(std::mem::take(&mut server.token));
				let Ok(token) = Secret::new(token.to_string()) else {
					return self.emit_stream(
						screen::Event::Failed("Discord sent invalid screen-share server data"),
						emit,
					);
				};
				self.emit_stream(
					screen::Event::Server {
						token: Some(token),
						endpoint: server.endpoint,
					},
					emit,
				)?;
			}
			"STREAM_DELETE" => {
				if decode::<stream::Deleted>(data).is_err() {
					return self.emit_stream(
						screen::Event::Failed("Discord sent invalid screen-share deletion"),
						emit,
					);
				}
				let stream = self.stream.take().unwrap();
				emit(Event::Voice(voice::Event::Stream {
					channel: stream.channel,
					request: stream.request,
					stream_request: stream.stream_request,
					event: screen::Event::Deleted,
				}))?;
			}
			_ => {}
		}
		Ok(())
	}
	fn state(
		&mut self,
		mut state: VoiceStateDto,
		member: Option<Member>,
		owner: Option<Id>,
		emit: &impl Fn(Event) -> Result<(), Failure>,
	) -> Result<(), Failure> {
		let session = state.session_id.take().map(Zeroizing::new);
		let own = owner == Some(state.user_id);
		if own && self.departing.is_some() {
			if state.guild_id == self.departing_guild && state.channel_id.is_none() {
				self.departing = None;
				self.departure_deadline = None;
			}
			// Roster departure still applies, but this old ack must never be retagged to the next call.
			if state.guild_id.is_none() {
				return Ok(());
			}
		}
		let allowed = state
			.channel_id
			.is_none_or(|channel| self.allowed.get(&channel) == Some(&state.guild_id));
		if !allowed && !own {
			return Ok(());
		}
		let request = self.active.and_then(|(_, request)| {
			(own || self.active_guild == state.guild_id).then_some(request)
		});
		let matches_active = allowed
			&& self
				.active
				.is_some_and(|(channel, _)| state.channel_id == Some(channel))
			&& self.active_guild == state.guild_id;
		let secret = if own && matches_active {
			session.map(|s| Secret::new(s.to_string())).transpose()?
		} else {
			None
		};
		let participant = participant(&state);
		emit(Event::Voice(voice::Event::State {
			request,
			guild: state.guild_id,
			channel: state.channel_id.filter(|_| allowed),
			user: state.user_id,
			session: secret,
			member: state
				.member
				.and_then(|m| self.member(m))
				.or(member)
				.map(Box::new),
			muted: participant.muted,
			deafened: participant.deafened,
			server_muted: participant.server_muted,
			server_deafened: participant.server_deafened,
		}))?;
		if own && self.active.is_some() && !matches_active {
			self.active = None;
			self.active_guild = None;
			self.stream = None;
			self.camera = false;
		}
		Ok(())
	}
	pub(super) fn dispatch(
		&mut self,
		kind: &str,
		data: &[u8],
		owner: Option<Id>,
		emit: &impl Fn(Event) -> Result<(), Failure>,
	) -> Result<(), Failure> {
		match kind {
			"STREAM_CREATE" | "STREAM_SERVER_UPDATE" | "STREAM_DELETE" => {
				self.stream_dispatch(kind, data, emit)?;
			}
			"CALL_CREATE" | "CALL_UPDATE" | "CALL_DELETE" => {
				let call: CallDto = decode(data).map_err(|_| Failure::Protocol)?;
				if self.allowed.get(&call.channel_id) != Some(&None) {
					return Ok(());
				}
				if kind == "CALL_DELETE" {
					if self
						.departing
						.is_some_and(|(channel, _)| channel == call.channel_id)
					{
						self.departing = None;
						self.departure_deadline = None;
					}
					if self
						.active
						.is_some_and(|(channel, _)| channel == call.channel_id)
					{
						self.active = None;
						self.active_guild = None;
						self.stream = None;
						self.camera = false;
					}
					emit(Event::Voice(voice::Event::Deleted {
						channel: call.channel_id,
					}))?;
				} else {
					if call.ringing.as_ref().is_some_and(|v| v.len() > 2)
						|| call.voice_states.as_ref().is_some_and(|v| v.len() > 2)
					{
						return Err(Failure::Capacity);
					}
					let participants = call.voice_states.map(|states| {
						states
							.into_iter()
							.map(|mut state| {
								let _secret = state.session_id.take().map(Zeroizing::new);
								participant(&state)
							})
							.collect()
					});
					emit(Event::Voice(voice::Event::Call {
						channel: call.channel_id,
						ringing: call.ringing,
						participants,
						unavailable: call.unavailable,
					}))?;
				}
			}
			"VOICE_STATE_UPDATE" => self.state(
				decode(data).map_err(|_| Failure::Protocol)?,
				None,
				owner,
				emit,
			)?,
			"VOICE_SERVER_UPDATE" => {
				let mut server: VoiceServerDto = decode(data).map_err(|_| Failure::Protocol)?;
				let token = Zeroizing::new(std::mem::take(&mut server.token));
				let Some((channel, request)) = self.active else {
					return Ok(());
				};
				if server.guild_id != self.active_guild
					|| self.allowed.get(&channel) != Some(&self.active_guild)
					|| (self.active_guild.is_none() && server.channel_id != Some(channel))
				{
					return Ok(());
				}
				if server.endpoint.as_ref().is_some_and(|s| s.len() > 512) {
					return Err(Failure::Capacity);
				}
				emit(Event::Voice(voice::Event::Server {
					request,
					channel,
					token: Some(Secret::new(token.to_string())?),
					endpoint: server.endpoint,
				}))?;
			}
			_ => {}
		}
		Ok(())
	}
}
fn participant(state: &VoiceStateDto) -> Participant {
	Participant {
		user: state.user_id,
		muted: state.self_mute || state.mute || state.suppress || state.self_deaf || state.deaf,
		deafened: state.self_deaf || state.deaf,
		server_muted: state.mute || state.suppress,
		server_deafened: state.deaf,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::sync::Mutex;
	#[test]
	fn camera_controls_preserve_mute_and_reject_stale_or_revoked_enable() {
		let mut calls = Calls::default();
		let channel = Id(20);
		calls.allowed.insert(channel, Some(Id(10)));
		calls
			.packet(Command::Join {
				channel,
				request: 1,
				ring: false,
			})
			.unwrap();
		calls
			.packet(Command::SetMute {
				channel,
				request: 1,
				mute: true,
				deaf: true,
			})
			.unwrap();
		for command in [
			Command::SetCamera {
				channel,
				request: 1,
				enabled: true,
			},
			Command::SetMute {
				channel,
				request: 1,
				mute: true,
				deaf: true,
			},
		] {
			let Frame::Text(frame) = calls.packet(command).unwrap().unwrap() else {
				panic!("voice state")
			};
			let value: serde_json::Value = serde_json::from_str(&frame).unwrap();
			assert_eq!(value["d"]["self_video"], true);
			assert_eq!(value["d"]["self_mute"], true);
			assert_eq!(value["d"]["self_deaf"], true);
		}
		assert!(
			calls
				.packet(Command::SetCamera {
					channel,
					request: 2,
					enabled: false
				})
				.unwrap()
				.is_none()
		);
		assert!(calls.camera);
		calls.allowed.clear();
		assert!(matches!(
			calls.packet(Command::SetCamera {
				channel,
				request: 1,
				enabled: true
			}),
			Err(Failure::Forbidden)
		));
		assert!(
			calls
				.packet(Command::SetCamera {
					channel,
					request: 1,
					enabled: false
				})
				.unwrap()
				.is_some()
		);
		assert!(!calls.camera);
	}
	#[test]
	fn revoked_voice_denies_mute_and_server_secrets_but_allows_departure() {
		let mut calls = Calls::default();
		calls.allowed.insert(Id(20), Some(Id(10)));
		calls
			.packet(Command::Join {
				channel: Id(20),
				request: 1,
				ring: false,
			})
			.unwrap();
		calls.allowed.remove(&Id(20));
		assert!(matches!(
			calls.packet(Command::SetMute {
				channel: Id(20),
				request: 1,
				mute: false,
				deaf: false
			}),
			Err(Failure::Forbidden)
		));
		let events = Mutex::new(Vec::new());
		let emit = |event| {
			events.lock().unwrap().push(event);
			Ok(())
		};
		calls.dispatch("VOICE_SERVER_UPDATE", br#"{"guild_id":"10","token":"synthetic-revoked-secret","endpoint":"voice.discord.media:443"}"#, Some(Id(1)), &emit).unwrap();
		assert!(events.lock().unwrap().is_empty());
		assert!(
			calls
				.packet(Command::Leave {
					channel: Id(20),
					request: 1
				})
				.unwrap()
				.is_some()
		);
		calls
			.dispatch(
				"VOICE_STATE_UPDATE",
				br#"{"guild_id":"10","channel_id":null,"user_id":"1"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		assert!(calls.active.is_none());
		assert!(calls.departing.is_none());
		calls.allowed.insert(Id(20), Some(Id(10)));
		calls
			.packet(Command::Join {
				channel: Id(20),
				request: 2,
				ring: false,
			})
			.unwrap();
		calls.allowed.remove(&Id(20));
		calls.dispatch("VOICE_STATE_UPDATE", br#"{"guild_id":"10","channel_id":"20","user_id":"1","session_id":"synthetic-revoked-session"}"#, Some(Id(1)), &emit).unwrap();
		assert!(calls.active.is_none());
		assert!(matches!(
			events.lock().unwrap().last(),
			Some(Event::Voice(voice::Event::State {
				channel: None,
				session: None,
				request: Some(2),
				..
			}))
		));
	}
	#[test]
	fn guild_join_roster_and_departure_are_scoped_and_bounded() {
		let mut calls = Calls::default();
		calls.allowed.insert(Id(20), Some(Id(10)));
		calls.allowed.insert(Id(21), Some(Id(10)));
		let events = Mutex::new(Vec::new());
		let emit = |event| {
			events.lock().unwrap().push(event);
			Ok(())
		};
		let mut guild: GuildDto = decode(br#"{"id":"10","voice_states":[{"channel_id":"20","user_id":"2","mute":true,"deaf":true}],"members":[{"user":{"id":"2","username":"Synthetic","avatar":"0123456789abcdef0123456789abcdef"},"nick":"Room member"}]}"#).unwrap();
		let Event::Voice(voice::Event::Snapshot { participants, .. }) =
			calls.snapshot(&mut guild, false).unwrap()
		else {
			panic!("roster");
		};
		assert_eq!(
			participants[0].member.as_ref().unwrap().nick.as_deref(),
			Some("Room member")
		);
		assert!(participants[0].participant.muted && participants[0].participant.deafened);
		assert!(
			participants[0].participant.server_muted && participants[0].participant.server_deafened
		);
		assert!(calls.active.is_none());
		let Frame::Text(join) = calls
			.packet(Command::Join {
				channel: Id(20),
				request: 5,
				ring: false,
			})
			.unwrap()
			.unwrap()
		else {
			panic!("join");
		};
		let join: serde_json::Value = serde_json::from_str(&join).unwrap();
		assert_eq!(join["d"]["guild_id"], "10");
		calls
			.dispatch(
				"VOICE_SERVER_UPDATE",
				br#"{"guild_id":"99","token":"synthetic","endpoint":"voice.discord.media:443"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		assert!(events.lock().unwrap().is_empty());
		calls
			.dispatch(
				"VOICE_SERVER_UPDATE",
				br#"{"guild_id":"10","token":"synthetic","endpoint":"voice.discord.media:443"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		assert!(matches!(
			events.lock().unwrap()[0],
			Event::Voice(voice::Event::Server {
				channel: Id(20),
				request: 5,
				..
			})
		));
		calls.dispatch("VOICE_STATE_UPDATE", br#"{"guild_id":"10","channel_id":"20","user_id":"1","session_id":"synthetic-session","suppress":true}"#, Some(Id(1)), &emit).unwrap();
		assert!(matches!(
			events.lock().unwrap()[1],
			Event::Voice(voice::Event::State {
				request: Some(5),
				server_muted: true,
				muted: true,
				..
			})
		));
		let Frame::Text(leave) = calls
			.packet(Command::Leave {
				channel: Id(20),
				request: 5,
			})
			.unwrap()
			.unwrap()
		else {
			panic!("leave");
		};
		let leave: serde_json::Value = serde_json::from_str(&leave).unwrap();
		assert_eq!(leave["d"]["guild_id"], "10");
		assert!(leave["d"]["channel_id"].is_null());
		calls
			.dispatch(
				"VOICE_STATE_UPDATE",
				br#"{"guild_id":"99","channel_id":null,"user_id":"1"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		assert!(calls.departing.is_some());
		calls
			.dispatch(
				"VOICE_STATE_UPDATE",
				br#"{"guild_id":"10","channel_id":null,"user_id":"1"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		assert!(calls.departing.is_none());
		calls
			.packet(Command::Join {
				channel: Id(21),
				request: 6,
				ring: false,
			})
			.unwrap();
		calls
			.passive(
				decode(br#"{"guild_id":"10","removed_voice_states":["1"]}"#).unwrap(),
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		assert!(calls.active.is_none());
		assert!(matches!(
			events.lock().unwrap().last(),
			Some(Event::Voice(voice::Event::State {
				request: Some(6),
				channel: None,
				..
			}))
		));
		guild.voice_states = (0..=voice::MAX_ROSTER)
			.map(|_| decode(br#"{"channel_id":"20","user_id":"2"}"#).unwrap())
			.collect();
		assert!(matches!(
			calls.snapshot(&mut guild, false),
			Err(Failure::Capacity)
		));
	}

	#[test]
	fn signaling_is_dm_scoped_secret_bounded_and_never_auto_joins() {
		let mut calls = Calls::default();
		calls.allowed.insert(Id(2), None);
		let events = Mutex::new(Vec::new());
		let emit = |event| {
			events.lock().unwrap().push(event);
			Ok(())
		};
		calls
			.dispatch(
				"CALL_CREATE",
				br#"{"channel_id":"2","ringing":["1"],"voice_states":[]}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		assert!(calls.active.is_none());
		assert!(
			calls
				.packet(Command::Join {
					channel: Id(9),
					request: 1,
					ring: true
				})
				.is_err()
		);
		let Frame::Text(text) = calls
			.packet(Command::Join {
				channel: Id(2),
				request: 7,
				ring: false,
			})
			.unwrap()
			.unwrap()
		else {
			panic!("expected control")
		};
		let value: serde_json::Value = serde_json::from_str(&text).unwrap();
		assert_eq!(value["op"], 4);
		assert!(value["d"]["guild_id"].is_null());
		assert_eq!(value["d"]["channel_id"], "2");
		calls.dispatch("VOICE_SERVER_UPDATE",br#"{"channel_id":"9","token":"synthetic-token","endpoint":"voice.discord.media:443"}"#,Some(Id(1)),&emit).unwrap();
		calls
			.dispatch(
				"VOICE_SERVER_UPDATE",
				br#"{"token":"synthetic-token","endpoint":"voice.discord.media:443"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		assert_eq!(events.lock().unwrap().len(), 1);
		calls
			.dispatch(
				"VOICE_STATE_UPDATE",
				br#"{"user_id":"1","channel_id":"2","session_id":"synthetic-session"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		calls.dispatch("VOICE_SERVER_UPDATE",br#"{"channel_id":"2","token":"synthetic-token","endpoint":"voice.discord.media:443"}"#,Some(Id(1)),&emit).unwrap();
		assert!(
			matches!(&events.lock().unwrap()[1],Event::Voice(voice::Event::State{request:Some(7),session:Some(secret),..}) if secret.expose()=="synthetic-session")
		);
		assert!(
			matches!(&events.lock().unwrap()[2],Event::Voice(voice::Event::Server{request:7,token:Some(secret),..}) if secret.expose()=="synthetic-token")
		);
		assert!(
			calls
				.packet(Command::Leave {
					channel: Id(2),
					request: 6
				})
				.unwrap()
				.is_none()
		);
		assert!(calls.active.is_some());
		let Frame::Text(text) = calls
			.packet(Command::Leave {
				channel: Id(2),
				request: 7,
			})
			.unwrap()
			.unwrap()
		else {
			panic!("expected leave")
		};
		let value: serde_json::Value = serde_json::from_str(&text).unwrap();
		assert!(value["d"]["channel_id"].is_null());
		calls.dispatch("VOICE_SERVER_UPDATE",br#"{"channel_id":"2","token":"synthetic-token","endpoint":"voice.discord.media:443"}"#,Some(Id(1)),&emit).unwrap();
		assert_eq!(events.lock().unwrap().len(), 3);
		assert!(
			calls
				.packet(Command::Join {
					channel: Id(2),
					request: 8,
					ring: false
				})
				.is_err()
		);
		assert!(calls.departure_expired().is_some());
		assert!(
			calls
				.packet(Command::Join {
					channel: Id(2),
					request: 8,
					ring: false
				})
				.is_err()
		);
		calls
			.dispatch(
				"VOICE_STATE_UPDATE",
				br#"{"channel_id":null,"user_id":"1","session_id":"synthetic-old-session"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		assert!(calls.departing.is_none());
		assert_eq!(events.lock().unwrap().len(), 3); // old null event is consumed, never retagged to request8
		calls
			.packet(Command::Join {
				channel: Id(2),
				request: 8,
				ring: false,
			})
			.unwrap();
		calls.dispatch("VOICE_STATE_UPDATE",br#"{"guild_id":"9","channel_id":"10","user_id":"1","session_id":"synthetic-session"}"#,Some(Id(1)),&emit).unwrap();
		assert!(calls.active.is_none());
	}

	#[test]
	fn screen_stream_matches_one_call_attempt_until_delete() {
		let mut calls = Calls::default();
		calls.allowed.insert(Id(20), Some(Id(10)));
		calls
			.packet(Command::Join {
				channel: Id(20),
				request: 7,
				ring: false,
			})
			.unwrap();
		let Frame::Text(create) = calls
			.stream_packet(
				Command::StartStream {
					channel: Id(20),
					request: 7,
					stream_request: 8,
				},
				Some(Id(1)),
			)
			.unwrap()
			.unwrap()
		else {
			panic!("expected stream create");
		};
		let packet: serde_json::Value = serde_json::from_str(&create).unwrap();
		assert_eq!(packet["op"], 18);
		assert_eq!(packet["d"]["type"], "guild");
		assert_eq!(packet["d"]["guild_id"], "10");
		assert!(packet["d"]["preferred_region"].is_null());

		let events = Mutex::new(Vec::new());
		let emit = |event| {
			events.lock().unwrap().push(event);
			Ok(())
		};
		calls
			.dispatch(
				"STREAM_CREATE",
				br#"{"stream_key":"guild:10:20:2","rtc_server_id":"30","rtc_channel_id":"31"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		calls
			.dispatch(
				"STREAM_CREATE",
				br#"{"stream_key":"guild:10:20:1","rtc_server_id":"30","rtc_channel_id":"31"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		calls.dispatch(
			"STREAM_SERVER_UPDATE",
			br#"{"stream_key":"guild:10:20:1","token":"synthetic-stream-token","endpoint":null}"#,
			Some(Id(1)),
			&emit,
		).unwrap();
		calls
			.dispatch(
				"STREAM_CREATE",
				br#"{"stream_key":"guild:10:20:1","rtc_server_id":"32","rtc_channel_id":"33"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		assert!(matches!(
			&events.lock().unwrap()[0],
			Event::Voice(voice::Event::Stream {
				channel: Id(20),
				request: 7,
				stream_request: 8,
				event: screen::Event::Created {
					rtc_server: Id(30),
					rtc_channel: Id(31)
				}
			})
		));
		assert!(matches!(
			&events.lock().unwrap()[1],
			Event::Voice(voice::Event::Stream {
				event: screen::Event::Server { token: Some(token), endpoint: None }, ..
			}) if token.expose() == "synthetic-stream-token"
		));
		assert!(matches!(
			&events.lock().unwrap()[2],
			Event::Voice(voice::Event::Stream {
				event: screen::Event::Failed("Discord changed screen-share connection identity"),
				..
			})
		));

		let Frame::Text(delete) = calls
			.stream_packet(
				Command::StopStream {
					channel: Id(20),
					request: 7,
					stream_request: 8,
				},
				Some(Id(1)),
			)
			.unwrap()
			.unwrap()
		else {
			panic!("expected stream delete");
		};
		assert_eq!(
			serde_json::from_str::<serde_json::Value>(&delete).unwrap()["op"],
			19
		);
		assert!(
			calls
				.stream_packet(
					Command::StartStream {
						channel: Id(20),
						request: 7,
						stream_request: 9
					},
					Some(Id(1)),
				)
				.is_err()
		);
		calls
			.dispatch(
				"STREAM_DELETE",
				br#"{"stream_key":"guild:10:20:1"}"#,
				Some(Id(1)),
				&emit,
			)
			.unwrap();
		assert!(matches!(
			&events.lock().unwrap()[3],
			Event::Voice(voice::Event::Stream {
				event: screen::Event::Deleted,
				..
			})
		));
		calls.dispatch(
			"STREAM_SERVER_UPDATE",
			br#"{"stream_key":"guild:10:20:1","token":"stale-token","endpoint":"stale.invalid"}"#,
			Some(Id(1)),
			&emit,
		).unwrap();
		assert_eq!(events.lock().unwrap().len(), 4);
	}
}
