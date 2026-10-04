mod activity;
mod all_threads;
mod attachments;
mod bookmark_folder;
mod capabilities;
mod channel_details;
mod connection;
mod control;
mod conversation_files;
mod drafts;
mod drafts_sent;
mod file_metadata;
mod file_shares;
mod file_staging;
mod file_uploads;
mod files;
mod huddle;
mod identifiers;
mod later;
mod launch;
mod media;
mod members;
mod message_draft;
mod mutations;
mod new_message;
mod notifications;
mod pins;
mod quick_search;
mod reaction_catalog;
mod reactions;
mod realtime;
mod search;
mod self_settings;
mod sidebar_section;
mod skin_tone;
mod types;
mod upload;
mod workspace_api;

use serde::{Deserialize, Serialize};

mod exports;

pub use exports::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatWorkspace {
    pub title: String,
    pub threads: Vec<ChatThreadSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatThreadSummary {
    pub id: String,
    pub title: String,
    pub unread_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatMessageRole {
    User,
    Assistant,
    System,
}
