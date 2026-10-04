use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatSearchState {
    pub query: String,
    pub open: bool,
    pub loading: bool,
    pub total: u32,
    pub error: Option<String>,
    pub results: Vec<ChatSearchResultSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatSearchResultSummary {
    pub result_id: String,
    pub conversation_id: String,
    pub message_id: String,
    pub thread_root_id: Option<String>,
    pub conversation_label: String,
    pub author: String,
    pub timestamp_label: String,
    pub body_preview: String,
    pub attachment_titles: Vec<String>,
    pub reactions: Vec<ChatReactionSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatActivityState {
    pub active_rail_view: ChatRailView,
    pub activity_count: Option<u32>,
    pub filter: ChatActivityFilter,
    pub unread_only: bool,
    pub loading: bool,
    pub load_error: Option<String>,
    pub read_pending: bool,
    pub pending_item_keys: Vec<String>,
    pub read_error: Option<String>,
    pub selected_item_key: Option<String>,
    pub rows: Vec<ChatActivityRowSummary>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRailView {
    Home,
    Dms,
    Activity,
    Files,
    Later,
    DraftsSent,
    More,
    Admin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatActivityFilter {
    All,
    Dms,
    Mentions,
    Threads,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatActivityRowSummary {
    pub item_key: String,
    pub unread: bool,
    pub selected: bool,
    pub read_pending: bool,
    pub read_target_kind: Option<ChatActivityReadTargetKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatActivityReadTargetKind {
    ThreadV2,
    AtUser,
    Dm,
    BotDmBundle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatMessageReactionState {
    pub message_id: String,
    pub pending: bool,
    pub reactions: Vec<ChatReactionSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatReactionSummary {
    pub emoji: String,
    pub count: u32,
    pub current_user_active: bool,
}
