use crate::ui::surface::tests::cases::interactions::*;

pub(super) fn send_slack_hi_with_enter(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    app.root.focus_slack_composer(cx);
    assert!(app
        .root
        .handle_slack_key_down(&text_key_down_event("H"), cx));
    assert!(app
        .root
        .handle_slack_key_down(&text_key_down_event("i"), cx));
    assert!(app
        .root
        .handle_slack_key_down(&named_key_down_event("enter"), cx));
}

pub(super) fn assert_slack_hi_loaded(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    let workspace = app
        .root
        .slack_workspace(cx)
        .expect("slack workspace should remain available");
    assert_eq!(
        workspace
            .messages
            .last()
            .map(|message| message.body.as_str()),
        Some("Hi")
    );
    assert_eq!(
        app.root.slack_message_list_item_count(cx),
        app.root.slack_message_chunk_count(cx)
    );
    assert!(app.root.slack_composer_text(cx).is_empty());
}
