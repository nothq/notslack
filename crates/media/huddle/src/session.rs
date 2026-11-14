//! One huddle connection, in the order the Amazon Chime SDK runs it: open
//! signaling, JOIN (TURN credentials come back in JOIN_ACK, then INDEX),
//! build the peer connection with the microphone, SUBSCRIBE with the SDP
//! offer and the audio host, apply the answer from SUBSCRIBE_ACK, then keep
//! the connection alive until someone leaves.

use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bytes::Bytes;
use tokio::{sync::mpsc, task::JoinHandle, time::timeout};
use webrtc::{
    api::{
        interceptor_registry::register_default_interceptors,
        media_engine::{MediaEngine, MIME_TYPE_OPUS},
        APIBuilder,
    },
    ice_transport::ice_server::RTCIceServer,
    interceptor::registry::Registry,
    media::Sample,
    peer_connection::{
        configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState,
        policy::ice_transport_policy::RTCIceTransportPolicy,
        sdp::session_description::RTCSessionDescription, RTCPeerConnection,
    },
    rtp_transceiver::rtp_codec::{RTCRtpCodecCapability, RTPCodecType},
    track::track_local::track_local_static_sample::TrackLocalStaticSample,
};

use crate::{
    audio::{open_audio, AudioDevices, AudioIo, CALL_SAMPLE_RATE},
    protocol::{
        AudioControlFrame, ClientDetails, FrameType, JoinFrame, LeaveFrame, PingPongFrame,
        PingPongType, SignalFrame, StreamDescriptor, StreamMediaType, StreamServiceType,
        SubscribeFrame, TurnCredentials, JOIN_FLAG_HAS_STREAM_UPDATE,
    },
    signaling::{connect, SignalingReceiver, SignalingSender},
    HuddleCommand, HuddleEvent, HuddleMediaConfig,
};

/// 20 ms of 48 kHz mono audio: one Opus frame.
const FRAME_SAMPLES: usize = (CALL_SAMPLE_RATE / 50) as usize;
const FRAME_DURATION: Duration = Duration::from_millis(20);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(20);
const LEAVE_TIMEOUT: Duration = Duration::from_secs(2);
/// The SDK pings every 10 seconds to keep signaling alive.
const PING_INTERVAL: Duration = Duration::from_secs(10);
/// What the SDK reports itself as; the server may adjust to it.
const CHIME_SDK_NAME: &str = "amazon-chime-sdk-js";
const CHIME_SDK_VERSION: &str = "3.33.0";

/// Where the call's audio comes from: the real devices, or a test's queues.
pub(crate) type AudioOpener = fn() -> Result<AudioIo, String>;

pub(crate) async fn run(
    config: HuddleMediaConfig,
    commands: mpsc::UnboundedReceiver<HuddleCommand>,
    events: async_channel::Sender<HuddleEvent>,
) {
    run_with_audio(config, commands, events, open_audio).await;
}

pub(crate) async fn run_with_audio(
    config: HuddleMediaConfig,
    mut commands: mpsc::UnboundedReceiver<HuddleCommand>,
    events: async_channel::Sender<HuddleEvent>,
    open_audio: AudioOpener,
) {
    let outcome = connect_and_run(&config, &mut commands, &events, open_audio).await;
    let _ = events
        .send(match outcome {
            Ok(()) => HuddleEvent::Ended,
            Err(error) => HuddleEvent::Failed(error),
        })
        .await;
}

