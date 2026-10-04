use std::collections::HashSet;

use crate::ui::surface::sidebar::{SlackSidebarRowKind, SlackSidebarSectionIndicator};
use crate::ui::SlackConversationKind;

use super::support::{
    sidebar_section_index, slack_channel_section, slack_sidebar_item, slack_sidebar_test_workspace,
    SlackSidebarItemSpec, SlackSidebarWorkspaceSpec,
};

#[gpui::test]
fn build_slack_sidebar_rows_recognizes_title_cased_direct_messages_section() {
    let workspace = slack_sidebar_test_workspace(SlackSidebarWorkspaceSpec {
        team_id: "TTEST".to_string(),
        conversation_id: "D_BORIS".to_string(),
        channel_kind: SlackConversationKind::DirectMessage,
        channel_name: "Grace".to_string(),
        member_count: None,
        sections: vec![crate::ui::SlackSidebarSection {
            label: "Direct Messages".to_string(),
            items: vec![slack_sidebar_item(SlackSidebarItemSpec {
                label: "Grace",
                icon: Some("person"),
                target_id: "D_BORIS",
                target_kind: SlackConversationKind::DirectMessage,
                active: true,
                unread: false,
                count: None,
            })],
        }],
        composer_placeholder: "Message Grace".to_string(),
    });

    let rows = crate::ui::surface::sidebar::rows::build_slack_sidebar_rows(
        Some(&workspace),
        &HashSet::new(),
    );

    assert!(rows.iter().any(|row| matches!(
        &row.kind,
        SlackSidebarRowKind::SectionHeader {
            label,
            indicator: SlackSidebarSectionIndicator::DirectMessages,
            ..
        } if label == "Direct messages"
    )));
}

#[gpui::test]
fn build_slack_sidebar_rows_adds_static_slack_prefix_for_live_workspaces() {
    let mut workspace = slack_sidebar_test_workspace(SlackSidebarWorkspaceSpec {
        team_id: "TTEST".to_string(),
        conversation_id: "C_BILLING".to_string(),
        channel_kind: SlackConversationKind::PrivateChannel,
        channel_name: "billing".to_string(),
        member_count: Some(6),
        sections: vec![slack_channel_section(vec![slack_sidebar_item(
            SlackSidebarItemSpec {
                label: "billing",
                icon: Some("lock"),
                target_id: "C_BILLING",
                target_kind: SlackConversationKind::PrivateChannel,
                active: true,
                unread: true,
                count: None,
            },
        )])],
        composer_placeholder: "Message #billing".to_string(),
    });
    workspace.rail_badges.drafts_sent = Some(5);

    let rows = crate::ui::surface::sidebar::rows::build_slack_sidebar_rows(
        Some(&workspace),
        &HashSet::new(),
    );

    assert!(matches!(
        &rows[0].kind,
        SlackSidebarRowKind::Shortcut { label, .. } if label == "Threads"
    ));
    assert!(matches!(
        &rows[1].kind,
        SlackSidebarRowKind::Shortcut { label, .. } if label == "Huddles"
    ));
    assert!(matches!(
        &rows[2].kind,
        SlackSidebarRowKind::Shortcut {
            label,
            badge: Some(badge),
            ..
        } if label == "Drafts & sent" && badge == "5"
    ));
    assert!(matches!(
        &rows[3].kind,
        SlackSidebarRowKind::Shortcut { label, .. } if label == "Directories"
    ));

    let starred_index = sidebar_section_index(&rows, "Starred");
    let channels_index = sidebar_section_index(&rows, "Channels");
    assert!(starred_index < channels_index);
}
