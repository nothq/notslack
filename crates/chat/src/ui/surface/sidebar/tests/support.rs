use crate::ui::surface::sidebar::*;
use crate::ui::{SlackConversationKind, SlackSidebarItem, SlackSidebarSection, SlackWorkspace};

pub(crate) struct SlackSidebarWorkspaceSpec {
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) channel_kind: SlackConversationKind,
    pub(crate) channel_name: String,
    pub(crate) member_count: Option<u32>,
    pub(crate) sections: Vec<SlackSidebarSection>,
    pub(crate) composer_placeholder: String,
}

pub(crate) struct SlackSidebarItemSpec<'a> {
    pub(crate) label: &'a str,
    pub(crate) icon: Option<&'a str>,
    pub(crate) target_id: &'a str,
    pub(crate) target_kind: SlackConversationKind,
    pub(crate) active: bool,
    pub(crate) unread: bool,
    pub(crate) count: Option<u32>,
}

pub(crate) fn slack_sidebar_test_workspace(spec: SlackSidebarWorkspaceSpec) -> SlackWorkspace {
    SlackWorkspace {
        team_id: spec.team_id,
        conversation_id: spec.conversation_id,
        channel_kind: spec.channel_kind,
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
        channel_name: spec.channel_name,
        channel_topic: String::new(),
        member_count: spec.member_count,
        tabs: Vec::new(),
        sections: spec.sections,
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
        composer_placeholder: spec.composer_placeholder,
    }
}

pub(crate) fn slack_channel_section(items: Vec<SlackSidebarItem>) -> SlackSidebarSection {
    SlackSidebarSection {
        label: "Channels".to_string(),
        items,
    }
}

pub(crate) fn slack_sidebar_channel(
    label: &str,
    target_id: &str,
    active: bool,
    unread: bool,
    count: Option<u32>,
) -> SlackSidebarItem {
    slack_sidebar_item(SlackSidebarItemSpec {
        label,
        icon: Some("hash"),
        target_id,
        target_kind: SlackConversationKind::Channel,
        active,
        unread,
        count,
    })
}

pub(crate) fn sidebar_item_index(rows: &[SlackSidebarRow], label: &str) -> usize {
    rows.iter()
        .position(
            |row| matches!(&row.kind, SlackSidebarRowKind::Item { item, .. } if item.label == label),
        )
        .expect("sidebar item should exist")
}

pub(crate) fn sidebar_section_index(rows: &[SlackSidebarRow], section_label: &str) -> usize {
    rows.iter()
        .position(|row| {
            matches!(
                &row.kind,
                SlackSidebarRowKind::SectionHeader { label, .. } if label == section_label
            )
        })
        .expect("sidebar section should exist")
}

pub(crate) fn slack_sidebar_item(spec: SlackSidebarItemSpec<'_>) -> SlackSidebarItem {
    SlackSidebarItem {
        label: spec.label.to_string(),
        secondary_context: None,
        is_external_connection: false,
        icon: spec.icon.map(ToString::to_string),
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        target_id: spec.target_id.to_string(),
        target_kind: spec.target_kind,
        user_id: None,
        presence: None,
        active: spec.active,
        unread: spec.unread,
        latest_message_timestamp: None,
        muted: false,
        count: spec.count,
    }
}
