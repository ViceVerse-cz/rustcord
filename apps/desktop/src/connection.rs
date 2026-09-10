use client_core::{
    COMMAND_SLOTS, Command, EVENT_SLOTS, Envelope, Event, MAX_EVENT_BYTES,
    auth::{AuthProvider, Failure, SessionSecret},
};
use discord_api::DiscordApi;
use eframe::egui;
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    runtime::Handle,
    sync::{mpsc, watch},
    task::JoinHandle,
};

pub struct Connection {
    pub commands: mpsc::Sender<Command>,
    pub uploads: mpsc::Sender<crate::uploads::UploadRequest>,
    pub events: mpsc::Receiver<Envelope>,
    pub typing: mpsc::Receiver<Envelope>,
    pub terminal: watch::Receiver<Option<Failure>>,
    typing_channel: Arc<AtomicU64>,
    task: JoinHandle<()>,
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.task.abort();
    }
}
struct AbortTask(JoinHandle<()>);
impl Drop for AbortTask {
    fn drop(&mut self) {
        self.0.abort();
    }
}
impl Connection {
    pub fn set_typing_channel(&self, channel: Option<model::Id>) {
        self.typing_channel
            .store(channel.map_or(0, |id| id.0), Ordering::Relaxed);
    }
    pub fn start(
        runtime: &Handle,
        secret: Arc<SessionSecret>,
        generation: u64,
        expected_user: Option<model::Id>,
        ctx: egui::Context,
    ) -> Self {
        let (commands, mut receive) = mpsc::channel(COMMAND_SLOTS);
        let (uploads, mut upload_receive) = mpsc::channel::<crate::uploads::UploadRequest>(1);
        let (send, events) = mpsc::channel(EVENT_SLOTS);
        let (typing_send, typing) = mpsc::channel(8);
        let (finished, terminal) = watch::channel(None);
        let wake = ctx.clone();
        let typing_channel = Arc::new(AtomicU64::new(0));
        let active_typing = typing_channel.clone();
        let typing_gate = Mutex::new(TypingGate::default());
        let task=runtime.spawn(async move {
            let emit=move |event:Event| -> Result<(),Failure> {
                if let Event::Typing(signal) = &event {
                    let active = active_typing.load(Ordering::Relaxed);
                    if !typing_gate.lock().is_ok_and(|mut gate| gate.accept(*signal, active, Instant::now())) { return Ok(()); }
                }
                emit_event(&send, &typing_send, Envelope {generation,event}, &ctx)
            };
            let result=async {
                let mut api=DiscordApi::new(secret.clone())?;
                let user=api.authenticate().await?;
                if expected_user.is_some_and(|id|id!=user.id){return Err(Failure::InvalidCredential);}
                let gateway=api.gateway_url().await?;
                let api=Arc::new(api);
                let emit=Arc::new(emit);
                let (member_send,member_receive)=watch::channel(None);
                let (voice_send,voice_receive)=mpsc::channel(8);
                let dm_channels=Arc::new(Mutex::new(BTreeSet::new()));
                let gateway_channels=dm_channels.clone();
                let (voice_online,mut voice_availability)=watch::channel(false);
                let gateway_api=api.clone();let gateway_emit=emit.clone();let terminal_send=finished.clone();
                let gateway_wake=wake.clone();
                let mut gateway_task=AbortTask(tokio::spawn(async move {
                    let error=discord_gateway::run_with_voice(secret,gateway,member_receive,voice_receive,|event|{
                        if let Event::Ready{user:ready_user,channels,..}=&event {
                            if ready_user.id!=user.id {return Err(Failure::InvalidCredential);}
                            *gateway_channels.lock().map_err(|_|Failure::Protocol)?=channels.iter().filter(|c|c.guild.is_none()&&c.kind==1&&c.recipients.len()==1).map(|c|c.id).collect();
                        }
                        if let Event::ChannelCreated(channel)=&event {
                            let mut channels=gateway_channels.lock().map_err(|_|Failure::Protocol)?;
                            channels.remove(&channel.id);
                            if channel.guild.is_none() && channel.kind==1 && channel.recipients.len()==1 && channels.len()<client_core::MAX_NAV {channels.insert(channel.id);}
                        }
                        if let Event::Unavailable(channel)=&event {gateway_channels.lock().map_err(|_|Failure::Protocol)?.remove(channel);}

                        if matches!(&event,Event::Ready{..}|Event::Resumed) {let _=voice_online.send(true);}
                        if matches!(&event,Event::Disconnected|Event::Resync) {let _=voice_online.send(false);}
                        gateway_emit(event)
                    }).await.err().unwrap_or(Failure::Network).protocol_at("Gateway connection: unsupported handshake or event");
                    gateway_api.stop();let _=terminal_send.send(Some(error));gateway_wake.request_repaint();
                }));
                // Keep hangup/mute controls responsive while an HTTP message write is awaiting Discord.
                let (write_send,mut write_receive)=mpsc::channel(COMMAND_SLOTS);
                let write_api=api.clone();let write_emit=emit.clone();let write_finished=finished.clone();let write_wake=wake.clone();
                let mut writes=AbortTask(tokio::spawn(async move {
                    while let Some(command)=write_receive.recv().await {
                        let event=write_api.execute(command).await;
                        let failure=match &event {Event::Failure(f)=>Some(*f),Event::SendResult{result:Err(f),..}=>Some(*f),Event::Reactions(client_core::reactions::Event::Written{result:Err(f),..})=>Some(*f),Event::ReadState(client_core::read_state::Event::Result{result:Err(f),..})=>Some(*f),_=>None};
                        let error=write_emit(event).err().or(failure.filter(|f|f.ends_session()));
                        if let Some(error)=error {write_api.stop();let _=write_finished.send(Some(error));write_wake.request_repaint();break;}
                    }
                }));
                let mut history:Option<AbortTask>=None;
                let mut profile:Option<AbortTask>=None;
                let mut search:Option<AbortTask>=None;
                let mut reaction_read:Option<AbortTask>=None;
                let mut ringing:Option<AbortTask>=None;
                let mut upload:Option<AbortTask>=None;
                let mut upload_cancel:Option<watch::Sender<bool>>=None;
                let mut voice_request=None;
                loop {
                    tokio::select! {
                        _=&mut gateway_task.0=>{break;}
                        _=&mut writes.0=>{break;}
                        changed=voice_availability.changed()=> {
                            if changed.is_err() {break;}
                            if !*voice_availability.borrow_and_update() {drop(ringing.take());drop(profile.take());drop(search.take());voice_request=None;if let Some(cancel)=&upload_cancel {let _=cancel.send(true);}}
                        }
                        request=upload_receive.recv()=>{
                            let Some(request)=request else {break;};
                            if !*voice_availability.borrow() || upload.as_ref().is_some_and(|job|!job.0.is_finished()) {
                                if let Command::Send{nonce,..}=request.command {
                                    emit(Event::SendResult{nonce,result:Err(Failure::ProtocolAt("Upload unavailable; reselect the file to retry"))})?;
                                }
                                request.progress.send_replace(discord_api::upload::Status::Failed("Upload unavailable; reselect the file to retry"));
                                continue;
                            }
                            upload_cancel=Some(request.cancel.clone());
                            let api=api.clone();let emit=emit.clone();let finished=finished.clone();let wake=wake.clone();
                            upload=Some(AbortTask(tokio::spawn(async move {
                                let mut updates=request.progress.subscribe();
                                let operation=api.upload_message(request.command,request.source,request.progress,request.cancel.subscribe());
                                tokio::pin!(operation);
                                let mut observing=true;
                                let event=loop {
                                    tokio::select! {
                                        event=&mut operation=>break event,
                                        changed=updates.changed(), if observing=>{observing=changed.is_ok();wake.request_repaint();}
                                    }
                                };
                                let failure=match &event {Event::SendResult{result:Err(f),..} if f.ends_session()=>Some(*f),_=>None};
                                let error=emit(event).err().or(failure);
                                if let Some(error)=error {api.stop();let _=finished.send(Some(error));}
                                wake.request_repaint();
                            })));
                        }
                        command=receive.recv()=>{
                            let Some(command)=command else {break;};
                            if matches!(command,Command::CancelSearch) {drop(search.take());continue;}
                            if matches!(command,Command::Search{..}|Command::Pins{..}|Command::Archives{..}) {
                                drop(search.take());
                                let api=api.clone();let emit=emit.clone();let finished=finished.clone();let wake=wake.clone();
                                search=Some(AbortTask(tokio::spawn(async move {
                                    let event=api.execute(command).await;
                                    let failure=match &event {Event::Search{result:Err(f),..}|Event::Archives{result:Err(f),..} if f.ends_session() && *f!=Failure::Capacity=>Some(*f),_=>None};
                                    let error=emit(event).err().or(failure);
                                    if let Some(error)=error {api.stop();let _=finished.send(Some(error));}
                                    wake.request_repaint();
                                })));
                                continue;
                            }
                            if matches!(command,Command::Reactions(client_core::reactions::Command::Read{..})) {
                                drop(reaction_read.take());
                                let api=api.clone();let emit=emit.clone();let finished=finished.clone();let wake=wake.clone();
                                reaction_read=Some(AbortTask(tokio::spawn(async move {
                                    let event=api.execute(command).await;
                                    let failure=match &event {Event::Reactions(client_core::reactions::Event::Read{result:Err(f),..}) if f.ends_session()=>Some(*f),_=>None};
                                    let error=emit(event).err().or(failure);
                                    if let Some(error)=error {api.stop();let _=finished.send(Some(error));}
                                    wake.request_repaint();
                                })));
                                continue;
                            }
                            if let Command::Voice(control)=command {
                                use client_core::voice::{Command as V,Event as E};
                                let (channel,request)=match control {V::Join{channel,request,..}|V::Ring{channel,request}|V::Leave{channel,request}|V::SetMute{channel,request,..}=>(channel,request),V::Decline{channel}=>(channel,0)};
                                if !*voice_availability.borrow() {
                                    emit(Event::Voice(E::Failed{channel,request,message:"Voice is disconnected; no call was started"}))?;continue;
                                }
                                let ring=match ring_action(control,user.id,&mut voice_request,dm_channels.lock().map_err(|_|Failure::Protocol)?.contains(&channel)) {
                                    Ok(action)=>action,
                                    Err(())=>{emit(Event::Voice(E::Failed{channel,request,message:"Call action expired; no ringing request was sent"}))?;continue;}
                                };
                                voice_send.try_send(control).map_err(|_|Failure::Capacity)?;
                                if let Some((recipient,stop))=ring {
                                    drop(ringing.take());
                                    let api=api.clone();let emit=emit.clone();let voice_send=voice_send.clone();let finished=finished.clone();let ring_wake=wake.clone();
                                    ringing=Some(AbortTask(tokio::spawn(async move {
                                        if let Err(failure)=api.ring_call(channel,recipient,stop).await {
                                            if !stop {let _=voice_send.try_send(V::Leave{channel,request});}
                                            let _=emit(Event::Voice(E::Failed{channel,request,message:failure.label()}));
                                            if failure.ends_session(){api.stop();let _=finished.send(Some(failure));ring_wake.request_repaint();}
                                        }
                                    })));
                                }
                                continue;
                            }
                            if matches!(command,Command::CancelProfile) {drop(profile.take());continue;}
                            if matches!(command,Command::Profile{..}) {
                                drop(profile.take());
                                let api=api.clone();let emit=emit.clone();let finished=finished.clone();let wake=wake.clone();
                                profile=Some(AbortTask(tokio::spawn(async move {
                                    let event=api.execute(command).await;
                                    let failure=match &event {Event::Profile{result:Err(failure),..} if failure.ends_session() && *failure!=Failure::Capacity=>Some(*failure),_=>None};
                                    let error=emit(event).err().or(failure);
                                    if let Some(error)=error {api.stop();let _=finished.send(Some(error));}
                                    wake.request_repaint();
                                })));
                                continue;
                            }
                            if let Command::Members {guild,channel,request,list_id} = command {
                                let subscription=match (guild,channel,list_id) { (Some(guild),Some(channel),Some(list_id)) => Some(discord_gateway::MemberSubscription {guild,channel,request,list_id}), _=>None };
                                member_send.send(subscription).map_err(|_|Failure::Network)?;
                                continue;
                            }
                            if let Command::History { channel, request, .. } = &command {
                                drop(search.take());
                                drop(reaction_read.take());
                                let (channel, request) = (*channel, *request);
                                drop(history.take());
                                let api=api.clone();let emit=emit.clone();let finished=finished.clone();let history_wake=wake.clone();
                                history=Some(AbortTask(tokio::spawn(async move {
                                    let event=scope_history_failure(api.execute(command).await,channel,request);
                                    if let Event::Failure(f)=&event&& f.ends_session(){api.stop();let _=finished.send(Some(*f));}
                                    if let Err(f)=emit(event){api.stop();let _=finished.send(Some(f));}
                                    history_wake.request_repaint();
                                })));
                            } else {
                                write_send.try_send(command).map_err(|_|Failure::Capacity)?;
                            }
                        }
                    }
                }
                Ok::<(),Failure>(())
            }.await;
            if let Err(f)=result {let _=finished.send(Some(f));wake.request_repaint();}
        });
        Self {
            commands,
            uploads,
            events,
            typing,
            terminal,
            typing_channel,
            task,
        }
    }
}

