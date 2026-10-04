use crate::model::{SlackSidebarItem, SlackSidebarSection, SlackWorkspace};

use super::slack_workspace_changed;

#[gpui::test]
fn workspace_changed_detects_sidebar_unread_updates() {
    let current = workspace_with_sidebar_item(false, None);
    let next = workspace_with_sidebar_item(true, Some(2));

    assert!(slack_workspace_changed(&next, &current));
}

fn workspace_with_sidebar_item(unread: bool, count: Option<u32>) -> SlackWorkspace {
    SlackWorkspace {
        team_id: "TTEST".to_string(),
        conversation_id: "D123".to_string(),
        channel_kind: crate::model::SlackConversationKind::DirectMessage,
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
        channel_name: "Grace".to_string(),
        channel_topic: String::new(),
        member_count: None,
        tabs: Vec::new(),
        sections: vec![SlackSidebarSection {
            label: "Direct messages".to_string(),
            items: vec![SlackSidebarItem {
                label: "Grace".to_string(),
                secondary_context: None,
                is_external_connection: false,
                icon: Some("person".to_string()),
                avatar_image_url: None,
                avatar_image_base64: None,
                avatar_image_mimetype: None,
                target_id: "D456".to_string(),
                target_kind: crate::model::SlackConversationKind::DirectMessage,
                user_id: Some("U456".to_string()),
                presence: None,
                active: false,
                unread,
                latest_message_timestamp: None,
                muted: false,
                count,
            }],
        }],
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
        composer_placeholder: "Message Grace".to_string(),
    }
}
