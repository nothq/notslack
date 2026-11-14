//! A whole huddle against a stand-in for the Chime media server: a local
//! WebSocket that speaks the signaling protocol, and a WebRTC peer that
//! answers the offer, counts the audio it gets and sends a tone back.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    accept_hdr_async,
    tungstenite::{
        handshake::server::{ErrorResponse, Request, Response},
        Message,
    },
    WebSocketStream,
};

use super::*;
use crate::protocol::{
    AudioStreamIdInfo, AudioStreamIdInfoFrame, IndexFrame, JoinAckFrame, SubscribeAckFrame,
};

static TEST_AUDIO: Mutex<Option<AudioIo>> = Mutex::new(None);

fn test_audio() -> Result<AudioIo, String> {
    TEST_AUDIO
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| "the test audio was already taken".to_string())
}

fn tone(start: usize) -> Vec<f32> {
    (start..start + FRAME_SAMPLES)
        .map(|index| (index as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.3)
        .collect()
}

async fn next_frame(socket: &mut WebSocketStream<TcpStream>) -> SignalFrame {
    loop {
        match socket.next().await.unwrap().unwrap() {
            Message::Binary(bytes) => return SignalFrame::from_wire(&bytes).unwrap(),
            _ => continue,
        }
    }
}

async fn send_frame(socket: &mut WebSocketStream<TcpStream>, frame: SignalFrame) {
    socket.send(Message::Binary(frame.to_wire())).await.unwrap();
}

/// Plays the media server's part until the client leaves.
async fn stand_in_server(listener: TcpListener, received_packets: Arc<AtomicUsize>) {
    let mut socket = accept_and_join(listener).await;
    let offer = receive_offer(&mut socket).await;
    let (peer, track) = answering_peer(received_packets).await;
    answer_offer(&mut socket, &peer, offer).await;
    let sender = tokio::spawn(send_tone(track));
    loop {
        let frame = next_frame(&mut socket).await;
        if frame.frame_type() == Some(FrameType::Leave) {
            send_frame(&mut socket, SignalFrame::new(FrameType::LeaveAck, 5)).await;
            break;
        }
    }
    sender.abort();
    let _ = peer.close().await;
}

/// Accepts the WebSocket like Chime does, then answers JOIN.
async fn accept_and_join(listener: TcpListener) -> WebSocketStream<TcpStream> {
    let (stream, _) = listener.accept().await.unwrap();
    let mut socket = accept_hdr_async(stream, check_handshake).await.unwrap();
    let join = next_frame(&mut socket).await;
    assert_eq!(join.frame_type(), Some(FrameType::Join));
    assert_eq!(join.join.unwrap().protocol_version, Some(2));
    let mut ack = SignalFrame::new(FrameType::JoinAck, 1);
    ack.joinack = Some(JoinAckFrame::default());
    send_frame(&mut socket, ack).await;
    let mut index = SignalFrame::new(FrameType::Index, 2);
    index.index = Some(IndexFrame {
        at_capacity: Some(false),
        num_participants: Some(1),
    });
    send_frame(&mut socket, index).await;
    socket
}

#[allow(clippy::result_large_err)]
fn check_handshake(request: &Request, mut response: Response) -> Result<Response, ErrorResponse> {
    let protocols = request.headers()["Sec-WebSocket-Protocol"]
        .to_str()
        .unwrap();
    assert_eq!(protocols, "_aws_wt_session, join-token");
    assert!(request
        .uri()
        .query()
        .unwrap()
        .contains("X-Chime-Control-Protocol-Version=3"));
    response
        .headers_mut()
        .insert("Sec-WebSocket-Protocol", "_aws_wt_session".parse().unwrap());
    Ok(response)
}

async fn receive_offer(socket: &mut WebSocketStream<TcpStream>) -> String {
    let subscribe = loop {
        let frame = next_frame(socket).await;
        if frame.frame_type() == Some(FrameType::Subscribe) {
            break frame.sub.unwrap();
        }
    };
    assert_eq!(subscribe.audio_host.as_deref(), Some("audio.test:3478"));
    let offer = subscribe.sdp_offer.unwrap();
    assert!(offer.contains("o=mozilla-chrome"));
    assert!(offer.to_lowercase().contains("opus"));
    offer
}

async fn answer_offer(
    socket: &mut WebSocketStream<TcpStream>,
    peer: &RTCPeerConnection,
    offer: String,
) {
    peer.set_remote_description(RTCSessionDescription::offer(offer).unwrap())
        .await
        .unwrap();
    let answer = peer.create_answer(None).await.unwrap();
    let mut gathered = peer.gathering_complete_promise().await;
    peer.set_local_description(answer).await.unwrap();
    let _ = gathered.recv().await;
    let mut suback = SignalFrame::new(FrameType::SubscribeAck, 3);
    suback.suback = Some(SubscribeAckFrame {
        sdp_answer: Some(peer.local_description().await.unwrap().sdp),
    });
    send_frame(socket, suback).await;
    let mut info = SignalFrame::new(FrameType::AudioStreamIdInfo, 4);
    info.audio_stream_id_info = Some(AudioStreamIdInfoFrame {
        streams: vec![AudioStreamIdInfo {
            audio_stream_id: Some(1),
            external_user_id: Some("U1".into()),
            ..AudioStreamIdInfo::default()
        }],
    });
    send_frame(socket, info).await;
}

async fn answering_peer(
    received_packets: Arc<AtomicUsize>,
) -> (Arc<RTCPeerConnection>, Arc<TrackLocalStaticSample>) {
    let peer = Arc::new(new_peer_connection(None).await.unwrap());
    let track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_owned(),
            clock_rate: CALL_SAMPLE_RATE,
            channels: 2,
            sdp_fmtp_line: "minptime=10;useinbandfec=1".to_owned(),
            rtcp_feedback: vec![],
        },
        "audio".to_owned(),
        "server".to_owned(),
    ));
    let rtp_sender = peer.add_track(track.clone()).await.unwrap();
    tokio::spawn(async move { while rtp_sender.read_rtcp().await.is_ok() {} });
    peer.on_track(Box::new(move |remote, _, _| {
        let received_packets = received_packets.clone();
        Box::pin(async move {
            tokio::spawn(async move {
                while remote.read_rtp().await.is_ok() {
                    received_packets.fetch_add(1, Ordering::Relaxed);
                }
            });
        })
    }));
    (peer, track)
}

