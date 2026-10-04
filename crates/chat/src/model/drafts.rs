use std::collections::HashSet;

use crate::model::{
    SlackDraftClientMutationTimestamp, SlackDraftDestination, SlackDraftTarget, SlackFileId,
    SlackFileStagingOperationId, SlackMessageClientId, SlackMessageTimestamp,
};

pub enum SlackDraftContent<'a, MessageDraft> {
    Message(&'a MessageDraft),
    FilesOnly,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlackScheduledDraftMutationFailure {
    NotSent { diagnostic: String },
    Rejected { diagnostic: String },
    Unknown { diagnostic: String },
}

impl SlackScheduledDraftMutationFailure {
    pub fn diagnostic(&self) -> &str {
        match self {
            Self::NotSent { diagnostic }
            | Self::Rejected { diagnostic }
            | Self::Unknown { diagnostic } => diagnostic,
        }
    }

    pub fn is_definite(&self) -> bool {
        matches!(self, Self::NotSent { .. } | Self::Rejected { .. })
    }
}

impl std::fmt::Display for SlackScheduledDraftMutationFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.diagnostic())
    }
}

impl std::error::Error for SlackScheduledDraftMutationFailure {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlackScheduledDraftLocalFile {
    operation_id: SlackFileStagingOperationId,
    file_id: SlackFileId,
}

impl SlackScheduledDraftLocalFile {
    pub fn new(operation_id: SlackFileStagingOperationId, file_id: SlackFileId) -> Self {
        Self {
            operation_id,
            file_id,
        }
    }

    pub fn operation_id(&self) -> &SlackFileStagingOperationId {
        &self.operation_id
    }

    pub fn file_id(&self) -> &SlackFileId {
        &self.file_id
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackDraftFileDeletion {
    Delete,
    Preserve,
}

impl SlackDraftFileDeletion {
    pub fn preserves_files(self) -> bool {
        matches!(self, Self::Preserve)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SlackDraftUpdateTarget<'a> {
    target: &'a SlackDraftTarget,
    client_mutation_timestamp: &'a SlackDraftClientMutationTimestamp,
}

impl<'a> SlackDraftUpdateTarget<'a> {
    pub fn new(
        target: &'a SlackDraftTarget,
        client_mutation_timestamp: &'a SlackDraftClientMutationTimestamp,
    ) -> Self {
        Self {
            target,
            client_mutation_timestamp,
        }
    }

    pub fn target(self) -> &'a SlackDraftTarget {
        self.target
    }

    pub fn client_mutation_timestamp(self) -> &'a SlackDraftClientMutationTimestamp {
        self.client_mutation_timestamp
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SlackScheduledDraftCreateTarget<'a> {
    client_message_id: &'a SlackMessageClientId,
    write_target: &'a SlackDraftWriteTarget,
    post_at_unix_seconds: i64,
}

impl<'a> SlackScheduledDraftCreateTarget<'a> {
    pub fn new(
        client_message_id: &'a SlackMessageClientId,
        write_target: &'a SlackDraftWriteTarget,
        post_at_unix_seconds: i64,
    ) -> Self {
        Self {
            client_message_id,
            write_target,
            post_at_unix_seconds,
        }
    }

    pub fn client_message_id(self) -> &'a SlackMessageClientId {
        self.client_message_id
    }

    pub fn write_target(self) -> &'a SlackDraftWriteTarget {
        self.write_target
    }

    pub fn post_at_unix_seconds(self) -> i64 {
        self.post_at_unix_seconds
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SlackScheduledDraftUpdateTarget<'a> {
    update_target: SlackDraftUpdateTarget<'a>,
    write_targets: &'a [SlackDraftWriteTarget],
    post_at_unix_seconds: i64,
}

impl<'a> SlackScheduledDraftUpdateTarget<'a> {
    pub fn new(
        update_target: SlackDraftUpdateTarget<'a>,
        write_targets: &'a [SlackDraftWriteTarget],
        post_at_unix_seconds: i64,
    ) -> Self {
        Self {
            update_target,
            write_targets,
            post_at_unix_seconds,
        }
    }

    pub fn update_target(self) -> SlackDraftUpdateTarget<'a> {
        self.update_target
    }

    pub fn write_targets(self) -> &'a [SlackDraftWriteTarget] {
        self.write_targets
    }

    pub fn post_at_unix_seconds(self) -> i64 {
        self.post_at_unix_seconds
    }
}