fn emit_event(
    reliable: &mpsc::Sender<Envelope>,
    typing: &mpsc::Sender<Envelope>,
    envelope: Envelope,
    ctx: &egui::Context,
) -> Result<(), Failure> {
    if matches!(envelope.event, Event::Typing(_)) {
        // Ephemeral signals have separate fixed slots and may be dropped under pressure.
        if typing.try_send(envelope).is_ok() {
            ctx.request_repaint();
        }
        return Ok(());
    }
    if envelope.event.bytes() > MAX_EVENT_BYTES {
        return Err(Failure::Capacity);
    }
    reliable.try_send(envelope).map_err(|_| Failure::Capacity)?;
    ctx.request_repaint();
    Ok(())
}

/// At most eight typing wakeups per two seconds in the selected conversation.
#[derive(Default)]
struct TypingGate {
    channel: u64,
    users: [Option<(model::Id, Instant)>; 8],
}
impl TypingGate {
    fn accept(&mut self, signal: client_core::typing::Signal, active: u64, now: Instant) -> bool {
        if active == 0 || signal.channel.0 != active || signal.user.0 == 0 {
            return false;
        }
        if self.channel != active {
            self.channel = active;
            self.users.fill(None);
        }
        for slot in &mut self.users {
            if slot.is_some_and(|(_, time)| {
                now.saturating_duration_since(time) >= Duration::from_secs(2)
            }) {
                *slot = None;
            }
        }
        if self
            .users
            .iter()
            .flatten()
            .any(|(user, _)| *user == signal.user)
        {
            return false;
        }
        let Some(slot) = self.users.iter_mut().find(|slot| slot.is_none()) else {
            return false;
        };
        *slot = Some((signal.user, now));
        true
    }
}

