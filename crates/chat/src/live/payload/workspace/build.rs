mod tabs;

use std::collections::HashMap;

use crate::model::{
    SlackConversationHistoryCursor, SlackConversationKind, SlackConversationSnapshot,
    SlackDmPeerLocalTimeContext, SlackIanaTimezone, SlackLastReadTimestamp, SlackRailBadges,
    SlackShellSnapshot, SlackSidebarSnapshot, SlackWorkspace,
};
use serde_json::Value;

use super::super::{
    message::slack_messages,
    sidebar::slack_sections,
    util::{
        channel_member_count, channel_topic, initials, slack_conversation_kind,
        slack_conversation_label, slack_user_avatar_image_url, slack_user_display_name,
        slack_workspace_logo_url, SlackAvatarPurpose, SlackTimezone,
    },
};
use tabs::slack_conversation_tabs;

const SLACKBOT_USER_IDS: [&str; 2] = ["USLACK", "USLACKBOT"];

pub(super) struct SlackWorkspacePayloads<'a> {
    pub(super) team_id: &'a str,
    pub(super) conversation_id: &'a str,
    pub(super) team_info: &'a Value,
    pub(super) self_user_id: Option<&'a str>,
    pub(super) self_user: Option<&'a Value>,
    pub(super) timezone: &'a SlackTimezone,
    pub(super) channel_info: &'a Value,
    pub(super) conversations: &'a Value,
    pub(super) history: &'a Value,
    pub(super) history_next_cursor: Option<SlackConversationHistoryCursor>,
    pub(super) users: &'a HashMap<String, Value>,
    pub(super) sidebar_snapshot: Option<&'a super::super::sidebar_dom::SlackSidebarSnapshot>,
    pub(super) peer_notifications_paused: bool,
}

pub(super) struct SlackConversationPayloads<'a> {
    pub(super) team_id: &'a str,
    pub(super) conversation_id: &'a str,
    pub(super) self_user_id: Option<&'a str>,
    pub(super) self_user: Option<&'a Value>,
    pub(super) self_timezone_id: Option<&'a str>,
    pub(super) timezone: chrono_tz::Tz,
    pub(super) channel_info: &'a Value,
    pub(super) history: &'a Value,
    pub(super) history_next_cursor: Option<SlackConversationHistoryCursor>,
    pub(super) users: &'a HashMap<String, Value>,
    pub(super) sidebar_snapshot: Option<&'a super::super::sidebar_dom::SlackSidebarSnapshot>,
    pub(super) peer_notifications_paused: bool,
}

pub(super) fn slack_workspace_from_payloads(
    payloads: SlackWorkspacePayloads<'_>,
) -> Result<SlackWorkspace, String> {
    let shell = slack_shell_from_payloads(
        payloads.team_id,
        payloads.team_info,
        payloads.self_user_id,
        payloads.self_user,
        payloads.timezone,
    );
    let sidebar = slack_sidebar_from_payloads(
        payloads.team_id,
        payloads.conversation_id,
        payloads.conversations,
        payloads.users,
        payloads.sidebar_snapshot,
    );
    let conversation = slack_conversation_from_payloads(SlackConversationPayloads {
        team_id: payloads.team_id,
        conversation_id: payloads.conversation_id,
        self_user_id: payloads.self_user_id,
        self_user: payloads.self_user,
        self_timezone_id: payloads.timezone.id.as_deref(),
        timezone: payloads.timezone.value,
        channel_info: payloads.channel_info,
        history: payloads.history,
        history_next_cursor: payloads.history_next_cursor,
        users: payloads.users,
        sidebar_snapshot: payloads.sidebar_snapshot,
        peer_notifications_paused: payloads.peer_notifications_paused,
    })?;
    Ok(SlackWorkspace::from_snapshots(
        shell,
        Some(sidebar),
        conversation,
    ))
}

pub(super) fn slack_shell_from_payloads(
    team_id: &str,
    team_info: &Value,
    self_user_id: Option<&str>,
    self_user: Option<&Value>,
    timezone: &SlackTimezone,
) -> SlackShellSnapshot {
    let self_display_name = self_user
        .and_then(slack_user_display_name)
        .map(|display_name| display_name.to_string());
    SlackShellSnapshot {
        team_id: team_id.to_string(),
        workspace_name: super::ops::workspace_name(team_info),
        workspace_logo_url: slack_workspace_logo_url(team_info),
        workspace_logo_image_base64: None,
        workspace_logo_image_mimetype: None,
        self_user_id: self_user_id.map(str::to_string),
        self_display_name: self_display_name.clone(),
        self_avatar_label: self_avatar_label(self_display_name.as_deref()),
        self_avatar_image_url: self_user
            .and_then(|user| slack_user_avatar_image_url(user, SlackAvatarPurpose::Message)),
        self_avatar_image_base64: None,
        self_avatar_image_mimetype: None,
        self_timezone_id: timezone.id.clone(),
        self_timezone_label: self_user
            .and_then(|user| super::super::util::string_at(user, &["tz_label"])),
    }
}

