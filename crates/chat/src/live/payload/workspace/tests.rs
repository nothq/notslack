use std::collections::HashMap;

use crate::model::SlackConversationKind;
use serde_json::{json, Value};

use crate::live::payload::{
    sidebar_dom::{SlackSidebarSnapshot, SlackSidebarSnapshotItem, SlackSidebarSnapshotSection},
    workspace::*,
};

#[gpui::test]
fn workspace_uses_live_sidebar_label_for_group_direct_messages() {
    let timezone = SlackTimezone::utc();
    let workspace = slack_workspace_from_payloads(SlackWorkspacePayloads {
        team_id: "TQRADTDAL",
        conversation_id: "C0AQN0JGSMP",
        team_info: &test_team_info(),
        self_user_id: None,
        self_user: None,
        timezone: &timezone,
        channel_info: &test_channel_info(),
        conversations: &test_conversations(),
        history: &test_history(),
        history_next_cursor: None,
        users: &HashMap::new(),
        sidebar_snapshot: Some(&test_sidebar_snapshot()),
        peer_notifications_paused: false,
    })
    .expect("Slack workspace payload should be valid");

    assert_eq!(workspace.channel_kind, SlackConversationKind::GroupMessage);
    assert_eq!(
        workspace.channel_name,
        "Edsger Dijkstra, Barbara Liskov, Ken, Donald Knuth"
    );
    assert_eq!(
        workspace.composer_placeholder,
        "Message to Edsger Dijkstra, Barbara Liskov, Ken, Donald Knuth"
    );
}

#[gpui::test]
fn workspace_rail_badges_are_derived_from_sidebar_unreads() {
    let conversations = rail_badge_conversations();
    let users = rail_badge_users();
    let timezone = SlackTimezone::utc();
    let workspace = slack_workspace_from_payloads(SlackWorkspacePayloads {
        team_id: "TQRADTDAL",
        conversation_id: "CMENTION",
        team_info: &test_team_info(),
        self_user_id: None,
        self_user: None,
        timezone: &timezone,
        channel_info: &rail_badge_channel_info(),
        conversations: &conversations,
        history: &test_history(),
        history_next_cursor: None,
        users: &users,
        sidebar_snapshot: Some(&rail_badge_sidebar_snapshot()),
        peer_notifications_paused: false,
    })
    .expect("Slack workspace payload should be valid");

    assert_eq!(workspace.rail_badges.dms, Some(2));
    assert_eq!(workspace.rail_badges.activity, Some(12));
    assert_eq!(workspace.rail_badges.drafts_sent, Some(5));
    assert_eq!(workspace.rail_badges.home, Some(1));
}

fn rail_badge_conversations() -> Value {
    json!({
        "channels": [
            {
                "id": "CMENTION",
                "name": "standup",
                "is_channel": true,
                "unread_count": 4,
                "unread_count_display": 2
            },
            {
                "id": "DNOBADGE",
                "user": "U123",
                "is_im": true,
                "unread_count": 3,
                "unread_count_display": 0
            },
            {
                "id": "DBADGE",
                "user": "U456",
                "is_im": true,
                "unread_count": 1,
                "unread_count_display": 1
            }
        ],
    })
}

fn rail_badge_users() -> HashMap<String, Value> {
    HashMap::from([
        (
            "U123".to_string(),
            json!({ "profile": { "display_name": "No Badge" } }),
        ),
        (
            "U456".to_string(),
            json!({ "profile": { "display_name": "Badge" } }),
        ),
    ])
}

fn rail_badge_channel_info() -> Value {
    json!({
        "channel": {
            "id": "CMENTION",
            "name": "standup",
            "is_channel": true,
        }
    })
}

fn rail_badge_sidebar_snapshot() -> SlackSidebarSnapshot {
    SlackSidebarSnapshot {
        draft_count: Some(5),
        activity_count: Some(12),
        dms_unread_messages: None,
        admin_visible: false,
        admin_attention: false,
        self_presence: None,
        self_notifications_paused: false,
        direct_message_unread_states: Vec::new(),
        sections: vec![SlackSidebarSnapshotSection {
            key: "direct_messages".to_string(),
            label: "Direct messages".to_string(),
            items: vec![
                snapshot_item_with_state("DNOBADGE", "No Badge", true, None),
                snapshot_item_with_state("DBADGE", "Badge", true, Some(1)),
            ],
        }],
    }
}

fn test_team_info() -> Value {
    json!({
        "team": {
            "name": "Acme",
        },
    })
}

fn test_channel_info() -> Value {
    json!({
        "channel": {
            "id": "C0AQN0JGSMP",
            "name": "mpdm-edsger-dijkstra--barbara-liskov--ken--donald-knuth-1",
            "is_mpim": true,
        },
    })
}

fn test_conversations() -> Value {
    json!({
        "channels": [
            {
                "id": "C0AQN0JGSMP",
                "name": "mpdm-edsger-dijkstra--barbara-liskov--ken--donald-knuth-1",
                "is_mpim": true,
                "updated": 1_769_000_000_000i64,
            }
        ],
    })
}

fn test_history() -> Value {
    json!({
        "messages": [],
    })
}

fn test_sidebar_snapshot() -> SlackSidebarSnapshot {
    SlackSidebarSnapshot {
        draft_count: None,
        activity_count: None,
        dms_unread_messages: None,
        admin_visible: false,
        admin_attention: false,
        self_presence: None,
        self_notifications_paused: false,
        direct_message_unread_states: Vec::new(),
        sections: vec![SlackSidebarSnapshotSection {
            key: "direct_messages".to_string(),
            label: "Direct messages".to_string(),
            items: vec![snapshot_item(
                "C0AQN0JGSMP",
                "Edsger Dijkstra, Barbara Liskov, Ken, Donald Knuth",
            )],
        }],
    }
}

fn snapshot_item(id: &str, label: &str) -> SlackSidebarSnapshotItem {
    SlackSidebarSnapshotItem {
        id: id.to_string(),
        label: label.to_string(),
        secondary_context: None,
        avatar_image_url: None,
        is_external_connection: false,
        user_id: None,
        presence: None,
        kind: None,
        selected: false,
        unread: false,
        count: None,
        latest_message_timestamp: None,
    }
}

fn snapshot_item_with_state(
    id: &str,
    label: &str,
    unread: bool,
    count: Option<u32>,
) -> SlackSidebarSnapshotItem {
    SlackSidebarSnapshotItem {
        id: id.to_string(),
        label: label.to_string(),
        secondary_context: None,
        avatar_image_url: None,
        is_external_connection: false,
        user_id: None,
        presence: None,
        kind: None,
        selected: false,
        unread,
        count,
        latest_message_timestamp: None,
    }
}