// Ring only after the media adapter confirms transport allocation, and only once per current call.
fn ring_action(
    control: client_core::voice::Command,
    owner: model::Id,
    active: &mut Option<(model::Id, u64, bool)>,
    dm: bool,
) -> Result<Option<(Option<model::Id>, bool)>, ()> {
    use client_core::voice::Command as V;
    if !dm {
        return match control {
            V::Ring { .. } | V::Decline { .. } | V::Join { ring: true, .. } => Err(()),
            V::Join { .. } | V::Leave { .. } | V::SetMute { .. } => Ok(None),
        };
    }
    match control {
        V::Join {
            channel,
            request,
            ring,
        } => {
            *active = Some((channel, request, !ring));
            Ok(None)
        }
        V::Ring { channel, request } => {
            let Some((current, current_request, rang)) = active else {
                return Err(());
            };
            if *current != channel || *current_request != request || *rang {
                return Err(());
            }
            *rang = true;
            Ok(Some((None, false)))
        }
        V::Leave { channel, request } => {
            if active.is_some_and(|(id, r, _)| id == channel && r == request) {
                *active = None;
                Ok(Some((None, true)))
            } else {
                Ok(None)
            }
        }
        V::Decline { .. } => Ok(Some((Some(owner), true))),
        V::SetMute { .. } => Ok(None),
    }
}

