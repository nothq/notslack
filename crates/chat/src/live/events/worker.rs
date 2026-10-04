mod accumulator;
mod connection;
mod diagnostics;
mod presence;
mod socket;

use std::{
    sync::{atomic::AtomicU64, mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use tokio::{
    net::TcpStream,
    sync::{broadcast, watch},
};
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};

use super::SlackPresenceSubscriptionIds;
use crate::{
    live::api::{SlackApiClient, SlackRealtimeSocketAuth},
    model::{SlackRealtimeBatch, SlackRealtimePresenceSnapshot},
};
use connection::{run_worker, SlackRealtimeWorkerInput};
use diagnostics::{SlackRealtimeDiagnosticWorker, SlackRealtimeDiagnostics};

const SLACK_REALTIME_COALESCE_INTERVAL: Duration = Duration::from_millis(100);
const SLACK_REALTIME_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
const SLACK_REALTIME_PRESENCE_SUB_DELAY: Duration = Duration::from_millis(150);
const SLACK_REALTIME_PONG_TIMEOUT: Duration = Duration::from_secs(90);
const SLACK_REALTIME_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const SLACK_REALTIME_INITIAL_RECONNECT_BACKOFF: Duration = Duration::from_secs(1);
const SLACK_REALTIME_MAX_RECONNECT_BACKOFF: Duration = Duration::from_secs(60);
const SLACK_REALTIME_INITIAL_PING_ID: u64 = 16_384;
const SLACK_REALTIME_ORIGIN: &str = "https://app.slack.com";
const SLACK_REALTIME_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_4) AppleWebKit/537.36 (KHTML, like Gecko) Slack/4.51.180 Chrome/150.0.7871.114 Electron/43.1.1 Safari/537.36 AppleSilicon Sonic Slack_SSB/4.51.180";
const SLACK_REALTIME_DIAGNOSTICS_ENV: &str = "NOTSLACK_SLACK_REALTIME_DIAGNOSTICS";
const SLACK_REALTIME_DIAGNOSTICS_CAPACITY: usize = 1_024;

static NEXT_SLACK_REALTIME_DIAGNOSTIC_WORKER_ID: AtomicU64 = AtomicU64::new(1);

type SlackRealtimeSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;
type SlackRealtimeSocketReader = futures_util::stream::SplitStream<SlackRealtimeSocket>;
type SlackRealtimeSocketWriter = futures_util::stream::SplitSink<SlackRealtimeSocket, Message>;

pub(super) struct SlackRealtimePresenceChannels {
    pub(super) subscription_ids: watch::Receiver<SlackPresenceSubscriptionIds>,
    pub(super) snapshot: watch::Sender<SlackRealtimePresenceSnapshot>,
    pub(super) commit: Arc<Mutex<()>>,
}

pub(super) struct SlackRealtimeWorkerControl {
    stop: watch::Sender<bool>,
    worker: thread::JoinHandle<Result<(), String>>,
}

impl SlackRealtimeWorkerControl {
    pub(super) fn start(
        api: SlackApiClient,
        team_id: String,
        sender: broadcast::Sender<SlackRealtimeBatch>,
        presence: SlackRealtimePresenceChannels,
    ) -> Result<Self, String> {
        let SlackRealtimePresenceChannels {
            subscription_ids: presence_subscription_ids,
            snapshot: presence_snapshot,
            commit: presence_commit,
        } = presence;
        let socket_auth = api.realtime_socket_auth()?;
        let (stop, stop_rx) = watch::channel(false);
        let (startup, startup_result) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name(format!("notslack-slack-realtime-{team_id}"))
            .spawn(move || {
                run_worker_thread(SlackRealtimeWorkerThreadInput {
                    api,
                    socket_auth,
                    sender,
                    presence_subscription_ids,
                    presence_snapshot,
                    presence_commit,
                    stop: stop_rx,
                    startup,
                })
            })
            .map_err(|error| format!("failed to start Slack realtime worker: {error}"))?;
        match startup_result.recv() {
            Ok(Ok(())) => Ok(Self { stop, worker }),
            Ok(Err(error)) => match worker.join() {
                Ok(Ok(())) | Ok(Err(_)) => Err(error),
                Err(_) => Err(format!(
                    "{error}; Slack realtime worker panicked while reporting startup failure"
                )),
            },
            Err(_) => Err(match worker.join() {
                Ok(Ok(())) => "Slack realtime worker exited without reporting startup".to_string(),
                Ok(Err(error)) => {
                    format!("Slack realtime worker exited without reporting startup: {error}")
                }
                Err(_) => "Slack realtime worker panicked during startup".to_string(),
            }),
        }
    }

    pub(super) fn require_running(&self) -> Result<(), String> {
        if self.worker.is_finished() {
            Err("Slack realtime worker finished unexpectedly".to_string())
        } else {
            Ok(())
        }
    }

    pub(super) fn stop(self) -> Result<(), String> {
        let mut failures = Vec::new();
        if self.worker.is_finished() {
            failures.push("Slack realtime worker had already finished unexpectedly".to_string());
        }
        if self.stop.send(true).is_err() {
            failures.push("Slack realtime worker rejected its stop signal".to_string());
        }
        match self.worker.join() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => failures.push(format!("Slack realtime worker failed: {error}")),
            Err(_) => failures.push("Slack realtime worker panicked while stopping".to_string()),
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }
}

struct SlackRealtimeWorkerThreadInput {
    api: SlackApiClient,
    socket_auth: SlackRealtimeSocketAuth,
    sender: broadcast::Sender<SlackRealtimeBatch>,
    presence_subscription_ids: watch::Receiver<SlackPresenceSubscriptionIds>,
    presence_snapshot: watch::Sender<SlackRealtimePresenceSnapshot>,
    presence_commit: Arc<Mutex<()>>,
    stop: watch::Receiver<bool>,
    startup: mpsc::SyncSender<Result<(), String>>,
}

fn run_worker_thread(input: SlackRealtimeWorkerThreadInput) -> Result<(), String> {
    let SlackRealtimeWorkerThreadInput {
        api,
        socket_auth,
        sender,
        presence_subscription_ids,
        presence_snapshot,
        presence_commit,
        stop,
        startup,
    } = input;
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let _ = startup.send(Err(format!(
                "failed to create Slack realtime runtime: {error}"
            )));
            return Err(format!("failed to create Slack realtime runtime: {error}"));
        }
    };
    let diagnostics = match SlackRealtimeDiagnostics::start() {
        Ok(diagnostics) => diagnostics,
        Err(error) => {
            let _ = startup.send(Err(error.clone()));
            return Err(error);
        }
    };
    let (diagnostic, diagnostic_worker) = match diagnostics {
        Some(diagnostics) => {
            let (diagnostic, worker) = diagnostics.into_parts();
            (Some(diagnostic), Some(worker))
        }
        None => (None, None),
    };
    if startup.send(Ok(())).is_err() {
        drop(diagnostic);
        return stop_diagnostics(diagnostic_worker);
    }
    runtime.block_on(run_worker(SlackRealtimeWorkerInput {
        api: &api,
        socket_auth: &socket_auth,
        sender,
        presence_subscription_ids,
        presence_snapshot,
        presence_commit,
        stop,
        diagnostic,
    }));
    stop_diagnostics(diagnostic_worker)
}

fn stop_diagnostics(worker: Option<SlackRealtimeDiagnosticWorker>) -> Result<(), String> {
    match worker {
        Some(worker) => worker.stop(),
        None => Ok(()),
    }
}
