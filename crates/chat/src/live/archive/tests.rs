use crate::live::archive::*;
use crate::live::{append_archived_slack_message, archived_workspace_for_conversation};
use crate::model::SlackConversationKind;

mod fixtures;

use fixtures::*;

#[gpui::test]
fn normalize_slack_archive_fills_missing_ids_and_kinds() {
    let workspace = normalize_slack_archive(unnormalized_archive_workspace());

    assert_eq!(workspace.team_id, "T_ARCHIVE");
    assert_eq!(workspace.conversation_id, "C_ARCHIVE_AI_CRAZE");
    assert_eq!(workspace.channel_kind, SlackConversationKind::Channel);
    assert_eq!(workspace.composer_placeholder, "Message #design");
    assert_eq!(workspace.sections[0].items[0].target_id, "C_ARCHIVE_RANDOM");
    assert_eq!(
        workspace.sections[0].items[0].target_kind,
        SlackConversationKind::Channel
    );
    assert_eq!(workspace.sections[0].label, "Channels");
    assert_eq!(workspace.sections.len(), 1);
    assert_eq!(workspace.sections[0].label, "Channels");
    assert_eq!(
        workspace.sections[0].items[1].target_id,
        "M_ARCHIVE_MPDM_ADA__GRACE_1"
    );
    assert_eq!(
        workspace.sections[0].items[1].target_kind,
        SlackConversationKind::GroupMessage
    );
    assert_eq!(workspace.sections[0].items[1].label, "Ada, Grace");
    assert_eq!(
        workspace.sections[0].items[1].icon.as_deref(),
        Some("person")
    );
}

fn unnormalized_archive_workspace() -> SlackWorkspace {
    SlackWorkspace {
        team_id: String::new(),
        conversation_id: String::new(),
        channel_kind: SlackConversationKind::Unknown,
        workspace_name: "Acme".to_string(),
        workspace_logo_url: None,
        workspace_logo_image_base64: None,
        workspace_logo_image_mimetype: None,
        self_user_id: None,
        self_display_name: None,
        self_avatar_label: None,
        self_avatar_image_url: None,
        self_avatar_image_base64: None,
        self_avatar_image_mimetype: None,
        self_timezone_id: None,
        self_timezone_label: None,
        channel_name: "design".to_string(),
        channel_topic: String::new(),
        member_count: Some(8),
        tabs: Vec::new(),
        sections: vec![slack_sidebar_section(
            "Channels",
            vec![
                slack_sidebar_item("random", Some("hash")),
                slack_sidebar_item("mpdm-ada--grace-1", Some("hash")),
            ],
        )],
        direct_message_unread_states: Vec::new(),
        rail_badges: Default::default(),
        messages: Vec::new(),
        last_read: None,
        last_read_boundary_loaded: true,
        history_next_cursor: None,
        mention_suggestions: Vec::new(),
        emoji_picker_sections: Vec::new(),
        composer_notice: None,
        peer_notifications_paused: false,
        dm_peer_local_time_context: None,
        composer_draft_text: None,
        composer_placeholder: String::new(),
    }
}

#[gpui::test]
fn normalize_slack_archive_marks_matching_sidebar_item_active() {
    let workspace = normalize_slack_archive(sidebar_activity_workspace());

    assert!(workspace.sections[0].items[0].active);
    assert!(!workspace.sections[0].items[1].active);
}

#[gpui::test]
fn load_and_save_slack_archive_support_gzip() {
    let temp = tempfile::tempdir().expect("tempdir should exist");
    let path = temp.path().join("slack-workspace.json.gz");
    let workspace = sample_workspace();

    save_json_pretty(&path, &workspace).expect("gzip slack archive should write");

    let loaded = load_slack_archive(&path).expect("gzip slack archive should load");
    assert_eq!(loaded.workspace_name, workspace.workspace_name);
    assert_eq!(loaded.channel_name, workspace.channel_name);
}

#[gpui::test]
fn archived_workspace_for_conversation_switches_to_selected_sidebar_item() {
    let archived = archived_runtime_test_workspace();

    let workspace = archived_workspace_for_conversation(&archived, "C_ARCHIVE_RANDOM")
        .expect("expected archived conversation to load");

    assert_eq!(workspace.conversation_id, "C_ARCHIVE_RANDOM");
    assert_eq!(workspace.channel_name, "random");
    assert_eq!(workspace.channel_kind, SlackConversationKind::Channel);
    assert_eq!(workspace.composer_placeholder, "Message #random");
    assert!(workspace.messages.is_empty());
    assert!(workspace.sections[0].items[0].active);
    assert!(!workspace.sections[0].items[1].active);
}

#[gpui::test]
fn append_archived_slack_message_adds_local_message() {
    let workspace = append_archived_slack_message(
        archived_runtime_test_workspace(),
        "archive hello",
        Vec::new(),
    );

    assert_eq!(
        workspace
            .messages
            .last()
            .map(|message| message.body.as_str()),
        Some("archive hello")
    );
    assert_eq!(
        workspace
            .messages
            .last()
            .and_then(|message| message.user_id.as_deref()),
        Some("U_ARCHIVE_SELF")
    );
}
