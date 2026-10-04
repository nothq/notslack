use crate::model::{SlackDirectMessageUnreadState, SlackMessageTimestamp, SlackUserPresence};
use serde::Deserialize;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct SlackSidebarSnapshot {
    #[serde(default)]
    pub(crate) draft_count: Option<u32>,
    #[serde(default)]
    pub(crate) activity_count: Option<u32>,
    #[serde(default)]
    pub(crate) dms_unread_messages: Option<u32>,
    #[serde(default)]
    pub(crate) admin_visible: bool,
    #[serde(default)]
    pub(crate) admin_attention: bool,
    #[serde(default)]
    pub(crate) self_presence: Option<SlackUserPresence>,
    #[serde(default)]
    pub(crate) self_notifications_paused: bool,
    #[serde(default)]
    pub(crate) direct_message_unread_states: Vec<SlackDirectMessageUnreadState>,
    #[serde(default)]
    pub(crate) sections: Vec<SlackSidebarSnapshotSection>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct SlackSidebarSnapshotSection {
    pub(crate) key: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) items: Vec<SlackSidebarSnapshotItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(crate) struct SlackSidebarSnapshotItem {
    pub(crate) id: String,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) secondary_context: Option<String>,
    #[serde(default)]
    pub(crate) avatar_image_url: Option<String>,
    #[serde(default)]
    pub(crate) is_external_connection: bool,
    #[serde(default)]
    pub(crate) user_id: Option<String>,
    #[serde(default)]
    pub(crate) presence: Option<SlackUserPresence>,
    #[serde(default)]
    pub(crate) kind: Option<String>,
    #[serde(default)]
    pub(crate) selected: bool,
    #[serde(default)]
    pub(crate) unread: bool,
    #[serde(default)]
    pub(crate) count: Option<u32>,
    #[serde(default)]
    pub(crate) latest_message_timestamp: Option<SlackMessageTimestamp>,
}

impl SlackSidebarSnapshot {
    pub(super) fn item(&self, id: &str) -> Option<&SlackSidebarSnapshotItem> {
        self.sections
            .iter()
            .flat_map(|section| section.items.iter())
            .find(|item| item.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::{SlackSidebarSnapshot, SlackSidebarSnapshotItem, SlackSidebarSnapshotSection};

    #[gpui::test]
    fn item_lookup_matches_id_across_sections() {
        let snapshot = SlackSidebarSnapshot {
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
                    items: vec![SlackSidebarSnapshotItem {
                        id: "C123".to_string(),
                        label: "standup".to_string(),
                        secondary_context: None,
                        avatar_image_url: None,
                        is_external_connection: false,
                        user_id: None,
                        kind: None,
                        presence: None,
                        selected: false,
                        unread: false,
                        count: None,
                        latest_message_timestamp: None,
                    }],
                },
                SlackSidebarSnapshotSection {
                    key: "direct_messages".to_string(),
                    label: "Direct messages".to_string(),
                    items: vec![SlackSidebarSnapshotItem {
                        id: "C456".to_string(),
                        label: "Edsger Dijkstra, Barbara Liskov".to_string(),
                        secondary_context: None,
                        avatar_image_url: None,
                        is_external_connection: false,
                        user_id: None,
                        kind: None,
                        presence: None,
                        selected: false,
                        unread: false,
                        count: None,
                        latest_message_timestamp: None,
                    }],
                },
            ],
        };
        assert_eq!(
            snapshot.item("C456").unwrap().label,
            "Edsger Dijkstra, Barbara Liskov"
        );
        assert!(snapshot.item("C999").is_none());
    }
}
