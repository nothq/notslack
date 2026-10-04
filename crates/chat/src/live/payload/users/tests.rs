use serde_json::json;
use time::{Duration, OffsetDateTime};

use crate::live::payload::users::*;

#[gpui::test]
fn workspace_user_ids_skip_workspace_wide_dm_fetch_when_sidebar_snapshot_exists() {
    let channel_info = json!({
        "channel": {
            "id": "D_ACTIVE",
            "is_im": true,
            "user": "U_ACTIVE",
        }
    });
    let conversations = json!({
        "channels": [
            {
                "id": "D_ACTIVE",
                "is_im": true,
                "user": "U_ACTIVE",
                "updated": 1_800_000_000_000i64,
            },
            {
                "id": "D_OTHER",
                "is_im": true,
                "user": "U_OTHER",
                "updated": 1_800_000_000_000i64,
            }
        ]
    });
    let history = json!({ "messages": [{ "user": "U_AUTHOR" }] });
    let sidebar_snapshot = direct_message_sidebar_snapshot("D_OTHER", "U_OTHER");

    let user_ids = collect_workspace_user_ids(&WorkspaceUserLoadInput {
        active_conversation_id: "D_ACTIVE",
        channel_info: &channel_info,
        conversations: &conversations,
        history: &history,
        sidebar_snapshot: Some(&sidebar_snapshot),
        now: OffsetDateTime::UNIX_EPOCH,
    });

    assert_eq!(
        user_ids,
        BTreeSet::from([
            "U_ACTIVE".to_string(),
            "U_AUTHOR".to_string(),
            "U_OTHER".to_string(),
        ])
    );
}

fn direct_message_sidebar_snapshot(
    conversation_id: &str,
    user_id: &str,
) -> super::super::sidebar_dom::SlackSidebarSnapshot {
    super::super::sidebar_dom::SlackSidebarSnapshot {
        draft_count: None,
        activity_count: None,
        dms_unread_messages: None,
        admin_visible: false,
        admin_attention: false,
        self_presence: None,
        self_notifications_paused: false,
        direct_message_unread_states: Vec::new(),
        sections: vec![super::super::sidebar_dom::SlackSidebarSnapshotSection {
            key: "direct_messages".to_string(),
            label: "Direct messages".to_string(),
            items: vec![super::super::sidebar_dom::SlackSidebarSnapshotItem {
                id: conversation_id.to_string(),
                label: String::new(),
                secondary_context: None,
                avatar_image_url: None,
                is_external_connection: false,
                user_id: Some(user_id.to_string()),
                presence: None,
                kind: None,
                selected: false,
                unread: false,
                count: None,
                latest_message_timestamp: None,
            }],
        }],
    }
}

#[gpui::test]
fn workspace_user_ids_include_recent_and_unread_dm_sidebar_entries_without_snapshot() {
    let now = OffsetDateTime::UNIX_EPOCH + Duration::days(60);
    let channel_info = json!({
        "channel": {
            "id": "D_ACTIVE",
            "is_im": true,
            "user": "U_ACTIVE",
        }
    });
    let conversations = json!({
        "channels": [
            {
                "id": "D_ACTIVE",
                "is_im": true,
                "user": "U_ACTIVE",
                "updated": 2_592_000_000i64,
            },
            {
                "id": "D_UNREAD_OLD",
                "is_im": true,
                "user": "U_UNREAD_OLD",
                "updated": 1_000i64,
                "latest": "1769000000.000100",
                "last_read": "1768000000.000100",
            },
            {
                "id": "D_OLD",
                "is_im": true,
                "user": "U_OLD",
                "updated": 1_000i64,
            }
        ]
    });
    let history = json!({
        "messages": [
            { "user": "U_AUTHOR" },
            { "user": "U_OTHER_AUTHOR" }
        ]
    });

    let user_ids = collect_workspace_user_ids(&WorkspaceUserLoadInput {
        active_conversation_id: "D_ACTIVE",
        channel_info: &channel_info,
        conversations: &conversations,
        history: &history,
        sidebar_snapshot: None,
        now,
    });

    assert_eq!(
        user_ids,
        BTreeSet::from([
            "U_ACTIVE".to_string(),
            "U_AUTHOR".to_string(),
            "U_OTHER_AUTHOR".to_string(),
            "U_UNREAD_OLD".to_string(),
        ])
    );
}
