use std::io::Write;

use super::{
    SlackPresenceSubscriptionDiagnostic, SlackRealtimeDiagnosticRecord,
    SlackTransportDiagnosticRecord,
};
use crate::live::events::worker::socket::{SlackSocketReadError, SlackSocketReconnectReason};

pub(super) fn write_slack_realtime_diagnostic(
    output: &mut impl Write,
    worker_id: u64,
    record: SlackRealtimeDiagnosticRecord,
) {
    let result = match record {
        SlackRealtimeDiagnosticRecord::SocketOpen {
            socket_sequence,
            reconnecting,
        } => write_socket_open_diagnostic(output, worker_id, socket_sequence, reconnecting),
        SlackRealtimeDiagnosticRecord::Hello {
            socket_sequence,
            reconnect,
        } => write_hello_diagnostic(output, worker_id, socket_sequence, reconnect),
        SlackRealtimeDiagnosticRecord::PresenceSubscription {
            socket_sequence,
            initial,
            id,
            users,
            hello_delay_ms,
        } => write_presence_subscription_diagnostic(
            output,
            worker_id,
            SlackPresenceSubscriptionDiagnostic {
                socket_sequence,
                initial,
                id,
                users,
                hello_delay_ms,
            },
        ),
        SlackRealtimeDiagnosticRecord::Ping {
            socket_sequence,
            id,
        } => write_ping_diagnostic(output, worker_id, socket_sequence, id),
        SlackRealtimeDiagnosticRecord::PresenceChange {
            socket_sequence,
            active,
            away,
        } => write_presence_change_diagnostic(output, worker_id, socket_sequence, active, away),
        SlackRealtimeDiagnosticRecord::SocketExit {
            socket_sequence,
            reason,
        } => write_socket_exit_record(output, worker_id, socket_sequence, reason),
        SlackRealtimeDiagnosticRecord::MalformedFrame {
            socket_sequence,
            connected,
        } => write_malformed_frame_diagnostic(output, worker_id, socket_sequence, connected),
    };
    let _ = result;
}

fn write_ping_diagnostic(
    output: &mut impl Write,
    worker_id: u64,
    socket_sequence: u64,
    id: u64,
) -> std::io::Result<()> {
    write_transport_diagnostic(
        output,
        worker_id,
        SlackTransportDiagnosticRecord::Ping {
            socket_sequence,
            id,
        },
    )
}

fn write_presence_change_diagnostic(
    output: &mut impl Write,
    worker_id: u64,
    socket_sequence: u64,
    active: usize,
    away: usize,
) -> std::io::Result<()> {
    write_transport_diagnostic(
        output,
        worker_id,
        SlackTransportDiagnosticRecord::PresenceChange {
            socket_sequence,
            active,
            away,
        },
    )
}

fn write_socket_exit_record(
    output: &mut impl Write,
    worker_id: u64,
    socket_sequence: u64,
    reason: SlackSocketReconnectReason,
) -> std::io::Result<()> {
    write_transport_diagnostic(
        output,
        worker_id,
        SlackTransportDiagnosticRecord::SocketExit {
            socket_sequence,
            reason,
        },
    )
}

fn write_malformed_frame_diagnostic(
    output: &mut impl Write,
    worker_id: u64,
    socket_sequence: u64,
    connected: bool,
) -> std::io::Result<()> {
    write_transport_diagnostic(
        output,
        worker_id,
        SlackTransportDiagnosticRecord::MalformedFrame {
            socket_sequence,
            connected,
        },
    )
}

fn write_socket_open_diagnostic(
    output: &mut impl Write,
    worker_id: u64,
    socket_sequence: u64,
    reconnecting: bool,
) -> std::io::Result<()> {
    writeln!(
        output,
        "[notslack-slack-realtime-profile] worker={worker_id} socket={socket_sequence} event=socket_open reconnecting={reconnecting}"
    )
}

fn write_hello_diagnostic(
    output: &mut impl Write,
    worker_id: u64,
    socket_sequence: u64,
    reconnect: bool,
) -> std::io::Result<()> {
    writeln!(
        output,
        "[notslack-slack-realtime-profile] worker={worker_id} socket={socket_sequence} event=hello reconnect={reconnect}"
    )
}

