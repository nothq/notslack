use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Condvar, Mutex,
    },
    time::Duration,
};

pub(super) struct SlackHostShutdown {
    stopped: AtomicBool,
    async_signal: tokio::sync::watch::Sender<bool>,
    wait_lock: Mutex<()>,
    wake: Condvar,
}

impl SlackHostShutdown {
    pub(super) fn new() -> Self {
        let (async_signal, _) = tokio::sync::watch::channel(false);
        Self {
            stopped: AtomicBool::new(false),
            async_signal,
            wait_lock: Mutex::new(()),
            wake: Condvar::new(),
        }
    }

    pub(super) fn request_stop(&self) {
        self.stopped.store(true, Ordering::Release);
        self.async_signal.send_replace(true);
        self.wake.notify_all();
    }

    pub(super) fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Acquire)
    }

    pub(super) fn subscribe(&self) -> tokio::sync::watch::Receiver<bool> {
        self.async_signal.subscribe()
    }

    pub(super) fn wait_or_stopped(&self, duration: Duration) -> bool {
        if self.is_stopped() {
            return true;
        }
        let guard = match self.wait_lock.lock() {
            Ok(guard) => guard,
            Err(_) => return true,
        };
        if self
            .wake
            .wait_timeout_while(guard, duration, |_| !self.is_stopped())
            .is_err()
        {
            return true;
        }
        self.is_stopped()
    }
}

pub(super) fn combine_shutdown_errors(
    mutex_error: Option<String>,
    realtime_error: Option<String>,
    finished_error: Option<String>,
    join_error: Option<String>,
) -> Result<(), String> {
    let failures = [mutex_error, realtime_error, finished_error, join_error]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}
