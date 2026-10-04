use std::{sync::Arc, time::Duration};

use gpui::{AppContext as _, Context};

use crate::ui::surface::tests::cases::{
    acquire_headless_test_lock, app_size, headless_test_context, SlackTestApp,
};
use crate::ui::{
    test_support::{slack_test_api, slack_test_board, slow_slack_test_api},
    SlackFilesFilter, SlackMainTab, SlackProfilePanelState, SlackRailView, SlackUploadFile,
};

#[gpui::test]
fn slack_sidebar_selection_switches_conversation() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api();
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open slack selection test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack selection root");
    root.update(&mut cx, prepare_draft_and_switch_conversation);
    cx.run_until_parked();

    root.update(&mut cx, verify_target_and_switch_back);
    cx.run_until_parked();

    root.update(&mut cx, verify_restored_draft);
}

fn prepare_draft_and_switch_conversation(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    app.root
        .set_slack_composer_text("draft for design".to_string(), cx)
        .expect("test composer text should be accepted");
    app.root.attach_slack_upload_files(
        vec![SlackUploadFile::from_bytes(
            "draft.txt",
            Arc::from(b"draft attachment".as_slice()),
            "text/plain",
        )
        .expect("test Slack upload should be valid")],
        cx,
    );
    app.root.select_conversation("C_DEPLOYS", cx);
}

fn verify_target_and_switch_back(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    let workspace = app
        .root
        .slack_workspace(cx)
        .expect("slack workspace should remain available");
    assert_eq!(workspace.conversation_id, "C_DEPLOYS");
    assert_eq!(workspace.channel_name, "deploys");
    assert!(workspace
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
        .any(|item| item.target_id == "C_DEPLOYS" && item.active));
    assert!(app.root.slack_composer_text(cx).is_empty());
    assert!(app.root.slack_draft_attachments_empty(cx));
    assert!(app.root.slack_draft_upload_files_empty(cx));
    app.root.select_conversation("C_AICRAZE", cx);
}

fn verify_restored_draft(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    assert_eq!(
        app.root.slack_composer_text(cx),
        "draft for design".to_string()
    );
    assert_eq!(app.root.slack_draft_attachment_count(cx), 1);
    assert_eq!(
        app.root.slack_draft_attachment_mimetypes(cx),
        vec!["text/plain".to_string()]
    );
    assert!(!app.root.slack_draft_upload_files_empty(cx));
}

#[gpui::test]
fn slack_channel_switching_is_latest_wins_with_parallel_loads() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _, load_metrics) = slow_slack_test_api(Duration::from_millis(30));
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open slack parallel channel switch test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack parallel channel switch root");
    root.update(&mut cx, |app, cx| {
        app.root.select_conversation("C_DEPLOYS", cx);
        app.root.select_conversation("C_RANDOM", cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(
            app.root.slack_conversation_id(cx).as_deref(),
            Some("C_RANDOM")
        );
    });

    let metrics = load_metrics
        .lock()
        .expect("slack load metrics mutex poisoned")
        .clone();
    let mut requested_conversation_ids = metrics.requested_conversation_ids;
    requested_conversation_ids.sort();
    assert_eq!(
        requested_conversation_ids,
        vec!["C_DEPLOYS".to_string(), "C_RANDOM".to_string()]
    );
}

#[gpui::test]
fn slack_user_click_loads_profile_panel() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api();
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open slack profile test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack profile root");
    root.update(&mut cx, |app, cx| {
        app.root.open_slack_profile("U_ADA", cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert!(matches!(
            app.root.slack_profile_panel(cx).as_ref(),
            Some(SlackProfilePanelState::Loaded(profile))
                if profile.display_name == "Ada Lovelace"
                    && profile.title.as_deref() == Some("Founder")
        ));
    });
}

#[gpui::test]
fn slack_rail_tabs_and_history_controls_update_state() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api();
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open slack rail test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack rail test root");
    root.update(&mut cx, exercise_slack_rail_switches);
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(
            app.root.slack_conversation_id(cx).as_deref(),
            Some("C_DEPLOYS")
        );
        assert!(app.root.can_navigate_slack_back(cx));
        app.root.navigate_slack_history_back(cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(
            app.root.slack_conversation_id(cx).as_deref(),
            Some("C_AICRAZE")
        );
        assert!(!app.root.can_navigate_slack_back(cx));
        app.root.navigate_slack_history_back(cx);
        assert!(app.root.slack_aux_panel(cx).is_none());
        assert!(app.root.can_navigate_slack_forward(cx));
        app.root.navigate_slack_history_forward(cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(
            app.root.slack_conversation_id(cx).as_deref(),
            Some("C_DEPLOYS")
        );
        assert!(!app.root.can_navigate_slack_forward(cx));
        app.root.navigate_slack_history_forward(cx);
        assert!(app.root.slack_aux_panel(cx).is_none());
    });
}

fn exercise_slack_rail_switches(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    app.root.select_slack_rail_view(SlackRailView::Files, cx);
    assert_eq!(app.root.slack_active_rail_view(cx), SlackRailView::Files);
    assert_eq!(app.root.slack_active_tab(cx), SlackMainTab::FilesLinks);
    app.root
        .select_slack_files_filter(SlackFilesFilter::Links, cx);
    assert_eq!(app.root.slack_files_filter(cx), SlackFilesFilter::Links);

    app.root.select_slack_tab(SlackMainTab::Messages, cx);
    assert_eq!(app.root.slack_active_rail_view(cx), SlackRailView::Home);
    assert_eq!(app.root.slack_active_tab(cx), SlackMainTab::Messages);

    app.root.select_conversation("C_DEPLOYS", cx);
}
