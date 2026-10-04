use crate::model::{
    SlackConversationKind, SlackMessage, SlackSidebarItem, SlackSidebarSection, SlackWorkspace,
};

pub(super) fn sidebar_activity_workspace() -> SlackWorkspace {
    SlackWorkspace {
        team_id: "TTEST".to_string(),
        conversation_id: "C_ARCHIVE_DESIGN".to_string(),
        channel_kind: SlackConversationKind::Channel,
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
                slack_sidebar_channel_item("design", "C_ARCHIVE_DESIGN", false),
                slack_sidebar_channel_item("random", "C_ARCHIVE_RANDOM", true),
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
        composer_placeholder: "Message #design".to_string(),
    }
}

pub(super) fn archived_runtime_test_workspace() -> SlackWorkspace {
    SlackWorkspace {
        team_id: "T_ARCHIVE".to_string(),
        conversation_id: "C_ARCHIVE_DESIGN".to_string(),
        channel_kind: SlackConversationKind::Channel,
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
        channel_topic: "Captured from archive".to_string(),
        member_count: Some(8),
        tabs: Vec::new(),
        sections: vec![SlackSidebarSection {
            label: "Channels".to_string(),
            items: vec![
                archived_runtime_test_sidebar_item(
                    "random",
                    "C_ARCHIVE_RANDOM",
                    false,
                    false,
                    None,
                ),
                archived_runtime_test_sidebar_item(
                    "design",
                    "C_ARCHIVE_DESIGN",
                    true,
                    true,
                    Some(1),
                ),
            ],
        }],
        direct_message_unread_states: Vec::new(),
        rail_badges: Default::default(),
        messages: vec![archived_runtime_test_message()],
        last_read: None,
        last_read_boundary_loaded: true,
        history_next_cursor: None,
        mention_suggestions: Vec::new(),
        emoji_picker_sections: Vec::new(),
        composer_notice: None,
        peer_notifications_paused: false,
        dm_peer_local_time_context: None,
        composer_draft_text: None,
        composer_placeholder: "Message #design".to_string(),
    }
}

pub(super) fn sample_workspace() -> SlackWorkspace {
    SlackWorkspace {
        team_id: "TTEST".to_string(),
        conversation_id: "C123".to_string(),
        channel_kind: SlackConversationKind::Channel,
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
            vec![slack_sidebar_item("design", Some("hash"))],
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
        composer_placeholder: "Message #design".to_string(),
    }
}

pub(super) fn slack_sidebar_section(
    label: &str,
    items: Vec<SlackSidebarItem>,
) -> SlackSidebarSection {
    SlackSidebarSection {
        label: label.to_string(),
        items,
    }
}

pub(super) fn slack_sidebar_item(label: &str, icon: Option<&str>) -> SlackSidebarItem {
    SlackSidebarItem {
        label: label.to_string(),
        secondary_context: None,
        is_external_connection: false,
        icon: icon.map(str::to_string),
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        target_id: String::new(),
        target_kind: SlackConversationKind::Unknown,
        user_id: None,
        presence: None,
        active: false,
        unread: false,
        latest_message_timestamp: None,
        muted: false,
        count: None,
    }
}

fn slack_sidebar_channel_item(label: &str, target_id: &str, active: bool) -> SlackSidebarItem {
    SlackSidebarItem {
        label: label.to_string(),
        secondary_context: None,
        is_external_connection: false,
        icon: Some("hash".to_string()),
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        target_id: target_id.to_string(),
        target_kind: SlackConversationKind::Channel,
        user_id: None,
        presence: None,
        active,
        unread: false,
        latest_message_timestamp: None,
        muted: false,
        count: None,
    }
}

fn archived_runtime_test_sidebar_item(
    label: &str,
    target_id: &str,
    active: bool,
    unread: bool,
    count: Option<u32>,
) -> SlackSidebarItem {
    SlackSidebarItem {
        label: label.to_string(),
        secondary_context: None,
        is_external_connection: false,
        icon: Some("hash".to_string()),
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        target_id: target_id.to_string(),
        target_kind: SlackConversationKind::Channel,
        user_id: None,
        presence: None,
        active,
        unread,
        latest_message_timestamp: None,
        muted: false,
        count,
    }
}

fn archived_runtime_test_message() -> SlackMessage {
    SlackMessage {
        id: "message-1".to_string(),
        client_message_id: None,
        author: "Ada Lovelace".to_string(),
        timestamp: "9:27 PM".to_string(),
        user_id: Some("U_ADA".to_string()),
        avatar_label: Some("IT".to_string()),
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        body: "Archived message".to_string(),
        rich_body: None,
        table_rows: Vec::new(),
        date_divider_label: None,
        edited_label: None,
        attachments: Vec::new(),
        reactions: Vec::new(),
        saved_state: None,
        reply_count: None,
        latest_reply_timestamp: None,
        reply_participants: Vec::new(),
        replies: Vec::new(),
    }
}