async fn connect_and_run(
    config: &HuddleMediaConfig,
    commands: &mut mpsc::UnboundedReceiver<HuddleCommand>,
    events: &async_channel::Sender<HuddleEvent>,
    open_audio: AudioOpener,
) -> Result<(), String> {
    let (mut sender, mut receiver) = connect(&config.signaling_url, &config.join_token).await?;
    sender.send(&join_frame()).await?;
    let turn = timeout(HANDSHAKE_TIMEOUT, receive_join(&mut sender, &mut receiver))
        .await
        .map_err(|_| "the huddle server did not answer the join".to_string())??;
    let mut call = Call::open(turn, config.muted, open_audio).await?;
    let offer = call.offer().await?;
    sender
        .send(&subscribe_frame(config, offer, call.muted()))
        .await?;
    let answer = timeout(
        HANDSHAKE_TIMEOUT,
        receive_answer(&mut sender, &mut receiver),
    )
    .await
    .map_err(|_| "the huddle server did not answer the audio offer".to_string())??;
    call.accept(answer).await?;
    let _ = events.send(HuddleEvent::Connected).await;
    let _ = events.send(HuddleEvent::Muted(call.muted())).await;
    let outcome = control_loop(&mut call, &mut sender, &mut receiver, commands, events).await;
    call.close().await;
    sender.close().await;
    outcome
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

fn join_frame() -> SignalFrame {
    let mut session_id = [0; 8];
    let _ = getrandom::fill(&mut session_id);
    let mut frame = SignalFrame::new(FrameType::Join, now_ms());
    frame.join = Some(JoinFrame {
        protocol_version: Some(2),
        flags: Some(JOIN_FLAG_HAS_STREAM_UPDATE),
        client_details: Some(ClientDetails {
            app_name: Some("notslack".into()),
            app_version: Some(env!("CARGO_PKG_VERSION").into()),
            platform_name: Some("notslack".into()),
            platform_version: Some(env!("CARGO_PKG_VERSION").into()),
            client_source: Some(CHIME_SDK_NAME.into()),
            chime_sdk_version: Some(CHIME_SDK_VERSION.into()),
            os_name: Some(std::env::consts::OS.into()),
        }),
        audio_session_id: Some(u64::from_le_bytes(session_id)),
        wants_compressed_sdp: Some(false),
    });
    frame
}

fn subscribe_frame(config: &HuddleMediaConfig, sdp_offer: String, muted: bool) -> SignalFrame {
    let mut frame = SignalFrame::new(FrameType::Subscribe, now_ms());
    frame.sub = Some(SubscribeFrame {
        duplex: Some(StreamServiceType::Rx as i32),
        send_streams: vec![StreamDescriptor {
            stream_id: Some(1),
            framerate: Some(15),
            max_bitrate_kbps: Some(600),
            track_label: Some("AmazonChimeExpressAudio".into()),
            group_id: Some(1),
            avg_bitrate_bps: Some(400_000),
            attendee_id: Some(config.attendee_id.clone()),
            media_type: Some(StreamMediaType::Audio as i32),
        }],
        sdp_offer: Some(unified_plan_sdp(&sdp_offer)),
        audio_host: Some(config.audio_host_url.clone()),
        audio_checkin: Some(false),
        audio_muted: Some(muted),
    });
    frame
}

/// The SDK marks its offers this way so the server reads them as
/// unified-plan SDP.
fn unified_plan_sdp(sdp: &str) -> String {
    if sdp.contains("mozilla") {
        sdp.to_string()
    } else {
        sdp.replacen("o=-", "o=mozilla-chrome", 1)
    }
}

fn frame_error(frame: &SignalFrame) -> Option<String> {
    let error = frame.error.as_ref()?;
    Some(format!(
        "the huddle server refused the connection ({}{})",
        error.status.unwrap_or_default(),
        error
            .description
            .as_deref()
            .map(|description| format!(": {description}"))
            .unwrap_or_default()
    ))
}

/// Answers a server ping; `true` when `frame` was one.
async fn answer_ping(sender: &mut SignalingSender, frame: &SignalFrame) -> Result<bool, String> {
    let Some(ping) = frame
        .ping_pong
        .as_ref()
        .filter(|ping| ping.r#type == PingPongType::Ping as i32)
    else {
        return Ok(false);
    };
    let mut pong = SignalFrame::new(FrameType::PingPong, now_ms());
    pong.ping_pong = Some(PingPongFrame {
        r#type: PingPongType::Pong as i32,
        ping_id: ping.ping_id,
    });
    sender.send(&pong).await?;
    Ok(true)
}

/// Waits for JOIN_ACK and the first INDEX, returning the TURN credentials.
async fn receive_join(
    sender: &mut SignalingSender,
    receiver: &mut SignalingReceiver,
) -> Result<Option<TurnCredentials>, String> {
    let mut turn = None;
    while let Some(frame) = receiver.next().await {
        let frame = frame?;
        if let Some(error) = frame_error(&frame) {
            return Err(error);
        }
        if answer_ping(sender, &frame).await? {
            continue;
        }
        match frame.frame_type() {
            Some(FrameType::JoinAck) => {
                turn = frame.joinack.and_then(|ack| ack.turn_credentials);
            }
            Some(FrameType::Index) => {
                if frame.index.and_then(|index| index.at_capacity) == Some(true) {
                    return Err("this huddle is full".to_string());
                }
                return Ok(turn);
            }
            _ => {}
        }
    }
    Err("the huddle server closed the connection while joining".to_string())
}

async fn receive_answer(
    sender: &mut SignalingSender,
    receiver: &mut SignalingReceiver,
) -> Result<String, String> {
    while let Some(frame) = receiver.next().await {
        let frame = frame?;
        if let Some(error) = frame_error(&frame) {
            return Err(error);
        }
        if answer_ping(sender, &frame).await? {
            continue;
        }
        if frame.frame_type() == Some(FrameType::SubscribeAck) {
            return frame
                .suback
                .and_then(|ack| ack.sdp_answer)
                .ok_or_else(|| "the huddle server sent no audio answer".to_string());
        }
    }
    Err("the huddle server closed the connection while connecting audio".to_string())
}

async fn control_loop(
    call: &mut Call,
    sender: &mut SignalingSender,
    receiver: &mut SignalingReceiver,
    commands: &mut mpsc::UnboundedReceiver<HuddleCommand>,
    events: &async_channel::Sender<HuddleEvent>,
) -> Result<(), String> {
    let mut ping = tokio::time::interval(PING_INTERVAL);
    let mut ping_id = 0_u32;
    let mut participants = BTreeMap::<u32, String>::new();
    loop {
        tokio::select! {
            command = commands.recv() => match command {
                Some(HuddleCommand::SetMuted(muted)) => {
                    let muted = call.set_muted(muted);
                    let mut frame = SignalFrame::new(FrameType::AudioControl, now_ms());
                    frame.audio_control = Some(AudioControlFrame { muted: Some(muted) });
                    sender.send(&frame).await?;
                    let _ = events.send(HuddleEvent::Muted(muted)).await;
                }
                Some(HuddleCommand::Leave) | None => {
                    leave(sender, receiver).await;
                    return Ok(());
                }
            },
            frame = receiver.next() => {
                // The server closing the connection ends the huddle.
                let Some(frame) = frame else { return Ok(()) };
                let frame = frame?;
                if let Some(error) = frame_error(&frame) {
                    return Err(error);
                }
                if answer_ping(sender, &frame).await? {
                    continue;
                }
                if update_participants(&mut participants, &frame) {
                    let mut people = participants.values().cloned().collect::<Vec<_>>();
                    people.sort();
                    people.dedup();
                    let _ = events.send(HuddleEvent::Participants(people)).await;
                }
            },
            _ = ping.tick() => {
                ping_id = ping_id.wrapping_add(1);
                let mut frame = SignalFrame::new(FrameType::PingPong, now_ms());
                frame.ping_pong = Some(PingPongFrame {
                    r#type: PingPongType::Ping as i32,
                    ping_id,
                });
                sender.send(&frame).await?;
            },
            failure = call.failures.recv() => {
                return Err(failure.unwrap_or_else(|| "the huddle audio stopped".to_string()));
            },
        }
    }
}

/// Applies AUDIO_STREAM_ID_INFO; `true` when the list of people changed.
fn update_participants(participants: &mut BTreeMap<u32, String>, frame: &SignalFrame) -> bool {
    let Some(info) = frame.audio_stream_id_info.as_ref() else {
        return false;
    };
    let before = participants.clone();
    for stream in &info.streams {
        let Some(stream_id) = stream.audio_stream_id else {
            continue;
        };
        if stream.dropped == Some(true) {
            participants.remove(&stream_id);
            continue;
        }
        let person = stream
            .external_user_id
            .clone()
            .or_else(|| stream.attendee_id.clone());
        if let Some(person) = person {
            participants.insert(stream_id, person);
        }
    }
    *participants != before
}

/// Says goodbye the way the SDK does, without waiting long for the reply.
async fn leave(sender: &mut SignalingSender, receiver: &mut SignalingReceiver) {
    let mut frame = SignalFrame::new(FrameType::Leave, now_ms());
    frame.leave = Some(LeaveFrame {});
    if sender.send(&frame).await.is_err() {
        return;
    }
    let _ = timeout(LEAVE_TIMEOUT, async {
        while let Some(Ok(frame)) = receiver.next().await {
            if frame.frame_type() == Some(FrameType::LeaveAck) {
                return;
            }
        }
    })
    .await;
}

/// The WebRTC side: the peer connection, the microphone track and the
/// speaker.
struct Call {
    peer: Arc<RTCPeerConnection>,
    muted: Arc<AtomicBool>,
    has_microphone: bool,
    failures: mpsc::UnboundedReceiver<String>,
    encoder: Option<JoinHandle<()>>,
    _devices: AudioDevices,
}

impl Call {
    async fn open(
        turn: Option<TurnCredentials>,
        muted: bool,
        open_audio: AudioOpener,
    ) -> Result<Self, String> {
        let audio = open_audio()?;
        let peer = Arc::new(new_peer_connection(turn).await?);
        let track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_OPUS.to_owned(),
                clock_rate: CALL_SAMPLE_RATE,
                channels: 2,
                sdp_fmtp_line: "minptime=10;useinbandfec=1".to_owned(),
                rtcp_feedback: vec![],
            },
            "audio".to_owned(),
            "notslack".to_owned(),
        ));
        let rtp_sender = peer
            .add_track(track.clone())
            .await
            .map_err(|error| format!("could not add the microphone to the huddle: {error}"))?;
        // RTCP has to be read for the sender's interceptors to run.
        tokio::spawn(async move { while rtp_sender.read_rtcp().await.is_ok() {} });

        let has_microphone = audio.microphone.is_some();
        let muted = Arc::new(AtomicBool::new(muted || !has_microphone));
        let encoder = audio
            .microphone
            .map(|microphone| tokio::spawn(encode_microphone(microphone, track, muted.clone())));
        play_remote_audio(&peer, audio.speaker);
        let (failure_sender, failures) = mpsc::unbounded_channel();
        peer.on_peer_connection_state_change(Box::new(move |state| {
            if matches!(
                state,
                RTCPeerConnectionState::Failed | RTCPeerConnectionState::Disconnected
            ) {
                let _ = failure_sender.send(format!("the huddle audio connection was {state}"));
            }
            Box::pin(async {})
        }));
        Ok(Self {
            peer,
            muted,
            has_microphone,
            failures,
            encoder,
            _devices: audio.devices,
        })
    }

    /// Creates the offer and waits for ICE gathering, so the SDP carries the
    /// relay candidates, as the SDK's offer does.
    async fn offer(&self) -> Result<String, String> {
        let offer = self
            .peer
            .create_offer(None)
            .await
            .map_err(|error| format!("could not create the huddle audio offer: {error}"))?;
        let mut gathered = self.peer.gathering_complete_promise().await;
        self.peer
            .set_local_description(offer)
            .await
            .map_err(|error| format!("could not apply the huddle audio offer: {error}"))?;
        let _ = timeout(HANDSHAKE_TIMEOUT, gathered.recv()).await;
        self.peer
            .local_description()
            .await
            .map(|description| description.sdp)
            .ok_or_else(|| "the huddle audio offer went missing".to_string())
    }

    async fn accept(&self, answer: String) -> Result<(), String> {
        let answer = RTCSessionDescription::answer(answer)
            .map_err(|error| format!("the huddle audio answer is invalid: {error}"))?;
        self.peer
            .set_remote_description(answer)
            .await
            .map_err(|error| format!("could not apply the huddle audio answer: {error}"))
    }

    fn muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// Mutes or unmutes; without a microphone the huddle stays muted.
    fn set_muted(&self, muted: bool) -> bool {
        let muted = muted || !self.has_microphone;
        self.muted.store(muted, Ordering::Relaxed);
        muted
    }

    async fn close(&mut self) {
        if let Some(encoder) = self.encoder.take() {
            encoder.abort();
        }
        let _ = self.peer.close().await;
    }
}

