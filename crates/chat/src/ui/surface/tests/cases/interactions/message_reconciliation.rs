use std::time::Duration;

use gpui::AppContext as _;

use super::{assert_slack_hi_loaded, send_slack_hi_with_enter, slack_send_payloads};
use crate::ui::surface::tests::cases::{
    acquire_headless_test_lock, app_size, headless_test_context, named_key_down_event, SlackTestApp,
};
use crate::ui::test_support::{
    slack_test_api_with_accepted_invalid_receipt, slack_test_api_with_accepted_send_error,
    slack_test_api_with_send_error, slack_test_board,
};

#[gpui::test]
fn slack_accepted_send_error_reconciles_without_retrying() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, sent_messages) =
        slack_test_api_with_accepted_send_error(Duration::ZERO, "Slack internal_error");
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open Slack ambiguous-send test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access Slack ambiguous-send test root");
    root.update(&mut cx, send_slack_hi_with_enter);
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert!(app.root.slack_composer_text(cx).is_empty());
        assert!(app.root.slack_error(cx).is_none());
        assert_slack_hi_loaded(app, cx);
    });
    let sent = sent_messages.lock().expect("sent messages mutex poisoned");
    assert_eq!(slack_send_payloads(&sent), [("C_AICRAZE", "Hi")]);
}

#[gpui::test]
fn slack_accepted_send_with_invalid_receipt_reconciles_without_retrying() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, sent_messages) = slack_test_api_with_accepted_invalid_receipt();
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open Slack invalid-receipt reconciliation test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access Slack invalid-receipt reconciliation test root");
    root.update(&mut cx, send_slack_hi_with_enter);
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert!(app.root.slack_composer_text(cx).is_empty());
        assert!(app.root.slack_error(cx).is_none());
        assert_slack_hi_loaded(app, cx);
    });
    let sent = sent_messages.lock().expect("sent messages mutex poisoned");
    assert_eq!(slack_send_payloads(&sent), [("C_AICRAZE", "Hi")]);
}

#[gpui::test]
fn slack_accepted_send_error_reconciles_after_an_older_refresh_finishes() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, sent_messages) =
        slack_test_api_with_accepted_send_error(Duration::ZERO, "Slack internal_error");
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open Slack in-flight reconciliation test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access Slack in-flight reconciliation test root");
    root.update(&mut cx, |app, cx| {
        app.root.begin_test_slack_conversation_refresh(cx);
        send_slack_hi_with_enter(app, cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(app.root.slack_composer_text(cx), "Hi");
        assert_eq!(
            app.root.slack_error(cx).as_deref(),
            Some("Slack internal_error")
        );
        app.root.set_test_slack_error("Newer composer error", cx);
        app.root
            .finish_test_slack_conversation_refresh_without_changes(cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert!(app.root.slack_composer_text(cx).is_empty());
        assert_eq!(
            app.root.slack_error(cx).as_deref(),
            Some("Newer composer error")
        );
        assert_slack_hi_loaded(app, cx);
    });
    let sent = sent_messages.lock().expect("sent messages mutex poisoned");
    assert_eq!(slack_send_payloads(&sent), [("C_AICRAZE", "Hi")]);
}

#[gpui::test]
fn slack_accepted_send_error_preserves_a_newer_draft() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, sent_messages) =
        slack_test_api_with_accepted_send_error(Duration::from_millis(50), "Slack internal_error");
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open Slack ambiguous-send edit test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access Slack ambiguous-send edit test root");
    root.update(&mut cx, |app, cx| {
        send_slack_hi_with_enter(app, cx);
        app.root
            .set_slack_composer_text("Follow-up".to_string(), cx)
            .expect("test composer text should be accepted");
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(app.root.slack_composer_text(cx), "Follow-up");
        assert!(app.root.slack_error(cx).is_none());
        assert!(app
            .root
            .slack_workspace(cx)
            .expect("accepted conversation should be loaded")
            .messages
            .iter()
            .any(|message| message.body == "Hi"));
    });
    let sent = sent_messages.lock().expect("sent messages mutex poisoned");
    assert_eq!(slack_send_payloads(&sent), [("C_AICRAZE", "Hi")]);
}

#[gpui::test]
fn slack_retry_reuses_client_message_id_until_the_draft_changes() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, sent_messages) = slack_test_api_with_send_error(Duration::ZERO, "Slack send failed");
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open Slack retry identity test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access Slack retry identity test root");
    root.update(&mut cx, send_slack_hi_with_enter);
    cx.run_until_parked();
    root.update(&mut cx, |app, cx| {
        assert!(app
            .root
            .handle_slack_key_down(&named_key_down_event("enter"), cx));
    });
    cx.run_until_parked();
    root.update(&mut cx, |app, cx| {
        app.root
            .set_slack_composer_text("Hi!".to_string(), cx)
            .expect("test composer text should be accepted");
        assert!(app
            .root
            .handle_slack_key_down(&named_key_down_event("enter"), cx));
    });
    cx.run_until_parked();

    let sent = sent_messages.lock().expect("sent messages mutex poisoned");
    assert_eq!(
        slack_send_payloads(&sent),
        [
            ("C_AICRAZE", "Hi"),
            ("C_AICRAZE", "Hi"),
            ("C_AICRAZE", "Hi!")
        ]
    );
    assert_eq!(sent[0].client_message_id, sent[1].client_message_id);
    assert_ne!(sent[1].client_message_id, sent[2].client_message_id);
}
