//! The subset of Amazon Chime's signaling protocol a huddle needs, from
//! `SignalingProtocol.proto` in the Amazon Chime SDK for JavaScript.
//!
//! Every frame on the signaling WebSocket is one `SdkSignalFrame`, encoded
//! with protobuf and prefixed with the frame type byte `0x05`.

use prost::Message;

/// The byte that starts every signaling message.
const FRAME_TYPE_RTC: u8 = 0x05;

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub(crate) enum FrameType {
    Join = 1,
    JoinAck = 2,
    Subscribe = 3,
    SubscribeAck = 4,
    Index = 5,
    Leave = 9,
    LeaveAck = 10,
    AudioControl = 16,
    AudioMetadata = 17,
    AudioStreamIdInfo = 18,
    PingPong = 19,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct SignalFrame {
    #[prost(uint64, required, tag = "1")]
    pub(crate) timestamp_ms: u64,
    #[prost(enumeration = "FrameType", required, tag = "2")]
    pub(crate) r#type: i32,
    #[prost(message, optional, tag = "3")]
    pub(crate) error: Option<ErrorFrame>,
    #[prost(message, optional, tag = "4")]
    pub(crate) join: Option<JoinFrame>,
    #[prost(message, optional, tag = "5")]
    pub(crate) joinack: Option<JoinAckFrame>,
    #[prost(message, optional, tag = "6")]
    pub(crate) sub: Option<SubscribeFrame>,
    #[prost(message, optional, tag = "7")]
    pub(crate) suback: Option<SubscribeAckFrame>,
    #[prost(message, optional, tag = "8")]
    pub(crate) index: Option<IndexFrame>,
    #[prost(message, optional, tag = "11")]
    pub(crate) leave: Option<LeaveFrame>,
    #[prost(message, optional, tag = "17")]
    pub(crate) audio_control: Option<AudioControlFrame>,
    #[prost(message, optional, tag = "19")]
    pub(crate) audio_stream_id_info: Option<AudioStreamIdInfoFrame>,
    #[prost(message, optional, tag = "20")]
    pub(crate) ping_pong: Option<PingPongFrame>,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct ErrorFrame {
    #[prost(uint32, optional, tag = "1")]
    pub(crate) status: Option<u32>,
    #[prost(string, optional, tag = "2")]
    pub(crate) description: Option<String>,
}

/// `SdkJoinFlags.HAS_STREAM_UPDATE`, which the SDK always sends.
pub(crate) const JOIN_FLAG_HAS_STREAM_UPDATE: u32 = 2;

#[derive(Clone, PartialEq, Message)]
pub(crate) struct JoinFrame {
    #[prost(uint32, optional, tag = "1")]
    pub(crate) protocol_version: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub(crate) flags: Option<u32>,
    #[prost(message, optional, tag = "4")]
    pub(crate) client_details: Option<ClientDetails>,
    #[prost(uint64, optional, tag = "6")]
    pub(crate) audio_session_id: Option<u64>,
    #[prost(bool, optional, tag = "7")]
    pub(crate) wants_compressed_sdp: Option<bool>,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientDetails {
    #[prost(string, optional, tag = "1")]
    pub(crate) app_name: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub(crate) app_version: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub(crate) platform_name: Option<String>,
    #[prost(string, optional, tag = "6")]
    pub(crate) platform_version: Option<String>,
    #[prost(string, optional, tag = "7")]
    pub(crate) client_source: Option<String>,
    #[prost(string, optional, tag = "8")]
    pub(crate) chime_sdk_version: Option<String>,
    #[prost(string, optional, tag = "10")]
    pub(crate) os_name: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct JoinAckFrame {
    #[prost(message, optional, tag = "1")]
    pub(crate) turn_credentials: Option<TurnCredentials>,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct TurnCredentials {
    #[prost(string, optional, tag = "1")]
    pub(crate) username: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub(crate) password: Option<String>,
    #[prost(uint32, optional, tag = "3")]
    pub(crate) ttl: Option<u32>,
    #[prost(string, repeated, tag = "4")]
    pub(crate) uris: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub(crate) enum StreamServiceType {
    Rx = 1,
    Tx = 2,
    Duplex = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub(crate) enum StreamMediaType {
    Audio = 1,
    Video = 2,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct SubscribeFrame {
    #[prost(enumeration = "StreamServiceType", optional, tag = "1")]
    pub(crate) duplex: Option<i32>,
    #[prost(message, repeated, tag = "2")]
    pub(crate) send_streams: Vec<StreamDescriptor>,
    #[prost(string, optional, tag = "4")]
    pub(crate) sdp_offer: Option<String>,
    #[prost(string, optional, tag = "5")]
    pub(crate) audio_host: Option<String>,
    #[prost(bool, optional, tag = "6")]
    pub(crate) audio_checkin: Option<bool>,
    #[prost(bool, optional, tag = "7")]
    pub(crate) audio_muted: Option<bool>,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct StreamDescriptor {
    #[prost(uint32, optional, tag = "1")]
    pub(crate) stream_id: Option<u32>,
    #[prost(uint32, optional, tag = "2")]
    pub(crate) framerate: Option<u32>,
    #[prost(uint32, optional, tag = "3")]
    pub(crate) max_bitrate_kbps: Option<u32>,
    #[prost(string, optional, tag = "4")]
    pub(crate) track_label: Option<String>,
    #[prost(uint32, optional, tag = "6")]
    pub(crate) group_id: Option<u32>,
    #[prost(uint32, optional, tag = "7")]
    pub(crate) avg_bitrate_bps: Option<u32>,
    #[prost(string, optional, tag = "8")]
    pub(crate) attendee_id: Option<String>,
    #[prost(enumeration = "StreamMediaType", optional, tag = "9")]
    pub(crate) media_type: Option<i32>,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct SubscribeAckFrame {
    #[prost(string, optional, tag = "3")]
    pub(crate) sdp_answer: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct IndexFrame {
    #[prost(bool, optional, tag = "1")]
    pub(crate) at_capacity: Option<bool>,
    #[prost(uint32, optional, tag = "4")]
    pub(crate) num_participants: Option<u32>,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct LeaveFrame {}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct AudioControlFrame {
    #[prost(bool, optional, tag = "1")]
    pub(crate) muted: Option<bool>,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct AudioStreamIdInfoFrame {
    #[prost(message, repeated, tag = "1")]
    pub(crate) streams: Vec<AudioStreamIdInfo>,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct AudioStreamIdInfo {
    #[prost(uint32, optional, tag = "1")]
    pub(crate) audio_stream_id: Option<u32>,
    #[prost(string, optional, tag = "2")]
    pub(crate) attendee_id: Option<String>,
    #[prost(bool, optional, tag = "3")]
    pub(crate) muted: Option<bool>,
    #[prost(string, optional, tag = "4")]
    pub(crate) external_user_id: Option<String>,
    #[prost(bool, optional, tag = "5")]
    pub(crate) dropped: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, prost::Enumeration)]
#[repr(i32)]
pub(crate) enum PingPongType {
    Ping = 1,
    Pong = 2,
}

#[derive(Clone, PartialEq, Message)]
pub(crate) struct PingPongFrame {
    #[prost(enumeration = "PingPongType", required, tag = "1")]
    pub(crate) r#type: i32,
    #[prost(uint32, required, tag = "2")]
    pub(crate) ping_id: u32,
}

impl SignalFrame {
    pub(crate) fn new(frame_type: FrameType, timestamp_ms: u64) -> Self {
        Self {
            timestamp_ms,
            r#type: frame_type as i32,
            ..Self::default()
        }
    }

    pub(crate) fn frame_type(&self) -> Option<FrameType> {
        FrameType::try_from(self.r#type).ok()
    }

    /// The bytes to send: the frame type byte, then the protobuf.
    pub(crate) fn to_wire(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.encoded_len() + 1);
        bytes.push(FRAME_TYPE_RTC);
        self.encode(&mut bytes)
            .expect("a Vec grows to fit any signal frame");
        bytes
    }

    /// Reads a received message. The server may start it with `0x05` or
    /// `0x02`; the SDK drops the first byte either way.
    pub(crate) fn from_wire(bytes: &[u8]) -> Result<Self, String> {
        let body = bytes
            .get(1..)
            .ok_or_else(|| "empty Chime signaling message".to_string())?;
        Self::decode(body).map_err(|error| format!("unreadable Chime signaling message: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_through_the_wire_format() {
        let mut frame = SignalFrame::new(FrameType::PingPong, 42);
        frame.ping_pong = Some(PingPongFrame {
            r#type: PingPongType::Ping as i32,
            ping_id: 7,
        });
        let wire = frame.to_wire();
        assert_eq!(wire[0], 0x05);
        let decoded = SignalFrame::from_wire(&wire).unwrap();
        assert_eq!(decoded.frame_type(), Some(FrameType::PingPong));
        assert_eq!(decoded.ping_pong.unwrap().ping_id, 7);
    }

    #[test]
    fn the_signal_frame_header_matches_the_chime_field_numbers() {
        // timestamp_ms = 1 (varint 1), type = 2 (varint 19, PING_PONG).
        let wire = SignalFrame::new(FrameType::PingPong, 1).to_wire();
        assert_eq!(wire, [0x05, 0x08, 0x01, 0x10, 0x13]);
    }

    #[test]
    fn a_join_ack_yields_its_turn_credentials() {
        let mut frame = SignalFrame::new(FrameType::JoinAck, 1);
        frame.joinack = Some(JoinAckFrame {
            turn_credentials: Some(TurnCredentials {
                username: Some("user".into()),
                password: Some("secret".into()),
                ttl: Some(300),
                uris: vec!["turn:turn.example:3478?transport=udp".into()],
            }),
        });
        let decoded = SignalFrame::from_wire(&frame.to_wire()).unwrap();
        let turn = decoded.joinack.unwrap().turn_credentials.unwrap();
        assert_eq!(turn.uris.len(), 1);
        assert_eq!(turn.username.as_deref(), Some("user"));
    }
}