async fn new_peer_connection(turn: Option<TurnCredentials>) -> Result<RTCPeerConnection, String> {
    let mut media = MediaEngine::default();
    media
        .register_default_codecs()
        .map_err(|error| format!("could not set up huddle codecs: {error}"))?;
    let registry = register_default_interceptors(Registry::new(), &mut media)
        .map_err(|error| format!("could not set up huddle audio: {error}"))?;
    let api = APIBuilder::new()
        .with_media_engine(media)
        .with_interceptor_registry(registry)
        .build();
    // Like the SDK: relay only, through the meeting's TURN servers.
    let configuration = match turn.filter(|turn| !turn.uris.is_empty()) {
        Some(turn) => RTCConfiguration {
            ice_servers: vec![RTCIceServer {
                urls: turn.uris,
                username: turn.username.unwrap_or_default(),
                credential: turn.password.unwrap_or_default(),
            }],
            ice_transport_policy: RTCIceTransportPolicy::Relay,
            ..RTCConfiguration::default()
        },
        None => RTCConfiguration::default(),
    };
    api.new_peer_connection(configuration)
        .await
        .map_err(|error| format!("could not start the huddle audio connection: {error}"))
}

/// Encodes the microphone into Opus frames; silence while muted, so the
/// stream keeps its timing.
async fn encode_microphone(
    mut microphone: rtrb::Consumer<f32>,
    track: Arc<TrackLocalStaticSample>,
    muted: Arc<AtomicBool>,
) {
    let Ok(mut encoder) = opus::Encoder::new(
        CALL_SAMPLE_RATE,
        opus::Channels::Mono,
        opus::Application::Voip,
    ) else {
        return;
    };
    let mut frame = vec![0.0_f32; FRAME_SAMPLES];
    let mut packet = vec![0_u8; 4000];
    let mut tick = tokio::time::interval(FRAME_DURATION);
    loop {
        tick.tick().await;
        while microphone.slots() >= FRAME_SAMPLES {
            for sample in frame.iter_mut() {
                *sample = microphone.pop().unwrap_or(0.0);
            }
            if muted.load(Ordering::Relaxed) {
                frame.fill(0.0);
            }
            let Ok(length) = encoder.encode_float(&frame, &mut packet) else {
                continue;
            };
            let sample = Sample {
                data: Bytes::copy_from_slice(&packet[..length]),
                duration: FRAME_DURATION,
                ..Sample::default()
            };
            if track.write_sample(&sample).await.is_err() {
                return;
            }
        }
    }
}

