//! One explicitly joined one-to-one DM call. No media or persistent credentials live here.
use crate::{
    State as ClientState,
    auth::{AuthState, Failure},
};
use model::Id;
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
    pub user: Id,
    pub peer: Id,
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
    Failed,
}
impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Connecting => "Connecting call…",
            Self::Ringing => "Ringing…",
            Self::Securing => "Securing audio…",
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
}
pub struct Call {
    pub channel: Id,
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
            Self::State { session, .. } => session.as_ref().map_or(0, Secret::bytes),
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
                c.id == channel && c.guild.is_none() && c.kind == 1 && c.recipients.len() == 1
            })
    }
    pub fn start_call(&mut self, channel: Id, ring: bool) -> Option<crate::Command> {
        if !self.can_call(channel) || self.voice.active.is_some() {
            return None;
        }
        self.voice.sequence = self.voice.sequence.wrapping_add(1);
        let request = self.voice.sequence;
        self.voice.active = Some(Call {
            channel,
            request,
            phase: Phase::Connecting,
            muted: false,
            deafened: false,
            participants: Vec::new(),
            error: None,
        });
        if self.voice.incoming == Some(channel) {
            self.voice.incoming = None;
        }
        Some(crate::Command::Voice(Command::Join {
            channel,
            request,
            ring,
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
                request,
                channel,
                user,
                muted,
                deafened,
                ..
            } => {
                let Some(call) = &mut self.voice.active else {
                    return;
                };
                if self.user.as_ref().is_some_and(|u| u.id == user) {
                    if request != Some(call.request) {
                        return;
                    }
                    if channel != Some(call.channel) {
                        self.voice.active = None;
                        return;
                    }
                }
                call.participants.retain(|p| p.user != user);
                if channel == Some(call.channel) && call.participants.len() < 2 {
                    call.participants.push(Participant {
                        user,
                        muted,
                        deafened,
                    });
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
    fn end_voice_channel(&mut self, channel: Id) {
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
