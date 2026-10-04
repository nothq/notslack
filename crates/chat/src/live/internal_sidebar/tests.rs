use super::{
    boot::{self, SlackBootState},
    sections::SlackChannelSectionsResponse,
    SlackWebSessionCredentials,
};
use crate::live::payload::sidebar_dom::SlackSidebarSnapshot;
use crate::live::SlackWebBuildTimestamp;

#[test]
fn requires_cookie_bound_client_credentials() {
    let web_build_timestamp = SlackWebBuildTimestamp::parse("1").expect("web build timestamp");
    let credential =
        SlackWebSessionCredentials::new("xoxc-token", "d=x; x=x", web_build_timestamp.clone())
            .expect("credential");
    assert_eq!(credential.xoxc_token(), "xoxc-token");
    assert_eq!(credential.cookie_header(), "d=x; x=x");
    assert!(SlackWebSessionCredentials::new("xoxb-token", "d=x", web_build_timestamp).is_err());
}

#[test]
fn empty_sections_and_boot_channels_fail_hard() {
    let boot = test_boot(r#"[]"#, r#"[]"#);
    let error = SlackSidebarSnapshot::from_internal_boot_state(test_sections(false), boot)
        .expect_err("hard failure");
    assert!(error.contains("no channel ids"));
}

#[test]
fn boot_channels_feed_empty_channel_section_in_slack_name_order() {
    let boot = test_boot(
        r#"[{"id":"C2","name":"billing","is_channel":true},{"id":"C1","name":"2fa","is_channel":true},{"id":"D1","is_im":true}]"#,
        r#"[{"id":"C2"},{"id":"C1"}]"#,
    );
    let snapshot = SlackSidebarSnapshot::from_internal_boot_state(test_sections(false), boot)
        .expect("snapshot");
    assert_eq!(snapshot.sections[0].items[0].id, "C1");
    assert_eq!(snapshot.sections[0].items[1].id, "C2");
}

#[test]
fn slack_connect_channels_are_not_mixed_into_channels() {
    let boot = SlackBootState::for_tests_with_dm_users(
        serde_json::from_str(&format!(
            r#"{{"ok":true,"self":{{"id":"U0","is_admin":false}},"channels":[{{"id":"C1","name":"2fa","is_channel":true}},{{"id":"C2","name":"eng-relay","is_channel":true,"is_shared":true}}],"ims":[{{"id":"D1","user":"U1","is_im":true,"is_shared":true}},{{"id":"D2","user":"U2","is_im":true,"is_shared":true}},{{"id":"D3","user":"U3","is_im":true,"is_shared":true}}],"channels_priority":{{"D1":0.04,"D2":0.07}},"prefs":{SUPPORTED_PREFS}}}"#
        ))
        .expect("boot"),
        serde_json::from_str(
            r#"{"ok":true,"channels":[{"id":"C1"},{"id":"C2"}],"ims":[{"id":"D1"},{"id":"D2"},{"id":"D3"}],"mpims":[]}"#,
        )
        .expect("counts"),
        vec![
            (
                "D1",
                "U1",
                "Northwind Traders",
                boot::SlackDirectMessagePresence::Away,
            ),
            (
                "D2",
                "U2",
                "Alex M",
                boot::SlackDirectMessagePresence::Active,
            ),
            (
                "D3",
                "U3",
                "John Backus",
                boot::SlackDirectMessagePresence::Active,
            ),
        ],
    );
    let snapshot = SlackSidebarSnapshot::from_internal_boot_state(test_two_sections(), boot)
        .expect("snapshot");
    assert_eq!(snapshot.sections[0].key, "external_connections");
    assert_eq!(snapshot.sections[0].label, "External connections");
    assert_eq!(snapshot.sections[0].items[0].id, "D1");
    assert_eq!(snapshot.sections[0].items[0].label, "Northwind Traders");
    assert_eq!(snapshot.sections[0].items[1].id, "D2");
    assert_eq!(snapshot.sections[0].items[2].id, "D3");
    assert_eq!(snapshot.sections[0].items[3].id, "C2");
    assert_eq!(snapshot.sections[1].key, "channels");
    assert_eq!(snapshot.sections[1].items[0].id, "C1");
}

#[test]
fn explicit_section_items_override_boot_order() {
    let boot = test_boot(
        r#"[{"id":"C1","name":"2fa","is_channel":true},{"id":"C2","name":"billing","is_channel":true}]"#,
        r#"[{"id":"C1"},{"id":"C2"}]"#,
    );
    let snapshot = SlackSidebarSnapshot::from_internal_boot_state(test_sections(true), boot)
        .expect("snapshot");
    assert_eq!(snapshot.sections[0].items[0].id, "C2");
    assert_eq!(snapshot.sections[0].items[1].id, "C1");
}

#[test]
fn client_counts_feed_activity_state_into_snapshot() {
    let boot = SlackBootState::for_tests(
        serde_json::from_str(&format!(
            r#"{{"ok":true,"self":{{"id":"U0","is_admin":false}},"channels":[{{"id":"C1","name":"standup","is_channel":true}}],"prefs":{SUPPORTED_PREFS}}}"#
        ))
        .expect("boot"),
        serde_json::from_str(
            r#"{"ok":true,"activity_v2":{"dm":1,"bot_dm_bundle":2,"generic_system_alert":9},"channels":[{"id":"C1","unread_count":0}],"ims":[],"mpims":[]}"#,
        )
        .expect("counts"),
    );

    let snapshot = SlackSidebarSnapshot::from_internal_boot_state(test_sections(false), boot)
        .expect("snapshot");

    assert_eq!(snapshot.activity_count, Some(12));
}

#[test]
fn empty_sections_use_priority_and_hide_dormant_count_channels() {
    let boot = test_boot_with_raw_boot(
        &format!(
            r#"{{"ok":true,"self":{{"id":"U0","is_admin":false}},"channels":[{{"id":"C1","name":"2fa","is_channel":true}},{{"id":"C2","name":"hidden","is_channel":true,"properties":{{"is_dormant":true}}}},{{"id":"C3","name":"billing","is_channel":true,"properties":{{"is_dormant":true}}}},{{"id":"C4","name":"news","is_channel":true,"properties":{{"is_dormant":true,"meeting_notes":{{}}}},"purpose":{{"value":""}},"topic":{{"value":""}}}},{{"id":"C5","name":"private-project","is_channel":true,"is_private":true,"properties":{{"is_dormant":true,"meeting_notes":{{}}}},"purpose":{{"value":""}},"topic":{{"value":""}}}}],"channels_priority":{{"C3":1.0}},"prefs":{SUPPORTED_PREFS}}}"#
        ),
        r#"[{"id":"C2"},{"id":"C1"},{"id":"C3"},{"id":"C4"},{"id":"C5"}]"#,
    );
    let snapshot = SlackSidebarSnapshot::from_internal_boot_state(test_sections(false), boot)
        .expect("snapshot");

    let ids = snapshot.sections[0]
        .items
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, vec!["C1", "C3", "C4"]);
}

