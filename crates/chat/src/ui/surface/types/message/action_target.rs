use std::sync::Arc;

use crate::ui::{SlackMessageTimestamp, SlackReactionTarget};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct SlackMessageActionTarget {
    team_id: String,
    reaction: SlackReactionTarget,
}

impl SlackMessageActionTarget {
    pub(crate) fn conversation_message(
        team_id: &str,
        conversation_id: &str,
        message_id: &str,
    ) -> Option<Arc<Self>> {
        if team_id.trim().is_empty() {
            return None;
        }
        let message_timestamp = SlackMessageTimestamp::parse(message_id).ok()?;
        let reaction = SlackReactionTarget::conversation_message(
            conversation_id.to_string(),
            message_timestamp,
        )
        .ok()?;
        Some(Arc::new(Self {
            team_id: team_id.to_string(),
            reaction,
        }))
    }

    pub(crate) fn thread_reply(
        team_id: &str,
        conversation_id: &str,
        thread_timestamp: &str,
        message_id: &str,
    ) -> Option<Arc<Self>> {
        if team_id.trim().is_empty() {
            return None;
        }
        let thread_timestamp = SlackMessageTimestamp::parse(thread_timestamp).ok()?;
        let message_timestamp = SlackMessageTimestamp::parse(message_id).ok()?;
        let reaction = SlackReactionTarget::thread_reply(
            conversation_id.to_string(),
            thread_timestamp,
            message_timestamp,
        )
        .ok()?;
        Some(Arc::new(Self {
            team_id: team_id.to_string(),
            reaction,
        }))
    }

    pub(crate) fn team_id(&self) -> &str {
        &self.team_id
    }

    pub(crate) fn reaction(&self) -> &SlackReactionTarget {
        &self.reaction
    }

    pub(crate) fn conversation_id(&self) -> &str {
        self.reaction.conversation_id()
    }

    pub(crate) fn message_timestamp(&self) -> &SlackMessageTimestamp {
        self.reaction.message_timestamp()
    }

    pub(crate) fn thread_timestamp(&self) -> Option<&SlackMessageTimestamp> {
        self.reaction.thread_timestamp()
    }

    pub(crate) fn root_timestamp(&self) -> &SlackMessageTimestamp {
        self.thread_timestamp()
            .unwrap_or_else(|| self.message_timestamp())
    }

    pub(crate) fn is_thread_reply(&self) -> bool {
        self.reaction.is_thread_reply()
    }
}
