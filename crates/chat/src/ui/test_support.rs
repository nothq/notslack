mod mock_api;

pub use mock_api::*;

use remote_image_model::RemoteImageData;
use std::{
    collections::HashMap,
    sync::{Arc, Condvar, Mutex},
};

use crate::model::{
    SlackAttachment, SlackConversationKind, SlackMessage, SlackProfile, SlackReaction,
    SlackSidebarItem, SlackSidebarSection, SlackUploadFile, SlackWorkspace, SlackWorkspaceApi,
};

fn wait_test_delay(delay: std::time::Duration) {
    if delay.is_zero() {
        return;
    }
    let lock = Mutex::new(());
    let guard = lock.lock().expect("Slack test delay mutex poisoned");
    let (_guard, _) = Condvar::new()
        .wait_timeout(guard, delay)
        .expect("Slack test delay condition variable failed");
}
mod api;

pub use api::{
    slack_test_api, slack_test_api_with_accepted_invalid_receipt,
    slack_test_api_with_accepted_send_error, slack_test_api_with_conversations,
    slack_test_api_with_remote_images, slack_test_api_with_send_delay,
    slack_test_api_with_send_error, slow_slack_test_api,
};

pub type SlackTestApi = (Arc<dyn SlackWorkspaceApi>, SentMessages);
pub type SlowSlackTestApi = (
    Arc<dyn SlackWorkspaceApi>,
    SentMessages,
    SlackLoadMetricsHandle,
);

pub fn slack_test_board_with_workspace(workspace: SlackWorkspace) -> SlackWorkspace {
    workspace
}

pub fn slack_test_workspace_with_message_count(
    conversation_id: &str,
    channel_name: &str,
    active_deploys: bool,
    message_count: usize,
) -> SlackWorkspace {
    slack_test_workspace_with_sections(
        conversation_id,
        channel_name,
        vec![slack_test_channels_section(active_deploys)],
        message_count,
    )
}

pub fn slack_test_workspace_with_sections(
    conversation_id: &str,
    channel_name: &str,
    sections: Vec<SlackSidebarSection>,
    message_count: usize,
) -> SlackWorkspace {
    let messages = (0..message_count)
        .map(|index| slack_test_message(index, channel_name))
        .collect();

    SlackWorkspace {
        team_id: "TTEST".to_string(),
        conversation_id: conversation_id.to_string(),
        channel_kind: SlackConversationKind::Channel,
        workspace_name: "Acme".to_string(),
        workspace_logo_url: None,
        workspace_logo_image_base64: None,
        workspace_logo_image_mimetype: None,
        self_user_id: Some("U_SELF".to_string()),
        self_display_name: None,
        self_avatar_label: None,
        self_avatar_image_url: None,
        self_avatar_image_base64: None,
        self_avatar_image_mimetype: None,
        self_timezone_id: None,
        self_timezone_label: None,
        channel_name: channel_name.to_string(),
        channel_topic: "Shared build lane".to_string(),
        member_count: Some(8),
        tabs: Vec::new(),
        rail_badges: Default::default(),
        sections,
        direct_message_unread_states: Vec::new(),
        messages,
        last_read: None,
        last_read_boundary_loaded: true,
        history_next_cursor: None,
        mention_suggestions: Vec::new(),
        emoji_picker_sections: Vec::new(),
        composer_notice: None,
        peer_notifications_paused: false,
        dm_peer_local_time_context: None,
        composer_draft_text: None,
        composer_placeholder: format!("Message to {channel_name}"),
    }
}

pub fn slack_test_workspace(
    conversation_id: &str,
    channel_name: &str,
    active_deploys: bool,
) -> SlackWorkspace {
    slack_test_workspace_with_message_count(conversation_id, channel_name, active_deploys, 1)
}

