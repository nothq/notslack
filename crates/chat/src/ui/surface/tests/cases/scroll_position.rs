use crate::ui::surface::tests::cases::*;

#[gpui::test]
fn slack_channel_selection_opens_at_last_message() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let deploys = slack_test_workspace_with_sections(
        "C_DEPLOYS",
        "deploys",
        vec![slack_test_channels_section(true)],
        24,
    );
    let (api, _) = slack_test_api_with_conversations(HashMap::from([
        (
            "C_DESIGN".to_string(),
            slack_test_workspace("C_DESIGN", "design", false),
        ),
        ("C_DEPLOYS".to_string(), deploys),
    ]));
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open slack scroll position test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack scroll position root");
    root.update(&mut cx, |app, cx| {
        app.root.scroll_message_list(-360.0, cx);
        app.root.select_conversation("C_DEPLOYS", cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(
            app.root.slack_conversation_id(cx).as_deref(),
            Some("C_DEPLOYS")
        );
        assert_eq!(app.root.slack_message_list_item_count(cx), 24);
        assert!(app.root.slack_message_list_item_visible(23, cx));
    });
}