async fn send_tone(track: Arc<TrackLocalStaticSample>) {
    let mut encoder = opus::Encoder::new(
        CALL_SAMPLE_RATE,
        opus::Channels::Mono,
        opus::Application::Voip,
    )
    .unwrap();
    let mut packet = vec![0_u8; 4000];
    let mut position = 0;
    let mut tick = tokio::time::interval(FRAME_DURATION);
    loop {
        tick.tick().await;
        let length = encoder.encode_float(&tone(position), &mut packet).unwrap();
        position += FRAME_SAMPLES;
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

async fn next_event(events: &async_channel::Receiver<HuddleEvent>) -> HuddleEvent {
    timeout(Duration::from_secs(30), events.recv())
        .await
        .expect("the huddle reports progress")
        .expect("the huddle event channel stays open")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_huddle_connects_carries_audio_both_ways_and_leaves() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let received_packets = Arc::new(AtomicUsize::new(0));
    let server = tokio::spawn(stand_in_server(listener, received_packets.clone()));

    let (mut microphone, microphone_queue) = rtrb::RingBuffer::new(48_000 * 2);
    let (speaker_queue, speaker) = rtrb::RingBuffer::new(48_000 * 4);
    *TEST_AUDIO.lock().unwrap() = Some(AudioIo {
        microphone: Some(microphone_queue),
        speaker: speaker_queue,
        devices: AudioDevices::none(),
    });
    let talking = tokio::spawn(async move {
        let mut position = 0;
        let mut tick = tokio::time::interval(FRAME_DURATION);
        loop {
            tick.tick().await;
            for sample in tone(position) {
                let _ = microphone.push(sample);
            }
            position += FRAME_SAMPLES;
        }
    });

    let config = HuddleMediaConfig {
        signaling_url: format!("ws://127.0.0.1:{port}/control/meeting"),
        audio_host_url: "audio.test:3478".into(),
        attendee_id: "attendee".into(),
        join_token: "join-token".into(),
        muted: false,
    };
    let (event_sender, events) = async_channel::unbounded();
    let (commands, command_receiver) = mpsc::unbounded_channel();
    let session = tokio::spawn(run_with_audio(
        config,
        command_receiver,
        event_sender,
        test_audio,
    ));

    assert_eq!(next_event(&events).await, HuddleEvent::Connected);
    assert_eq!(next_event(&events).await, HuddleEvent::Muted(false));
    assert_eq!(
        next_event(&events).await,
        HuddleEvent::Participants(vec!["U1".into()])
    );

    // The microphone reaches the server, and the server's tone reaches the
    // speaker.
    let speaker = Arc::new(Mutex::new(speaker));
    let audio_flowed = timeout(Duration::from_secs(20), async {
        loop {
            let heard = speaker.lock().unwrap().slots();
            if received_packets.load(Ordering::Relaxed) >= 25 && heard >= FRAME_SAMPLES * 10 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await;
    assert!(
        audio_flowed.is_ok(),
        "audio did not flow: server got {} packets, speaker got {} samples",
        received_packets.load(Ordering::Relaxed),
        speaker.lock().unwrap().slots()
    );
    let heard = {
        let mut speaker = speaker.lock().unwrap();
        (0..FRAME_SAMPLES * 10)
            .filter_map(|_| speaker.pop().ok())
            .map(f32::abs)
            .fold(0.0_f32, f32::max)
    };
    assert!(heard > 0.05, "the speaker got silence (peak {heard})");

    commands.send(HuddleCommand::SetMuted(true)).unwrap();
    assert_eq!(next_event(&events).await, HuddleEvent::Muted(true));
    commands.send(HuddleCommand::Leave).unwrap();
    assert_eq!(next_event(&events).await, HuddleEvent::Ended);
    talking.abort();
    timeout(Duration::from_secs(10), session)
        .await
        .unwrap()
        .unwrap();
    timeout(Duration::from_secs(10), server)
        .await
        .unwrap()
        .unwrap();
}