pub fn slack_test_workspace_with_channel_count(channel_count: usize) -> SlackWorkspace {
    let active_channel_id = "C_CHANNEL_0000".to_string();
    let channels = (0..channel_count)
        .map(|index| slack_test_channel(index, &active_channel_id))
        .collect();

    SlackWorkspace {
        team_id: "TTEST".to_string(),
        conversation_id: active_channel_id,
        channel_kind: SlackConversationKind::Channel,
        workspace_name: "Acme".to_string(),
        workspace_logo_url: None,
        workspace_logo_image_base64: None,
        workspace_logo_image_mimetype: None,
        self_user_id: Some("U_SELF".to_string()),
        self_display_name: None,
        self_avatar_label: None,
        self_avatar_image_url: None,
        self_avatar_image_base64: None,
        self_avatar_image_mimetype: None,
        self_timezone_id: None,
        self_timezone_label: None,
        channel_name: "design".to_string(),
        channel_topic: "Shared build lane".to_string(),
        member_count: Some(8),
        tabs: Vec::new(),
        rail_badges: Default::default(),
        sections: vec![SlackSidebarSection {
            label: "Channels".to_string(),
            items: channels,
        }],
        direct_message_unread_states: Vec::new(),
        messages: vec![slack_test_message(0, "design")],
        last_read: None,
        last_read_boundary_loaded: true,
        history_next_cursor: None,
        mention_suggestions: Vec::new(),
        emoji_picker_sections: Vec::new(),
        composer_notice: None,
        peer_notifications_paused: false,
        dm_peer_local_time_context: None,
        composer_draft_text: None,
        composer_placeholder: "Message to design".to_string(),
    }
}

fn slack_test_channel(index: usize, active_channel_id: &str) -> SlackSidebarItem {
    let channel_id = format!("C_CHANNEL_{index:04}");
    let channel_name = if index == 0 {
        "design".to_string()
    } else {
        format!("channel-{index:04}")
    };
    SlackSidebarItem {
        label: channel_name,
        secondary_context: None,
        is_external_connection: false,
        icon: Some("hash".to_string()),
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        target_id: channel_id.clone(),
        target_kind: SlackConversationKind::Channel,
        user_id: None,
        presence: None,
        active: channel_id == active_channel_id,
        unread: index.is_multiple_of(7),
        latest_message_timestamp: None,
        muted: index.is_multiple_of(11),
        count: index.is_multiple_of(9).then_some((index % 5 + 1) as u32),
    }
}

pub fn slack_test_board() -> SlackWorkspace {
    slack_test_board_with_workspace(slack_test_workspace("C_DESIGN", "design", false))
}

fn slack_test_message(index: usize, channel_name: &str) -> SlackMessage {
    let is_ada = index.is_multiple_of(2);
    SlackMessage {
        id: format!("1700000000.{:06}", index + 1),
        client_message_id: None,
        author: if is_ada {
            "Ada Lovelace"
        } else {
            "Acme"
        }
        .to_string(),
        timestamp: format!("9:{:02} PM", (27 + index) % 60),
        user_id: Some(if is_ada { "U_ADA" } else { "U_ACME" }.to_string()),
        avatar_label: Some(if is_ada { "IT" } else { "PB" }.to_string()),
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        body: slack_test_message_body(index, channel_name),
        rich_body: None,
        table_rows: Vec::new(),
        date_divider_label: None,
        edited_label: index.is_multiple_of(6).then(|| "(edited)".to_string()),
        attachments: if index.is_multiple_of(7) {
            vec![SlackAttachment::label_only(format!(
                "Attachment block for {channel_name} #{}",
                index + 1
            ))]
        } else {
            Vec::new()
        },
        reactions: if index.is_multiple_of(5) {
            vec![SlackReaction {
                emoji: "white_check_mark".to_string(),
                count: (index % 3 + 1) as u32,
                active: index.is_multiple_of(10),
            }]
        } else {
            Vec::new()
        },
        saved_state: None,
        reply_count: None,
        latest_reply_timestamp: None,
        reply_participants: Vec::new(),
        replies: Vec::new(),
    }
}

fn slack_test_message_body(index: usize, channel_name: &str) -> String {
    match index % 4 {
        0 => format!("Checking {channel_name} update #{}", index + 1),
        1 => format!(
            "Deploy check-in for {channel_name} #{} with a longer body for scroll profiling",
            index + 1
        ),
        2 => format!(
            "Investigating scroll performance in {channel_name} thread #{} and comparing against GPUI list virtualization",
            index + 1
        ),
        _ => format!("Reviewing logs for {channel_name} #{}", index + 1),
    }
}

pub fn slack_test_channels_section(active_deploys: bool) -> SlackSidebarSection {
    SlackSidebarSection {
        label: "Channels".to_string(),
        items: vec![
            slack_test_sidebar_item("design", "C_DESIGN", !active_deploys, Some(3), true),
            slack_test_sidebar_item("deploys", "C_DEPLOYS", active_deploys, None, false),
        ],
    }
}

fn slack_test_sidebar_item(
    label: &str,
    target_id: &str,
    active: bool,
    count: Option<u32>,
    unread: bool,
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
