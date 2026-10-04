use std::collections::HashSet;

use crate::model::{SlackFileId, SlackMessageClientId, SlackMessageTimestamp};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlackFileShareTarget {
    destination: SlackFileShareDestination,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum SlackFileShareDestination {
    Conversation {
        conversation_id: String,
    },
    Thread {
        conversation_id: String,
        thread_timestamp: SlackMessageTimestamp,
        broadcast: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackFileShareTargetView<'a> {
    Conversation {
        conversation_id: &'a str,
    },
    Thread {
        conversation_id: &'a str,
        thread_timestamp: &'a SlackMessageTimestamp,
        broadcast: bool,
    },
}

impl SlackFileShareTarget {
    pub fn conversation(conversation_id: String) -> Result<Self, String> {
        require_conversation_id(&conversation_id)?;
        Ok(Self {
            destination: SlackFileShareDestination::Conversation { conversation_id },
        })
    }

    pub fn thread(
        conversation_id: String,
        thread_timestamp: SlackMessageTimestamp,
        broadcast: bool,
    ) -> Result<Self, String> {
        require_conversation_id(&conversation_id)?;
        Ok(Self {
            destination: SlackFileShareDestination::Thread {
                conversation_id,
                thread_timestamp,
                broadcast,
            },
        })
    }

    pub fn conversation_id(&self) -> &str {
        match &self.destination {
            SlackFileShareDestination::Conversation { conversation_id }
            | SlackFileShareDestination::Thread {
                conversation_id, ..
            } => conversation_id,
        }
    }

    pub fn view(&self) -> SlackFileShareTargetView<'_> {
        match &self.destination {
            SlackFileShareDestination::Conversation { conversation_id } => {
                SlackFileShareTargetView::Conversation { conversation_id }
            }
            SlackFileShareDestination::Thread {
                conversation_id,
                thread_timestamp,
                broadcast,
            } => SlackFileShareTargetView::Thread {
                conversation_id,
                thread_timestamp,
                broadcast: *broadcast,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackFileShareRequest<MessageDraft> {
    target: SlackFileShareTarget,
    file_ids: Vec<SlackFileId>,
    client_message_id: SlackMessageClientId,
    content: SlackFileShareContent<MessageDraft>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SlackFileShareContent<MessageDraft> {
    Message(MessageDraft),
    FilesOnly,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackFileShareContentView<'a, MessageDraft> {
    Message(&'a MessageDraft),
    FilesOnly,
}

impl<MessageDraft> SlackFileShareRequest<MessageDraft> {
    pub fn message(
        target: SlackFileShareTarget,
        file_ids: Vec<SlackFileId>,
        client_message_id: SlackMessageClientId,
        message: MessageDraft,
    ) -> Result<Self, String> {
        Self::new(
            target,
            file_ids,
            client_message_id,
            SlackFileShareContent::Message(message),
        )
    }

    pub fn files_only(
        target: SlackFileShareTarget,
        file_ids: Vec<SlackFileId>,
        client_message_id: SlackMessageClientId,
    ) -> Result<Self, String> {
        Self::new(
            target,
            file_ids,
            client_message_id,
            SlackFileShareContent::FilesOnly,
        )
    }

    pub fn target(&self) -> &SlackFileShareTarget {
        &self.target
    }

    pub fn file_ids(&self) -> &[SlackFileId] {
        &self.file_ids
    }

    pub fn client_message_id(&self) -> &SlackMessageClientId {
        &self.client_message_id
    }

    pub fn content(&self) -> SlackFileShareContentView<'_, MessageDraft> {
        match &self.content {
            SlackFileShareContent::Message(message) => SlackFileShareContentView::Message(message),
            SlackFileShareContent::FilesOnly => SlackFileShareContentView::FilesOnly,
        }
    }

    fn new(
        target: SlackFileShareTarget,
        file_ids: Vec<SlackFileId>,
        client_message_id: SlackMessageClientId,
        content: SlackFileShareContent<MessageDraft>,
    ) -> Result<Self, String> {
        require_unique_file_ids(&file_ids)?;
        Ok(Self {
            target,
            file_ids,
            client_message_id,
            content,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackFileShareReceipt<ConversationSnapshot, ThreadSnapshot> {
    Conversation(ConversationSnapshot),
    Thread(ThreadSnapshot),
}

fn require_unique_file_ids(file_ids: &[SlackFileId]) -> Result<(), String> {
    if file_ids.is_empty() {
        return Err("Slack file share must contain at least one file id".to_string());
    }
    let mut unique_file_ids = HashSet::with_capacity(file_ids.len());
    for file_id in file_ids {
        if !unique_file_ids.insert(file_id) {
            return Err(format!(
                "Slack file share contains duplicate file id {}",
                file_id.as_str()
            ));
        }
    }
    Ok(())
}

fn require_conversation_id(conversation_id: &str) -> Result<(), String> {
    if conversation_id.is_empty()
        || conversation_id.trim() != conversation_id
        || conversation_id
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(
            "Slack file-share conversation id must be non-empty and contain no whitespace or control characters"
                .to_string(),
        );
    }
    Ok(())
}
