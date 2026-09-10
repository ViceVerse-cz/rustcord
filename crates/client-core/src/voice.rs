//! One explicitly joined DM or guild voice channel; bounded ephemeral participant state.
use crate::{
    State as ClientState,
    auth::{AuthState, Failure},
};
use model::{Id, Member};
use std::time::Instant;
pub const MAX_PARTICIPANTS: usize = 64;
pub const MAX_ROSTER: usize = 4096;
pub const MAX_ROSTER_BYTES: usize = 1024 * 1024;
use std::fmt;
use zeroize::Zeroizing;

pub struct Secret(Zeroizing<String>);
impl Secret {
    pub fn new(value: String) -> Result<Self, Failure> {
        let value = Zeroizing::new(value);
        if value.is_empty() || value.len() > 2048 || !value.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(Failure::Protocol);
        }
        Ok(Self(value))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
    fn bytes(&self) -> usize {
        self.0.capacity()
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("VoiceSecret([REDACTED])")
    }
}
pub struct VoiceConnection {
    pub channel: Id,
    pub guild: Option<Id>,
    pub user: Id,
    pub peer: Option<Id>,
    pub session: Secret,
    pub token: Secret,
    pub endpoint: String,
    pub request: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Connecting,
    Ringing,
    Securing,
    Connected,
    Waiting,
    Failed,
}
impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Connecting => "Connecting call…",
            Self::Ringing => "Ringing…",
            Self::Securing => "Securing audio…",
            Self::Waiting => "Connected · waiting for others",
            Self::Connected => "Voice connected",
            Self::Failed => "Call failed",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Participant {
    pub user: Id,
    pub muted: bool,
    pub deafened: bool,
    pub server_muted: bool,
    pub server_deafened: bool,
}
#[derive(Clone)]
pub struct RosterEntry {
    pub guild: Id,
    pub channel: Id,
    pub participant: Participant,
    pub member: Option<Member>,
}
impl RosterEntry {
    pub fn bytes(&self) -> usize {
        size_of::<Self>() + self.member.as_ref().map_or(0, Member::bytes)
    }
}
pub struct Call {
    pub channel: Id,
    pub guild: Option<Id>,
    pub connected_at: Option<Instant>,
    pub server_muted: bool,
    pub server_deafened: bool,
    pub request: u64,
    pub phase: Phase,
    pub muted: bool,
    pub deafened: bool,
    pub participants: Vec<Participant>,
    pub error: Option<&'static str>,
}
#[derive(Default)]
pub struct State {
    pub active: Option<Call>,
    pub incoming: Option<Id>,
    pub roster: Vec<RosterEntry>,
    sequence: u64,
}
#[derive(Clone, Copy, Debug)]
pub enum Command {
    Ring {
        channel: Id,
        request: u64,
    },
    Join {
        channel: Id,
        request: u64,
        ring: bool,
    },
    Leave {
        channel: Id,
        request: u64,
    },
    SetMute {
        channel: Id,
        request: u64,
        mute: bool,
        deaf: bool,
    },
    Decline {
        channel: Id,
    },
}
pub enum Event {
    Snapshot {
        partial: bool,
        guild: Option<Id>,
        participants: Vec<RosterEntry>,
    },
    Call {
        channel: Id,
        ringing: Option<Vec<Id>>,
        participants: Option<Vec<Participant>>,
        unavailable: bool,
    },
    Deleted {
        channel: Id,
    },
    State {
        guild: Option<Id>,
        member: Option<Member>,
        server_muted: bool,
        server_deafened: bool,
        request: Option<u64>,
        channel: Option<Id>,
        user: Id,
        session: Option<Secret>,
        muted: bool,
        deafened: bool,
    },
    Server {
        request: u64,
        channel: Id,
        token: Option<Secret>,
        endpoint: Option<String>,
    },
    Progress {
        channel: Id,
        request: u64,
        phase: Phase,
    },
    Failed {
        channel: Id,
        request: u64,
        message: &'static str,
    },
}
impl Event {
    pub fn bytes(&self) -> usize {
        match self {
            Self::Snapshot { participants, .. } => {
                participants.capacity() * size_of::<RosterEntry>()
                    + participants.iter().map(RosterEntry::bytes).sum::<usize>()
            }
            Self::Call {
                ringing,
                participants,
                ..
            } => {
                ringing
                    .as_ref()
                    .map_or(0, |v| v.capacity() * size_of::<Id>())
                    + participants
                        .as_ref()
                        .map_or(0, |v| v.capacity() * size_of::<Participant>())
            }
            Self::State {
                session, member, ..
            } => {
                session.as_ref().map_or(0, Secret::bytes) + member.as_ref().map_or(0, Member::bytes)
            }
            Self::Server {
                token, endpoint, ..
            } => {
                token.as_ref().map_or(0, Secret::bytes)
                    + endpoint.as_ref().map_or(0, String::capacity)
            }
            _ => 0,
        }
    }
}
impl ClientState {
    pub fn can_call(&self, channel: Id) -> bool {
        !self.demo
            && self.auth == AuthState::Authenticated
            && self.gateway_connected
            && self.channels.iter().any(|c| {
                c.id == channel
                    && ((c.guild.is_some() && c.kind == 2)
                        || (c.guild.is_none() && c.kind == 1 && c.recipients.len() == 1))
            })
    }
    pub fn start_call(&mut self, channel: Id, ring: bool) -> Option<crate::Command> {
        if !self.can_call(channel) || self.voice.active.is_some() {
            return None;
        }
        let guild = self.channels.iter().find(|c| c.id == channel)?.guild;
        let participants: Vec<_> = self
            .voice
            .roster
            .iter()
            .filter(|r| r.channel == channel)
            .map(|r| r.participant)
            .collect();
        if participants.len() >= MAX_PARTICIPANTS {
            self.status = "Voice channel exceeds the 64 participant limit";
            return None;
        }
        let own = participants
            .iter()
            .find(|p| self.user.as_ref().is_some_and(|u| u.id == p.user));
        let server_muted = own.is_some_and(|p| p.server_muted);
        let server_deafened = own.is_some_and(|p| p.server_deafened);
        self.voice.sequence = self.voice.sequence.wrapping_add(1);
        let request = self.voice.sequence;
        self.voice.active = Some(Call {
            channel,
            guild,
            connected_at: None,
            server_muted,
            server_deafened,
            request,
            phase: Phase::Connecting,
            muted: false,
            deafened: false,
            participants,
            error: None,
        });
        if self.voice.incoming == Some(channel) {
            self.voice.incoming = None;
        }
        Some(crate::Command::Voice(Command::Join {
            channel,
            request,
            ring: ring && guild.is_none(),
        }))
    }
    pub fn leave_call(&mut self) -> Option<crate::Command> {
        let call = self.voice.active.take()?;
        Some(crate::Command::Voice(Command::Leave {
            channel: call.channel,
            request: call.request,
        }))
    }
    pub fn set_call_mute(&mut self, muted: bool, deafened: bool) -> Option<crate::Command> {
        let call = self.voice.active.as_mut()?;
        if call.phase == Phase::Failed {
            return None;
        }
        call.muted = muted;
        call.deafened = deafened;
        Some(crate::Command::Voice(Command::SetMute {
            channel: call.channel,
            request: call.request,
            mute: muted || deafened,
            deaf: deafened,
        }))
    }
    pub fn decline_call(&mut self) -> Option<crate::Command> {
        Some(crate::Command::Voice(Command::Decline {
            channel: self.voice.incoming.take()?,
        }))
    }
    pub fn apply_voice(&mut self, event: Event) {
        match event {
            Event::Snapshot {
                partial,
                guild,
                participants,
            } => {
                if !partial {
                    self.voice
                        .roster
                        .retain(|r| Some(r.guild) != guild && guild.is_some());
                }
                for entry in participants {
                    if guild.is_some_and(|guild| entry.guild != guild) {
                        continue;
                    }
                    if !self.update_roster(entry) {
                        break;
                    }
                }
                self.refresh_voice_participants();
            }
            Event::Call {
                channel,
                ringing,
                participants,
                unavailable,
            } => {
                if !self.can_call(channel) {
                    return;
                }
                if ringing.as_ref().is_some_and(|r| r.len() > 2)
                    || participants.as_ref().is_some_and(|p| p.len() > 2)
                {
                    return;
                }
                if unavailable {
                    self.end_voice_channel(channel);
                    return;
                }
                if let Some(ringing) = ringing {
                    if self.user.as_ref().is_some_and(|u| ringing.contains(&u.id))
                        && self
                            .voice
                            .active
                            .as_ref()
                            .is_none_or(|c| c.channel != channel)
                    {
                        if self.voice.incoming.is_none() {
                            self.voice.incoming = Some(channel);
                        }
                    } else if self.voice.incoming == Some(channel) {
                        self.voice.incoming = None;
                    }
                }
                if let Some(call) = &mut self.voice.active
                    && call.channel == channel
                    && let Some(participants) = participants
                {
                    call.participants = participants;
                }
            }
            Event::Deleted { channel } => self.end_voice_channel(channel),
            Event::State {
                guild,
                member,
                server_muted,
                server_deafened,
                request,
                channel,
                user,
                muted,
                deafened,
                ..
            } => {
                let participant = Participant {
                    user,
                    muted,
                    deafened,
                    server_muted,
                    server_deafened,
                };
                if let Some(guild) = guild {
                    let previous = self
                        .voice
                        .roster
                        .iter()
                        .find(|r| r.guild == guild && r.participant.user == user)
                        .and_then(|r| r.member.clone());
                    self.voice
                        .roster
                        .retain(|r| r.guild != guild || r.participant.user != user);
                    if let Some(channel) = channel {
                        self.update_roster(RosterEntry {
                            guild,
                            channel,
                            participant,
                            member: member.or(previous),
                        });
                    }
                }
                let Some(call) = &mut self.voice.active else {
                    return;
                };
                if self.user.as_ref().is_some_and(|u| u.id == user) {
                    if request != Some(call.request) {
                        return;
                    }
                    if channel != Some(call.channel) || guild != call.guild {
                        self.voice.active = None;
                        return;
                    }
                }
                if guild != call.guild {
                    return;
                }
                if self.user.as_ref().is_some_and(|u| u.id == user) {
                    call.server_muted = server_muted;
                    call.server_deafened = server_deafened;
                }
                call.participants.retain(|p| p.user != user);
                if channel == Some(call.channel) {
                    if call.participants.len() >= MAX_PARTICIPANTS {
                        self.disconnect_voice();
                        self.status = "Voice channel exceeds the 64 participant limit";
                        return;
                    }
                    call.participants.push(participant);
                }
            }
            Event::Progress {
                channel,
                request,
                phase,
            } => {
                if let Some(call) = &mut self.voice.active
                    && call.channel == channel
                    && call.request == request
                    && call.phase != Phase::Failed
                {
                    call.phase = phase;
                    if matches!(phase, Phase::Connected | Phase::Waiting)
                        && call.connected_at.is_none()
                    {
                        call.connected_at = Some(Instant::now());
                    }
                }
            }
            Event::Failed {
                channel,
                request,
                message,
            } => {
                if request == 0 || self.voice.active.is_none() {
                    self.status = message;
                }
                if let Some(call) = &mut self.voice.active
                    && call.channel == channel
                    && call.request == request
                {
                    call.phase = Phase::Failed;
                    call.error = Some(message);
                    call.participants.clear();
                }
            }
            Event::Server { .. } => {} // The desktop consumes negotiation material; core never retains it.
        }
    }
    fn update_roster(&mut self, entry: RosterEntry) -> bool {
        if !self
            .channels
            .iter()
            .any(|c| c.id == entry.channel && c.guild == Some(entry.guild) && c.kind == 2)
        {
            return true;
        }
        self.voice
            .roster
            .retain(|r| r.guild != entry.guild || r.participant.user != entry.participant.user);
        if self.voice.roster.len() >= MAX_ROSTER
            || self
                .voice
                .roster
                .iter()
                .map(RosterEntry::bytes)
                .sum::<usize>()
                + entry.bytes()
                > MAX_ROSTER_BYTES
        {
            self.disconnect_voice();
            self.status = "Voice roster exceeds safe capacity; reconnect to refresh";
            return false;
        }
        self.voice.roster.push(entry);
        true
    }
    fn refresh_voice_participants(&mut self) {
        if let Some(call) = &mut self.voice.active
            && call.guild.is_some()
        {
            call.participants = self
                .voice
                .roster
                .iter()
                .filter(|r| r.channel == call.channel)
                .map(|r| r.participant)
                .collect();
            if let Some(own) = call
                .participants
                .iter()
                .find(|p| self.user.as_ref().is_some_and(|u| u.id == p.user))
            {
                call.server_muted = own.server_muted;
                call.server_deafened = own.server_deafened;
            }
            if call.participants.len() > MAX_PARTICIPANTS {
                self.disconnect_voice();
            }
        }
    }
    pub(crate) fn end_voice_channel(&mut self, channel: Id) {
        self.voice.roster.retain(|r| r.channel != channel);
        if self.voice.incoming == Some(channel) {
            self.voice.incoming = None;
        }
        if self
            .voice
            .active
            .as_ref()
            .is_some_and(|c| c.channel == channel)
        {
            self.voice.active = None;
        }
    }
    pub fn disconnect_voice(&mut self) {
        self.voice.roster.clear();
        self.voice.incoming = None;
        if let Some(call) = &mut self.voice.active {
            call.phase = Phase::Failed;
            call.error = Some("Call disconnected; start a new call explicitly");
            call.participants.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Envelope, Event as CoreEvent};
    use model::{Channel, User};
    #[test]
    fn guild_roster_moves_mutes_limits_and_selection_never_join_implicitly() {
        let mut state = ClientState {
            auth: AuthState::Authenticated,
            gateway_connected: true,
            user: Some(User {
                id: Id(1),
                name: "Owner".into(),
                avatar: None,
                discriminator: 0,
            }),
            channels: [20, 21]
                .into_iter()
                .map(|id| Channel {
                    id: Id(id),
                    guild: Some(Id(10)),
                    kind: 2,
                    name: "Room".into(),
                    last_message: None,
                    parent_id: None,
                    position: 0,
                    recipients: vec![],
                    member_list_id: None,
                })
                .collect(),
            ..ClientState::default()
        };
        let entry = |user, channel| RosterEntry {
            guild: Id(10),
            channel: Id(channel),
            participant: Participant {
                user: Id(user),
                muted: true,
                deafened: false,
                server_muted: true,
                server_deafened: false,
            },
            member: None,
        };
        state.apply_voice(Event::Snapshot {
            guild: None,
            partial: false,
            participants: vec![entry(2, 20)],
        });
        assert!(state.voice.active.is_none());
        assert!(state.select(Id(20)).is_none());
        assert_eq!(state.selected, Some(Id(20)));
        assert!(!state.history_pending);
        state.apply_voice(Event::Snapshot {
            guild: None,
            partial: true,
            participants: vec![entry(3, 21)],
        });
        assert_eq!(state.voice.roster.len(), 2);
        assert!(matches!(
            state.start_call(Id(20), true),
            Some(crate::Command::Voice(Command::Join { ring: false, .. }))
        ));
        let call = state.voice.active.as_ref().unwrap();
        let request = call.request;
        assert_eq!(call.guild, Some(Id(10)));
        assert!(call.connected_at.is_none());
        assert_eq!(call.participants.len(), 1);
        state.apply_voice(Event::Progress {
            channel: Id(20),
            request,
            phase: Phase::Waiting,
        });
        let connected_at = state.voice.active.as_ref().unwrap().connected_at;
        assert!(connected_at.is_some());
        state.apply_voice(Event::Progress {
            channel: Id(20),
            request,
            phase: Phase::Connected,
        });
        assert_eq!(
            state.voice.active.as_ref().unwrap().connected_at,
            connected_at
        );
        state.apply_voice(Event::State {
            guild: Some(Id(10)),
            channel: Some(Id(21)),
            user: Id(2),
            request: None,
            session: None,
            member: None,
            muted: false,
            deafened: true,
            server_muted: false,
            server_deafened: true,
        });
        assert!(state.voice.active.as_ref().unwrap().participants.is_empty());
        assert_eq!(
            state
                .voice
                .roster
                .iter()
                .find(|r| r.participant.user == Id(2))
                .unwrap()
                .channel,
            Id(21)
        );
        state.apply(Envelope {
            generation: state.generation,
            event: CoreEvent::Disconnected,
        });
        assert_eq!(state.voice.roster.len(), 2);
        assert_eq!(state.voice.active.as_ref().unwrap().phase, Phase::Failed);
        state.apply(Envelope {
            generation: state.generation,
            event: CoreEvent::PermissionsChanged,
        });
        assert!(state.voice.roster.is_empty());
        let mut oversized = entry(2, 20);
        oversized.member = Some(Member {
            user: User {
                id: Id(2),
                name: "x".repeat(MAX_ROSTER_BYTES),
                avatar: None,
                discriminator: 0,
            },
            nick: None,
            status: None,
        });
        state.apply_voice(Event::Snapshot {
            guild: None,
            partial: false,
            participants: vec![oversized],
        });
        assert!(state.voice.roster.is_empty());
        assert!(state.status.contains("capacity"));
        state.apply_voice(Event::Snapshot {
            guild: None,
            partial: false,
            participants: (1..=MAX_PARTICIPANTS as u64)
                .map(|user| entry(user, 20))
                .collect(),
        });
        state.leave_call();
        state.gateway_connected = true;
        assert!(state.start_call(Id(20), false).is_none());
        state.apply(Envelope {
            generation: state.generation,
            event: CoreEvent::Unavailable(Id(20)),
        });
        assert!(state.voice.roster.is_empty());
    }

    #[test]
    fn dm_calls_require_gesture_and_reject_late_states() {
        let mut state = ClientState {
            auth: AuthState::Authenticated,
            gateway_connected: true,
            user: Some(User {
                id: Id(1),
                name: "Owner".into(),
                avatar: None,
                discriminator: 0,
            }),
            channels: vec![Channel {
                last_message: None,
                id: Id(2),
                name: "DM".into(),
                guild: None,
                parent_id: None,
                position: 0,
                kind: 1,
                recipients: vec![User {
                    id: Id(3),
                    name: "Peer".into(),
                    avatar: None,
                    discriminator: 0,
                }],
                member_list_id: None,
            }],
            ..ClientState::default()
        };
        state.apply_voice(Event::Call {
            channel: Id(2),
            ringing: Some(vec![Id(1)]),
            participants: None,
            unavailable: false,
        });
        assert_eq!(state.voice.incoming, Some(Id(2)));
        assert!(state.voice.active.is_none()); // Incoming call never grants microphone access.
        assert!(state.start_call(Id(9), true).is_none());
        assert!(state.start_call(Id(2), false).is_some());
        let request = state.voice.active.as_ref().unwrap().request;
        assert_eq!(
            state.voice.active.as_ref().unwrap().phase,
            Phase::Connecting
        );
        assert!(state.start_call(Id(2), true).is_none());
        state.apply(Envelope {
            generation: state.generation + 1,
            event: CoreEvent::Voice(Event::Progress {
                channel: Id(2),
                request,
                phase: Phase::Connected,
            }),
        });
        state.apply_voice(Event::Progress {
            channel: Id(2),
            request: request + 1,
            phase: Phase::Connected,
        });
        assert_eq!(
            state.voice.active.as_ref().unwrap().phase,
            Phase::Connecting
        );
        state.apply_voice(Event::Progress {
            channel: Id(2),
            request,
            phase: Phase::Connected,
        });
        assert_eq!(state.voice.active.as_ref().unwrap().phase, Phase::Connected);
        assert!(matches!(
            state.set_call_mute(false, true),
            Some(crate::Command::Voice(Command::SetMute {
                mute: true,
                deaf: true,
                ..
            }))
        ));
        state.apply(Envelope {
            generation: state.generation,
            event: CoreEvent::Disconnected,
        });
        assert_eq!(state.voice.active.as_ref().unwrap().phase, Phase::Failed);
        state.apply_voice(Event::Progress {
            channel: Id(2),
            request,
            phase: Phase::Connected,
        });
        assert_eq!(state.voice.active.as_ref().unwrap().phase, Phase::Failed);
        assert!(state.leave_call().is_some());
        state.gateway_connected = true;
        state.channels[0].kind = 3;
        assert!(state.start_call(Id(2), true).is_none());
        let secret = Secret::new("SYNTHETIC_VOICE_SECRET".into()).unwrap();
        assert!(!format!("{secret:?}").contains("SYNTHETIC"));
        assert!(Secret::new("bad\nheader".into()).is_err());
        state.channels[0].kind = 1;
        let command = state.start_call(Id(2), true).unwrap();
        state.command_rejected(command);
        assert_eq!(state.voice.active.as_ref().unwrap().phase, Phase::Failed);
        assert!(
            state
                .voice
                .active
                .as_ref()
                .unwrap()
                .error
                .unwrap()
                .contains("queue")
        );
        state.leave_call();
        let command = state.start_call(Id(2), false).unwrap();
        assert!(matches!(
            command,
            crate::Command::Voice(Command::Join { ring: false, .. })
        ));
        let mute = state.set_call_mute(true, false).unwrap();
        state.command_rejected(mute);
        assert!(state.voice.active.as_ref().unwrap().muted);
        assert_eq!(
            state.voice.active.as_ref().unwrap().phase,
            Phase::Connecting
        );
        state.logout();
        assert!(state.voice.active.is_none());
        assert!(state.voice.incoming.is_none());
    }
}
