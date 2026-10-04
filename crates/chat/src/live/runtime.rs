use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Arc, Mutex, OnceLock,
};

use crate::model::{SlackRealtimePresenceSnapshot, SlackShellSnapshot};

use crate::live::{events::SlackRealtimeHub, SlackLiveWorkspaceLoader, SlackWorkspaceCacheStore};

mod beginning;
mod confirmed_sends;
mod media_proxy;
mod notifications;
mod presence;
mod storage;
#[cfg(test)]
mod tests;
mod workspace_api;

#[cfg(test)]
use confirmed_sends::{
    insert_slack_confirmed_send, reconcile_slack_confirmed_sends, slack_message_timestamp_key,
};
use confirmed_sends::{
    merge_slack_conversation_windows, validate_slack_conversation_message_order,
    SlackConfirmedSendMap,
};
use media_proxy::SlackMediaProxy;
use notifications::SlackNotificationPolicy;
use presence::SlackPresenceState;

type SlackWorkspaceCacheCell = OnceLock<Result<SlackWorkspaceCacheStore, String>>;

pub(crate) trait SlackSelectionStore: Send + Sync + 'static {
    fn remember_slack_team_conversation(
        &self,
        team_id: &str,
        conversation_id: &str,
    ) -> Result<(), String>;
}

impl<F> SlackSelectionStore for F
where
    F: Fn(&str, &str) -> Result<(), String> + Send + Sync + 'static,
{
    fn remember_slack_team_conversation(
        &self,
        team_id: &str,
        conversation_id: &str,
    ) -> Result<(), String> {
        self(team_id, conversation_id)
    }
}

#[derive(Clone)]
pub struct SlackWorkspaceRuntime {
    loader: SlackLiveWorkspaceLoader,
    cache: Arc<SlackWorkspaceCacheCell>,
    selection_store: Arc<dyn SlackSelectionStore>,
    startup_conversation_id: Arc<Mutex<Option<String>>>,
    shell_snapshot: Arc<Mutex<Option<SlackShellSnapshot>>>,
    confirmed_sends: Arc<Mutex<SlackConfirmedSendMap>>,
    media_proxy: SlackMediaProxy,
    realtime: Arc<SlackRealtimeHub>,
    presence_state: Arc<Mutex<SlackPresenceState>>,
    presence_roster_generation: Arc<AtomicU64>,
    presence_roster_commit: Arc<Mutex<()>>,
    app_realtime_claimed: Arc<Mutex<bool>>,
    presentation_realtime: tokio::sync::broadcast::Sender<crate::model::SlackRealtimeBatch>,
    presentation_presence_snapshot: tokio::sync::watch::Sender<SlackRealtimePresenceSnapshot>,
    presentation_realtime_closed: Arc<AtomicBool>,
    notification_policy: Arc<Mutex<SlackNotificationPolicy>>,
    notification_read_receipts:
        std::sync::mpsc::SyncSender<crate::live::SlackNotificationReadReceipt>,
}

impl SlackWorkspaceRuntime {
    pub(crate) fn new(
        loader: SlackLiveWorkspaceLoader,
        startup_conversation_id: Option<&str>,
        selection_store: impl SlackSelectionStore,
        notification_read_receipts: std::sync::mpsc::SyncSender<
            crate::live::SlackNotificationReadReceipt,
        >,
    ) -> Self {
        let media_proxy = SlackMediaProxy::new(loader.api_client());
        let realtime = Arc::new(SlackRealtimeHub::new(loader.api_client(), loader.team_id()));
        let (presentation_realtime, _) = tokio::sync::broadcast::channel(128);
        let (presentation_presence_snapshot, _) = tokio::sync::watch::channel(
            SlackRealtimePresenceSnapshot::empty(loader.team_id().to_string()),
        );
        Self {
            loader,
            cache: Arc::new(OnceLock::new()),
            selection_store: Arc::new(selection_store),
            startup_conversation_id: Arc::new(Mutex::new(
                startup_conversation_id.map(str::to_string),
            )),
            shell_snapshot: Arc::new(Mutex::new(None)),
            confirmed_sends: Arc::new(Mutex::new(SlackConfirmedSendMap::new())),
            media_proxy,
            realtime,
            presence_state: Arc::new(Mutex::new(SlackPresenceState::default())),
            presence_roster_generation: Arc::new(AtomicU64::new(0)),
            presence_roster_commit: Arc::new(Mutex::new(())),
            app_realtime_claimed: Arc::new(Mutex::new(false)),
            presentation_realtime,
            presentation_presence_snapshot,
            presentation_realtime_closed: Arc::new(AtomicBool::new(false)),
            notification_policy: Arc::new(Mutex::new(SlackNotificationPolicy::default())),
            notification_read_receipts,
        }
    }

    pub(crate) fn validate_authentication(&self) -> Result<(), String> {
        let authentication = self.loader.api_client().post("auth.test", &[])?;
        if authentication
            .get("team_id")
            .and_then(serde_json::Value::as_str)
            != Some(self.loader.team_id())
        {
            return Err(format!(
                "Slack auth.test did not confirm runtime team {}",
                self.loader.team_id()
            ));
        }
        Ok(())
    }

    pub(crate) fn require_realtime_running(&self) -> Result<(), String> {
        self.realtime.require_running()
    }

    pub(crate) fn retire_for_replacement(&self) -> Result<(), String> {
        let realtime_error = self.realtime.retire().err();
        self.publish_presentation_batch(crate::model::SlackRealtimeBatch::closed());
        match realtime_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(crate) fn resolve_startup_conversation_id(&self) -> Result<String, String> {
        let mut startup_conversation_id = self
            .startup_conversation_id
            .lock()
            .map_err(|_| "Slack startup conversation mutex poisoned".to_string())?;
        if let Some(conversation_id) = startup_conversation_id.as_ref() {
            return Ok(conversation_id.clone());
        }
        let conversation_id =
            crate::live::slack_auth::resolve_default_conversation_for_loader(&self.loader)?;
        self.selection_store
            .remember_slack_team_conversation(self.loader.team_id(), &conversation_id)?;
        *startup_conversation_id = Some(conversation_id.clone());
        Ok(conversation_id)
    }

    pub(crate) fn claim_app_realtime_subscription(
        &self,
    ) -> Result<Box<dyn crate::model::SlackRealtimeSubscription>, String> {
        let mut claimed = self
            .app_realtime_claimed
            .lock()
            .map_err(|_| "Slack app realtime ownership mutex poisoned".to_string())?;
        if *claimed {
            return Err(format!(
                "Slack app realtime subscription for team {} was claimed twice",
                self.loader.team_id()
            ));
        }
        let subscription = self.realtime.subscribe()?;
        *claimed = true;
        Ok(subscription)
    }
}
