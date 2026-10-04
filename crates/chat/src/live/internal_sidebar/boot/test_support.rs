use std::collections::HashMap;

use super::types::{
    SlackBootState, SlackClientBootResponse, SlackClientCountsResponse, SlackDirectMessagePresence,
    SlackDirectMessageUser,
};

type SlackDirectMessageUserFixture<'a> = (&'a str, &'a str, &'a str, SlackDirectMessagePresence);
type SlackDirectMessageUserFixtures<'a> = Vec<SlackDirectMessageUserFixture<'a>>;

impl SlackBootState {
    pub(in crate::live::internal_sidebar) fn for_tests(
        boot: SlackClientBootResponse,
        counts: SlackClientCountsResponse,
    ) -> Self {
        Self {
            boot,
            counts,
            dm_users: HashMap::new(),
            group_message_labels: HashMap::new(),
            self_presence: None,
            self_notifications_paused: false,
        }
    }

    pub(in crate::live::internal_sidebar) fn for_tests_with_dm_users(
        boot: SlackClientBootResponse,
        counts: SlackClientCountsResponse,
        dm_users: SlackDirectMessageUserFixtures<'_>,
    ) -> Self {
        Self {
            boot,
            counts,
            dm_users: dm_users
                .into_iter()
                .map(|(conversation_id, user_id, label, presence)| {
                    (
                        conversation_id.to_string(),
                        SlackDirectMessageUser {
                            user_id: user_id.to_string(),
                            label: label.to_string(),
                            avatar_image_url: None,
                            secondary_context: None,
                            presence: Some(presence),
                        },
                    )
                })
                .collect(),
            group_message_labels: HashMap::new(),
            self_presence: None,
            self_notifications_paused: false,
        }
    }

    pub(in crate::live::internal_sidebar) fn with_group_message_label(
        mut self,
        conversation_id: &str,
        label: &str,
    ) -> Self {
        self.group_message_labels
            .insert(conversation_id.to_string(), label.to_string());
        self
    }
}
