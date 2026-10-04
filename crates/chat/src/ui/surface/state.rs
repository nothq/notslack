use std::sync::Arc;

use crate::model::{SlackRemoteImagePrefetchRequest, SlackRichTextBroadcastRange};

#[cfg(test)]
use super::SlackMediaState;
use super::{
    build_slack_dm_rows, build_slack_message_chunks, build_slack_sidebar_rows, div,
    keystroke_input_text, prepare_slack_conversation_history_page,
    prepare_slack_conversation_snapshot, prepare_slack_dm_inbox_snapshot,
    prepare_slack_quick_search_snapshot, prepare_slack_search_snapshot,
    prepare_slack_shell_snapshot, prepare_slack_sidebar_snapshot,
    prepare_slack_thread_reply_receipt, prepare_slack_thread_snapshot, prepare_slack_workspace, px,
    AnyElement, ChatStartup, Context, Image, IntoElement, KeyDownEvent, ParentElement,
    PreparedSlackConversationHistoryPage, PreparedSlackConversationSnapshot,
    PreparedSlackDmInboxSnapshot, PreparedSlackQuickSearchSnapshot, PreparedSlackSearchSnapshot,
    PreparedSlackShellSnapshot, PreparedSlackSidebarSnapshot, PreparedSlackThreadReplyReceipt,
    PreparedSlackThreadSnapshot, PreparedSlackWorkspace, SlackActivationLoadProfile,
    SlackActivationStageFrame, SlackAuxPanelQueryBehavior, SlackAuxPanelRow,
    SlackAuxPanelRowAction, SlackAuxPanelSection, SlackAuxPanelState, SlackComposerFormatAction,
    SlackComposerLinkDialog, SlackComposerLinkEdit, SlackComposerLinkTarget, SlackComposerTarget,
    SlackConversationHistoryRequest, SlackConversationLoadProfile, SlackConversationLoadResult,
    SlackConversationLoadRoute, SlackFilesFilter, SlackInitialRefreshIdentity, SlackMainTab,
    SlackMentionInsertionMode, SlackMentionPickerState, SlackMessageRow, SlackProfilePanelState,
    SlackQuickSearchTarget, SlackRailView, SlackReactionRequest, SlackReplyComposerTarget,
    SlackSidebarRow, SlackSidebarRowKind, SlackSurfaceActivationTiming, SlackThreadPanelState,
    Styled, SurfaceState, Window, WorkspaceApi,
};

mod activity;
mod all_threads;
mod audio_clip;
mod aux_actions;
mod bookmark_folder;
mod channel_details;
mod composer;
mod composer_capture;
mod conversation_files;
mod data;
mod date_jump;
mod directory;
mod dm_finder;
mod draft_autosave;
mod draft_hydration;
mod drafts_sent;
mod emoji;
mod file_staging;
mod files;
mod history;
mod home_finder;
mod later;
mod live_conversation;
mod main_composer;
mod media;
mod members;
mod mention;
mod message_actions;
mod message_forward;
mod message_history;
mod message_mutation;
mod navigation;
mod new_message;
mod notifications;
mod panels_channel_shortcuts;
mod panels_details;
mod panels_history_help;
mod panels_workspace_members;
mod permalink;
mod pins;
mod presence;
mod profile;
mod query;
mod reaction_picker;
mod reactions;
mod realtime;
mod remote_draft_files;
mod schedule;
mod search;
mod selection;
mod self_settings;
mod sending;
mod sidebar_section;
mod slackbot;
mod thread;
mod thread_reply;
mod utils;
mod video_clip;
mod workspace;
mod workspace_dms;
mod workspace_embed;

pub use data::SurfaceStateData;
pub(crate) use date_jump::{
    SlackDateJumpCalendarCell, SlackDateJumpCalendarCellStatus, SlackDateJumpMenuAction,
    SlackDateJumpMenuState, SlackDateJumpMenuTarget, SlackDateJumpOverlay,
    SlackDateJumpPickerState, SlackDateJumpRequest,
};
pub(crate) use permalink::{SlackMessageNavigationHighlight, SlackMessageNavigationRequest};
pub(in crate::ui::surface) use presence::SlackPresenceAuthority;
pub(crate) use schedule::{
    SlackScheduleAnchor, SlackScheduleCalendarCell, SlackScheduleCalendarCellStatus,
    SlackScheduleCustomState, SlackScheduleDatePickerState, SlackScheduleMenuState,
    SlackScheduleNestedPicker, SlackScheduleOverlay, SlackScheduleOverlayState,
    SlackScheduleTimeOption, SlackScheduleTimePickerState,
};
type SlackSelfIdentity = (String, Option<String>, Option<String>, Option<String>);

#[derive(Clone, Debug, PartialEq, Eq)]
struct SlackVisiblePersonRow {
    user_id: String,
    label: String,
    accessory: Option<String>,
    detail: Option<String>,
}

const SLACK_COMMON_EMOJI_SHORTCODES: [&str; 36] = [
    "+1",
    "fire",
    "heart",
    "eyes",
    "joy",
    "raised_hands",
    "pray",
    "white_check_mark",
    "muscle",
    "rocket",
    "clap",
    "cry",
    "face_palm",
    "saluting_face",
    "star-struck",
    "confused",
    "face_vomiting",
    "open_mouth",
    "wave",
    "heart_eyes",
    "innocent",
    "partying_face",
    "rolling_on_the_floor_laughing",
    "smiling_face_with_tear",
    "grinning",
    "smiley",
    "smile",
    "grin",
    "laughing",
    "slightly_smiling_face",
    "upside_down_face",
    "melting_face",
    "wink",
    "blush",
    "smiling_face_with_3_hearts",
    "kissing_heart",
];

const SLACK_SPECIAL_MENTIONS: [(SlackRichTextBroadcastRange, &str); 3] = [
    (
        SlackRichTextBroadcastRange::Here,
        "Notify people who are currently active in this conversation.",
    ),
    (
        SlackRichTextBroadcastRange::Channel,
        "Notify everyone in the current channel.",
    ),
    (
        SlackRichTextBroadcastRange::Everyone,
        "Notify everyone in the entire workspace.",
    ),
];

const SLACK_CONVERSATION_LOAD_CONCURRENCY: usize = 2;
const SLACK_REMOTE_IMAGE_LOAD_CONCURRENCY: usize = 8;
const SLACK_REMOTE_IMAGE_PENDING_LIMIT: usize = 256;

pub(crate) use self::utils::*;
