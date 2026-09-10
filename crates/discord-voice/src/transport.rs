use crate::{
    Controls, Frame, Status,
    crypto::{Dave, Encryption, MAX_PACKET, MAX_SIGNAL, MODE},
};
use client_core::voice::VoiceConnection;
use futures_util::{SinkExt, StreamExt};
use opus2::{Application, Bitrate, Channels, Encoder};
use serde_json::{Value, json};
use std::{
    net::{IpAddr, SocketAddr},
    sync::mpsc::{Receiver, SyncSender},
    time::Duration,
};
use tokio::{
    net::{TcpStream, UdpSocket},
    sync::watch,
    time::{Instant, MissedTickBehavior, timeout},
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{Message, protocol::WebSocketConfig},
};
use zeroize::Zeroizing;
type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

fn negotiation_timeout(
    hello: bool,
    transport: bool,
    key: bool,
    dave: &Dave,
    resuming: bool,
) -> &'static str {
    if resuming {
        "Discord voice resume acknowledgement timed out; rejoin the call"
    } else if !hello {
        "Discord voice Hello timed out; rejoin the call"
    } else if !transport {
        "Discord voice Ready timed out; rejoin the call"
    } else if !key {
        "Discord voice protocol selection timed out; no transport key was received"
    } else if dave.session.is_ready() && dave.pending.is_some() {
        "Discord DAVE transition execution timed out; no audio was enabled"
    } else {
        "Discord DAVE group negotiation timed out; no accepted commit or welcome was received"
    }
}

fn endpoint(raw: &str) -> Result<String, &'static str> {
    if raw.len() > 256
        || raw.contains('/')
        || raw.contains('@')
        || raw.contains('?')
        || raw.contains('#')
    {
        return Err("Invalid Discord voice endpoint");
    }
    let url = url::Url::parse(&format!("wss://{raw}/?v=8"))
        .map_err(|_| "Invalid Discord voice endpoint")?;
    let host = url.host_str().ok_or("Missing Discord voice host")?;
    if !host.ends_with(".discord.media") || url.username() != "" || url.password().is_some() {
        return Err("Voice endpoint is outside Discord media");
    }
    Ok(url.into())
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            !ip.is_private()
                && !ip.is_loopback()
                && !ip.is_link_local()
                && !ip.is_broadcast()
                && !ip.is_multicast()
                && !ip.is_unspecified()
                && !ip.is_documentation()
                && ip.octets()[0] != 0
                && ip.octets()[0] < 240
                && !(ip.octets()[0] == 100 && (64..128).contains(&ip.octets()[1]))
        }
        IpAddr::V6(ip) => {
            !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_multicast()
                && !ip.is_unique_local()
                && !ip.is_unicast_link_local()
                && ip.to_ipv4_mapped().is_none()
                && !(ip.segments()[0] == 0x2001 && ip.segments()[1] == 0xdb8)
        }
    }
}
async fn send(ws: &mut Socket, message: Message) -> Result<(), &'static str> {
    timeout(Duration::from_secs(5), ws.send(message))
        .await
        .map_err(|_| "Voice signaling write timed out")?
        .map_err(|_| "Voice signaling disconnected")
}
async fn json_send(ws: &mut Socket, value: Value) -> Result<(), &'static str> {
    send(ws, Message::Text(value.to_string().into())).await
}
fn number(data: &Value, key: &str) -> Result<u64, &'static str> {
    data[key].as_u64().ok_or("Malformed voice signaling field")
}
fn id(data: &Value, key: &str) -> Result<u64, &'static str> {
    data[key]
        .as_str()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v != 0)
        .ok_or("Malformed voice participant")
}
fn transition(data: &Value) -> Result<u16, &'static str> {
    u16::try_from(number(data, "transition_id")?).map_err(|_| "Invalid voice transition")
}
fn discovery(packet: &[u8], ssrc: u32) -> Result<(IpAddr, u16), &'static str> {
    if packet.len() != 74 || packet[..4] != [0, 2, 0, 70] || packet[4..8] != ssrc.to_be_bytes() {
        return Err("Invalid voice UDP discovery reply");
    }
    let end = packet[8..72]
        .iter()
        .position(|b| *b == 0)
        .ok_or("Invalid voice discovery address")?;
    let address = std::str::from_utf8(&packet[8..8 + end])
        .map_err(|_| "Invalid voice discovery address")?
        .parse()
        .map_err(|_| "Invalid voice discovery address")?;
    let port = u16::from_be_bytes([packet[72], packet[73]]);
    if port == 0 {
        return Err("Invalid voice discovery port");
    }
    Ok((address, port))
}

