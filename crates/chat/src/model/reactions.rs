use crate::model::SlackMessageTimestamp;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SlackReactionTarget {
    ConversationMessage {
        conversation_id: String,
        message_timestamp: SlackMessageTimestamp,
    },
    ThreadReply {
        conversation_id: String,
        thread_timestamp: SlackMessageTimestamp,
        message_timestamp: SlackMessageTimestamp,
    },
}

impl SlackReactionTarget {
    pub fn conversation_message(
        conversation_id: String,
        message_timestamp: SlackMessageTimestamp,
    ) -> Result<Self, String> {
        require_conversation_id(&conversation_id)?;
        Ok(Self::ConversationMessage {
            conversation_id,
            message_timestamp,
        })
    }

    pub fn thread_reply(
        conversation_id: String,
        thread_timestamp: SlackMessageTimestamp,
        message_timestamp: SlackMessageTimestamp,
    ) -> Result<Self, String> {
        require_conversation_id(&conversation_id)?;
        if thread_timestamp == message_timestamp {
            return Err(
                "Slack thread-reply reaction target must differ from its thread timestamp"
                    .to_string(),
            );
        }
        Ok(Self::ThreadReply {
            conversation_id,
            thread_timestamp,
            message_timestamp,
        })
    }

    pub fn conversation_id(&self) -> &str {
        match self {
            Self::ConversationMessage {
                conversation_id, ..
            }
            | Self::ThreadReply {
                conversation_id, ..
            } => conversation_id,
        }
    }

    pub fn message_timestamp(&self) -> &SlackMessageTimestamp {
        match self {
            Self::ConversationMessage {
                message_timestamp, ..
            }
            | Self::ThreadReply {
                message_timestamp, ..
            } => message_timestamp,
        }
    }

    pub fn thread_timestamp(&self) -> Option<&SlackMessageTimestamp> {
        match self {
            Self::ConversationMessage { .. } => None,
            Self::ThreadReply {
                thread_timestamp, ..
            } => Some(thread_timestamp),
        }
    }

    pub fn is_thread_reply(&self) -> bool {
        matches!(self, Self::ThreadReply { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackReactionMutationReceipt<ConversationSnapshot, ThreadSnapshot> {
    Conversation {
        snapshot: ConversationSnapshot,
        scope: SlackReactionConversationScope,
    },
    Thread(ThreadSnapshot),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackReactionConversationScope {
    Latest,
    TargetedWindow,
}

fn require_conversation_id(conversation_id: &str) -> Result<(), String> {
    if conversation_id.trim().is_empty() {
        return Err("Slack reaction conversation id must not be empty".to_string());
    }
    Ok(())
}
