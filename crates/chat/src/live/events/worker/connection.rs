use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::{
    sync::{broadcast, watch},
    time::{sleep, timeout},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        client::IntoClientRequest,
        http::{
            header::{COOKIE, ORIGIN},
            HeaderValue,
        },
    },
};

use crate::{
    live::api::{SlackApiClient, SlackRealtimeSocketAuth, SlackRealtimeSocketUrl},
    model::{SlackRealtimeBatch, SlackRealtimeConnectionState, SlackRealtimePresenceSnapshot},
};

use super::{
    super::SlackPresenceSubscriptionIds,
    diagnostics::{SlackRealtimeDiagnostic, SlackRealtimeDiagnosticRecord},
    presence::SlackSocketPresence,
    socket::{run_socket, SlackSocketOutcome, SlackSocketRunContext},
    SlackRealtimeSocket, SLACK_REALTIME_CONNECT_TIMEOUT, SLACK_REALTIME_INITIAL_RECONNECT_BACKOFF,
    SLACK_REALTIME_MAX_RECONNECT_BACKOFF, SLACK_REALTIME_ORIGIN, SLACK_REALTIME_USER_AGENT,
};

pub(super) struct SlackRealtimeWorkerInput<'a> {
    pub(super) api: &'a SlackApiClient,
    pub(super) socket_auth: &'a SlackRealtimeSocketAuth,
    pub(super) sender: broadcast::Sender<SlackRealtimeBatch>,
    pub(super) presence_subscription_ids: watch::Receiver<SlackPresenceSubscriptionIds>,
    pub(super) presence_snapshot: watch::Sender<SlackRealtimePresenceSnapshot>,
    pub(super) presence_commit: Arc<Mutex<()>>,
    pub(super) stop: watch::Receiver<bool>,
    pub(super) diagnostic: Option<SlackRealtimeDiagnostic>,
}

pub(super) async fn run_worker(input: SlackRealtimeWorkerInput<'_>) {
    let SlackRealtimeWorkerInput {
        api,
        socket_auth,
        sender,
        presence_subscription_ids,
        presence_snapshot,
        presence_commit,
        stop,
        diagnostic,
    } = input;
    SlackRealtimeWorker {
        api,
        socket_auth,
        sender,
        presence_subscription_ids,
        presence_snapshot,
        presence_commit,
        stop,
        diagnostic,
        reconnect_url: None,
        reconnecting: false,
        previous_backoff: SLACK_REALTIME_INITIAL_RECONNECT_BACKOFF,
        socket_sequence: 0,
    }
    .run()
    .await;
}

struct SlackRealtimeWorker<'a> {
    api: &'a SlackApiClient,
    socket_auth: &'a SlackRealtimeSocketAuth,
    sender: broadcast::Sender<SlackRealtimeBatch>,
    presence_subscription_ids: watch::Receiver<SlackPresenceSubscriptionIds>,
    presence_snapshot: watch::Sender<SlackRealtimePresenceSnapshot>,
    presence_commit: Arc<Mutex<()>>,
    stop: watch::Receiver<bool>,
    diagnostic: Option<SlackRealtimeDiagnostic>,
    reconnect_url: Option<SlackRealtimeSocketUrl>,
    reconnecting: bool,
    previous_backoff: Duration,
    socket_sequence: u64,
}

enum SlackWorkerConnectOutcome {
    Socket(Box<SlackRealtimeSocket>),
    Retry,
    Stop,
}