/// Drop the control sender or abort this future to stop the socket, UDP, codecs and ephemeral keys.
/// PCM queues must contain at most eight 20ms mono48k frames each. No audio device opens here.
pub async fn run(
    credentials: VoiceConnection,
    capture: Receiver<Frame>,
    playback: SyncSender<Frame>,
    controls: watch::Receiver<Controls>,
    emit: impl Fn(Status) -> Result<(), ()>,
) -> Result<(), &'static str> {
    let url = endpoint(&credentials.endpoint)?;
    run_inner(credentials, capture, playback, controls, emit, url, false).await
}
async fn run_inner(
    credentials: VoiceConnection,
    capture: Receiver<Frame>,
    playback: SyncSender<Frame>,
    mut controls: watch::Receiver<Controls>,
    emit: impl Fn(Status) -> Result<(), ()>,
    url: String,
    local_test: bool,
) -> Result<(), &'static str> {
    emit(Status::Connecting).map_err(|_| "Call interface closed")?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_SIGNAL))
        .max_frame_size(Some(MAX_SIGNAL))
        .write_buffer_size(0)
        .max_write_buffer_size(MAX_SIGNAL * 2);
    let (mut ws, _) = timeout(
        Duration::from_secs(15),
        tokio_tungstenite::connect_async_with_config(&url, Some(config), false),
    )
    .await
    .map_err(|_| "Voice connection timed out")?
    .map_err(|_| "Voice TLS connection failed")?;
    // Only voice-scoped credentials go to this validated endpoint; no account Authorization header.
    json_send(&mut ws,json!({"op":0,"d":{"server_id":credentials.guild.unwrap_or(credentials.channel).to_string(),"user_id":credentials.user.to_string(),"session_id":credentials.session.expose(),"token":credentials.token.expose(),"video":false,"max_dave_protocol_version":1}})).await?;
    let mut dave = Dave::new(
        credentials.user.0,
        credentials.peer.map(|peer| peer.0),
        credentials.channel.0,
    )?;
    let mut encryption: Option<Encryption> = None;
    let mut udp: Option<UdpSocket> = None;
    let mut discovering = false;
    let mut discovery_deadline = Instant::now();
    let mut ssrc = 0u32;
    let mut mixer = crate::mixer::Mixer::default();
    let mut seq_ack: i64 = -1;
    let mut heartbeat_ms: Option<u64> = None;
    let mut heartbeat_at = Instant::now();
    let mut awaiting_ack = None;
    let mut heartbeat_nonce = 0u64;
    let mut deadline = Some(Instant::now() + Duration::from_secs(90));
    let mut ready_announced = false;
    let mut waiting_announced = false;
    let mut resuming = false;
    let mut resume_attempts = 0u8;
    let mut heard = false;
    let mut speaking = false;
    let mut silence = 0u8;
    let mut encoder = Encoder::new(48_000, Channels::Stereo, Application::Voip)
        .map_err(|_| "Opus encoder initialization failed")?;
    encoder
        .set_bitrate(Bitrate::Bits(64_000))
        .map_err(|_| "Opus bitrate configuration failed")?;
    let mut random = [0; 6];
    getrandom::fill(&mut random).map_err(|_| "Voice random initialization failed")?;
    let mut sequence = u16::from_be_bytes([random[0], random[1]]);
    let mut timestamp = u32::from_be_bytes(random[2..].try_into().unwrap());
    let mut packet = [0u8; MAX_PACKET + 1];
    let mut encoded = [0u8; 1275];
    let mut stereo = [0.0f32; 1920];
    let mut tick = tokio::time::interval(Duration::from_millis(20));
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut signal_window = Instant::now();
    let mut signal_count = 0u16;
    loop {
        tokio::select! {
            changed=controls.changed()=>{ if changed.is_err(){return Ok(());} },
            _=tick.tick()=>{
                let now=Instant::now();
                if deadline.is_some_and(|d|now>=d) {return Err(negotiation_timeout(heartbeat_ms.is_some(),udp.is_some(),encryption.is_some(),&dave,resuming));}
                if discovering && now>=discovery_deadline {return Err("Discord voice UDP discovery timed out; check the network firewall");}
                if let Some(interval)=heartbeat_ms && now>=heartbeat_at {
                    if awaiting_ack.is_some() {return Err("Discord voice heartbeat was not acknowledged; rejoin the call");}
                    heartbeat_nonce=heartbeat_nonce.wrapping_add(1);
                    json_send(&mut ws,json!({"op":3,"d":{"t":heartbeat_nonce,"seq_ack":seq_ack}})).await?;
                    awaiting_ack=Some(heartbeat_nonce);heartbeat_at=now+Duration::from_millis(interval);
                }
                let enabled=dave.ready && encryption.is_some() && !discovering && !resuming;
                let waiting=dave.waiting && encryption.is_some() && !discovering && !resuming;
                if (!enabled && ready_announced) || (!waiting && waiting_announced) {ready_announced=false;waiting_announced=false;emit(Status::Securing).map_err(|_|"Call interface closed")?;}
                if waiting && !waiting_announced {deadline=None;waiting_announced=true;emit(Status::WaitingForPeer).map_err(|_|"Call interface closed")?;}
                if !waiting {waiting_announced=false;}
                if enabled && !ready_announced {
                    deadline=None;ready_announced=true;
                    emit(Status::Ready{privacy_code:dave.session.voice_privacy_code().unwrap_or_default().into()}).map_err(|_|"Call interface closed")?;
                }
                let control=*controls.borrow();
                let mut latest=None;
                // A delayed tick never transmits a burst of stale microphone audio.
                for _ in 0..8 {match capture.try_recv(){Ok(frame)=>latest=Some(frame),Err(_)=>break}}
                let active=enabled && !control.muted && !control.deafened && latest.is_some();
                if active && !speaking {json_send(&mut ws,json!({"op":5,"d":{"speaking":1,"delay":0,"ssrc":ssrc}})).await?;speaking=true;}
                if !active && speaking && silence==0 {silence=5;}
                if enabled && (active || silence>0) {
                    let data=if active {
                        let frame=latest.unwrap();
                        for (sample,pair) in frame.iter().zip(stereo.as_chunks_mut::<2>().0.iter_mut()) {pair.fill(if sample.is_finite(){sample.clamp(-1.0,1.0)}else{0.0});}
                        let length=encoder.encode_float(&stereo,&mut encoded).map_err(|_|"Opus encoding failed")?;
                        dave.session.encrypt_opus(&encoded[..length]).map_err(|_|"DAVE audio encryption failed")?.into_owned()
                    } else {silence-=1;davey::OPUS_SILENCE_PACKET.to_vec()};
                    let mut header=[0;12];header[0]=0x80;header[1]=120;header[2..4].copy_from_slice(&sequence.to_be_bytes());header[4..8].copy_from_slice(&timestamp.to_be_bytes());header[8..12].copy_from_slice(&ssrc.to_be_bytes());
                    let wire=encryption.as_mut().ok_or("Missing voice transport key")?.seal(&header,&data)?;
                    if let Some(socket)=&udp {socket.send(&wire).await.map_err(|_|"Voice UDP send failed")?;}
                    sequence=sequence.wrapping_add(1);
                    if !active && silence==0 {json_send(&mut ws,json!({"op":5,"d":{"speaking":0,"delay":0,"ssrc":ssrc}})).await?;speaking=false;}
                    if active {silence=0;}
                }
                timestamp=timestamp.wrapping_add(960);
                if enabled && !control.deafened {
                    let (frame,remote_audio)=mixer.pop();
                    if let Some(frame)=frame {let _=playback.try_send(frame);}
                    if !heard && remote_audio {heard=true;emit(Status::RemoteAudio).map_err(|_|"Call interface closed")?;}
                } else {mixer.clear();}
            },
            result=async {match &udp {Some(socket)=>socket.recv(&mut packet).await,None=>std::future::pending().await}}=>{
                let length=result.map_err(|_|"Voice UDP receive failed")?;
                if length>MAX_PACKET {continue;}
                if discovering {
                    let (address,port)=discovery(&packet[..length],ssrc)?;
                    json_send(&mut ws,json!({"op":1,"d":{"protocol":"udp","data":{"address":address.to_string(),"port":port,"mode":MODE},"codecs":[{"name":"opus","type":"audio","priority":1000,"payload_type":120}]}})).await?;
                    discovering=false;continue;
                }
                let Some(crypto)=&encryption else{continue;};
                let Some((source,seq,frame))=crypto.open(&packet[..length]) else{continue;};
                let Some(user)=mixer.user(source) else{continue;};
                if !dave.ready || !dave.contains(user) || controls.borrow().deafened {continue;}
                let Ok(opus)=dave.session.decrypt(user,davey::MediaType::AUDIO,&frame) else{continue;};
                mixer.push(source,seq,opus);
            },
            event=ws.next()=>{
                let event=match event {
                    // A server crash preserves the voice session. Other close codes remain terminal.
                    Some(Ok(message)) if !matches!(&message, Message::Close(Some(frame)) if u16::from(frame.code)==4015)=>message,
                    _=>{
                        if encryption.is_none() || resume_attempts>=2 {return Err("Voice socket failed; rejoin the call");}
                        resume_attempts+=1;resuming=true;ready_announced=false;waiting_announced=false;
                        emit(Status::Securing).map_err(|_|"Call interface closed")?;
                        deadline=Some(Instant::now()+Duration::from_secs(30));heartbeat_ms=None;awaiting_ack=None;
                        let config=WebSocketConfig::default().max_message_size(Some(MAX_SIGNAL)).max_frame_size(Some(MAX_SIGNAL)).write_buffer_size(0).max_write_buffer_size(MAX_SIGNAL*2);
                        let (replacement,_)=timeout(Duration::from_secs(15),tokio_tungstenite::connect_async_with_config(&url,Some(config),false)).await.map_err(|_|"Voice resume timed out; rejoin the call")?.map_err(|_|"Voice resume failed; rejoin the call")?;
                        ws=replacement;
                        json_send(&mut ws,json!({"op":7,"d":{"server_id":credentials.guild.unwrap_or(credentials.channel).to_string(),"session_id":credentials.session.expose(),"token":credentials.token.expose(),"seq_ack":seq_ack}})).await?;
                        continue;
                    }
                };
                if signal_window.elapsed()>Duration::from_secs(1){signal_window=Instant::now();signal_count=0;}
                signal_count+=1;if signal_count>256{return Err("Voice signaling exceeded the bounded processing rate");}
                match event {
                    Message::Text(text)=>{
                        let mut event:Value=serde_json::from_str(&text).map_err(|_|"Invalid voice JSON")?;
                        if let Some(seq)=event["seq"].as_i64(){seq_ack=seq;}
                        let op=number(&event,"op")?;let data=&mut event["d"];
                        match op {
                            8=>{
                                let interval=data["heartbeat_interval"].as_f64().filter(|v|v.is_finite() && *v>=100.0 && *v<=120_000.0).ok_or("Invalid voice heartbeat interval")? as u64;
                                if !(100..=120000).contains(&interval) || heartbeat_ms.is_some(){return Err("Invalid voice heartbeat negotiation");}
                                heartbeat_ms=Some(interval.min(5000));heartbeat_at=Instant::now();
                            },
                            6=>{if awaiting_ack.is_none() || data["t"].as_u64()!=awaiting_ack {return Err("Invalid voice heartbeat acknowledgement");}awaiting_ack=None;},
                            2=>{
                                if udp.is_some(){return Err("Unexpected voice transport replacement; rejoin the call");}
                                ssrc=u32::try_from(number(data,"ssrc")?).map_err(|_|"Invalid voice SSRC")?;
                                let address:IpAddr=data["ip"].as_str().ok_or("Missing voice server address")?.parse().map_err(|_|"Invalid voice server address")?;
                                if !public_ip(address) && !(cfg!(test) && local_test && address.is_loopback()){return Err("Voice server advertised a nonpublic address");}
                                let port=u16::try_from(number(data,"port")?).ok().filter(|p|*p>0).ok_or("Invalid voice server port")?;
                                if !data["modes"].as_array().is_some_and(|m|m.iter().any(|m|m.as_str()==Some(MODE))){return Err("Required voice transport encryption is unavailable");}
                                let socket=UdpSocket::bind(if address.is_ipv4(){"0.0.0.0:0"}else{"[::]:0"}).await.map_err(|_|"Could not bind voice UDP socket")?;
                                socket.connect(SocketAddr::new(address,port)).await.map_err(|_|"Could not connect voice UDP socket")?;
                                let mut probe=[0;74];probe[..4].copy_from_slice(&[0,1,0,70]);probe[4..8].copy_from_slice(&ssrc.to_be_bytes());socket.send(&probe).await.map_err(|_|"Voice UDP discovery failed")?;
                                udp=Some(socket);discovering=true;discovery_deadline=Instant::now()+Duration::from_secs(8);
                                emit(Status::Discovering).map_err(|_|"Call interface closed")?;
                            },
                            4=>{
                                if encryption.is_some() || udp.is_none() || discovering {return Err("Unexpected voice session description");}
                                if data["mode"].as_str()!=Some(MODE) || data["dave_protocol_version"].as_u64()!=Some(1){return Err("Required DAVE version 1 encryption was not negotiated");}
                                let values=data["secret_key"].take();let values=values.as_array().ok_or("Missing voice transport key")?;
                                if values.len()!=32{return Err("Invalid voice transport key");}
                                let mut key=Zeroizing::new([0;32]);for (out,v) in key.iter_mut().zip(values){*out=v.as_u64().and_then(|v|u8::try_from(v).ok()).ok_or("Invalid voice transport key")?;}
                                encryption=Some(Encryption::new(&key));
                                send(&mut ws,Message::Binary(dave.key_package()?.into())).await?;
                                emit(Status::TransportReady).map_err(|_|"Call interface closed")?;
                                emit(Status::Securing).map_err(|_|"Call interface closed")?;
                            },
                            5=>{
                                let user=id(data,"user_id")?;
                                if user!=credentials.user.0 && dave.contains(user) {let value=u32::try_from(number(data,"ssrc")?).map_err(|_|"Invalid voice SSRC")?;mixer.announce(user,value)?;}
                            },
                            11=>{
                                let ids=data["user_ids"].as_array().ok_or("Missing voice participants")?;
                                if ids.len()>crate::crypto::MAX_PARTICIPANTS {return Err("Voice channel exceeds the 64 participant limit");}
                                let ids=ids.iter().map(|v|v.as_str().and_then(|v|v.parse::<u64>().ok()).filter(|v|*v!=0).ok_or("Malformed voice participant")).collect::<Result<Vec<_>,_>>()?;
                                if dave.connect(&ids)? {deadline=Some(Instant::now()+Duration::from_secs(90));mixer.clear();}
                            },
                            13=>{
                                let user=id(data,"user_id")?;mixer.remove(user);
                                if dave.disconnect(user)? {deadline=Some(Instant::now()+Duration::from_secs(30));mixer.clear();}
                            },
                            21=>{
                                if number(data,"protocol_version")?!=1 {return Err("Discord requested a voice encryption downgrade; call stopped");}
                                dave.pending=Some(transition(data)?);
                                if dave.pending==Some(0) {if dave.session.is_ready(){dave.execute(0)?;}else if credentials.guild.is_some(){dave.wait_for_peer()?;}else{dave.pending=None;dave.ready=false;}} else {json_send(&mut ws,json!({"op":23,"d":{"transition_id":dave.pending}})).await?;}
                            },
                            22=>{dave.execute(transition(data)?)?;},
                            24=>{
                                if number(data,"protocol_version")?!=1 {return Err("Unsupported DAVE protocol version; call stopped");}
                                if number(data,"epoch")?==1 {dave.reinitialize()?;deadline=Some(Instant::now()+Duration::from_secs(30));send(&mut ws,Message::Binary(dave.key_package()?.into())).await?;}
                            },
                            9=>{if !resuming{return Err("Unexpected voice resumption");}resuming=false;},
                            12=>{
                                let user=id(data,"user_id")?;
                                if user!=credentials.user.0 && dave.contains(user) && let Some(value)=data["audio_ssrc"].as_u64().and_then(|v|u32::try_from(v).ok()) {mixer.announce(user,value)?;}
                            },
                            14..=20=>{},
                            _=>return Err("Unsupported voice signaling opcode; call stopped"),
                        }
                    },
                    Message::Binary(bytes)=>{
                        if bytes.len()<3 {return Err("Truncated DAVE signaling");}
                        seq_ack=i64::from(u16::from_be_bytes([bytes[0],bytes[1]]));
                        let opcode=bytes[2];let data=&bytes[3..];
                        match opcode {
                            25=>dave.session.set_external_sender(data).map_err(|_|"DAVE external sender validation failed")?,
                            27=>{if let Some(response)=dave.proposals(data)?{send(&mut ws,Message::Binary(response.into())).await?;}},
                            29|30=>{
                                deadline=Some(Instant::now()+Duration::from_secs(30));ready_announced=false;waiting_announced=false;mixer.clear();
                                emit(Status::Securing).map_err(|_|"Call interface closed")?;
                                match dave.group_changed(opcode,data){
                                    Ok(id)=>{if id!=0 {json_send(&mut ws,json!({"op":23,"d":{"transition_id":id}})).await?;}},
                                    Err(_)=>{
                                        if data.len()<2 {return Err("Truncated DAVE transition");}
                                        let id=u16::from_be_bytes([data[0],data[1]]);
                                        json_send(&mut ws,json!({"op":31,"d":{"transition_id":id}})).await?;
                                        dave.reset()?;send(&mut ws,Message::Binary(dave.key_package()?.into())).await?;
                                    }
                                }
                            },
                            _=>return Err("Unsupported DAVE opcode; call stopped"),
                        }
                    },
                    Message::Ping(data)=>send(&mut ws,Message::Pong(data)).await?,
                    Message::Close(_)=>return Err("Discord voice connection closed; rejoin the call"),
                    Message::Pong(_)=>{},
                    _=>return Err("Unsupported voice websocket frame"),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opus2::Decoder;
    #[test]
    fn negotiation_timeout_distinguishes_missing_group_from_unexecuted_transition() {
        let server = crate::test_mls::Delivery::new();
        let mut alice = Dave::new(1, Some(2), 3).unwrap();
        let mut bob = Dave::new(2, Some(1), 3).unwrap();
        alice.session.set_external_sender(&server.external).unwrap();
        bob.session.set_external_sender(&server.external).unwrap();
        assert_eq!(
            negotiation_timeout(false, false, false, &alice, false),
            "Discord voice Hello timed out; rejoin the call"
        );
        assert_eq!(
            negotiation_timeout(true, false, false, &alice, false),
            "Discord voice Ready timed out; rejoin the call"
        );
        assert_eq!(
            negotiation_timeout(true, true, false, &alice, false),
            "Discord voice protocol selection timed out; no transport key was received"
        );
        assert_eq!(
            negotiation_timeout(true, true, true, &alice, false),
            "Discord DAVE group negotiation timed out; no accepted commit or welcome was received"
        );
        let (_, welcome) = server.add(&mut bob, &alice.key_package().unwrap());
        alice
            .group_changed(30, &[&[0, 7], welcome.as_slice()].concat())
            .unwrap();
        assert!(!alice.ready);
        assert_eq!(
            negotiation_timeout(true, true, true, &alice, false),
            "Discord DAVE transition execution timed out; no audio was enabled"
        );
        assert_eq!(
            negotiation_timeout(false, true, true, &alice, true),
            "Discord voice resume acknowledgement timed out; rejoin the call"
        );
    }
    #[test]
    fn validated_endpoints_discovery_and_real_opus() {
        assert!(endpoint("voice-1.discord.media:443").is_ok());
        for bad in [
            "127.0.0.1",
            "voice.discord.media.evil.test",
            "voice.discord.media/path",
            "token@voice.discord.media",
            "voice.discord.media?token=x",
        ] {
            assert!(endpoint(bad).is_err());
        }
        assert!(!public_ip("127.0.0.1".parse().unwrap()));
        assert!(!public_ip("192.168.1.1".parse().unwrap()));
        let mut reply = [0; 74];
        reply[..4].copy_from_slice(&[0, 2, 0, 70]);
        reply[4..8].copy_from_slice(&42u32.to_be_bytes());
        reply[8..17].copy_from_slice(b"127.0.0.1");
        reply[72..].copy_from_slice(&1234u16.to_be_bytes());
        assert_eq!(discovery(&reply, 42).unwrap().1, 1234);
        assert!(discovery(&reply, 43).is_err());
        let mut encoder = Encoder::new(48_000, Channels::Stereo, Application::Voip).unwrap();
        let mut decoder = Decoder::new(48_000, Channels::Mono).unwrap();
        let input = std::array::from_fn::<_, 1920, _>(|i| ((i / 2) as f32 * 0.06).sin() * 0.2);
        let mut encoded = [0; 1275];
        let length = encoder.encode_float(&input, &mut encoded).unwrap();
        let mut output = [0.0; 960];
        assert_eq!(
            decoder
                .decode_float(&encoded[..length], &mut output, false)
                .unwrap(),
            960
        );
        assert!(output.iter().any(|sample| sample.abs() > 0.01));
    }
    #[tokio::test]
    async fn local_voice_websocket_udp_dave_and_opus_exchange() {
        local_voice_exchange(false).await;
    }
    #[tokio::test]
    async fn local_guild_voice_waiting_mixed_audio_and_resume() {
        local_voice_exchange(true).await;
    }
    async fn local_voice_exchange(guild: bool) {
        use client_core::voice::Secret;
        use model::Id;
        use tokio::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = udp.local_addr().unwrap().port();
        let (done_tx, done_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let identify = ws.next().await.unwrap().unwrap().into_text().unwrap();
            let identify: Value = serde_json::from_str(&identify).unwrap();
            assert_eq!(identify["op"], 0);
            assert_eq!(identify["d"]["server_id"], if guild { "30" } else { "3" });
            assert_eq!(identify["d"]["max_dave_protocol_version"], 1);
            ws.send(Message::Text(
                json!({"op":8,"d":{"heartbeat_interval":5000}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            ws.send(Message::Text(
                json!({"op":2,"d":{"ssrc":42,"ip":"127.0.0.1","port":port,"modes":[MODE]}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            let mut probe = [0; 4096];
            let (n, client) = udp.recv_from(&mut probe).await.unwrap();
            assert_eq!(n, 74);
            probe[..4].copy_from_slice(&[0, 2, 0, 70]);
            probe[8..17].copy_from_slice(b"127.0.0.1");
            probe[72..74].copy_from_slice(&client.port().to_be_bytes());
            udp.send_to(&probe[..74], client).await.unwrap();
            let delivery = crate::test_mls::Delivery::new();
            let mut bob = Dave::new(2, (!guild).then_some(1), 3).unwrap();
            bob.session.set_external_sender(&delivery.external).unwrap();
            let mut charlie = Dave::new(4, None, 3).unwrap();
            if guild {
                bob.connect(&[1, 2, 4]).unwrap();
                charlie
                    .session
                    .set_external_sender(&delivery.external)
                    .unwrap();
                charlie.connect(&[1, 2, 4]).unwrap();
                let (commit, welcome) = delivery.add(&mut bob, &charlie.key_package().unwrap());
                bob.group_changed(29, &[&[0, 0], commit.as_slice()].concat())
                    .unwrap();
                charlie
                    .group_changed(30, &[&[0, 0], welcome.as_slice()].concat())
                    .unwrap();
            }
            loop {
                let message = ws.next().await.unwrap().unwrap();
                let event: Value = serde_json::from_str(message.to_text().unwrap()).unwrap();
                if event["op"] == 3 {
                    ws.send(Message::Text(
                        json!({"op":6,"d":{"t":event["d"]["t"]}}).to_string().into(),
                    ))
                    .await
                    .unwrap();
                    continue;
                }
                assert_eq!(event["op"], 1);
                assert_eq!(event["d"]["data"]["mode"], MODE);
                break;
            }
            let mut external = vec![0, 1, 25];
            external.extend(&delivery.external);
            ws.send(Message::Binary(external.into())).await.unwrap();
            ws.send(Message::Text(
                json!({"op":4,"d":{"mode":MODE,"secret_key":vec![7;32],"dave_protocol_version":1}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            let package = loop {
                match ws.next().await.unwrap().unwrap() {
                    Message::Binary(bytes) => break bytes,
                    Message::Text(text) => {
                        let value: Value = serde_json::from_str(&text).unwrap();
                        assert_eq!(value["op"], 3);
                        ws.send(Message::Text(
                            json!({"op":6,"d":{"t":value["d"]["t"]}}).to_string().into(),
                        ))
                        .await
                        .unwrap();
                    }
                    _ => panic!("unexpected test client frame"),
                }
            };
            assert_eq!(package[0], 26);
            if guild {
                ws.send(Message::Text(
                    json!({"op":21,"d":{"protocol_version":1,"transition_id":0}})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
                // Even queued synthetic capture cannot leave while the empty room lacks media keys.
                assert!(
                    timeout(Duration::from_millis(80), udp.recv_from(&mut probe))
                        .await
                        .is_err()
                );
                ws.send(Message::Text(
                    json!({"op":11,"d":{"user_ids":["1","2","4"]}})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
                let proposal = delivery.add_proposal(&bob, &package);
                charlie.proposals(&proposal).unwrap();
            }
            let (commit, welcome) = delivery.add(&mut bob, &package);
            let mut committed = vec![0, 0];
            committed.extend(commit);
            bob.group_changed(29, &committed).unwrap();
            if guild {
                charlie.group_changed(29, &committed).unwrap();
            }
            assert!(bob.ready);
            let mut welcome_frame = vec![0, 2, 30, 0, 0];
            welcome_frame.extend(welcome);
            ws.send(Message::Binary(welcome_frame.into()))
                .await
                .unwrap();
            ws.send(Message::Text(
                json!({"op":5,"seq":3,"d":{"user_id":"2","ssrc":43,"speaking":1}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            if guild {
                ws.send(Message::Text(
                    json!({"op":5,"seq":4,"d":{"user_id":"4","ssrc":44,"speaking":1}})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            }
            let (length, _) = udp.recv_from(&mut probe).await.unwrap();
            let mut transport = Encryption::new(&[7; 32]);
            let (ssrc, _, ciphertext) = transport.open(&probe[..length]).unwrap();
            assert_eq!(ssrc, 42);
            let encoded = bob
                .session
                .decrypt(1, davey::MediaType::AUDIO, &ciphertext)
                .unwrap();
            let mut decoder = Decoder::new(48_000, Channels::Mono).unwrap();
            let mut out = [0.0; 960];
            assert_eq!(
                decoder.decode_float(&encoded, &mut out, false).unwrap(),
                960
            );
            assert!(out.iter().any(|s| s.abs() > 0.01));
            let mut encoder = Encoder::new(48_000, Channels::Mono, Application::Voip).unwrap();
            let mut encoded = [0; 1275];
            let length = encoder.encode_float(&out, &mut encoded).unwrap();
            let encrypted = bob.session.encrypt_opus(&encoded[..length]).unwrap();
            let header = [0x80, 120, 0, 1, 0, 0, 0, 1, 0, 0, 0, 43];
            let packet = transport.seal(&header, &encrypted).unwrap();
            udp.send_to(&packet, client).await.unwrap();
            if guild {
                let encrypted = charlie.session.encrypt_opus(&encoded[..length]).unwrap();
                let mut header = header;
                header[11] = 44;
                let packet = transport.seal(&header, &encrypted).unwrap();
                udp.send_to(&packet, client).await.unwrap();
            }
            tokio::time::sleep(Duration::from_millis(120)).await;
            if guild {
                ws.send(Message::Close(Some(
                    tokio_tungstenite::tungstenite::protocol::CloseFrame {
                        code: 4015.into(),
                        reason: "synthetic server restart".into(),
                    },
                )))
                .await
                .unwrap();
            }
            drop(ws);
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let resume: Value =
                serde_json::from_str(ws.next().await.unwrap().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(resume["op"], 7);
            assert_eq!(resume["d"]["seq_ack"], if guild { 4 } else { 3 });
            assert_eq!(resume["d"]["server_id"], if guild { "30" } else { "3" });
            ws.send(Message::Text(
                json!({"op":8,"d":{"heartbeat_interval":5000}})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            ws.send(Message::Text(json!({"op":9,"d":null}).to_string().into()))
                .await
                .unwrap();
            // A new encrypted frame after resume reuses the live MLS group, never its old nonce.
            let encrypted = bob.session.encrypt_opus(&encoded[..length]).unwrap();
            let mut header = header;
            header[3] = 2;
            let packet = transport.seal(&header, &encrypted).unwrap();
            udp.send_to(&packet, client).await.unwrap();
            done_rx.await.unwrap();
            if guild {
                ws.send(Message::Close(Some(
                    tokio_tungstenite::tungstenite::protocol::CloseFrame {
                        code: 4014.into(),
                        reason: "synthetic terminal disconnect".into(),
                    },
                )))
                .await
                .unwrap();
                // A terminal disconnect must never attempt another connection.
                assert!(
                    timeout(Duration::from_millis(100), listener.accept())
                        .await
                        .is_err()
                );
            }
        });
        let credentials = VoiceConnection {
            channel: Id(3),
            user: Id(1),
            peer: (!guild).then_some(Id(2)),
            guild: guild.then_some(Id(30)),
            session: Secret::new("synthetic-session".into()).unwrap(),
            token: Secret::new("synthetic-token".into()).unwrap(),
            endpoint: "not-used-in-test".into(),
            request: 1,
        };
        let (capture_tx, capture) = std::sync::mpsc::sync_channel(8);
        capture_tx.try_send([0.25; 960]).unwrap();
        let (playback, playback_rx) = std::sync::mpsc::sync_channel(8);
        let (control_tx, control_rx) = watch::channel(Controls::default());
        let (status_tx, mut status_rx) = tokio::sync::mpsc::channel(8);
        let task = tokio::spawn(run_inner(
            credentials,
            capture,
            playback,
            control_rx,
            move |status| {
                if matches!(status, Status::Ready { .. }) {
                    capture_tx
                        .try_send(std::array::from_fn(|i| (i as f32 * 0.06).sin() * 0.3))
                        .unwrap();
                }
                status_tx.try_send(status).map_err(|_| ())
            },
            format!("ws://{address}"),
            true,
        ));
        timeout(Duration::from_secs(10), async {
            assert!(matches!(
                status_rx.recv().await.unwrap(),
                Status::Connecting
            ));
            assert!(matches!(
                status_rx.recv().await.unwrap(),
                Status::Discovering
            ));
            assert!(matches!(
                status_rx.recv().await.unwrap(),
                Status::TransportReady
            ));
            assert!(matches!(status_rx.recv().await.unwrap(), Status::Securing));
            let mut ready = 0;
            let mut heard = false;
            let mut waiting = false;
            loop {
                match status_rx.recv().await.unwrap() {
                    Status::Ready { .. } => ready += 1,
                    Status::RemoteAudio => heard = true,
                    Status::WaitingForPeer => waiting = true,
                    Status::Connecting
                    | Status::Discovering
                    | Status::Securing
                    | Status::TransportReady => {}
                }
                if ready == 2 && heard {
                    assert_eq!(waiting, guild);
                    break;
                }
            }
        })
        .await
        .unwrap();
        let pcm = playback_rx.try_recv().unwrap();
        assert!(pcm.iter().any(|s| s.abs() > 0.01));
        if guild {
            done_tx.send(()).unwrap();
            assert_eq!(
                timeout(Duration::from_secs(2), task)
                    .await
                    .unwrap()
                    .unwrap(),
                Err("Discord voice connection closed; rejoin the call")
            );
            drop(control_tx);
        } else {
            drop(control_tx);
            assert!(task.await.unwrap().is_ok());
            done_tx.send(()).unwrap();
        }
        server.await.unwrap();
    }
}
