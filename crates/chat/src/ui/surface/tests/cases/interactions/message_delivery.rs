use std::time::Duration;

use gpui::AppContext as _;

use super::{assert_slack_hi_loaded, send_slack_hi_with_enter, slack_send_payloads};
use crate::ui::surface::tests::cases::{
    acquire_headless_test_lock, app_size, headless_test_context, SlackTestApp,
};
use crate::ui::test_support::{
    slack_test_api, slack_test_api_with_send_delay, slack_test_api_with_send_error,
    slack_test_board,
};

#[gpui::test]
fn slack_enter_sends_message_and_reloads_workspace() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, sent_messages) = slack_test_api();
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open slack interaction test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack interaction root");
    root.update(&mut cx, send_slack_hi_with_enter);
    cx.run_until_parked();

    let sent = sent_messages
        .lock()
        .expect("sent messages mutex poisoned")
        .clone();
    assert_eq!(slack_send_payloads(&sent), [("C_AICRAZE", "Hi")]);
    root.update(&mut cx, assert_slack_hi_loaded);
}

#[gpui::test]
fn slack_switch_during_send_clears_only_the_submitted_draft() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, sent_messages) = slack_test_api_with_send_delay(Duration::from_millis(50));
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open Slack delayed-send test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access Slack delayed-send test root");
    root.update(&mut cx, |app, cx| {
        send_slack_hi_with_enter(app, cx);
        app.root.select_conversation("C_DEPLOYS", cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(
            app.root.slack_conversation_id(cx).as_deref(),
            Some("C_DEPLOYS")
        );
        app.root.select_conversation("C_AICRAZE", cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(
            app.root.slack_conversation_id(cx).as_deref(),
            Some("C_AICRAZE")
        );
        assert!(app.root.slack_composer_text(cx).is_empty());
        assert!(app
            .root
            .slack_workspace(cx)
            .expect("sent conversation should be loaded")
            .messages
            .iter()
            .any(|message| message.body == "Hi"));
    });
    let sent = sent_messages.lock().expect("sent messages mutex poisoned");
    assert_eq!(slack_send_payloads(&sent), [("C_AICRAZE", "Hi")]);
}

#[gpui::test]
fn slack_edit_during_send_preserves_the_newer_draft() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, sent_messages) = slack_test_api_with_send_delay(Duration::from_millis(50));
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open Slack edit-during-send test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access Slack edit-during-send test root");
    root.update(&mut cx, |app, cx| {
        send_slack_hi_with_enter(app, cx);
        app.root
            .set_slack_composer_text("Follow-up".to_string(), cx)
            .expect("test composer text should be accepted");
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(app.root.slack_composer_text(cx), "Follow-up");
        assert!(app
            .root
            .slack_workspace(cx)
            .expect("sent conversation should be loaded")
            .messages
            .iter()
            .any(|message| message.body == "Hi"));
    });
    let sent = sent_messages.lock().expect("sent messages mutex poisoned");
    assert_eq!(slack_send_payloads(&sent), [("C_AICRAZE", "Hi")]);
}

#[gpui::test]
fn slack_switch_during_failed_send_restores_the_draft_and_error() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api_with_send_error(Duration::from_millis(50), "Slack send failed");
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open Slack failed-send test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access Slack failed-send test root");
    root.update(&mut cx, |app, cx| {
        send_slack_hi_with_enter(app, cx);
        app.root.select_conversation("C_DEPLOYS", cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        app.root.select_conversation("C_AICRAZE", cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        assert_eq!(app.root.slack_composer_text(cx), "Hi");
        assert_eq!(
            app.root.slack_error(cx).as_deref(),
            Some("Slack send failed")
        );
    });
}

#[gpui::test]
fn slack_new_message_send_clears_the_exact_stored_draft_after_leaving() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, sent_messages) = slack_test_api_with_send_delay(Duration::from_millis(50));
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open Slack New Message send test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access Slack New Message send test root");
    root.update(&mut cx, |app, cx| {
        app.root
            .enter_test_slack_new_message_target("conversation:C_AICRAZE", cx);
        send_slack_hi_with_enter(app, cx);
        app.root.leave_test_slack_new_message_target(cx);
    });
    cx.run_until_parked();

    root.update(&mut cx, |app, cx| {
        app.root
            .enter_test_slack_new_message_target("conversation:C_AICRAZE", cx);
        assert!(app.root.slack_composer_text(cx).is_empty());
        assert!(app
            .root
            .slack_workspace(cx)
            .expect("sent New Message conversation should remain loaded")
            .messages
            .iter()
            .any(|message| message.body == "Hi"));
    });
    let sent = sent_messages.lock().expect("sent messages mutex poisoned");
    assert_eq!(slack_send_payloads(&sent), [("C_AICRAZE", "Hi")]);
}

#[gpui::test]
fn slack_pending_send_in_another_conversation_does_not_block_delivery() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, sent_messages) = slack_test_api();
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open Slack concurrent-send test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access Slack concurrent-send test root");
    root.update(&mut cx, |app, cx| {
        app.root.insert_test_slack_pending_send("C_DEPLOYS", cx);
        send_slack_hi_with_enter(app, cx);
    });
    cx.run_until_parked();

    let sent = sent_messages.lock().expect("sent messages mutex poisoned");
    assert_eq!(slack_send_payloads(&sent), [("C_AICRAZE", "Hi")]);
}