#[test]
fn empty_direct_messages_sort_by_presence_then_label() {
    let boot = SlackBootState::for_tests_with_dm_users(
        serde_json::from_str(&format!(
            r#"{{"ok":true,"self":{{"id":"U0","is_admin":false}},"channels":[{{"id":"C4","name":"mpdm-edsger--margaret--self-1","is_mpim":true}}],"ims":[{{"id":"D1","is_im":true,"user":"U1"}},{{"id":"D2","is_im":true,"user":"U2"}},{{"id":"D3","is_im":true,"user":"U3"}}],"channels_priority":{{"D1":0.9,"D2":0.1,"D3":0.8}},"prefs":{SUPPORTED_PREFS}}}"#
        ))
        .expect("boot"),
        serde_json::from_str(
            r#"{"ok":true,"channels":[],"ims":[{"id":"D1"},{"id":"D2"},{"id":"D3"}],"mpims":[{"id":"C4","unread_count":3,"unread_count_display":2}]}"#,
        )
        .expect("counts"),
        vec![
            (
                "D1",
                "U1",
                "Zed User",
                boot::SlackDirectMessagePresence::Away,
            ),
            (
                "D2",
                "U2",
                "Margaret User",
                boot::SlackDirectMessagePresence::Active,
            ),
            (
                "D3",
                "U3",
                "Bob User",
                boot::SlackDirectMessagePresence::Active,
            ),
        ],
    )
    .with_group_message_label(
        "C4",
        "Edsger Dijkstra, Linus Torvalds, Barbara Liskov, Ken",
    );
    let snapshot =
        SlackSidebarSnapshot::from_internal_boot_state(test_direct_message_sections(), boot)
            .expect("snapshot");

    let items = &snapshot.sections[0].items;
    assert_eq!(items[0].id, "C4");
    assert_eq!(
        items[0].label,
        "Edsger Dijkstra, Linus Torvalds, Barbara Liskov, Ken"
    );
    assert_eq!(items[0].label.split(',').count(), 4);
    assert_eq!(items[0].kind.as_deref(), Some("group_message"));
    assert_eq!(items[0].count, Some(2));
    assert!(items[0].user_id.is_none());
    assert!(items[0].avatar_image_url.is_none());
    assert_eq!(items[1].id, "D2");
    assert_eq!(items[1].label, "Margaret User");
    assert_eq!(items[1].user_id.as_deref(), Some("U2"));
    assert_eq!(items[2].id, "D3");
    assert_eq!(items[3].id, "D1");
}

