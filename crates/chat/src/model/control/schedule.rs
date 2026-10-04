use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatScheduleState {
    pub pending_create_or_update: Option<ChatSchedulePendingMutation>,
    pub pending_phase: Option<ChatSchedulePendingPhase>,
    pub pending_delete_draft_id: Option<String>,
    pub conversation_recovery: Option<ChatScheduleConversationRecovery>,
    pub edit: Option<ChatScheduleEditState>,
    pub submission_error: Option<String>,
    pub scheduled_list_error: Option<String>,
    pub items: Vec<ChatScheduledItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatScheduleConversationRecovery {
    pub team_id: String,
    pub self_user_id: String,
    pub conversation_id: String,
    pub file_ids: Vec<String>,
    pub failure_diagnostic: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChatSchedulePendingPhase {
    Submitting,
    ReconcilingUnknown { attempt: u8, diagnostic: String },
    ProtectedUnknown { diagnostic: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChatSchedulePendingMutation {
    Create {
        conversation_id: String,
        file_ids: Vec<String>,
        post_at_unix_seconds: i64,
    },
    Update {
        conversation_id: String,
        draft_id: String,
        file_ids: Vec<String>,
        post_at_unix_seconds: i64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ChatScheduleEditState {
    PendingNavigation {
        conversation_id: String,
        draft_id: String,
        failure_diagnostic: Option<String>,
    },
    Active {
        conversation_id: String,
        draft_id: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatScheduledItem {
    pub draft_id: String,
    pub revision: String,
    pub conversation_id: String,
    pub body: String,
    pub file_ids: Vec<String>,
    pub post_at_unix_seconds: i64,
}