pub(super) fn slack_sidebar_from_payloads(
    team_id: &str,
    conversation_id: &str,
    conversations: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&super::super::sidebar_dom::SlackSidebarSnapshot>,
) -> SlackSidebarSnapshot {
    let channels = conversations
        .get("channels")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let active_conversation_kind = channels
        .iter()
        .find(|channel| channel.get("id").and_then(Value::as_str) == Some(conversation_id))
        .map(slack_conversation_kind)
        .unwrap_or_default();
    let sections = slack_sections(&channels, conversation_id, users, sidebar_snapshot);
    let rail_badges = slack_rail_badges(&sections, sidebar_snapshot, conversation_id);
    SlackSidebarSnapshot {
        team_id: team_id.to_string(),
        conversation_id: conversation_id.to_string(),
        active_conversation_kind,
        sections,
        direct_message_unread_states: sidebar_snapshot
            .map(|snapshot| snapshot.direct_message_unread_states.clone())
            .unwrap_or_default(),
        rail_badges,
    }
}

pub(super) fn slack_conversation_from_payloads(
    payloads: SlackConversationPayloads<'_>,
) -> Result<SlackConversationSnapshot, String> {
    let channel = payloads
        .channel_info
        .get("channel")
        .cloned()
        .unwrap_or(Value::Null);
    let channel_kind = slack_conversation_kind(&channel);
    let channel_name = payloads
        .sidebar_snapshot
        .and_then(|snapshot| snapshot.item(payloads.conversation_id))
        .map(|item| item.label.clone())
        .filter(|label| !label.trim().is_empty())
        .or_else(|| slack_conversation_label(&channel, payloads.users))
        .unwrap_or_else(|| payloads.conversation_id.to_string());
    let last_read = slack_last_read_timestamp(&channel, payloads.conversation_id)?;
    let dm_peer_local_time_context =
        slack_dm_peer_local_time_context(&payloads, &channel, channel_kind, &channel_name)?;
    Ok(SlackConversationSnapshot {
        team_id: payloads.team_id.to_string(),
        conversation_id: payloads.conversation_id.to_string(),
        self_timezone_id: payloads.self_timezone_id.map(str::to_string),
        channel_kind,
        channel_name: channel_name.clone(),
        channel_topic: channel_topic(&channel, channel_kind),
        member_count: channel_member_count(&channel, channel_kind),
        tabs: slack_conversation_tabs(&channel, payloads.conversation_id)?,
        messages: slack_messages(
            payloads.history,
            payloads.users,
            payloads.sidebar_snapshot,
            payloads.self_user_id,
            payloads.timezone,
        ),
        last_read,
        last_read_boundary_loaded: true,
        history_next_cursor: payloads.history_next_cursor,
        mention_suggestions: Vec::new(),
        emoji_picker_sections: Vec::new(),
        composer_notice: payloads
            .peer_notifications_paused
            .then(|| format!("{channel_name} has paused their notifications")),
        peer_notifications_paused: payloads.peer_notifications_paused,
        dm_peer_local_time_context,
        composer_draft_text: None,
        composer_placeholder: channel_kind.composer_placeholder(&channel_name),
    })
}

