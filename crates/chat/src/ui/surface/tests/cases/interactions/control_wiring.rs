use gpui::{AppContext as _, Context};

use crate::ui::surface::tests::cases::{
    acquire_headless_test_lock, app_size, headless_test_context, named_key_down_event,
    text_key_down_event, SlackTestApp,
};
use crate::ui::test_support::{slack_test_api, slack_test_board};

#[gpui::test]
fn slack_panels_composer_tools_and_section_toggles_are_wired() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api();
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open slack controls test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack controls root");
    root.update(&mut cx, exercise_slack_panel_and_composer_controls);
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(
            app.root.slack_conversation_id(cx).as_deref(),
            Some("C_DEPLOYS")
        );
    });
}

fn exercise_slack_panel_and_composer_controls(
    app: &mut SlackTestApp,
    cx: &mut Context<SlackTestApp>,
) {
    exercise_slack_search_and_sections(app, cx);
    exercise_slack_panel_navigation(app, cx);
    exercise_slack_channel_controls(app, cx);
    exercise_slack_composer_controls(app, cx);
}

fn exercise_slack_search_and_sections(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    app.root.open_slack_search_panel(cx);
    assert_slack_aux_panel_title(app, "Search", cx);
    assert!(app
        .root
        .handle_slack_key_down(&text_key_down_event("d"), cx));
    assert!(app
        .root
        .handle_slack_key_down(&text_key_down_event("e"), cx));
    assert!(app
        .root
        .handle_slack_key_down(&text_key_down_event("p"), cx));
    assert_eq!(
        app.root
            .slack_aux_panel(cx)
            .as_ref()
            .and_then(|panel| panel.query.as_deref()),
        Some("dep")
    );
    assert!(app
        .root
        .handle_slack_key_down(&named_key_down_event("enter"), cx));
    app.root.toggle_slack_section("Channels", cx);
    assert!(app.root.is_slack_section_collapsed("Channels", cx));
    app.root.toggle_slack_formatting(cx);
    assert!(app.root.slack_formatting_enabled(cx));
}

fn exercise_slack_panel_navigation(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    app.root.open_slack_help_panel(cx);
    assert_slack_aux_panel_title(app, "Slack help", cx);
    app.root.open_slack_workspace_panel(cx);
    assert_slack_aux_panel_title(app, "Workspace", cx);
    app.root.open_slack_self_panel(cx);
    assert_slack_aux_panel_title(app, "Profile", cx);
    app.root.open_slack_members_panel(cx);
    assert_slack_aux_panel_title(app, "Members", cx);
}

fn exercise_slack_channel_controls(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    app.root.open_slack_channel_menu(cx);
    assert_slack_aux_panel_title(app, "#design", cx);
    app.root.toggle_slack_conversation_starred(cx);
    assert!(app.root.is_slack_conversation_starred(cx));
    app.root.toggle_slack_channel_details(cx);
    assert!(app.root.slack_channel_details_visible(cx));
    assert_slack_aux_panel_title(app, "#design", cx);
    app.root.toggle_slack_conversation_muted(cx);
    assert!(app.root.slack_conversation_muted(cx));
    app.root.toggle_slack_huddle(cx);
    assert!(app.root.slack_huddle_active(cx));
    assert_slack_aux_panel_title(app, "Huddle", cx);
}

fn exercise_slack_composer_controls(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    app.root.insert_slack_composer_snippet("@here", cx);
    assert!(app.root.slack_composer_text(cx).contains("@here"));
    assert!(app.root.slack_composer_focused(cx));
    app.root.open_slack_send_options(cx);
    assert_slack_aux_panel_title(app, "Send options", cx);
}

fn assert_slack_aux_panel_title(
    app: &mut SlackTestApp,
    expected: &str,
    cx: &mut Context<SlackTestApp>,
) {
    assert_eq!(
        app.root
            .slack_aux_panel(cx)
            .as_ref()
            .map(|panel| panel.title.as_str()),
        Some(expected)
    );
}