#[derive(Clone, Copy, Debug)]
pub enum SlackScheduledDraftReconcileTarget<'a> {
    Create(SlackScheduledDraftCreateTarget<'a>),
    Update(SlackScheduledDraftUpdateTarget<'a>),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlackDraftWriteTarget {
    destination: SlackDraftWriteDestination,
    user_ids: Vec<String>,
    message_timestamp: Option<SlackMessageTimestamp>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum SlackDraftWriteDestination {
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
pub enum SlackDraftWriteTargetView<'a> {
    Conversation {
        conversation_id: &'a str,
        user_ids: &'a [String],
        message_timestamp: Option<&'a SlackMessageTimestamp>,
    },
    Thread {
        conversation_id: &'a str,
        user_ids: &'a [String],
        thread_timestamp: &'a SlackMessageTimestamp,
        message_timestamp: Option<&'a SlackMessageTimestamp>,
        broadcast: bool,
    },
}

impl SlackDraftWriteTarget {
    pub fn conversation(conversation_id: String) -> Result<Self, String> {
        require_identifier("conversation id", &conversation_id)?;
        Ok(Self {
            destination: SlackDraftWriteDestination::Conversation { conversation_id },
            user_ids: Vec::new(),
            message_timestamp: None,
        })
    }

    pub fn thread(
        conversation_id: String,
        thread_timestamp: SlackMessageTimestamp,
        broadcast: bool,
    ) -> Result<Self, String> {
        require_identifier("conversation id", &conversation_id)?;
        Ok(Self {
            destination: SlackDraftWriteDestination::Thread {
                conversation_id,
                thread_timestamp,
                broadcast,
            },
            user_ids: Vec::new(),
            message_timestamp: None,
        })
    }

    pub fn from_loaded(destination: &SlackDraftDestination) -> Result<Self, String> {
        require_identifier("conversation id", &destination.conversation_id)?;
        validate_user_ids(&destination.user_ids)?;
        let message_timestamp = destination
            .message_timestamp
            .as_deref()
            .map(SlackMessageTimestamp::parse)
            .transpose()?;
        let write_destination = match destination.thread_timestamp.as_deref() {
            Some(thread_timestamp) => SlackDraftWriteDestination::Thread {
                conversation_id: destination.conversation_id.clone(),
                thread_timestamp: SlackMessageTimestamp::parse(thread_timestamp)?,
                broadcast: destination.broadcast,
            },
            None if destination.broadcast => {
                return Err(
                    "Slack draft destination cannot broadcast without a thread timestamp"
                        .to_string(),
                );
            }
            None => SlackDraftWriteDestination::Conversation {
                conversation_id: destination.conversation_id.clone(),
            },
        };
        Ok(Self {
            destination: write_destination,
            user_ids: destination.user_ids.clone(),
            message_timestamp,
        })
    }

    pub fn conversation_id(&self) -> &str {
        match &self.destination {
            SlackDraftWriteDestination::Conversation { conversation_id }
            | SlackDraftWriteDestination::Thread {
                conversation_id, ..
            } => conversation_id,
        }
    }

    pub fn user_ids(&self) -> &[String] {
        &self.user_ids
    }

    pub fn message_timestamp(&self) -> Option<&SlackMessageTimestamp> {
        self.message_timestamp.as_ref()
    }

    pub fn is_thread(&self) -> bool {
        matches!(&self.destination, SlackDraftWriteDestination::Thread { .. })
    }

    pub fn broadcast(&self) -> bool {
        match &self.destination {
            SlackDraftWriteDestination::Conversation { .. } => false,
            SlackDraftWriteDestination::Thread { broadcast, .. } => *broadcast,
        }
    }

    pub fn with_broadcast(&self, broadcast: bool) -> Result<Self, String> {
        let mut target = self.clone();
        match &mut target.destination {
            SlackDraftWriteDestination::Conversation { .. } if broadcast => Err(
                "Slack draft destination cannot broadcast without a thread timestamp".to_string(),
            ),
            SlackDraftWriteDestination::Conversation { .. } => Ok(target),
            SlackDraftWriteDestination::Thread {
                broadcast: current, ..
            } => {
                *current = broadcast;
                Ok(target)
            }
        }
    }

    pub fn view(&self) -> SlackDraftWriteTargetView<'_> {
        match &self.destination {
            SlackDraftWriteDestination::Conversation { conversation_id } => {
                SlackDraftWriteTargetView::Conversation {
                    conversation_id,
                    user_ids: &self.user_ids,
                    message_timestamp: self.message_timestamp.as_ref(),
                }
            }
            SlackDraftWriteDestination::Thread {
                conversation_id,
                thread_timestamp,
                broadcast,
            } => SlackDraftWriteTargetView::Thread {
                conversation_id,
                user_ids: &self.user_ids,
                thread_timestamp,
                message_timestamp: self.message_timestamp.as_ref(),
                broadcast: *broadcast,
            },
        }
    }
}

fn validate_user_ids(user_ids: &[String]) -> Result<(), String> {
    let mut unique_user_ids = HashSet::with_capacity(user_ids.len());
    for user_id in user_ids {
        require_identifier("destination user id", user_id)?;
        if !unique_user_ids.insert(user_id.as_str()) {
            return Err(format!(
                "Slack draft destination contains duplicate user id {user_id}"
            ));
        }
    }
    Ok(())
}

fn require_identifier(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.trim() != value
        || value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(format!(
            "Slack draft {field} must be non-empty and contain no whitespace or control characters"
        ));
    }
    Ok(())
}
