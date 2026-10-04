use serde_json::{json, Value};
use time::OffsetDateTime;

use crate::live::payload::sidebar::*;
use crate::live::payload::sidebar_dom::{
    SlackSidebarSnapshot, SlackSidebarSnapshotItem, SlackSidebarSnapshotSection,
};

#[gpui::test]
fn slack_sections_require_internal_sidebar_snapshot() {
    let result = std::panic::catch_unwind(|| {
        slack_sections_at(
            &snapshot_order_channels(),
            "C200",
            &active_only_users(),
            test_now(),
            None,
        )
    });

    assert!(result.is_err());
}

#[gpui::test]
fn slack_sections_sort_snapshot_channels_by_live_label() {
    let sections = slack_sections_at(
        &snapshot_order_channels(),
        "C100",
        &active_only_users(),
        test_now(),
        Some(&snapshot_order_sidebar()),
    );

    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].label, "Channels");
    assert_eq!(sections[0].items[0].target_id, "C100");
    assert_eq!(sections[0].items[0].label, "AI Craze");
    assert!(sections[0].items[0].active);
    assert_eq!(sections[0].items[1].target_id, "C200");
    assert_eq!(sections[0].items[1].label, "Standup");
}

#[gpui::test]
fn slack_sections_preserve_snapshot_direct_message_order() {
    let sections = slack_sections_at(
        &snapshot_order_channels(),
        "D123",
        &active_only_users(),
        test_now(),
        Some(&snapshot_order_sidebar()),
    );

    assert_eq!(sections[1].label, "Direct messages");
    assert_eq!(sections[1].items[0].target_id, "C0AQSA89UQJ");
    assert_eq!(sections[1].items[0].label, "Grace, Alan Turing");
    assert_eq!(sections[1].items[1].target_id, "D123");
    assert_eq!(sections[1].items[1].label, "Zed User");
    assert!(sections[1].items[1].active);
}

#[gpui::test]
fn snapshot_channel_items_keep_unread_and_badge_state() {
    let sections = slack_sections_at(
        &snapshot_order_channels(),
        "C999",
        &active_only_users(),
        test_now(),
        Some(&snapshot_order_sidebar()),
    );

    let item = sections[0]
        .items
        .iter()
        .find(|item| item.target_id == "C100")
        .expect("channel item");
    assert!(item.unread);
    assert_eq!(item.count, Some(3));
}

#[gpui::test]
fn snapshot_items_use_conversation_name_when_label_empty() {
    let snapshot = SlackSidebarSnapshot {
        draft_count: None,
        activity_count: None,
        dms_unread_messages: None,
        admin_visible: false,
        admin_attention: false,
        self_presence: None,
        self_notifications_paused: false,
        direct_message_unread_states: Vec::new(),
        sections: vec![SlackSidebarSnapshotSection {
            key: "channels".to_string(),
            label: "Channels".to_string(),
            items: vec![SlackSidebarSnapshotItem {
                id: "C100".to_string(),
                label: String::new(),
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
            }],
        }],
    };

    let sections = slack_sections_at(
        &snapshot_order_channels(),
        "C999",
        &active_only_users(),
        test_now(),
        Some(&snapshot),
    );

    assert_eq!(sections[0].items[0].label, "design");
}

#[gpui::test]
fn empty_snapshot_direct_message_label_uses_user_metadata() {
    let snapshot = SlackSidebarSnapshot {
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
            items: vec![SlackSidebarSnapshotItem {
                id: "D123".to_string(),
                label: String::new(),
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
            }],
        }],
    };

    let sections = slack_sections_at(
        &snapshot_order_channels(),
        "C999",
        &active_only_users(),
        test_now(),
        Some(&snapshot),
    );

    assert_eq!(sections[0].items[0].label, "Zed User");
}

#[gpui::test]
fn missing_active_dm_is_appended_to_direct_messages() {
    let snapshot = SlackSidebarSnapshot {
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
            items: vec![SlackSidebarSnapshotItem {
                id: "D404".to_string(),
                label: "Missing".to_string(),
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
            }],
        }],
    };

    let sections = slack_sections_at(
        &snapshot_order_channels(),
        "D123",
        &active_only_users(),
        test_now(),
        Some(&snapshot),
    );

    assert_eq!(sections[0].items.len(), 1);
    assert_eq!(sections[0].items[0].target_id, "D123");
    assert_eq!(sections[0].items[0].label, "Zed User");
}

fn snapshot_order_channels() -> Vec<Value> {
    vec![
        json!({
            "id": "C100",
            "name": "design",
            "is_channel": true,
        }),
        json!({
            "id": "C200",
            "name": "standup",
            "is_channel": true,
        }),
        json!({
            "id": "C0AQSA89UQJ",
            "name": "mpdm-grace--alan-1",
            "is_mpim": true,
        }),
        json!({
            "id": "D123",
            "user": "U999",
            "is_im": true,
        }),
    ]
}

fn snapshot_order_sidebar() -> SlackSidebarSnapshot {
    SlackSidebarSnapshot {
        draft_count: None,
        activity_count: None,
        dms_unread_messages: None,
        admin_visible: false,
        admin_attention: false,
        self_presence: None,
        self_notifications_paused: false,
        direct_message_unread_states: Vec::new(),
        sections: vec![
            SlackSidebarSnapshotSection {
                key: "channels".to_string(),
                label: "Channels".to_string(),
                items: vec![
                    snapshot_item("C200", "Standup", false, None),
                    snapshot_item("C100", "AI Craze", true, Some(3)),
                ],
            },
            SlackSidebarSnapshotSection {
                key: "direct_messages".to_string(),
                label: "Direct messages".to_string(),
                items: vec![
                    snapshot_item("C0AQSA89UQJ", "Grace, Alan Turing", false, None),
                    SlackSidebarSnapshotItem {
                        id: "D123".to_string(),
                        label: "Zed User".to_string(),
                        secondary_context: None,
                        avatar_image_url: None,
                        is_external_connection: false,
                        user_id: Some("U999".to_string()),
                        presence: None,
                        kind: None,
                        selected: false,
                        unread: true,
                        count: Some(2),
                        latest_message_timestamp: None,
                    },
                ],
            },
        ],
    }
}

fn snapshot_item(
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

fn active_only_users() -> std::collections::HashMap<String, Value> {
    std::collections::HashMap::from([(
        "U999".to_string(),
        json!({
            "profile": {
                "display_name": "Zed User",
            },
        }),
    )])
}

fn test_now() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_770_000_000).expect("valid now")
}
