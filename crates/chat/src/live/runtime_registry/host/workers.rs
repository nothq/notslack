use std::{
    sync::{mpsc, Arc},
    thread,
};

use super::{
    loops::{
        slack_notification_policy_loop, slack_notification_read_receipt_loop,
        slack_realtime_thread, SlackRealtimeWorkerSource,
    },
    shutdown::{combine_shutdown_errors, SlackHostShutdown},
    SlackHostEventEmitter,
};
use crate::live::runtime_registry::{SlackNotificationReadReceipt, SlackWorkspaceRuntimeRegistry};

const POLICY_REQUEST_CAPACITY: usize = 1;

pub(super) struct SlackHostWorkers {
    shutdown: Arc<SlackHostShutdown>,
    workers: Vec<SlackHostWorker>,
}

impl SlackHostWorkers {
    pub(super) fn require_running(&self) -> Result<(), String> {
        if self.shutdown.is_stopped() {
            return Err("Slack host realtime producer is stopped".to_string());
        }
        let finished = self.finished_worker_names();
        if finished.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Slack host workers finished unexpectedly: {}",
                finished.join(", ")
            ))
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn worker_count(&self) -> usize {
        self.workers.len()
    }

    pub(super) fn finished_workers_error(&self) -> Option<String> {
        let finished = self.finished_worker_names();
        if finished.is_empty() {
            None
        } else {
            Some(format!(
                "Slack host workers had already finished unexpectedly: {}",
                finished.join(", ")
            ))
        }
    }

    fn finished_worker_names(&self) -> Vec<&str> {
        self.workers
            .iter()
            .filter(|worker| worker.handle.is_finished())
            .map(|worker| worker.name.as_str())
            .collect()
    }

    pub(super) fn request_stop(&self) {
        self.shutdown.request_stop();
    }

    pub(super) fn stop_requested(&self) -> bool {
        self.shutdown.is_stopped()
    }

    pub(super) fn join(&mut self) -> Result<(), String> {
        let mut failures = Vec::new();
        for worker in self.workers.drain(..) {
            if worker.handle.join().is_err() {
                failures.push(format!("{} panicked", worker.name));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }
}

struct SlackHostWorker {
    name: String,
    handle: thread::JoinHandle<()>,
}

pub(super) struct SlackHostWorkerStartup {
    shutdown: Arc<SlackHostShutdown>,
    workers: Vec<SlackHostWorker>,
}

impl SlackHostWorkerStartup {
    pub(super) fn new() -> Self {
        Self {
            shutdown: Arc::new(SlackHostShutdown::new()),
            workers: Vec::new(),
        }
    }

    pub(super) fn spawn<S>(
        &mut self,
        realtime_sources: Vec<S>,
        read_receipts: mpsc::Receiver<SlackNotificationReadReceipt>,
        events: SlackHostEventEmitter,
    ) -> Result<(), String>
    where
        S: SlackRealtimeWorkerSource,
    {
        for source in realtime_sources {
            self.spawn_team(source, events.clone())?;
        }
        self.spawn_read_receipts(read_receipts, events)
    }

    fn spawn_team<S>(&mut self, source: S, events: SlackHostEventEmitter) -> Result<(), String>
    where
        S: SlackRealtimeWorkerSource,
    {
        let team_id = source.team_id().to_string();
        let policy_source = source.notification_policy_source();
        let (policy_requests, policy_request_receiver) =
            mpsc::sync_channel(POLICY_REQUEST_CAPACITY);
        let policy_worker = spawn_named_worker(
            format!("notslack-slack-host-policy-{team_id}"),
            "Slack notification policy manager",
            &team_id,
            {
                let shutdown = self.shutdown.clone();
                let events = events.clone();
                move || {
                    slack_notification_policy_loop(
                        policy_source,
                        policy_request_receiver,
                        events,
                        shutdown,
                    )
                }
            },
        )?;
        self.workers.push(policy_worker);

        let (startup, startup_result) = mpsc::sync_channel(1);
        let realtime_worker = spawn_named_worker(
            format!("notslack-slack-host-realtime-{team_id}"),
            "Slack realtime manager",
            &team_id,
            {
                let shutdown = self.shutdown.clone();
                move || slack_realtime_thread(source, policy_requests, events, shutdown, startup)
            },
        )?;
        wait_for_realtime_startup(&team_id, realtime_worker, startup_result)
            .map(|worker| self.workers.push(worker))
    }

    fn spawn_read_receipts(
        &mut self,
        receipts: mpsc::Receiver<SlackNotificationReadReceipt>,
        events: SlackHostEventEmitter,
    ) -> Result<(), String> {
        let worker_name = "notslack-slack-host-read-receipts".to_string();
        let shutdown = self.shutdown.clone();
        let handle = thread::Builder::new()
            .name(worker_name.clone())
            .spawn(move || slack_notification_read_receipt_loop(receipts, events, shutdown))
            .map_err(|error| {
                format!("failed to start Slack notification read receipt routing: {error}")
            })?;
        self.workers.push(SlackHostWorker {
            name: worker_name,
            handle,
        });
        Ok(())
    }

    pub(super) fn stop_and_join(
        &mut self,
        registry: &SlackWorkspaceRuntimeRegistry,
    ) -> Result<(), String> {
        self.shutdown.request_stop();
        let realtime_error = registry.stop_realtime_sources().err();
        let mut workers = SlackHostWorkers {
            shutdown: self.shutdown.clone(),
            workers: std::mem::take(&mut self.workers),
        };
        let join_error = workers.join().err();
        combine_shutdown_errors(None, realtime_error, None, join_error)
    }

    pub(super) fn finish(self) -> SlackHostWorkers {
        SlackHostWorkers {
            shutdown: self.shutdown,
            workers: self.workers,
        }
    }
}

fn spawn_named_worker(
    name: String,
    role: &str,
    team_id: &str,
    run: impl FnOnce() + Send + 'static,
) -> Result<SlackHostWorker, String> {
    let handle = thread::Builder::new()
        .name(name.clone())
        .spawn(run)
        .map_err(|error| format!("failed to start {role} for team {team_id}: {error}"))?;
    Ok(SlackHostWorker { name, handle })
}

fn wait_for_realtime_startup(
    team_id: &str,
    worker: SlackHostWorker,
    startup: mpsc::Receiver<Result<(), String>>,
) -> Result<SlackHostWorker, String> {
    match startup.recv() {
        Ok(Ok(())) => Ok(worker),
        Ok(Err(error)) => {
            let _ = worker.handle.join();
            Err(error)
        }
        Err(_) => {
            let panicked = worker.handle.join().is_err();
            Err(if panicked {
                format!("Slack host realtime manager for team {team_id} panicked during startup")
            } else {
                format!(
                    "Slack host realtime manager for team {team_id} exited without reporting startup"
                )
            })
        }
    }
}
