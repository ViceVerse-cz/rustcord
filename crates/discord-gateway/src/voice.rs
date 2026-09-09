//! Unofficial normal-user DM call signaling. Never joins on incoming events or reconnect.
use client_core::{
    Event,
    auth::Failure,
    voice::{self, Command, Participant, Secret},
};
use discord_protocol::{CallDto, VoiceServerDto, VoiceStateDto, decode};
use model::Id;
use serde_json::json;
use std::{collections::BTreeSet, time::Duration};
use tokio::time::Instant;
use tokio_tungstenite::tungstenite::Message as Frame;
use zeroize::Zeroizing;

#[derive(Default)]
pub(super) struct Calls {
    pub(super) allowed: BTreeSet<Id>,
    pub(super) active: Option<(Id, u64)>,
    departing: Option<(Id, u64)>,
    pub(super) departure_deadline: Option<Instant>,
}
impl Calls {
    pub(super) fn disconnected(&mut self) {
        self.active = None;
        self.departing = None;
        self.departure_deadline = None;
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

    pub(super) fn packet(&mut self, command: Command) -> Result<Option<Frame>, Failure> {
        let (channel, mute, deaf) = match command {
            Command::Join {
                channel, request, ..
            } => {
                if !self.allowed.contains(&channel)
                    || self.active.is_some()
                    || self.departing.is_some()
                {
                    return Err(Failure::Protocol);
                }
                self.active = Some((channel, request));
                (Some(channel), false, false)
            }
            Command::Leave { channel, request } => {
                if self.active != Some((channel, request)) {
                    return Ok(None);
                }
                self.departing = self.active.take();
                self.departure_deadline = Some(Instant::now() + Duration::from_secs(10));
                (None, true, true)
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
                (Some(channel), mute || deaf, deaf)
            }
            Command::Decline { .. } | Command::Ring { .. } => return Ok(None),
        };
        Ok(Some(Frame::Text(json!({"op":4,"d":{"guild_id":null,"channel_id":channel,"self_mute":mute,"self_deaf":deaf,"self_video":false}}).to_string().into())))
    }
    pub(super) fn dispatch(
        &mut self,
        kind: &str,
        data: &[u8],
        owner: Option<Id>,
        emit: &impl Fn(Event) -> Result<(), Failure>,
    ) -> Result<(), Failure> {
        match kind {
            "CALL_CREATE" | "CALL_UPDATE" | "CALL_DELETE" => {
                let call: CallDto = decode(data).map_err(|_| Failure::Protocol)?;
                if !self.allowed.contains(&call.channel_id) {
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
                                Participant {
                                    user: state.user_id,
                                    muted: state.self_mute,
                                    deafened: state.self_deaf,
                                }
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
            "VOICE_STATE_UPDATE" => {
                let mut state: VoiceStateDto = decode(data).map_err(|_| Failure::Protocol)?;
                let session = state.session_id.take().map(Zeroizing::new);
                if owner == Some(state.user_id) && self.departing.is_some() {
                    // The old null acknowledgement must never be assigned the next call's request.
                    if state.channel_id.is_none() {
                        self.departing = None;
                        self.departure_deadline = None;
                    }
                    return Ok(());
                }
                if state.guild_id.is_some() {
                    if owner == Some(state.user_id)
                        && let Some((_, request)) = self.active.take()
                    {
                        emit(Event::Voice(voice::Event::State {
                            request: Some(request),
                            channel: None,
                            user: state.user_id,
                            session: None,
                            muted: true,
                            deafened: true,
                        }))?;
                    }
                    return Ok(());
                }
                let request = self.active.map(|(_, request)| request);
                let own = owner == Some(state.user_id);
                let secret = if own && request.is_some() && state.channel_id.is_some() {
                    session.map(|s| Secret::new(s.to_string())).transpose()?
                } else {
                    None
                };
                if state
                    .channel_id
                    .is_some_and(|channel| !self.allowed.contains(&channel))
                {
                    if own && let Some(request) = request {
                        self.active = None;
                        emit(Event::Voice(voice::Event::State {
                            request: Some(request),
                            channel: None,
                            user: state.user_id,
                            session: None,
                            muted: true,
                            deafened: true,
                        }))?;
                    }
                    return Ok(());
                }
                emit(Event::Voice(voice::Event::State {
                    request,
                    channel: state.channel_id,
                    user: state.user_id,
                    session: secret,
                    muted: state.self_mute,
                    deafened: state.self_deaf,
                }))?;
                if own
                    && self
                        .active
                        .is_some_and(|(channel, _)| state.channel_id != Some(channel))
                {
                    self.active = None;
                }
            }
            "VOICE_SERVER_UPDATE" => {
                let mut server: VoiceServerDto = decode(data).map_err(|_| Failure::Protocol)?;
                let token = Zeroizing::new(std::mem::take(&mut server.token));
                let Some((channel, request)) = self.active else {
                    return Ok(());
                };
                if server.guild_id.is_some() || server.channel_id != Some(channel) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    #[test]
    fn signaling_is_dm_scoped_secret_bounded_and_never_auto_joins() {
        let mut calls = Calls::default();
        calls.allowed.insert(Id(2));
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
}
