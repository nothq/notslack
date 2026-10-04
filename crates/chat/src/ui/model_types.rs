use crate::model::{ChatThreadSummary, SlackProfile};

pub type MediaCaptureApi = dyn media_capture::MediaCaptureService;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatThreadRow {
    pub id: String,
    pub title: String,
    pub unread_badge: Option<String>,
}

pub fn chat_thread_rows(threads: &[ChatThreadSummary]) -> Vec<ChatThreadRow> {
    threads
        .iter()
        .map(|thread| ChatThreadRow {
            id: thread.id.clone(),
            title: thread.title.clone(),
            unread_badge: (thread.unread_count > 0).then(|| thread.unread_count.to_string()),
        })
        .collect()
}

#[derive(Clone)]
pub enum SlackProfilePanelState {
    Loading {
        user_id: String,
        label: String,
    },
    Loaded(Box<SlackProfile>),
    Error {
        user_id: String,
        label: String,
        message: String,
    },
}

impl SlackProfilePanelState {
    pub fn user_id(&self) -> &str {
        match self {
            Self::Loading { user_id, .. } => user_id,
            Self::Loaded(profile) => &profile.user_id,
            Self::Error { user_id, .. } => user_id,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::Loading { label, .. } => label,
            Self::Loaded(profile) => &profile.display_name,
            Self::Error { label, .. } => label,
        }
    }
}