fn write_transport_diagnostic(
    output: &mut impl Write,
    worker_id: u64,
    record: SlackTransportDiagnosticRecord,
) -> std::io::Result<()> {
    match record {
        SlackTransportDiagnosticRecord::Ping {
            socket_sequence,
            id,
        } => writeln!(
            output,
            "[notslack-slack-realtime-profile] worker={worker_id} socket={socket_sequence} event=ping id={id}"
        ),
        SlackTransportDiagnosticRecord::PresenceChange {
            socket_sequence,
            active,
            away,
        } => writeln!(
            output,
            "[notslack-slack-realtime-profile] worker={worker_id} socket={socket_sequence} event=presence_change active={active} away={away}"
        ),
        SlackTransportDiagnosticRecord::SocketExit {
            socket_sequence,
            reason,
        } => write_socket_exit_diagnostic(output, worker_id, socket_sequence, reason),
        SlackTransportDiagnosticRecord::MalformedFrame {
            socket_sequence,
            connected,
        } => writeln!(
            output,
            "[notslack-slack-realtime-profile] worker={worker_id} socket={socket_sequence} event=malformed_frame connected={connected}"
        ),
    }
}

fn write_socket_exit_diagnostic(
    output: &mut impl Write,
    worker_id: u64,
    socket_sequence: u64,
    reason: SlackSocketReconnectReason,
) -> std::io::Result<()> {
    match reason {
        SlackSocketReconnectReason::StreamRead(error) => {
            write_stream_read_exit_diagnostic(output, worker_id, socket_sequence, error)
        }
        SlackSocketReconnectReason::CloseFrame(close_code) => {
            if let Some(close_code) = close_code {
                writeln!(
                    output,
                    "[notslack-slack-realtime-profile] worker={worker_id} socket={socket_sequence} event=socket_exit reason=close_frame close_code={close_code}"
                )
            } else {
                writeln!(
                    output,
                    "[notslack-slack-realtime-profile] worker={worker_id} socket={socket_sequence} event=socket_exit reason=close_frame close_code=none"
                )
            }
        }
        reason => writeln!(
            output,
            "[notslack-slack-realtime-profile] worker={worker_id} socket={socket_sequence} event=socket_exit reason={}",
            reason.as_str(),
        ),
    }
}

fn write_stream_read_exit_diagnostic(
    output: &mut impl Write,
    worker_id: u64,
    socket_sequence: u64,
    error: SlackSocketReadError,
) -> std::io::Result<()> {
    match error {
        SlackSocketReadError::Io(kind) => writeln!(
            output,
            "[notslack-slack-realtime-profile] worker={worker_id} socket={socket_sequence} event=socket_exit reason=stream_read read_error=io io_kind={kind:?}"
        ),
        SlackSocketReadError::Protocol(protocol_error) => writeln!(
            output,
            "[notslack-slack-realtime-profile] worker={worker_id} socket={socket_sequence} event=socket_exit reason=stream_read read_error=protocol protocol_error={}",
            protocol_error.as_str(),
        ),
        error => writeln!(
            output,
            "[notslack-slack-realtime-profile] worker={worker_id} socket={socket_sequence} event=socket_exit reason=stream_read read_error={}",
            error.as_str(),
        ),
    }
}

fn write_presence_subscription_diagnostic(
    output: &mut impl Write,
    worker_id: u64,
    diagnostic: SlackPresenceSubscriptionDiagnostic,
) -> std::io::Result<()> {
    let reason = if diagnostic.initial {
        "initial"
    } else {
        "replacement"
    };
    if let Some(hello_delay_ms) = diagnostic.hello_delay_ms {
        writeln!(
            output,
            "[notslack-slack-realtime-profile] worker={worker_id} socket={} event=presence_sub reason={reason} id={} users={} hello_delay_ms={hello_delay_ms}",
            diagnostic.socket_sequence,
            diagnostic.id,
            diagnostic.users,
        )
    } else {
        writeln!(
            output,
            "[notslack-slack-realtime-profile] worker={worker_id} socket={} event=presence_sub reason={reason} id={} users={}",
            diagnostic.socket_sequence,
            diagnostic.id,
            diagnostic.users,
        )
    }
}