#[test]
fn unsupported_sidebar_sort_preference_fails_hard() {
    let boot = test_boot_with_raw_boot(
        r#"{"ok":true,"self":{"id":"U0","is_admin":false},"channels":[{"id":"C1","name":"2fa","is_channel":true}],"prefs":{"channel_sort":"recent","sidebar_behavior":"hide_inactive_channels","separate_shared_channels":true,"separate_private_channels":false,"hide_muted_channels_from_sidebar":false,"remove_sidebar_customizations":false,"undo_channel_intermix":false}}"#,
        r#"[{"id":"C1"}]"#,
    );
    let error = SlackSidebarSnapshot::from_internal_boot_state(test_sections(false), boot)
        .expect_err("unsupported preference should fail");

    assert!(error.contains("channel_sort=recent"));
}

#[test]
fn client_counts_without_unread_state_fails_hard() {
    let error =
        boot::decode_client_counts(r#"{"ok":true,"channels":[{"id":"C1"}],"ims":[],"mpims":[]}"#)
            .expect_err("untrusted counts shape should fail");

    assert!(error.contains("unread state fields"));
}

#[test]
fn client_counts_without_activity_state_fails_hard() {
    let error = boot::decode_client_counts(
        r#"{"ok":true,"channels":[{"id":"C1","unread_count":0}],"ims":[],"mpims":[]}"#,
    )
    .expect_err("untrusted counts shape should fail");

    assert!(error.contains("activity rail count fields"));
}

const SUPPORTED_PREFS: &str = r#"{"channel_sort":"default","sidebar_behavior":"hide_inactive_channels","separate_shared_channels":true,"separate_private_channels":false,"hide_muted_channels_from_sidebar":false,"remove_sidebar_customizations":false,"undo_channel_intermix":false}"#;

fn test_boot(channels: &str, counts: &str) -> SlackBootState {
    let boot = format!(
        r#"{{"ok":true,"self":{{"id":"U0","is_admin":false}},"channels":{channels},"prefs":{SUPPORTED_PREFS}}}"#
    );
    test_boot_with_raw_boot(&boot, counts)
}

fn test_boot_with_raw_boot(boot: &str, counts: &str) -> SlackBootState {
    SlackBootState::for_tests(
        serde_json::from_str(boot).expect("boot"),
        serde_json::from_str(&format!(
            r#"{{"ok":true,"channels":{counts},"ims":[],"mpims":[]}}"#
        ))
        .expect("counts"),
    )
}

fn test_sections(with_ids: bool) -> SlackChannelSectionsResponse {
    let ids = if with_ids { r#"["C2","C1"]"# } else { "[]" };
    serde_json::from_str(&format!(
        r#"{{"ok":true,"channel_sections":[{{"channel_section_id":"S1","type":"channels","name":"Channels","channel_ids_page":{{"channel_ids":{ids},"count":0}}}}]}}"#
    ))
    .expect("sections")
}

fn test_two_sections() -> SlackChannelSectionsResponse {
    serde_json::from_str(
        r#"{"ok":true,"channel_sections":[{"channel_section_id":"S0","type":"slack_connect","name":"Slack Connect","channel_ids_page":{"channel_ids":[],"count":0}},{"channel_section_id":"S1","type":"channels","name":"Channels","channel_ids_page":{"channel_ids":[],"count":0}}]}"#,
    )
    .expect("sections")
}

fn test_direct_message_sections() -> SlackChannelSectionsResponse {
    serde_json::from_str(
        r#"{"ok":true,"channel_sections":[{"channel_section_id":"S1","type":"direct_messages","name":"Direct messages","channel_ids_page":{"channel_ids":[],"count":0}}]}"#,
    )
    .expect("sections")
}
