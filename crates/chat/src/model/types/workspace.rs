use std::collections::HashMap;

use crate::model::SlackAttachmentSource;

use super::{
    SlackConversationSnapshot, SlackConversationTab, SlackMessage, SlackShellSnapshot,
    SlackSidebarSnapshot, SlackWorkspace,
};

impl SlackWorkspace {
    pub fn canvas_tab(&self) -> Option<&SlackConversationTab> {
        self.tabs
            .iter()
            .find(|tab| !tab.is_disabled && tab.is_canvas())
    }

    pub fn files_tab(&self) -> Option<&SlackConversationTab> {
        self.tabs
            .iter()
            .find(|tab| !tab.is_disabled && tab.is_files())
    }

    pub fn pins_tab(&self) -> Option<&SlackConversationTab> {
        self.tabs
            .iter()
            .find(|tab| !tab.is_disabled && tab.is_pins())
    }

    pub fn shell_snapshot(&self) -> SlackShellSnapshot {
        SlackShellSnapshot {
            team_id: self.team_id.clone(),
            workspace_name: self.workspace_name.clone(),
            workspace_logo_url: self.workspace_logo_url.clone(),
            workspace_logo_image_base64: self.workspace_logo_image_base64.clone(),
            workspace_logo_image_mimetype: self.workspace_logo_image_mimetype.clone(),
            self_user_id: self.self_user_id.clone(),
            self_display_name: self.self_display_name.clone(),
            self_avatar_label: self.self_avatar_label.clone(),
            self_avatar_image_url: self.self_avatar_image_url.clone(),
            self_avatar_image_base64: self.self_avatar_image_base64.clone(),
            self_avatar_image_mimetype: self.self_avatar_image_mimetype.clone(),
            self_timezone_id: self.self_timezone_id.clone(),
            self_timezone_label: self.self_timezone_label.clone(),
        }
    }

    pub fn sidebar_snapshot(&self) -> SlackSidebarSnapshot {
        SlackSidebarSnapshot {
            team_id: self.team_id.clone(),
            conversation_id: self.conversation_id.clone(),
            active_conversation_kind: self.channel_kind,
            sections: self.sections.clone(),
            direct_message_unread_states: self.direct_message_unread_states.clone(),
            rail_badges: self.rail_badges.clone(),
        }
    }

    pub fn conversation_snapshot(&self) -> SlackConversationSnapshot {
        SlackConversationSnapshot {
            team_id: self.team_id.clone(),
            conversation_id: self.conversation_id.clone(),
            self_timezone_id: self.self_timezone_id.clone(),
            channel_kind: self.channel_kind,
            channel_name: self.channel_name.clone(),
            channel_topic: self.channel_topic.clone(),
            member_count: self.member_count,
            tabs: self.tabs.clone(),
            messages: self.messages.clone(),
            last_read: self.last_read.clone(),
            last_read_boundary_loaded: self.last_read_boundary_loaded,
            history_next_cursor: self.history_next_cursor.clone(),
            mention_suggestions: self.mention_suggestions.clone(),
            emoji_picker_sections: self.emoji_picker_sections.clone(),
            composer_notice: self.composer_notice.clone(),
            peer_notifications_paused: self.peer_notifications_paused,
            dm_peer_local_time_context: self.dm_peer_local_time_context.clone(),
            composer_draft_text: self.composer_draft_text.clone(),
            composer_placeholder: self.composer_placeholder.clone(),
        }
    }

    pub fn from_snapshots(
        shell: SlackShellSnapshot,
        sidebar: Option<SlackSidebarSnapshot>,
        conversation: SlackConversationSnapshot,
    ) -> Self {
        validate_snapshot_relationships(&shell, sidebar.as_ref(), &conversation);
        let mut workspace = workspace_from_snapshots(shell, sidebar, conversation);
        workspace.resolve_missing_attachment_channel_labels();
        workspace
    }