// Reads can finish after navigation/cancellation. Only a session-ending failure is global.
fn scope_history_failure(event: Event, channel: model::Id, request: u64) -> Event {
    let failure = match event {
        Event::Failure(failure) if !failure.ends_session() => failure,
        Event::Unavailable(_) => Failure::Forbidden,
        event => return event,
    };
    Event::HistoryFailed {
        channel,
        request,
        failure,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn typing_burst_cannot_consume_reliable_message_slots() {
        let (send, mut events) = tokio::sync::mpsc::channel(client_core::EVENT_SLOTS);
        let (typing_send, mut typing) = tokio::sync::mpsc::channel(8);
        let ctx = eframe::egui::Context::default();
        for user in 1..=100 {
            super::emit_event(
                &send,
                &typing_send,
                client_core::Envelope {
                    generation: 1,
                    event: client_core::Event::Typing(client_core::typing::Signal {
                        channel: model::Id(10),
                        user: model::Id(user),
                        timestamp: 1,
                    }),
                },
                &ctx,
            )
            .unwrap();
        }
        assert_eq!(typing.len(), 8);
        assert!(events.is_empty());
        super::emit_event(
            &send,
            &typing_send,
            client_core::Envelope {
                generation: 1,
                event: client_core::Event::Delete {
                    channel: model::Id(10),
                    id: model::Id(20),
                },
            },
            &ctx,
        )
        .unwrap();
        assert!(matches!(
            events.try_recv().unwrap().event,
            client_core::Event::Delete { .. }
        ));
        for _ in 0..8 {
            typing.try_recv().unwrap();
        }
    }
    #[test]
    fn typing_wakeups_are_selected_bounded_and_coalesced() {
        use client_core::typing::Signal;
        use model::Id;
        use std::time::{Duration, Instant};
        let mut gate = super::TypingGate::default();
        let now = Instant::now();
        let signal = Signal {
            channel: Id(10),
            user: Id(1),
            timestamp: 1,
        };
        assert!(!gate.accept(signal, 0, now));
        assert!(!gate.accept(signal, 11, now));
        for user in 1..=8 {
            let signal = Signal {
                user: Id(user),
                ..signal
            };
            assert!(gate.accept(signal, 10, now));
            assert!(!gate.accept(signal, 10, now));
        }
        for user in 9..=1_000 {
            assert!(!gate.accept(
                Signal {
                    user: Id(user),
                    ..signal
                },
                10,
                now
            ));
        }
        assert!(!gate.accept(signal, 10, now + Duration::from_millis(1_999)));
        assert!(gate.accept(signal, 10, now + Duration::from_secs(2)));
        assert!(gate.accept(
            Signal {
                channel: Id(11),
                ..signal
            },
            11,
            now
        ));
        assert!(!gate.accept(signal, 11, now));
    }
    use super::*;
    #[test]
    fn ringing_waits_for_transport_confirmation_and_rejects_old_requests() {
        use client_core::voice::Command as V;
        let mut active = None;
        let channel = model::Id(2);
        let owner = model::Id(1);
        assert_eq!(
            ring_action(
                V::Join {
                    channel,
                    request: 7,
                    ring: true
                },
                owner,
                &mut active,
                true
            ),
            Ok(None)
        );
        assert!(
            ring_action(
                V::Ring {
                    channel,
                    request: 6
                },
                owner,
                &mut active,
                true
            )
            .is_err()
        );
        assert_eq!(
            ring_action(
                V::Ring {
                    channel,
                    request: 7
                },
                owner,
                &mut active,
                true
            ),
            Ok(Some((None, false)))
        );
        assert!(
            ring_action(
                V::Ring {
                    channel,
                    request: 7
                },
                owner,
                &mut active,
                true
            )
            .is_err()
        );
        assert_eq!(
            ring_action(
                V::Leave {
                    channel,
                    request: 7
                },
                owner,
                &mut active,
                true
            ),
            Ok(Some((None, true)))
        );
        assert!(
            ring_action(
                V::Ring {
                    channel,
                    request: 7
                },
                owner,
                &mut active,
                true
            )
            .is_err()
        );
        assert_eq!(
            ring_action(
                V::Join {
                    channel,
                    request: 8,
                    ring: false
                },
                owner,
                &mut active,
                true
            ),
            Ok(None)
        );
        assert_eq!(
            ring_action(
                V::Leave {
                    channel,
                    request: 7
                },
                owner,
                &mut active,
                true
            ),
            Ok(None)
        );
        assert_eq!(active, Some((channel, 8, true)));
        assert!(
            ring_action(
                V::Ring {
                    channel,
                    request: 8
                },
                owner,
                &mut active,
                true
            )
            .is_err()
        );
    }
    #[test]
    fn guild_join_mute_leave_never_ring_a_dm() {
        use client_core::voice::Command as V;
        let mut active = None;
        let channel = model::Id(20);
        let owner = model::Id(1);
        for control in [
            V::Join {
                channel,
                request: 1,
                ring: false,
            },
            V::SetMute {
                channel,
                request: 1,
                mute: true,
                deaf: false,
            },
            V::Leave {
                channel,
                request: 1,
            },
        ] {
            assert_eq!(ring_action(control, owner, &mut active, false), Ok(None));
        }
        assert!(
            ring_action(
                V::Ring {
                    channel,
                    request: 1
                },
                owner,
                &mut active,
                false
            )
            .is_err()
        );
        assert!(ring_action(V::Decline { channel }, owner, &mut active, false).is_err());
        assert!(active.is_none());
    }
    #[test]
    fn reads_scope_permission_errors_but_expiry_stays_global() {
        assert!(matches!(
            scope_history_failure(Event::Unavailable(model::Id(1)), model::Id(1), 9),
            Event::HistoryFailed {
                channel: model::Id(1),
                request: 9,
                failure: Failure::Forbidden
            }
        ));
        assert!(matches!(
            scope_history_failure(Event::Failure(Failure::Network), model::Id(1), 9),
            Event::HistoryFailed {
                request: 9,
                failure: Failure::Network,
                ..
            }
        ));
        assert!(matches!(
            scope_history_failure(Event::Failure(Failure::Expired), model::Id(1), 9),
            Event::Failure(Failure::Expired)
        ));
    }
}
