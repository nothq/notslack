use std::collections::{HashMap, HashSet};

use crate::model::{
    SlackChannelNotificationMode, SlackChannelNotificationMutation,
    SlackChannelNotificationPreference, SlackRealtimeNotification, SlackSidebarSnapshot,
};

use super::SlackWorkspaceRuntime;

#[derive(Default)]
pub(super) struct SlackNotificationPolicy {
    self_user_id: Option<String>,
    notifications_paused: bool,
    muted_conversations: HashSet<String>,
    channel_preferences: HashMap<String, SlackChannelNotificationPreference>,
    badge_count: u32,
    initialized: bool,
}

impl SlackWorkspaceRuntime {
    pub(super) fn publish_notification_read_receipt(
        &self,
        receipt: crate::live::SlackNotificationReadReceipt,
    ) {
        if self.notification_read_receipts.send(receipt).is_err() {
            eprintln!(
                "Slack notification read receipt coordinator closed after a successful server mutation"
            );
        }
    }

    pub(crate) fn initialize_notification_policy(&self) -> Result<(), String> {
        let startup_conversation_id = self.resolve_startup_conversation_id()?;
        let (shell, sidebar) = std::thread::scope(|scope| {
            let shell = scope.spawn(|| self.load_and_cache_shell());
            let sidebar = scope.spawn(|| self.load_and_cache_sidebar(&startup_conversation_id));
            let shell = shell
                .join()
                .map_err(|_| "Slack notification shell policy load panicked".to_string())??;
            let sidebar = sidebar
                .join()
                .map_err(|_| "Slack notification sidebar policy load panicked".to_string())??;
            Ok::<_, String>((shell, sidebar))
        })?;
        self.apply_notification_policy(shell.self_user_id, &sidebar)
    }

    pub(crate) fn refresh_notification_policy(&self) -> Result<(), String> {
        let self_user_id = {
            let policy = self
                .notification_policy
                .lock()
                .map_err(|_| "Slack notification policy mutex poisoned".to_string())?;
            if !policy.initialized {
                drop(policy);
                return self.initialize_notification_policy();
            }
            policy.self_user_id.clone()
        };
        let startup_conversation_id = self.resolve_startup_conversation_id()?;
        let sidebar = self.refresh_sidebar_from_realtime(&startup_conversation_id)?;
        self.apply_notification_policy(self_user_id, &sidebar)
    }

    pub(crate) fn notification_badge_count(&self) -> Result<u32, String> {
        self.notification_policy
            .lock()
            .map_err(|_| "Slack notification policy mutex poisoned".to_string())
            .map(|policy| policy.badge_count)
    }

    pub(crate) fn should_deliver_notification(
        &self,
        notification: &SlackRealtimeNotification,
    ) -> Result<bool, String> {
        if notification.team_id.as_deref() != Some(self.loader.team_id()) {
            return Err(format!(
                "Slack realtime notification team did not match runtime {}",
                self.loader.team_id()
            ));
        }
        let cached_preference = {
            let policy = self
                .notification_policy
                .lock()
                .map_err(|_| "Slack notification policy mutex poisoned".to_string())?;
            if !policy.initialized {
                return Ok(false);
            }
            if policy.notifications_paused
                || policy.self_user_id.as_deref().is_some_and(|self_user_id| {
                    notification.sender_user_id.as_deref() == Some(self_user_id)
                })
                || policy
                    .muted_conversations
                    .contains(&notification.channel_id)
            {
                return Ok(false);
            }
            policy
                .channel_preferences
                .get(&notification.channel_id)
                .cloned()
        };
        let preference = match cached_preference {
            Some(preference) => preference,
            None => {
                self.load_and_cache_channel_notification_preference(&notification.channel_id)?
            }
        };
        let should_deliver =
            !preference.muted && preference.desktop_mode != SlackChannelNotificationMode::Nothing;
        Ok(should_deliver)
    }

    pub(super) fn load_and_cache_channel_notification_preference(
        &self,
        conversation_id: &str,
    ) -> Result<SlackChannelNotificationPreference, String> {
        let preference = self
            .loader
            .load_channel_notification_preference(conversation_id)?;
        self.cache_channel_notification_preference(conversation_id, preference)
    }

    pub(super) fn mutate_and_cache_channel_notification_preference(
        &self,
        conversation_id: &str,
        mutation: SlackChannelNotificationMutation,
    ) -> Result<SlackChannelNotificationPreference, String> {
        let preference = self
            .loader
            .mutate_channel_notification_preference(conversation_id, mutation)?;
        self.cache_channel_notification_preference(conversation_id, preference)
    }

    fn cache_channel_notification_preference(
        &self,
        conversation_id: &str,
        preference: SlackChannelNotificationPreference,
    ) -> Result<SlackChannelNotificationPreference, String> {
        if preference.team_id != self.loader.team_id()
            || preference.conversation_id != conversation_id
        {
            return Err(
                "Slack channel notification policy targeted another conversation".to_string(),
            );
        }
        let mut policy = self
            .notification_policy
            .lock()
            .map_err(|_| "Slack notification policy mutex poisoned".to_string())?;
        if policy.initialized {
            policy
                .channel_preferences
                .insert(conversation_id.to_string(), preference.clone());
        }
        Ok(preference)
    }

    fn apply_notification_policy(
        &self,
        self_user_id: Option<String>,
        sidebar: &SlackSidebarSnapshot,
    ) -> Result<(), String> {
        if sidebar.team_id != self.loader.team_id() {
            return Err("Slack notification sidebar policy targeted another team".to_string());
        }
        let muted_conversations = sidebar
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .filter(|item| item.muted)
            .map(|item| item.target_id.clone())
            .collect();
        let mut policy = self
            .notification_policy
            .lock()
            .map_err(|_| "Slack notification policy mutex poisoned".to_string())?;
        *policy = SlackNotificationPolicy {
            self_user_id,
            notifications_paused: sidebar.rail_badges.self_notifications_paused,
            muted_conversations,
            channel_preferences: HashMap::new(),
            badge_count: sidebar.rail_badges.activity.unwrap_or_default(),
            initialized: true,
        };
        Ok(())
    }
}