fn slack_dm_peer_local_time_context(
    payloads: &SlackConversationPayloads<'_>,
    channel: &Value,
    channel_kind: SlackConversationKind,
    channel_name: &str,
) -> Result<Option<SlackDmPeerLocalTimeContext>, String> {
    if channel_kind != SlackConversationKind::DirectMessage {
        return Ok(None);
    }
    let Some(sidebar_item) = payloads
        .sidebar_snapshot
        .and_then(|snapshot| snapshot.item(payloads.conversation_id))
    else {
        return Ok(None);
    };
    if sidebar_item.is_external_connection {
        return Ok(None);
    }
    let Some(self_user_id) = payloads.self_user_id else {
        return Ok(None);
    };
    let Some(peer_user_id) = channel.get("user").and_then(Value::as_str) else {
        return Ok(None);
    };
    if peer_user_id == self_user_id || SLACKBOT_USER_IDS.contains(&peer_user_id) {
        return Ok(None);
    }
    let Some(peer_user) = payloads.users.get(peer_user_id) else {
        return Ok(None);
    };
    if slack_user_flag(peer_user, "is_bot")
        || slack_user_flag(peer_user, "is_app_user")
        || slack_user_flag(peer_user, "is_invited_user")
    {
        return Ok(None);
    }
    let Some(self_user) = payloads.self_user else {
        return Ok(None);
    };
    let Some(self_timezone) = slack_user_iana_timezone(self_user, self_user_id)? else {
        return Ok(None);
    };
    let Some(peer_timezone) = slack_user_iana_timezone(peer_user, peer_user_id)? else {
        return Ok(None);
    };
    if self_timezone == peer_timezone {
        return Ok(None);
    }
    Ok(Some(SlackDmPeerLocalTimeContext {
        conversation_label: channel_name.to_string(),
        timezone: peer_timezone,
    }))
}

fn slack_user_iana_timezone(
    user: &Value,
    user_id: &str,
) -> Result<Option<SlackIanaTimezone>, String> {
    let Some(value) = user.get("tz") else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        if value.is_null() {
            return Ok(None);
        }
        return Err(format!("Slack user {user_id} returned a non-string tz"));
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    value.parse().map(Some).map_err(|error| {
        format!("Slack user {user_id} returned invalid timezone {value:?}: {error}")
    })
}

fn slack_user_flag(user: &Value, field: &str) -> bool {
    user.get(field).and_then(Value::as_bool).unwrap_or(false)
}

pub(super) fn slack_last_read_timestamp(
    channel: &Value,
    conversation_id: &str,
) -> Result<Option<SlackLastReadTimestamp>, String> {
    let Some(value) = channel.get("last_read") else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let value = value.as_str().ok_or_else(|| {
        format!("Slack conversations.info returned non-string last_read for {conversation_id}")
    })?;
    SlackLastReadTimestamp::parse(value)
        .map(Some)
        .map_err(|error| {
            format!(
                "Slack conversations.info returned invalid last_read for {conversation_id}: {error}"
            )
        })
}

fn self_avatar_label(self_display_name: Option<&str>) -> Option<String> {
    self_display_name
        .map(initials)
        .filter(|label| !label.is_empty())
}

fn slack_rail_badges(
    sections: &[crate::model::SlackSidebarSection],
    sidebar_snapshot: Option<&super::super::sidebar_dom::SlackSidebarSnapshot>,
    conversation_id: &str,
) -> SlackRailBadges {
    let direct_message_unreads = sidebar_snapshot
        .filter(|snapshot| !snapshot.direct_message_unread_states.is_empty())
        .map(|snapshot| {
            snapshot
                .direct_message_unread_states
                .iter()
                .filter(|state| state.conversation_id != conversation_id && state.unread)
                .count()
        })
        .unwrap_or_else(|| {
            sections
                .iter()
                .flat_map(|section| section.items.iter())
                .filter(|item| {
                    matches!(
                        item.target_kind,
                        crate::model::SlackConversationKind::DirectMessage
                            | crate::model::SlackConversationKind::GroupMessage
                    ) && !item.active
                        && item.unread
                })
                .count()
        });
    let activity_count = sections
        .iter()
        .flat_map(|section| section.items.iter())
        .filter(|item| !item.active)
        .filter_map(|item| item.count)
        .sum::<u32>();
    let slack_activity_count = sidebar_snapshot.and_then(|snapshot| snapshot.activity_count);
    SlackRailBadges {
        drafts_sent: sidebar_snapshot.and_then(|snapshot| snapshot.draft_count),
        home: slack_activity_count.filter(|count| *count > 0).map(|_| 1),
        dms: positive_u32(direct_message_unreads),
        dms_unread_messages: sidebar_snapshot.and_then(|snapshot| snapshot.dms_unread_messages),
        activity: slack_activity_count.or_else(|| positive_u32(activity_count as usize)),
        admin_visible: sidebar_snapshot.is_some_and(|snapshot| snapshot.admin_visible),
        admin_attention: sidebar_snapshot.is_some_and(|snapshot| snapshot.admin_attention),
        self_presence: sidebar_snapshot.and_then(|snapshot| snapshot.self_presence),
        self_notifications_paused: sidebar_snapshot
            .is_some_and(|snapshot| snapshot.self_notifications_paused),
        ..SlackRailBadges::default()
    }
}

fn positive_u32(value: usize) -> Option<u32> {
    u32::try_from(value).ok().filter(|value| *value > 0)
}
