use std::collections::HashSet;

use crate::ui::surface::sidebar::rows::{
    build_slack_sidebar_rows, shows_slack_more_unreads_above_pill,
    shows_slack_more_unreads_below_pill,
};
use crate::ui::SlackConversationKind;

use super::support::{
    sidebar_item_index, slack_channel_section, slack_sidebar_channel, slack_sidebar_item,
    slack_sidebar_test_workspace, SlackSidebarItemSpec, SlackSidebarWorkspaceSpec,
};

#[gpui::test]
fn more_unread_pill_requires_hidden_unread_row_above_viewport() {
    let workspace = slack_sidebar_test_workspace(SlackSidebarWorkspaceSpec {
        team_id: "TTEST".to_string(),
        conversation_id: "C_AICRAZE".to_string(),
        channel_kind: SlackConversationKind::Channel,
        channel_name: "design".to_string(),
        member_count: Some(8),
        sections: vec![slack_channel_section(vec![
            slack_sidebar_channel("standup", "C_DAILY", false, true, None),
            slack_sidebar_channel("design", "C_AICRAZE", true, false, None),
            slack_sidebar_channel("customer_success", "C_CUSTOMER", false, false, None),
        ])],
        composer_placeholder: "Message #design".to_string(),
    });
    let rows = build_slack_sidebar_rows(Some(&workspace), &HashSet::new());
    let unread_index = sidebar_item_index(&rows, "standup");

    assert!(!shows_slack_more_unreads_above_pill(&rows, |_| false));
    assert!(shows_slack_more_unreads_above_pill(&rows, |index| {
        index == unread_index
    }));
}

#[gpui::test]
fn more_unread_pill_ignores_visible_and_active_unread_rows() {
    let workspace = slack_sidebar_test_workspace(SlackSidebarWorkspaceSpec {
        team_id: "TTEST".to_string(),
        conversation_id: "C_AICRAZE".to_string(),
        channel_kind: SlackConversationKind::Channel,
        channel_name: "design".to_string(),
        member_count: Some(8),
        sections: vec![slack_channel_section(vec![
            slack_sidebar_channel("design", "C_AICRAZE", true, true, Some(2)),
            slack_sidebar_channel("standup", "C_DAILY", false, false, None),
        ])],
        composer_placeholder: "Message #design".to_string(),
    });
    let rows = build_slack_sidebar_rows(Some(&workspace), &HashSet::new());

    assert!(!shows_slack_more_unreads_above_pill(&rows, |_| true));
    assert!(!shows_slack_more_unreads_below_pill(&rows, |_| true));
}

#[gpui::test]
fn more_unreads_below_pill_requires_hidden_unread_row() {
    let workspace = slack_sidebar_test_workspace(SlackSidebarWorkspaceSpec {
        team_id: "TTEST".to_string(),
        conversation_id: "C_AICRAZE".to_string(),
        channel_kind: SlackConversationKind::Channel,
        channel_name: "design".to_string(),
        member_count: Some(8),
        sections: vec![
            slack_channel_section(vec![
                slack_sidebar_channel("design", "C_AICRAZE", true, false, None),
                slack_sidebar_channel("customer_success", "C_CUSTOMER", false, true, Some(2)),
            ]),
            crate::ui::SlackSidebarSection {
                label: "Direct messages".to_string(),
                items: vec![slack_sidebar_item(SlackSidebarItemSpec {
                    label: "Donald Knuth",
                    icon: Some("person"),
                    target_id: "D_OMAR",
                    target_kind: SlackConversationKind::DirectMessage,
                    active: false,
                    unread: true,
                    count: Some(1),
                })],
            },
        ],
        composer_placeholder: "Message #design".to_string(),
    });
    let rows = build_slack_sidebar_rows(Some(&workspace), &HashSet::new());
    let unread_index = sidebar_item_index(&rows, "customer_success");

    assert!(!shows_slack_more_unreads_below_pill(&rows, |_| false));
    assert!(shows_slack_more_unreads_below_pill(&rows, |index| {
        index == unread_index
    }));
}

#[gpui::test]
fn more_unreads_below_pill_ignores_activity_count_without_hidden_unread_row() {
    let mut workspace = slack_sidebar_test_workspace(SlackSidebarWorkspaceSpec {
        team_id: "TTEST".to_string(),
        conversation_id: "C_AICRAZE".to_string(),
        channel_kind: SlackConversationKind::Channel,
        channel_name: "design".to_string(),
        member_count: Some(8),
        sections: vec![slack_channel_section(vec![slack_sidebar_channel(
            "design",
            "C_AICRAZE",
            true,
            false,
            None,
        )])],
        composer_placeholder: "Message #design".to_string(),
    });
    workspace.rail_badges.activity = Some(12);
    let rows = build_slack_sidebar_rows(Some(&workspace), &HashSet::new());

    assert!(!shows_slack_more_unreads_below_pill(&rows, |_| true));
}

#[gpui::test]
fn more_unreads_below_pill_uses_unread_state_without_mention_count() {
    let workspace = slack_sidebar_test_workspace(SlackSidebarWorkspaceSpec {
        team_id: "TTEST".to_string(),
        conversation_id: "C_AICRAZE".to_string(),
        channel_kind: SlackConversationKind::Channel,
        channel_name: "design".to_string(),
        member_count: Some(8),
        sections: vec![slack_channel_section(vec![
            slack_sidebar_channel("design", "C_AICRAZE", true, false, None),
            slack_sidebar_channel("customer_success", "C_CUSTOMER", false, true, Some(0)),
            slack_sidebar_channel("random", "C_RANDOM", false, true, None),
        ])],
        composer_placeholder: "Message #design".to_string(),
    });
    let rows = build_slack_sidebar_rows(Some(&workspace), &HashSet::new());
    let unread_index = sidebar_item_index(&rows, "random");

    assert!(shows_slack_more_unreads_below_pill(&rows, |index| {
        index == unread_index
    }));
}