    pub fn apply_shell_snapshot(&mut self, shell: SlackShellSnapshot) {
        assert_eq!(
            self.team_id, shell.team_id,
            "Slack shell snapshot must belong to the active team"
        );
        self.workspace_name = shell.workspace_name;
        self.workspace_logo_url = shell.workspace_logo_url;
        self.workspace_logo_image_base64 = shell.workspace_logo_image_base64;
        self.workspace_logo_image_mimetype = shell.workspace_logo_image_mimetype;
        self.self_user_id = shell.self_user_id;
        self.self_display_name = shell.self_display_name;
        self.self_avatar_label = shell.self_avatar_label;
        self.self_avatar_image_url = shell.self_avatar_image_url;
        self.self_avatar_image_base64 = shell.self_avatar_image_base64;
        self.self_avatar_image_mimetype = shell.self_avatar_image_mimetype;
        self.self_timezone_id = shell.self_timezone_id;
        self.self_timezone_label = shell.self_timezone_label;
    }

    pub fn apply_sidebar_snapshot(&mut self, sidebar: SlackSidebarSnapshot) -> bool {
        assert_eq!(
            self.team_id, sidebar.team_id,
            "Slack sidebar snapshot must belong to the active team"
        );
        assert_eq!(
            self.conversation_id, sidebar.conversation_id,
            "Slack sidebar snapshot must match the active conversation"
        );
        self.sections = sidebar.sections;
        self.direct_message_unread_states = sidebar.direct_message_unread_states;
        self.rail_badges = sidebar.rail_badges;
        self.resolve_missing_attachment_channel_labels()
    }

    pub fn apply_conversation_snapshot(&mut self, conversation: SlackConversationSnapshot) -> bool {
        self.apply_shaped_conversation_snapshot(conversation);
        self.resolve_missing_attachment_channel_labels()
    }

    pub fn apply_shaped_conversation_snapshot(&mut self, conversation: SlackConversationSnapshot) {
        assert_eq!(
            self.team_id, conversation.team_id,
            "Slack conversation snapshot must belong to the active team"
        );
        let same_conversation = self.conversation_id == conversation.conversation_id;
        self.conversation_id = conversation.conversation_id;
        self.self_timezone_id = conversation.self_timezone_id;
        self.channel_kind = conversation.channel_kind;
        self.channel_name = conversation.channel_name;
        self.channel_topic = conversation.channel_topic;
        self.member_count = conversation.member_count;
        let mut tabs = conversation.tabs;
        if same_conversation {
            merge_resolved_tab_metadata(&mut tabs, &self.tabs);
        }
        self.tabs = tabs;
        self.messages = conversation.messages;
        self.last_read = conversation.last_read;
        self.last_read_boundary_loaded = conversation.last_read_boundary_loaded;
        self.history_next_cursor = conversation.history_next_cursor;
        self.mention_suggestions = conversation.mention_suggestions;
        self.emoji_picker_sections = conversation.emoji_picker_sections;
        self.composer_notice = conversation.composer_notice;
        self.peer_notifications_paused = conversation.peer_notifications_paused;
        self.dm_peer_local_time_context = conversation.dm_peer_local_time_context;
        self.composer_draft_text = conversation.composer_draft_text;
        self.composer_placeholder = conversation.composer_placeholder;
    }

    fn resolve_missing_attachment_channel_labels(&mut self) -> bool {
        let channel_labels = self
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .filter(|item| !item.target_id.is_empty())
            .map(|item| (item.target_id.as_str(), item.label.as_str()))
            .collect::<HashMap<_, _>>();
        resolve_message_attachment_channel_labels(&mut self.messages, &channel_labels)
    }
}

impl SlackConversationSnapshot {
    pub fn merge_resolved_tab_metadata_from(&mut self, existing: &Self) {
        if self.team_id != existing.team_id || self.conversation_id != existing.conversation_id {
            return;
        }
        merge_resolved_tab_metadata(&mut self.tabs, &existing.tabs);
    }
}

