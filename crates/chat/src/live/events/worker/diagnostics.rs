use std::{
    sync::{
        atomic::Ordering,
        mpsc::{sync_channel, SyncSender},
    },
    thread,
};

mod output;

use super::{
    socket::SlackSocketReconnectReason, NEXT_SLACK_REALTIME_DIAGNOSTIC_WORKER_ID,
    SLACK_REALTIME_DIAGNOSTICS_CAPACITY, SLACK_REALTIME_DIAGNOSTICS_ENV,
};

pub(super) struct SlackRealtimeDiagnostic {
    output: SyncSender<SlackRealtimeDiagnosticRecord>,
}

pub(super) struct SlackRealtimeDiagnostics {
    recorder: SlackRealtimeDiagnostic,
    worker: SlackRealtimeDiagnosticWorker,
}

pub(super) struct SlackRealtimeDiagnosticWorker {
    worker: Option<thread::JoinHandle<()>>,
}

pub(super) enum SlackRealtimeDiagnosticRecord {
    SocketOpen {
        socket_sequence: u64,
        reconnecting: bool,
    },
    Hello {
        socket_sequence: u64,
        reconnect: bool,
    },
    PresenceSubscription {
        socket_sequence: u64,
        initial: bool,
        id: u64,
        users: usize,
        hello_delay_ms: Option<u128>,
    },
    Ping {
        socket_sequence: u64,
        id: u64,
    },
    PresenceChange {
        socket_sequence: u64,
        active: usize,
        away: usize,
    },
    SocketExit {
        socket_sequence: u64,
        reason: SlackSocketReconnectReason,
    },
    MalformedFrame {
        socket_sequence: u64,
        connected: bool,
    },
}

struct SlackPresenceSubscriptionDiagnostic {
    socket_sequence: u64,
    initial: bool,
    id: u64,
    users: usize,
    hello_delay_ms: Option<u128>,
}

enum SlackTransportDiagnosticRecord {
    Ping {
        socket_sequence: u64,
        id: u64,
    },
    PresenceChange {
        socket_sequence: u64,
        active: usize,
        away: usize,
    },
    SocketExit {
        socket_sequence: u64,
        reason: SlackSocketReconnectReason,
    },
    MalformedFrame {
        socket_sequence: u64,
        connected: bool,
    },
}

impl SlackRealtimeDiagnostics {
    pub(super) fn start() -> Result<Option<Self>, String> {
        if std::env::var_os(SLACK_REALTIME_DIAGNOSTICS_ENV).is_none() {
            return Ok(None);
        }
        let worker_id = NEXT_SLACK_REALTIME_DIAGNOSTIC_WORKER_ID.fetch_add(1, Ordering::Relaxed);
        let (output, records) =
            sync_channel::<SlackRealtimeDiagnosticRecord>(SLACK_REALTIME_DIAGNOSTICS_CAPACITY);
        let worker = thread::Builder::new()
            .name("notslack-slack-realtime-diagnostics".to_string())
            .spawn(move || {
                let stderr = std::io::stderr();
                while let Ok(record) = records.recv() {
                    let mut stderr = stderr.lock();
                    output::write_slack_realtime_diagnostic(&mut stderr, worker_id, record);
                }
            })
            .map_err(|error| {
                format!("failed to start Slack realtime diagnostics worker: {error}")
            })?;
        Ok(Some(Self {
            recorder: SlackRealtimeDiagnostic { output },
            worker: SlackRealtimeDiagnosticWorker {
                worker: Some(worker),
            },
        }))
    }

    pub(super) fn into_parts(self) -> (SlackRealtimeDiagnostic, SlackRealtimeDiagnosticWorker) {
        (self.recorder, self.worker)
    }
}

impl SlackRealtimeDiagnostic {
    pub(super) fn record(&self, record: SlackRealtimeDiagnosticRecord) {
        let _ = self.output.try_send(record);
    }
}

impl SlackRealtimeDiagnosticWorker {
    pub(super) fn stop(mut self) -> Result<(), String> {
        self.join()
    }

    fn join(&mut self) -> Result<(), String> {
        if let Some(worker) = self.worker.take() {
            if worker.join().is_err() {
                return Err("Slack realtime diagnostics worker panicked while stopping".to_string());
            }
        }
        Ok(())
    }
}

impl Drop for SlackRealtimeDiagnosticWorker {
    fn drop(&mut self) {
        if let Err(error) = self.join() {
            eprintln!("{error}");
        }
    }
}
