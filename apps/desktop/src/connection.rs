use client_core::{
    COMMAND_SLOTS, Command, EVENT_SLOTS, Envelope, Event, MAX_EVENT_BYTES,
    auth::{AuthProvider, Failure, SessionSecret},
};
use discord_api::DiscordApi;
use eframe::egui;
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};
use tokio::{
    runtime::Handle,
    sync::{mpsc, watch},
    task::JoinHandle,
};

pub struct Connection {
    pub commands: mpsc::Sender<Command>,
    pub events: mpsc::Receiver<Envelope>,
    pub terminal: watch::Receiver<Option<Failure>>,
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
    pub fn start(
        runtime: &Handle,
        secret: Arc<SessionSecret>,
        generation: u64,
        expected_user: Option<model::Id>,
        ctx: egui::Context,
    ) -> Self {
        let (commands, mut receive) = mpsc::channel(COMMAND_SLOTS);
        let (send, events) = mpsc::channel(EVENT_SLOTS);
        let (finished, terminal) = watch::channel(None);
        let wake = ctx.clone();
        let task=runtime.spawn(async move {
            let emit=move |event:Event| -> Result<(),Failure> {
                if event.bytes()>MAX_EVENT_BYTES {return Err(Failure::Capacity);}
                send.try_send(Envelope {generation,event}).map_err(|_|Failure::Capacity)?;ctx.request_repaint();Ok(())
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
                    }).await.err().unwrap_or(Failure::Network);
                    gateway_api.stop();let _=terminal_send.send(Some(error));gateway_wake.request_repaint();
                }));
                // Keep hangup/mute controls responsive while an HTTP message write is awaiting Discord.
                let (write_send,mut write_receive)=mpsc::channel(COMMAND_SLOTS);
                let write_api=api.clone();let write_emit=emit.clone();let write_finished=finished.clone();let write_wake=wake.clone();
                let mut writes=AbortTask(tokio::spawn(async move {
                    while let Some(command)=write_receive.recv().await {
                        let event=write_api.execute(command).await;
                        let failure=match &event {Event::Failure(f)=>Some(*f),Event::SendResult{result:Err(f),..}=>Some(*f),_=>None};
                        let error=write_emit(event).err().or(failure.filter(|f|f.ends_session()));
                        if let Some(error)=error {write_api.stop();let _=write_finished.send(Some(error));write_wake.request_repaint();break;}
                    }
                }));
                let mut history:Option<AbortTask>=None;
                let mut ringing:Option<AbortTask>=None;
                let mut voice_request=None;
                loop {
                    tokio::select! {
                        _=&mut gateway_task.0=>{break;}
                        _=&mut writes.0=>{break;}
                        changed=voice_availability.changed()=> {
                            if changed.is_err() {break;}
                            if !*voice_availability.borrow_and_update() {drop(ringing.take());voice_request=None;}
                        }
                        command=receive.recv()=>{
                            let Some(command)=command else {break;};
                            if let Command::Voice(control)=command {
                                use client_core::voice::{Command as V,Event as E};
                                let (channel,request)=match control {V::Join{channel,request,..}|V::Ring{channel,request}|V::Leave{channel,request}|V::SetMute{channel,request,..}=>(channel,request),V::Decline{channel}=>(channel,0)};
                                if !*voice_availability.borrow() || !dm_channels.lock().map_err(|_|Failure::Protocol)?.contains(&channel) {
                                    emit(Event::Voice(E::Failed{channel,request,message:"The DM is unavailable; no call was started"}))?;continue;
                                }
                                let ring=match ring_action(control,user.id,&mut voice_request) {
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
                            if let Command::Members {guild,channel,request,list_id} = command {
                                let subscription=match (guild,channel,list_id) { (Some(guild),Some(channel),Some(list_id)) => Some(discord_gateway::MemberSubscription {guild,channel,request,list_id}), _=>None };
                                member_send.send(subscription).map_err(|_|Failure::Network)?;
                                continue;
                            }
                            if let Command::History { channel, request, .. } = &command {
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
            events,
            terminal,
            task,
        }
    }
}

// Ring only after the media adapter confirms transport allocation, and only once per current call.
fn ring_action(
    control: client_core::voice::Command,
    owner: model::Id,
    active: &mut Option<(model::Id, u64, bool)>,
) -> Result<Option<(Option<model::Id>, bool)>, ()> {
    use client_core::voice::Command as V;
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
                &mut active
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
                &mut active
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
                &mut active
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
                &mut active
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
                &mut active
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
                &mut active
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
                &mut active
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
                &mut active
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
                &mut active
            )
            .is_err()
        );
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