impl SlackRealtimeWorker<'_> {
    async fn run(mut self) {
        loop {
            if *self.stop.borrow() {
                return;
            }
            let socket = match self.connect().await {
                SlackWorkerConnectOutcome::Socket(socket) => *socket,
                SlackWorkerConnectOutcome::Retry => continue,
                SlackWorkerConnectOutcome::Stop => return,
            };
            self.record_socket_open();
            let outcome = self.run_connected_socket(socket).await;
            if self.handle_socket_outcome(outcome).await {
                return;
            }
        }
    }

    async fn connect(&mut self) -> SlackWorkerConnectOutcome {
        send_connection_state(
            &self.sender,
            if self.reconnecting {
                SlackRealtimeConnectionState::Reconnecting
            } else {
                SlackRealtimeConnectionState::Connecting
            },
        );
        let socket_result = if let Some(url) = self.reconnect_url.take() {
            connect_slack_socket(&url, self.socket_auth, &mut self.stop).await
        } else {
            connect_fresh_socket(self.api, self.socket_auth, &mut self.stop).await
        };
        match socket_result {
            Ok(Some(socket)) => SlackWorkerConnectOutcome::Socket(Box::new(socket)),
            Ok(None) => SlackWorkerConnectOutcome::Stop,
            Err(()) if self.wait_after_failure().await => SlackWorkerConnectOutcome::Stop,
            Err(()) => SlackWorkerConnectOutcome::Retry,
        }
    }

    fn record_socket_open(&mut self) {
        let Some(diagnostic) = self.diagnostic.as_ref() else {
            return;
        };
        self.socket_sequence = self.socket_sequence.wrapping_add(1);
        diagnostic.record(SlackRealtimeDiagnosticRecord::SocketOpen {
            socket_sequence: self.socket_sequence,
            reconnecting: self.reconnecting,
        });
    }

    async fn run_connected_socket(&mut self, socket: SlackRealtimeSocket) -> SlackSocketOutcome {
        run_socket(
            socket,
            &self.sender,
            SlackSocketPresence {
                subscription_ids: &mut self.presence_subscription_ids,
                snapshot: &self.presence_snapshot,
                commit: &self.presence_commit,
            },
            &mut self.stop,
            SlackSocketRunContext::new(
                self.reconnecting,
                self.diagnostic.as_ref(),
                self.socket_sequence,
            ),
        )
        .await
    }

    async fn handle_socket_outcome(&mut self, outcome: SlackSocketOutcome) -> bool {
        let SlackSocketOutcome::Reconnect {
            reconnect_url,
            connected,
            reason,
        } = outcome
        else {
            return true;
        };
        if let Some(diagnostic) = self.diagnostic.as_ref() {
            diagnostic.record(SlackRealtimeDiagnosticRecord::SocketExit {
                socket_sequence: self.socket_sequence,
                reason,
            });
        }
        self.reconnect_url = reconnect_url;
        self.reconnecting = true;
        if connected {
            self.previous_backoff = SLACK_REALTIME_INITIAL_RECONNECT_BACKOFF;
            false
        } else {
            self.wait_after_failure().await
        }
    }

    async fn wait_after_failure(&mut self) -> bool {
        self.reconnecting = true;
        let backoff = next_backoff(self.previous_backoff);
        let stopped = wait_for_reconnect(backoff, &mut self.stop).await;
        self.previous_backoff = backoff;
        stopped
    }
}

async fn connect_fresh_socket(
    api: &SlackApiClient,
    socket_auth: &SlackRealtimeSocketAuth,
    stop: &mut watch::Receiver<bool>,
) -> Result<Option<SlackRealtimeSocket>, ()> {
    let api = api.clone();
    let urls = tokio::task::spawn_blocking(move || api.realtime_socket_urls())
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
    match connect_slack_socket(&urls.primary, socket_auth, stop).await {
        Ok(Some(socket)) => Ok(Some(socket)),
        Ok(None) => Ok(None),
        Err(()) => connect_slack_socket(&urls.fallback, socket_auth, stop).await,
    }
}

async fn connect_slack_socket(
    url: &SlackRealtimeSocketUrl,
    socket_auth: &SlackRealtimeSocketAuth,
    stop: &mut watch::Receiver<bool>,
) -> Result<Option<SlackRealtimeSocket>, ()> {
    let mut request = url.as_str().into_client_request().map_err(|_| ())?;
    request
        .headers_mut()
        .insert(ORIGIN, HeaderValue::from_static(SLACK_REALTIME_ORIGIN));
    request.headers_mut().insert(
        "user-agent",
        HeaderValue::from_static(SLACK_REALTIME_USER_AGENT),
    );
    request
        .headers_mut()
        .insert(COOKIE, socket_auth.cookie_header().clone());
    let connection = timeout(SLACK_REALTIME_CONNECT_TIMEOUT, connect_async(request));
    tokio::select! {
        result = connection => {
            result.map_err(|_| ())?.map(|(socket, _)| Some(socket)).map_err(|_| ())
        }
        changed = stop.changed() => {
            let _ = changed;
            Ok(None)
        }
    }
}

fn send_connection_state(
    sender: &broadcast::Sender<SlackRealtimeBatch>,
    connection_state: SlackRealtimeConnectionState,
) {
    let _ = sender.send(SlackRealtimeBatch {
        connection_state: Some(connection_state),
        ..SlackRealtimeBatch::default()
    });
}

async fn wait_for_reconnect(backoff: Duration, stop: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = sleep(backoff) => false,
        changed = stop.changed() => {
            let _ = changed;
            true
        }
    }
}

fn next_backoff(backoff: Duration) -> Duration {
    let minimum_millis = u64::try_from(SLACK_REALTIME_INITIAL_RECONNECT_BACKOFF.as_millis())
        .expect("Slack realtime minimum reconnect backoff must fit u64 milliseconds");
    let random_upper_millis = u64::try_from(backoff.as_millis().saturating_mul(3))
        .expect("Slack realtime reconnect backoff must fit u64 milliseconds");
    let sample_range = random_upper_millis
        .saturating_sub(minimum_millis)
        .saturating_add(1);
    let mut random_bytes = [0_u8; 8];
    if getrandom::fill(&mut random_bytes).is_err() {
        return SLACK_REALTIME_INITIAL_RECONNECT_BACKOFF;
    }
    let sampled_millis = minimum_millis + u64::from_le_bytes(random_bytes) % sample_range;
    Duration::from_millis(sampled_millis).min(SLACK_REALTIME_MAX_RECONNECT_BACKOFF)
}
