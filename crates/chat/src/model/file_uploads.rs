use crate::model::SlackMessageTimestamp;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlackFileUploadTarget {
    destination: SlackFileUploadDestination,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum SlackFileUploadDestination {
    Conversation {
        conversation_id: String,
    },
    Thread {
        conversation_id: String,
        thread_timestamp: SlackMessageTimestamp,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackFileUploadTargetView<'a> {
    Conversation {
        conversation_id: &'a str,
    },
    Thread {
        conversation_id: &'a str,
        thread_timestamp: &'a SlackMessageTimestamp,
    },
}

impl SlackFileUploadTarget {
    pub fn conversation(conversation_id: String) -> Result<Self, String> {
        require_conversation_id(&conversation_id)?;
        Ok(Self {
            destination: SlackFileUploadDestination::Conversation { conversation_id },
        })
    }

    pub fn thread(
        conversation_id: String,
        thread_timestamp: SlackMessageTimestamp,
    ) -> Result<Self, String> {
        require_conversation_id(&conversation_id)?;
        Ok(Self {
            destination: SlackFileUploadDestination::Thread {
                conversation_id,
                thread_timestamp,
            },
        })
    }

    pub fn conversation_id(&self) -> &str {
        match &self.destination {
            SlackFileUploadDestination::Conversation { conversation_id }
            | SlackFileUploadDestination::Thread {
                conversation_id, ..
            } => conversation_id,
        }
    }

    pub fn view(&self) -> SlackFileUploadTargetView<'_> {
        match &self.destination {
            SlackFileUploadDestination::Conversation { conversation_id } => {
                SlackFileUploadTargetView::Conversation { conversation_id }
            }
            SlackFileUploadDestination::Thread {
                conversation_id,
                thread_timestamp,
            } => SlackFileUploadTargetView::Thread {
                conversation_id,
                thread_timestamp,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackFileUploadReceipt<ConversationSnapshot, ThreadSnapshot> {
    Conversation(ConversationSnapshot),
    Thread(ThreadSnapshot),
}

fn require_conversation_id(conversation_id: &str) -> Result<(), String> {
    if conversation_id.is_empty()
        || conversation_id.trim() != conversation_id
        || conversation_id
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(
            "Slack file-upload conversation id must be non-empty and contain no whitespace or control characters"
                .to_string(),
        );
    }
    Ok(())
}
