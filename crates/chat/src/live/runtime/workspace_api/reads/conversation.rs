use super::super::SlackWorkspaceRuntime;
use crate::model::{SlackConversationReadReceipt, SlackMessageTimestamp};

impl SlackWorkspaceRuntime {
    pub(in crate::live::runtime::workspace_api) fn mark_slack_conversation_read_and_publish(
        &self,
        conversation_id: &str,
        message_timestamp: &SlackMessageTimestamp,
    ) -> Result<SlackConversationReadReceipt, String> {
        self.loader
            .mark_conversation_read(conversation_id, message_timestamp)?;
        let receipt = SlackConversationReadReceipt {
            team_id: self.loader.team_id().to_string(),
            conversation_id: conversation_id.to_string(),
            last_read: crate::model::SlackLastReadTimestamp::parse(message_timestamp.as_str())
                .expect("typed Slack message timestamp must be a valid last_read timestamp"),
        };
        if let Some(cache) = self.cache() {
            if let Err(error) = cache.persist_conversation_read_receipt(&receipt) {
                eprintln!("Slack read receipt cache write failed: {error}");
            }
        }
        self.publish_notification_read_receipt(
            crate::live::SlackNotificationReadReceipt::Conversation(receipt.clone()),
        );
        Ok(receipt)
    }
}
