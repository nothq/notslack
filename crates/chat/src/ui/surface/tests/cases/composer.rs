use crate::ui::surface::tests::cases::*;

#[gpui::test]
fn slack_composer_attachment_and_schedule_flows_are_wired() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api();
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open slack composer flow test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack composer flow root");
    root.update(
        &mut cx,
        exercise_slack_composer_attachment_and_schedule_flow,
    );
}

fn exercise_slack_composer_attachment_and_schedule_flow(
    app: &mut SlackTestApp,
    cx: &mut Context<SlackTestApp>,
) {
    attach_slack_composer_test_file(app, cx);
    app.root
        .apply_slack_composer_format(SlackComposerFormatAction::Bold, cx);
    assert!(app.root.slack_composer_text(cx).starts_with("**"));
    exercise_slack_composer_picker_insertions(app, cx);
    exercise_slack_composer_media_attachments(app, cx);
    exercise_slack_composer_schedule_flow(app, cx);
}

fn attach_slack_composer_test_file(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    app.root.attach_slack_draft_attachment(
        SlackAttachment {
            title: "Slack parity checklist.pdf".to_string(),
            source: Default::default(),
            mimetype: "application/pdf".to_string(),
            description: "Latest review checklist for the Slack parity pass.".to_string(),
            link_url: String::new(),
            source_label: String::new(),
            preview_image_url: None,
            preview_image_base64: None,
            preview_image_mimetype: None,
            ..SlackAttachment::default()
        },
        cx,
    );
    assert_eq!(app.root.slack_draft_attachment_count(cx), 1);
}

fn exercise_slack_composer_picker_insertions(
    app: &mut SlackTestApp,
    cx: &mut Context<SlackTestApp>,
) {
    exercise_slack_composer_emoji_insertion(app, cx);
    exercise_slack_composer_mention_insertion(app, cx);
}

fn exercise_slack_composer_emoji_insertion(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    app.root.open_slack_emoji_picker(cx);
    assert_eq!(
        app.root
            .slack_aux_panel(cx)
            .as_ref()
            .and_then(|panel| panel.query.as_deref()),
        Some("")
    );
    for query_part in ["r", "o", "c"] {
        assert!(app
            .root
            .handle_slack_key_down(&text_key_down_event(query_part), cx));
    }
    let emoji_label = app
        .root
        .slack_aux_panel(cx)
        .as_ref()
        .and_then(|panel| panel.sections.first())
        .and_then(|section| section.rows.first())
        .map(|row| row.label.clone())
        .expect("expected emoji picker row");
    assert!(
        emoji_label.contains("🚀"),
        "expected rocket row, got {emoji_label}"
    );
    assert!(app
        .root
        .handle_slack_key_down(&named_key_down_event("enter"), cx));
    assert!(app.root.slack_composer_text(cx).contains("🚀"));
}

fn exercise_slack_composer_mention_insertion(
    app: &mut SlackTestApp,
    cx: &mut Context<SlackTestApp>,
) {
    app.root.open_slack_mention_picker(cx);
    let mention_special_label = app
        .root
        .slack_aux_panel(cx)
        .as_ref()
        .and_then(|panel| panel.sections.first())
        .and_then(|section| section.rows.first())
        .map(|row| row.label.clone())
        .expect("expected special mention row");
    assert_eq!(mention_special_label, "@here");
    for query_part in ["i", "g", "n", "a"] {
        assert!(app
            .root
            .handle_slack_key_down(&text_key_down_event(query_part), cx));
    }
    let mention_label = app
        .root
        .slack_aux_panel(cx)
        .as_ref()
        .and_then(|panel| {
            panel
                .sections
                .iter()
                .find_map(|section| section.rows.first().map(|row| row.label.clone()))
        })
        .expect("expected mention picker row");
    assert_eq!(mention_label, "Ada Lovelace");
    assert!(app
        .root
        .handle_slack_key_down(&named_key_down_event("enter"), cx));
    assert!(app
        .root
        .slack_composer_text(cx)
        .contains("@Ada Lovelace"));
}

fn exercise_slack_composer_media_attachments(
    app: &mut SlackTestApp,
    cx: &mut Context<SlackTestApp>,
) {
    app.root.open_slack_video_panel(cx);
    let clip_action = app
        .root
        .slack_aux_panel(cx)
        .as_ref()
        .and_then(|panel| panel.sections.first())
        .and_then(|section| section.rows.first())
        .and_then(|row| row.action.clone())
        .expect("expected clip row action");
    app.root.activate_slack_aux_panel_action(clip_action, cx);
    assert!(app
        .root
        .slack_draft_attachment_mimetypes(cx)
        .iter()
        .any(|mimetype| mimetype.starts_with("video/")));

    app.root.open_slack_audio_panel(cx);
    let audio_action = app
        .root
        .slack_aux_panel(cx)
        .as_ref()
        .and_then(|panel| panel.sections.first())
        .and_then(|section| section.rows.first())
        .and_then(|row| row.action.clone())
        .expect("expected audio row action");
    app.root.activate_slack_aux_panel_action(audio_action, cx);
    assert!(app
        .root
        .slack_draft_attachment_mimetypes(cx)
        .iter()
        .any(|mimetype| mimetype.starts_with("audio/")));
}

fn exercise_slack_composer_schedule_flow(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    app.root.open_slack_send_options(cx);
    let schedule_action = app
        .root
        .slack_aux_panel(cx)
        .as_ref()
        .and_then(|panel| panel.sections.first())
        .and_then(|section| section.rows.get(1))
        .and_then(|row| row.action.clone())
        .expect("expected schedule row action");
    app.root
        .activate_slack_aux_panel_action(schedule_action, cx);

    assert_eq!(app.root.slack_active_rail_view(cx), SlackRailView::Later);
    assert_eq!(app.root.slack_scheduled_message_count(cx), 1);
    assert!(app.root.slack_composer_text(cx).is_empty());
    assert!(app.root.slack_draft_attachments_empty(cx));
}

#[gpui::test]
fn slack_uploaded_files_use_api_and_reload_workspace() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, sent_messages) = slack_test_api();
    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = slack_test_board();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open slack upload test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack upload test root");
    root.update(&mut cx, queue_slack_upload_message);
    cx.run_until_parked();

    let sent = sent_messages
        .lock()
        .expect("sent messages mutex poisoned")
        .clone();
    assert!(sent.is_empty());

    root.update(&mut cx, |app, cx| {
        assert!(app.root.slack_draft_attachments_empty(cx));
        assert!(app.root.slack_draft_upload_files_empty(cx));
        let workspace = app
            .root
            .slack_workspace(cx)
            .expect("slack workspace should remain available");
        let message = workspace
            .messages
            .last()
            .expect("expected uploaded message");
        assert_eq!(message.body, "Uploading from notslack");
        assert_eq!(message.attachments.len(), 1);
        assert_eq!(message.attachments[0].title, "build-report.txt");
        assert_eq!(message.attachments[0].mimetype, "text/plain");
    });
}

fn queue_slack_upload_message(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    app.root
        .set_slack_composer_text("Uploading from notslack".to_string(), cx)
        .expect("test composer text should be accepted");
    app.root.attach_slack_upload_files(
        vec![SlackUploadFile::from_bytes(
            "build-report.txt",
            Arc::from(b"upload body".as_slice()),
            "text/plain",
        )
        .expect("test Slack upload should be valid")],
        cx,
    );
    app.root.send_slack_message(cx);
}
