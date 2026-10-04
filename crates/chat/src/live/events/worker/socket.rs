mod diagnostic_context;
mod session;
use crate::{live::api, model};
use tokio::sync as tokio_sync;
use tokio_tungstenite::tungstenite;

pub(super) use diagnostic_context::SlackSocketRunContext;

pub(super) enum SlackSocketOutcome {
    Stopped,
    Reconnect {
        reconnect_url: Option<api::SlackRealtimeSocketUrl>,
        connected: bool,
        reason: SlackSocketReconnectReason,
    },
}

#[derive(Clone, Copy)]
pub(super) enum SlackSocketReconnectReason {
    InitialPresenceSend,
    ReplacementPresenceSend,
    PongTimeout,
    PingSend,
    StreamEnded,
    StreamRead(SlackSocketReadError),
    Goodbye,
    PongSend,
    CloseFrame(Option<u16>),
}

impl SlackSocketReconnectReason {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::InitialPresenceSend => "initial_presence_send",
            Self::ReplacementPresenceSend => "replacement_presence_send",
            Self::PongTimeout => "pong_timeout",
            Self::PingSend => "ping_send",
            Self::StreamEnded => "stream_ended",
            Self::StreamRead(_) => "stream_read",
            Self::Goodbye => "goodbye",
            Self::PongSend => "pong_send",
            Self::CloseFrame(_) => "close_frame",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum SlackSocketReadError {
    ConnectionClosed,
    AlreadyClosed,
    Io(std::io::ErrorKind),
    Tls,
    Capacity,
    Protocol(SlackSocketProtocolError),
    WriteBufferFull,
    Utf8,
    AttackAttempt,
    Url,
    Http,
    HttpFormat,
}

impl From<&tungstenite::Error> for SlackSocketReadError {
    fn from(error: &tungstenite::Error) -> Self {
        match error {
            tungstenite::Error::ConnectionClosed => Self::ConnectionClosed,
            tungstenite::Error::AlreadyClosed => Self::AlreadyClosed,
            tungstenite::Error::Io(error) => Self::Io(error.kind()),
            tungstenite::Error::Tls(_) => Self::Tls,
            tungstenite::Error::Capacity(_) => Self::Capacity,
            tungstenite::Error::Protocol(error) => Self::Protocol(error.into()),
            tungstenite::Error::WriteBufferFull(_) => Self::WriteBufferFull,
            tungstenite::Error::Utf8 => Self::Utf8,
            tungstenite::Error::AttackAttempt => Self::AttackAttempt,
            tungstenite::Error::Url(_) => Self::Url,
            tungstenite::Error::Http(_) => Self::Http,
            tungstenite::Error::HttpFormat(_) => Self::HttpFormat,
        }
    }
}

impl SlackSocketReadError {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::ConnectionClosed => "connection_closed",
            Self::AlreadyClosed => "already_closed",
            Self::Io(_) => "io",
            Self::Tls => "tls",
            Self::Capacity => "capacity",
            Self::Protocol(_) => "protocol",
            Self::WriteBufferFull => "write_buffer_full",
            Self::Utf8 => "utf8",
            Self::AttackAttempt => "attack_attempt",
            Self::Url => "url",
            Self::Http => "http",
            Self::HttpFormat => "http_format",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum SlackSocketProtocolError {
    ResetWithoutClosingHandshake,
    Other,
}

impl From<&tungstenite::error::ProtocolError> for SlackSocketProtocolError {
    fn from(error: &tungstenite::error::ProtocolError) -> Self {
        if matches!(
            error,
            tungstenite::error::ProtocolError::ResetWithoutClosingHandshake
        ) {
            Self::ResetWithoutClosingHandshake
        } else {
            Self::Other
        }
    }
}

impl SlackSocketProtocolError {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::ResetWithoutClosingHandshake => "reset_without_closing_handshake",
            Self::Other => "other",
        }
    }
}

pub(super) async fn run_socket(
    socket: super::SlackRealtimeSocket,
    sender: &tokio_sync::broadcast::Sender<model::SlackRealtimeBatch>,
    presence: super::presence::SlackSocketPresence<'_>,
    stop: &mut tokio_sync::watch::Receiver<bool>,
    context: SlackSocketRunContext<'_>,
) -> SlackSocketOutcome {
    session::SlackSocketSession::new(socket, sender, presence, stop, context)
        .run()
        .await
}