fn validate_snapshot_relationships(
    shell: &SlackShellSnapshot,
    sidebar: Option<&SlackSidebarSnapshot>,
    conversation: &SlackConversationSnapshot,
) {
    assert_eq!(
        shell.team_id, conversation.team_id,
        "Slack shell and conversation snapshots must belong to the same team"
    );
    if let Some(sidebar) = sidebar {
        assert_eq!(
            shell.team_id, sidebar.team_id,
            "Slack shell and sidebar snapshots must belong to the same team"
        );
        assert_eq!(
            conversation.conversation_id, sidebar.conversation_id,
            "Slack sidebar and conversation snapshots must share the active conversation"
        );
    }
}

fn workspace_from_snapshots(
    shell: SlackShellSnapshot,
    sidebar: Option<SlackSidebarSnapshot>,
    conversation: SlackConversationSnapshot,
) -> SlackWorkspace {
    let (sections, direct_message_unread_states, rail_badges) = sidebar
        .map(|sidebar| {
            (
                sidebar.sections,
                sidebar.direct_message_unread_states,
                sidebar.rail_badges,
            )
        })
        .unwrap_or_default();
    let self_timezone_id = conversation
        .self_timezone_id
        .clone()
        .or(shell.self_timezone_id.clone());
    SlackWorkspace {
        team_id: shell.team_id,
        conversation_id: conversation.conversation_id,
        channel_kind: conversation.channel_kind,
        workspace_name: shell.workspace_name,
        workspace_logo_url: shell.workspace_logo_url,
        workspace_logo_image_base64: shell.workspace_logo_image_base64,
        workspace_logo_image_mimetype: shell.workspace_logo_image_mimetype,
        self_user_id: shell.self_user_id,
        self_display_name: shell.self_display_name,
        self_avatar_label: shell.self_avatar_label,
        self_avatar_image_url: shell.self_avatar_image_url,
        self_avatar_image_base64: shell.self_avatar_image_base64,
        self_avatar_image_mimetype: shell.self_avatar_image_mimetype,
        self_timezone_id,
        self_timezone_label: shell.self_timezone_label,
        channel_name: conversation.channel_name,
        channel_topic: conversation.channel_topic,
        member_count: conversation.member_count,
        tabs: conversation.tabs,
        sections,
        direct_message_unread_states,
        rail_badges,
        messages: conversation.messages,
        last_read: conversation.last_read,
        last_read_boundary_loaded: conversation.last_read_boundary_loaded,
        history_next_cursor: conversation.history_next_cursor,
        mention_suggestions: conversation.mention_suggestions,
        emoji_picker_sections: conversation.emoji_picker_sections,
        composer_notice: conversation.composer_notice,
        peer_notifications_paused: conversation.peer_notifications_paused,
        dm_peer_local_time_context: conversation.dm_peer_local_time_context,
        composer_draft_text: conversation.composer_draft_text,
        composer_placeholder: conversation.composer_placeholder,
    }
}

fn merge_resolved_tab_metadata(
    incoming: &mut [SlackConversationTab],
    existing: &[SlackConversationTab],
) {
    for tab in incoming {
        let Some(existing_tab) = existing
            .iter()
            .find(|existing_tab| existing_tab.id == tab.id)
        else {
            continue;
        };
        tab.merge_resolved_metadata_from(existing_tab);
    }
}

fn resolve_message_attachment_channel_labels(
    messages: &mut [SlackMessage],
    channel_labels: &HashMap<&str, &str>,
) -> bool {
    let mut resolved = false;
    for message in messages {
        for attachment in &mut message.attachments {
            let SlackAttachmentSource::LegacyMessage(metadata) = &mut attachment.source else {
                continue;
            };
            if metadata.channel_label.is_some() {
                continue;
            }
            let Some(channel_id) = metadata.channel_id.as_deref() else {
                continue;
            };
            let Some(channel_label) = channel_labels.get(channel_id) else {
                continue;
            };
            metadata.channel_label = Some((*channel_label).to_string());
            resolved = true;
        }
        resolved |= resolve_message_attachment_channel_labels(&mut message.replies, channel_labels);
    }
    resolved
}