/// Decodes the meeting's audio (Chime mixes everyone into one stream) to
/// the speaker.
fn play_remote_audio(peer: &RTCPeerConnection, speaker: rtrb::Producer<f32>) {
    let speaker = Arc::new(Mutex::new(Some(speaker)));
    peer.on_track(Box::new(move |track, _, _| {
        let speaker = speaker.clone();
        Box::pin(async move {
            if track.kind() != RTPCodecType::Audio {
                return;
            }
            let Some(mut speaker) = speaker.lock().ok().and_then(|mut speaker| speaker.take())
            else {
                return;
            };
            let Ok(mut decoder) = opus::Decoder::new(CALL_SAMPLE_RATE, opus::Channels::Mono) else {
                return;
            };
            // Up to 120 ms per packet, the most Opus allows.
            let mut pcm = vec![0.0_f32; FRAME_SAMPLES * 6];
            tokio::spawn(async move {
                while let Ok((packet, _)) = track.read_rtp().await {
                    if packet.payload.is_empty() {
                        continue;
                    }
                    let Ok(samples) = decoder.decode_float(&packet.payload, &mut pcm, false) else {
                        continue;
                    };
                    for sample in &pcm[..samples] {
                        let _ = speaker.push(*sample);
                    }
                }
            });
        })
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AudioStreamIdInfo, AudioStreamIdInfoFrame};

    #[test]
    fn offers_are_marked_as_unified_plan_like_the_sdk_does() {
        let sdp = "v=0\r\no=- 123 2 IN IP4 0.0.0.0\r\n";
        assert_eq!(
            unified_plan_sdp(sdp),
            "v=0\r\no=mozilla-chrome 123 2 IN IP4 0.0.0.0\r\n"
        );
        assert_eq!(unified_plan_sdp("o=mozilla 1"), "o=mozilla 1");
    }

    #[test]
    fn the_subscribe_frame_sends_one_audio_stream_to_the_audio_host() {
        let config = HuddleMediaConfig {
            signaling_url: "wss://signal.example/control/m".into(),
            audio_host_url: "audio.example:3478".into(),
            attendee_id: "attendee".into(),
            join_token: "token".into(),
            muted: true,
        };
        let frame = subscribe_frame(&config, "o=- 1".into(), true);
        let sub = frame.sub.unwrap();
        assert_eq!(sub.audio_host.as_deref(), Some("audio.example:3478"));
        assert_eq!(sub.audio_muted, Some(true));
        assert_eq!(sub.audio_checkin, Some(false));
        assert_eq!(sub.send_streams.len(), 1);
        assert_eq!(sub.send_streams[0].attendee_id.as_deref(), Some("attendee"));
        assert_eq!(sub.sdp_offer.as_deref(), Some("o=mozilla-chrome 1"));
    }

    #[test]
    fn audio_stream_info_tracks_who_is_in_the_huddle() {
        let mut participants = BTreeMap::new();
        let mut frame = SignalFrame::new(FrameType::AudioStreamIdInfo, 1);
        frame.audio_stream_id_info = Some(AudioStreamIdInfoFrame {
            streams: vec![
                AudioStreamIdInfo {
                    audio_stream_id: Some(1),
                    external_user_id: Some("U1".into()),
                    ..AudioStreamIdInfo::default()
                },
                AudioStreamIdInfo {
                    audio_stream_id: Some(2),
                    attendee_id: Some("a2".into()),
                    ..AudioStreamIdInfo::default()
                },
            ],
        });
        assert!(update_participants(&mut participants, &frame));
        assert_eq!(participants.len(), 2);
        frame.audio_stream_id_info = Some(AudioStreamIdInfoFrame {
            streams: vec![AudioStreamIdInfo {
                audio_stream_id: Some(1),
                dropped: Some(true),
                ..AudioStreamIdInfo::default()
            }],
        });
        assert!(update_participants(&mut participants, &frame));
        assert_eq!(participants.values().collect::<Vec<_>>(), ["a2"]);
        assert!(!update_participants(&mut participants, &frame));
    }
}

#[cfg(test)]
mod loopback;
