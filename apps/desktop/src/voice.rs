//! Desktop ownership for one explicitly authorized DM media session.
use client_core::{
    Command, Event, State,
    voice::{self, Phase, Secret},
};
use discord_voice::{
    Controls, Status,
    audio::{Audio, Devices},
};
use eframe::egui;
use model::Id;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use tokio::{runtime::Runtime, sync::watch, task::JoinHandle};
use zeroize::Zeroizing;

struct Pending {
    generation: u64,
    channel: Id,
    request: u64,
    ring: bool,
    user: Id,
    peer: Id,
    session: Option<Secret>,
    server: Option<(Secret, String)>,
    started: Instant,
}
enum Notice {
    TransportReady,
    Securing,
    MediaReady(String),
    DeviceReady,
    RemoteAudio,
    Failed(&'static str),
}
struct Live {
    generation: u64,
    channel: Id,
    request: u64,
    ring_pending: bool,
    session: Zeroizing<String>,
    audio: Audio,
    controls: watch::Sender<Controls>,
    events: mpsc::Receiver<Notice>,
    task: JoinHandle<()>,
    devices: Devices,
}
#[derive(Default)]
pub struct Voice {
    pending: Option<Pending>,
    live: Option<Live>,
    retiring: Option<mpsc::Receiver<()>>,
    device_scan: Option<mpsc::Receiver<Result<discord_voice::audio::DeviceList, &'static str>>>,
}
impl Voice {
    pub fn stop(&mut self) {
        self.pending = None;
        if let Some(live) = self.live.take() {
            live.audio.set_ready(false);
            live.task.abort();
            self.retiring = Some(live.audio.shutdown());
        }
    }
    fn reap(&mut self) {
        if self
            .retiring
            .as_ref()
            .is_some_and(|done| !matches!(done.try_recv(), Err(mpsc::TryRecvError::Empty)))
        {
            self.retiring = None;
        }
    }
    pub fn begin(&mut self, state: &State, ring: bool) -> Result<(), &'static str> {
        self.reap();
        if self.retiring.is_some() {
            return Err("Previous audio devices are still closing; try again shortly");
        }
        if self.pending.is_some() || self.live.is_some() {
            return Err("A voice call is already active");
        }
        let call = state.voice.active.as_ref().ok_or("No call was requested")?;
        if !state.can_call(call.channel) {
            return Err("Only an existing one-to-one DM can be called");
        }
        let user = state.user.as_ref().ok_or("Sign in before calling")?.id;
        let peer = state
            .channels
            .iter()
            .find(|c| c.id == call.channel)
            .and_then(|c| c.recipients.first())
            .ok_or("The DM recipient is unavailable")?
            .id;
        self.pending = Some(Pending {
            generation: state.generation,
            channel: call.channel,
            request: call.request,
            user,
            peer,
            ring,
            session: None,
            server: None,
            started: Instant::now(),
        });
        Ok(())
    }
    /// Take negotiation secrets before reducing the UI event. Nothing is persisted.
    pub fn observe(&mut self, state: &State, event: &mut Event) -> Option<&'static str> {
        let Event::Voice(event) = event else {
            return None;
        };
        if let Some(live) = &self.live {
            match event {
                voice::Event::State {
                    request: Some(request),
                    user,
                    channel: Some(channel),
                    session: Some(session),
                    ..
                } if live.generation == state.generation
                    && *request == live.request
                    && *channel == live.channel
                    && state.user.as_ref().is_some_and(|owner| owner.id == *user) =>
                {
                    if session.expose() != live.session.as_str() {
                        return Some("Voice session changed; start a new call");
                    }
                }
                voice::Event::Server {
                    request, channel, ..
                } if live.generation == state.generation
                    && *request == live.request
                    && *channel == live.channel =>
                {
                    return Some("Voice server changed; start a new encrypted call");
                }
                _ => {}
            }
        }
        let pending = self.pending.as_mut()?;
        if pending.generation != state.generation
            || state.voice.active.as_ref().is_none_or(|c| {
                c.channel != pending.channel
                    || c.request != pending.request
                    || c.phase == Phase::Failed
            })
        {
            return None;
        }
        match event {
            voice::Event::State {
                request: Some(request),
                channel: Some(channel),
                user,
                session,
                ..
            } if *channel == pending.channel
                && *request == pending.request
                && *user == pending.user =>
            {
                if let Some(session) = session.take() {
                    if pending
                        .session
                        .as_ref()
                        .is_some_and(|old| old.expose() != session.expose())
                    {
                        return Some("Voice session changed during connection; try a new call");
                    }
                    pending.session = Some(session);
                }
            }
            voice::Event::Server {
                request,
                channel,
                token,
                endpoint,
            } if *request == pending.request && *channel == pending.channel => {
                let Some(endpoint) = endpoint.take() else {
                    pending.server = None;
                    let _ = token.take();
                    return None;
                };
                let Some(token) = token.take() else {
                    return Some("Discord omitted the voice connection token");
                };
                pending.server = Some((token, endpoint));
            }
            _ => {}
        }
        None
    }
    pub fn fail(&mut self, state: &mut State, message: &'static str) -> Option<Command> {
        self.stop();
        let call = state.voice.active.as_ref()?;
        let (channel, request) = (call.channel, call.request);
        state.apply_voice(voice::Event::Failed {
            channel,
            request,
            message,
        });
        Some(Command::Voice(voice::Command::Leave { channel, request }))
    }
    pub fn poll(
        &mut self,
        runtime: &Runtime,
        state: &mut State,
        ui: &mut ui::MessagingUi,
        ctx: &egui::Context,
    ) -> Option<Command> {
        self.reap();
        if ui.voice_refresh_devices {
            ui.voice_refresh_devices = false;
            if !state.demo && self.device_scan.is_none() {
                let (send, receive) = mpsc::sync_channel(1);
                let wake = ctx.clone();
                match std::thread::Builder::new()
                    .name("audio-devices".into())
                    .spawn(move || {
                        let _ = send.send(discord_voice::audio::devices());
                        wake.request_repaint();
                    }) {
                    Ok(_) => {
                        self.device_scan = Some(receive);
                        ui.voice_device_status = "Looking for audio devices…";
                    }
                    Err(_) => ui.voice_device_status = "Could not start audio device discovery",
                }
            }
        }
        if let Some(scan) = &self.device_scan {
            match scan.try_recv() {
                Ok(Ok(devices)) => {
                    ui.voice_inputs = devices.inputs;
                    ui.voice_outputs = devices.outputs;
                    ui.voice_device_status =
                        "Audio devices loaded · headphones avoid microphone echo";
                    self.device_scan = None;
                }
                Ok(Err(error)) => {
                    ui.voice_device_status = error;
                    self.device_scan = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    ui.voice_device_status = "Audio device discovery stopped";
                    self.device_scan = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let expected = state
            .voice
            .active
            .as_ref()
            .filter(|call| call.phase != Phase::Failed)
            .map(|call| (state.generation, call.channel, call.request));
        if expected.is_none() {
            ui.voice_privacy_code = None;
        }
        if self
            .live
            .as_ref()
            .is_some_and(|c| Some((c.generation, c.channel, c.request)) != expected)
            || self
                .pending
                .as_ref()
                .is_some_and(|c| Some((c.generation, c.channel, c.request)) != expected)
        {
            self.stop();
        }
        if let Some(pending) = &self.pending {
            if pending.started.elapsed() >= Duration::from_secs(30) {
                return self.fail(
                    state,
                    "Discord did not provide DM voice connection details in time",
                );
            }
            ctx.request_repaint_after(
                Duration::from_secs(30).saturating_sub(pending.started.elapsed()),
            );
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|p| p.session.is_some() && p.server.is_some())
        {
            let pending = self.pending.take().expect("pending negotiation");
            if let Err(error) = self.start_media(runtime, pending, ui, ctx) {
                return self.fail(state, error);
            }
        }
        let mut failure = None;
        let mut command = None;
        if let Some(live) = &mut self.live {
            let call = state.voice.active.as_ref().expect("matching active call");
            let muted =
                call.muted || call.deafened || (ui.voice_push_to_talk && !ui.voice_ptt_active);
            live.audio.set_controls(muted, call.deafened);
            live.controls.send_if_modified(|control| {
                if control.muted == muted && control.deafened == call.deafened {
                    false
                } else {
                    *control = Controls {
                        muted,
                        deafened: call.deafened,
                    };
                    true
                }
            });
            let devices = Devices {
                input: ui.voice_input.clone(),
                output: ui.voice_output.clone(),
            };
            if devices != live.devices {
                live.audio.set_devices(devices.clone());
                live.devices = devices;
            }
            for _ in 0..8 {
                let Ok(event) = live.events.try_recv() else {
                    break;
                };
                match event {
                    Notice::TransportReady => {
                        if live.ring_pending {
                            live.ring_pending = false;
                            command = Some(Command::Voice(voice::Command::Ring {
                                channel: live.channel,
                                request: live.request,
                            }));
                        }
                    }
                    Notice::Securing => {
                        ui.voice_privacy_code = None;
                        live.audio.set_ready(false);
                        state.apply_voice(voice::Event::Progress {
                            channel: live.channel,
                            request: live.request,
                            phase: Phase::Securing,
                        });
                    }
                    Notice::MediaReady(code) => {
                        ui.voice_privacy_code = Some(code);
                        live.audio.set_ready(true);
                    }
                    Notice::DeviceReady => {
                        if live
                            .audio
                            .gate
                            .ready
                            .load(std::sync::atomic::Ordering::Acquire)
                        {
                            state.apply_voice(voice::Event::Progress {
                                channel: live.channel,
                                request: live.request,
                                phase: Phase::Connected,
                            });
                        }
                    }
                    Notice::RemoteAudio => {}
                    Notice::Failed(error) => {
                        failure = Some(error);
                        break;
                    }
                }
            }
            if live.audio.is_stopped() && failure.is_none() {
                failure =
                    Some("Audio devices stopped; check microphone permission and device selection");
            }
            if live.task.is_finished() && failure.is_none() {
                failure = Some("Voice connection ended; start a new call explicitly");
            }
        }
        if let Some(error) = failure {
            self.fail(state, error)
        } else {
            command
        }
    }
    fn start_media(
        &mut self,
        runtime: &Runtime,
        pending: Pending,
        ui: &ui::MessagingUi,
        ctx: &egui::Context,
    ) -> Result<(), &'static str> {
        let (capture_send, capture) = mpsc::sync_channel(8);
        let (playback, playback_receive) = mpsc::sync_channel(8);
        let (send, events) = mpsc::sync_channel(8);
        let audio_send = send.clone();
        let wake = ctx.clone();
        let devices = Devices {
            input: ui.voice_input.clone(),
            output: ui.voice_output.clone(),
        };
        let audio = Audio::start(
            devices.clone(),
            capture_send,
            playback_receive,
            move |result| {
                let _ = audio_send.try_send(match result {
                    Ok(()) => Notice::DeviceReady,
                    Err(error) => Notice::Failed(error),
                });
                wake.request_repaint();
            },
        )?;
        let (controls, control_receive) = watch::channel(Controls {
            muted: ui.voice_push_to_talk,
            deafened: false,
        });
        audio.set_controls(ui.voice_push_to_talk, false);
        let session = pending.session.ok_or("Missing voice session")?;
        let session_copy = Zeroizing::new(session.expose().to_owned());
        let (token, endpoint) = pending.server.ok_or("Missing voice server")?;
        let credentials = voice::VoiceConnection {
            channel: pending.channel,
            request: pending.request,
            user: pending.user,
            peer: pending.peer,
            session,
            token,
            endpoint,
        };
        let wake = ctx.clone();
        let task = runtime.spawn(async move {
            let status = send.clone();
            let status_wake = wake.clone();
            let result = discord_voice::run(
                credentials,
                capture,
                playback,
                control_receive,
                move |event| {
                    let notice = match event {
                        Status::TransportReady => Notice::TransportReady,
                        Status::Securing => Notice::Securing,
                        Status::Ready { privacy_code } => {
                            if privacy_code.len() > 256 {
                                return Err(());
                            }
                            Notice::MediaReady(privacy_code)
                        }
                        Status::RemoteAudio => Notice::RemoteAudio,
                    };
                    status.try_send(notice).map_err(|_| ())?;
                    status_wake.request_repaint();
                    Ok(())
                },
            )
            .await;
            if let Err(error) = result {
                let _ = send.try_send(Notice::Failed(error));
            }
            wake.request_repaint();
        });
        self.live = Some(Live {
            generation: pending.generation,
            channel: pending.channel,
            request: pending.request,
            session: session_copy,
            ring_pending: pending.ring,
            audio,
            controls,
            events,
            task,
            devices,
        });
        Ok(())
    }
}
impl Drop for Voice {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negotiation_requires_matching_request_owner_and_session_without_opening_devices() {
        let mut state = test_support::demo_state();
        state.demo = false;
        state.start_call(Id(22), true).unwrap();
        let request = state.voice.active.as_ref().unwrap().request;
        let mut manager = Voice::default();
        manager.begin(&state, true).unwrap();
        let mut stale = Event::Voice(voice::Event::Server {
            channel: Id(22),
            request: request + 1,
            token: Some(Secret::new("synthetic-token".into()).unwrap()),
            endpoint: Some("synthetic.discord.media".into()),
        });
        assert!(manager.observe(&state, &mut stale).is_none());
        assert!(manager.pending.as_ref().unwrap().server.is_none());
        let mut server = Event::Voice(voice::Event::Server {
            channel: Id(22),
            request,
            token: Some(Secret::new("synthetic-token".into()).unwrap()),
            endpoint: Some("synthetic.discord.media".into()),
        });
        assert!(manager.observe(&state, &mut server).is_none());
        assert!(manager.pending.as_ref().unwrap().server.is_some());
        let session = |user, id: &str| {
            Event::Voice(voice::Event::State {
                request: Some(request),
                channel: Some(Id(22)),
                user: Id(user),
                session: Some(Secret::new(id.into()).unwrap()),
                muted: false,
                deafened: false,
            })
        };
        assert!(
            manager
                .observe(&state, &mut session(2, "other-session"))
                .is_none()
        );
        assert!(manager.pending.as_ref().unwrap().session.is_none());
        assert!(
            manager
                .observe(&state, &mut session(1, "synthetic-session"))
                .is_none()
        );
        assert!(
            manager
                .observe(&state, &mut session(1, "changed-session"))
                .is_some()
        );
        assert!(manager.live.is_none());
        manager.stop();
        assert!(manager.pending.is_none());
    }
}
