use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use tokio::sync::{broadcast, watch};

use super::{
    subscription::SlackRealtimeSubscription,
    worker::{SlackRealtimePresenceChannels, SlackRealtimeWorkerControl},
    SlackPresenceSubscriptionIds,
};
use crate::{
    live::api::SlackApiClient,
    model::{SlackRealtimeBatch, SlackRealtimePresenceSnapshot},
};

const SLACK_REALTIME_BROADCAST_CAPACITY: usize = 128;

pub(crate) struct SlackRealtimeHub {
    api: SlackApiClient,
    team_id: String,
    sender: broadcast::Sender<SlackRealtimeBatch>,
    presence_subscription_ids: watch::Sender<SlackPresenceSubscriptionIds>,
    presence_snapshot: watch::Sender<SlackRealtimePresenceSnapshot>,
    presence_commit: Arc<Mutex<()>>,
    state: Mutex<SlackRealtimeHubState>,
}

struct SlackRealtimeHubState {
    worker: Option<SlackRealtimeWorkerControl>,
    retired: bool,
}

impl SlackRealtimeHub {
    pub(crate) fn new(api: SlackApiClient, team_id: impl Into<String>) -> Self {
        let team_id = team_id.into();
        let (sender, _) = broadcast::channel(SLACK_REALTIME_BROADCAST_CAPACITY);
        let (presence_subscription_ids, _) = watch::channel(None);
        let (presence_snapshot, _) =
            watch::channel(SlackRealtimePresenceSnapshot::empty(team_id.clone()));
        Self {
            api,
            team_id,
            sender,
            presence_subscription_ids,
            presence_snapshot,
            presence_commit: Arc::new(Mutex::new(())),
            state: Mutex::new(SlackRealtimeHubState {
                worker: None,
                retired: false,
            }),
        }
    }

    pub(crate) fn set_presence_subscription_ids(&self, ids: Option<Arc<[String]>>) {
        let _commit = self
            .presence_commit
            .lock()
            .expect("Slack realtime presence commit mutex poisoned while setting the roster");
        let unchanged = self
            .presence_subscription_ids
            .borrow()
            .as_ref()
            .map(Arc::as_ref)
            == ids.as_ref().map(Arc::as_ref);
        if !unchanged {
            let desired = ids
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(String::as_str)
                .collect::<HashSet<_>>();
            let mut committed = None;
            self.presence_snapshot.send_if_modified(|snapshot| {
                let previous_len = snapshot.entries.len();
                snapshot
                    .entries
                    .retain(|entry| desired.contains(entry.user_id.as_str()));
                if snapshot.entries.len() == previous_len {
                    return false;
                }
                snapshot.revision = snapshot
                    .revision
                    .checked_add(1)
                    .expect("Slack raw presence revision overflowed");
                committed = Some(snapshot.clone());
                true
            });
            if let Some(snapshot) = committed {
                let _ = self.sender.send(SlackRealtimeBatch {
                    presence_revision: Some(snapshot.revision),
                    presence_snapshot: Some(snapshot),
                    ..SlackRealtimeBatch::default()
                });
            }
            self.presence_subscription_ids.send_replace(ids);
        }
    }

    pub(crate) fn subscribe(
        &self,
    ) -> Result<Box<dyn crate::model::SlackRealtimeSubscription>, String> {
        let receiver = self.sender.subscribe();
        let mut state = self
            .state
            .lock()
            .map_err(|_| "Slack realtime hub mutex poisoned".to_string())?;
        if state.retired {
            return Err(format!(
                "Slack realtime runtime for team {} was retired",
                self.team_id
            ));
        }
        if state.worker.is_none() {
            state.worker = Some(SlackRealtimeWorkerControl::start(
                self.api.clone(),
                self.team_id.clone(),
                self.sender.clone(),
                SlackRealtimePresenceChannels {
                    subscription_ids: self.presence_subscription_ids.subscribe(),
                    snapshot: self.presence_snapshot.clone(),
                    commit: self.presence_commit.clone(),
                },
            )?);
        }
        Ok(Box::new(SlackRealtimeSubscription::new(
            receiver,
            self.presence_snapshot.subscribe(),
        )))
    }

    pub(crate) fn require_running(&self) -> Result<(), String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Slack realtime hub mutex poisoned".to_string())?;
        if state.retired {
            return Err("Slack realtime hub was retired".to_string());
        }
        state
            .worker
            .as_ref()
            .ok_or_else(|| "Slack realtime hub has no raw worker".to_string())?
            .require_running()
    }

    pub(crate) fn retire(&self) -> Result<(), String> {
        let (mut state, mutex_error) = match self.state.lock() {
            Ok(state) => (state, None),
            Err(poisoned) => (
                poisoned.into_inner(),
                Some("Slack realtime hub mutex poisoned while retiring a runtime".to_string()),
            ),
        };
        let publish_closed = !state.retired;
        state.retired = true;
        let worker = state.worker.take();
        drop(state);
        if publish_closed {
            let _ = self.sender.send(SlackRealtimeBatch::closed());
        }
        let worker_error = worker.and_then(|worker| worker.stop().err());
        let failures = [mutex_error, worker_error]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }
}

impl Drop for SlackRealtimeHub {
    fn drop(&mut self) {
        if let Err(error) = self.retire() {
            eprintln!(
                "failed to retire Slack realtime hub for team {}: {error}",
                self.team_id
            );
        }
    }
}
