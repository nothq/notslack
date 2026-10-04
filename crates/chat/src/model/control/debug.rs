use serde::{Deserialize, Serialize};

use super::{
    ChatAudioClipCaptureState, ChatComposerFileSummary, ChatConversationSummary,
    ChatFileCleanupSummary, ChatMediaAttachmentSummary, ChatMediaPlaybackState,
    ChatMessageRowTiming, ChatMessageTiming, ChatRemoteDraftFileCleanupSummary,
    ChatThreadPanelSummary, ChatVideoClipCaptureState,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatDebugState {
    pub team_id: String,
    pub conversation_id: String,
    pub presence_revision: Option<u64>,
    pub presence_known_user_count: usize,
    pub self_timezone_id: Option<String>,
    pub last_read_boundary_loaded: bool,
    pub recent_message_times: Vec<ChatMessageTiming>,
    pub recent_message_row_times: Vec<ChatMessageRowTiming>,
    pub composer_text: String,
    pub composer_focused: bool,
    pub composer_placeholder: String,
    pub composer_files: Vec<ChatComposerFileSummary>,
    pub audio_clip_capture: ChatAudioClipCaptureState,
    pub video_clip_capture: ChatVideoClipCaptureState,
    pub reaction_picker: ChatReactionPickerState,
    pub file_cleanups: Vec<ChatFileCleanupSummary>,
    pub remote_draft_file_cleanups: Vec<ChatRemoteDraftFileCleanupSummary>,
    pub conversations: Vec<ChatConversationSummary>,
    pub thread_panel: Option<ChatThreadPanelSummary>,
    pub media_attachments: Vec<ChatMediaAttachmentSummary>,
    pub media_playback: Option<ChatMediaPlaybackState>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatReactionPickerState {
    pub open: Option<ChatReactionPickerOpenState>,
    pub catalog: ChatReactionPickerCatalogState,
    pub skin_tone: ChatReactionPickerSkinToneState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatReactionPickerOpenState {
    pub target: ChatReactionPickerTarget,
    pub active_category: ChatReactionPickerCategory,
    pub query: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatReactionPickerTarget {
    pub team_id: String,
    pub conversation_id: String,
    pub message_id: String,
    pub thread_parent_message_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatReactionPickerCategory {
    Search,
    SmileysAndPeople,
    AnimalsAndNature,
    FoodAndDrink,
    TravelAndPlaces,
    Activities,
    Objects,
    Symbols,
    Flags,
    Custom,
}

impl TryFrom<usize> for ChatReactionPickerCategory {
    type Error = String;

    fn try_from(index: usize) -> Result<Self, Self::Error> {
        match index {
            0 => Ok(Self::Search),
            1 => Ok(Self::SmileysAndPeople),
            2 => Ok(Self::AnimalsAndNature),
            3 => Ok(Self::FoodAndDrink),
            4 => Ok(Self::TravelAndPlaces),
            5 => Ok(Self::Activities),
            6 => Ok(Self::Objects),
            7 => Ok(Self::Symbols),
            8 => Ok(Self::Flags),
            9 => Ok(Self::Custom),
            _ => Err(format!(
                "Slack reaction picker category index {index} is invalid"
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatReactionPickerCatalogState {
    pub request_pending: bool,
    pub installed: bool,
    pub frequent_count: usize,
    pub custom_count: usize,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatReactionPickerSkinToneState {
    pub loaded: bool,
    pub load_pending: bool,
    pub active: Option<ChatReactionPickerSkinTone>,
    pub pending: Option<ChatReactionPickerSkinTone>,
    pub menu_open: bool,
    pub menu_selection: Option<ChatReactionPickerSkinTone>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatReactionPickerSkinTone {
    Default,
    Light,
    MediumLight,
    Medium,
    MediumDark,
    Dark,
}

impl From<crate::model::SlackSkinTone> for ChatReactionPickerSkinTone {
    fn from(value: crate::model::SlackSkinTone) -> Self {
        match value {
            crate::model::SlackSkinTone::Default => Self::Default,
            crate::model::SlackSkinTone::Light => Self::Light,
            crate::model::SlackSkinTone::MediumLight => Self::MediumLight,
            crate::model::SlackSkinTone::Medium => Self::Medium,
            crate::model::SlackSkinTone::MediumDark => Self::MediumDark,
            crate::model::SlackSkinTone::Dark => Self::Dark,
        }
    }
}
