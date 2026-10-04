use crate::ui::surface::tests::cases::*;

#[gpui::test]
fn slack_attachment_menu_and_external_connections_are_actionable() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api();
    let window = open_slack_media_window(
        &mut cx,
        slack_test_board_with_workspace(slack_recording_attachment_workspace()),
        api,
    );
    cx.run_until_parked();
    let root = slack_media_root(&window, &mut cx, "attachment action root");
    root.update(&mut cx, slack_run_attachment_menu_actions);
}

#[gpui::test]
fn slack_preview_images_are_cached_on_workspace_load() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api();
    let mut workspace = slack_test_workspace("C_AICRAZE", "design", false);
    workspace.workspace_logo_url = Some("https://files.slack.com/workspace-logo.png".to_string());
    workspace.messages[0].avatar_image_url =
        Some("https://files.slack.com/profile-image.png".to_string());
    workspace.messages[0].attachments = vec![SlackAttachment {
        title: "notslack preview".to_string(),
        source: Default::default(),
        mimetype: "image/png".to_string(),
        description: "Small cached preview".to_string(),
        link_url: "https://example.com".to_string(),
        source_label: "example.com".to_string(),
        preview_image_url: None,
        preview_image_base64: Some(
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jc1EAAAAASUVORK5CYII="
                .to_string(),
        ),
        preview_image_mimetype: Some("image/png".to_string()),
        ..SlackAttachment::default()
    }];
    let app = cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api));

    app.update(&mut cx, |app, cx| {
        assert_eq!(app.root.slack_remote_images(cx).len(), 1);
    });
}

#[gpui::test]
fn slack_remote_images_load_on_initial_render() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let logo_url = "https://files.slack.com/workspace-logo.png".to_string();
    let avatar_url = "https://files.slack.com/profile-image.png".to_string();
    let remote_image = RemoteImageData {
        base64:
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jc1EAAAAASUVORK5CYII="
                .to_string(),
        mimetype: "image/png".to_string(),
    };
    let (api, _) = slack_test_api_with_remote_images(HashMap::from([
        (logo_url.clone(), remote_image.clone()),
        (avatar_url.clone(), remote_image),
    ]));
    let mut workspace = slack_test_workspace("C_AICRAZE", "design", false);
    workspace.workspace_logo_url = Some(logo_url);
    workspace.messages[0].avatar_image_url = Some(avatar_url);

    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = workspace.clone();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open slack remote image test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack remote image test root");
    root.update(&mut cx, |app, cx| {
        let remote_images = app.root.slack_remote_images(cx);
        assert!(remote_images.contains_key("https://files.slack.com/workspace-logo.png"));
        assert!(remote_images.contains_key("https://files.slack.com/profile-image.png"));
    });
}

#[gpui::test]
fn slack_visible_attachment_preview_images_prefetch_on_initial_render() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let preview_url = "https://files.slack.com/attachment-preview.png".to_string();
    let remote_image = RemoteImageData {
        base64:
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jc1EAAAAASUVORK5CYII="
                .to_string(),
        mimetype: "image/png".to_string(),
    };
    let (api, _) =
        slack_test_api_with_remote_images(HashMap::from([(preview_url.clone(), remote_image)]));
    let mut workspace = slack_test_workspace("C_AICRAZE", "design", false);
    workspace.messages[0].attachments = vec![SlackAttachment {
        title: "notslack profile run preview".to_string(),
        source: Default::default(),
        mimetype: "image/png".to_string(),
        description: "Attachment preview should stay lazy in message view.".to_string(),
        link_url: "https://example.com/builds/notslack-profile".to_string(),
        source_label: "example.com".to_string(),
        preview_image_url: Some(preview_url.clone()),
        preview_image_base64: None,
        preview_image_mimetype: None,
        ..SlackAttachment::default()
    }];

    let window = cx
        .open_window(app_size(), |_, cx| {
            let workspace = workspace.clone();
            let api = api.clone();
            cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
        })
        .expect("failed to open slack visible attachment preview test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack visible attachment preview root");
    root.update(&mut cx, |app, cx| {
        assert!(
            app.root.slack_remote_images(cx).contains_key(&preview_url),
            "message view should prefetch the first visible attachment preview"
        );
    });
}

#[gpui::test]
fn slack_large_preview_images_are_downscaled_on_workspace_load() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api();
    let mut workspace = slack_test_workspace("C_AICRAZE", "design", false);
    let mut encoded = Vec::new();
    let preview = image::RgbImage::from_pixel(3558, 2304, image::Rgb([0x2a, 0x2d, 0x30]));
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut encoded, 80)
        .encode_image(&preview)
        .expect("failed to encode large jpeg preview");
    workspace.messages[0].attachments = vec![SlackAttachment {
        title: "Large notslack preview".to_string(),
        source: Default::default(),
        mimetype: "image/jpeg".to_string(),
        description: "Large cached preview".to_string(),
        link_url: "https://example.com".to_string(),
        source_label: "example.com".to_string(),
        preview_image_url: None,
        preview_image_base64: Some(BASE64_STANDARD.encode(encoded)),
        preview_image_mimetype: Some("image/jpeg".to_string()),
        ..SlackAttachment::default()
    }];
    let app = cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api));

    app.update(&mut cx, |app, cx| {
        let remote_images = app.root.slack_remote_images(cx);
        let image = remote_images
            .values()
            .next()
            .expect("expected cached preview image");
        let decoded = image::load_from_memory(&image.bytes).expect("failed to decode cached image");
        assert_eq!(image.format, gpui::ImageFormat::Jpeg);
        assert_eq!(decoded.dimensions(), (1456, 943));
    });
}

#[gpui::test]
#[ignore = "headless attachment interaction still aborts in the gpui test harness"]
fn slack_attachment_controls_and_reactions_are_wired() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api();
    let workspace = slack_test_board();
    let api = api.clone();
    let app = cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api));

    app.update(&mut cx, slack_exercise_attachment_controls);
}

fn open_slack_media_window(
    cx: &mut HeadlessAppContext,
    workspace: SlackWorkspace,
    api: Arc<dyn crate::ui::SlackWorkspaceApi>,
) -> gpui::WindowHandle<SlackTestApp> {
    cx.open_window(app_size(), |_, cx| {
        let api = api.clone();
        cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
    })
    .expect("failed to open slack media test window")
}

fn slack_media_root(
    window: &gpui::WindowHandle<SlackTestApp>,
    cx: &mut HeadlessAppContext,
    label: &str,
) -> gpui::Entity<SlackTestApp> {
    window
        .root(cx)
        .unwrap_or_else(|_| panic!("failed to access slack {label}"))
}

fn slack_recording_attachment_workspace() -> SlackWorkspace {
    let mut workspace = slack_test_workspace("C_AICRAZE", "design", false);
    workspace.messages[0].attachments = vec![SlackAttachment {
        title: "Screen Recording 2026-04-04 at 1.39.16 PM.mov".to_string(),
        source: Default::default(),
        mimetype: "video/mp4".to_string(),
        duration_millis: std::num::NonZeroU32::new(90_000),
        description: "Walk through the latest Slack parity pass.".to_string(),
        link_url: String::new(),
        source_label: String::new(),
        preview_image_url: None,
        preview_image_base64: None,
        preview_image_mimetype: None,
        ..SlackAttachment::default()
    }];
    workspace
}

fn slack_first_attachment_title(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) -> String {
    app.root
        .slack_workspace(cx)
        .and_then(|workspace| workspace.messages[0].attachments.first().cloned())
        .expect("expected recording attachment")
        .title
}

fn slack_run_attachment_menu_actions(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    let attachment = slack_first_attachment_title(app, cx);
    app.root.open_slack_attachment_menu(&attachment, cx);
    let panel = app
        .root
        .slack_aux_panel(cx)
        .expect("expected attachment action panel");
    for row in &panel.sections[0].rows[0..3] {
        let action = row.action.clone().expect("expected attachment action");
        app.root.activate_slack_aux_panel_action(action, cx);
    }
    let media_state = app.root.slack_media_state(&attachment, cx);
    assert!(media_state.playing);
    assert!(media_state.transcript_visible);
    assert!(media_state.transcript_generated);
    assert_eq!(
        media_state.playback_speed,
        SlackPlaybackSpeed::OnePointFiveX
    );
    app.root
        .open_slack_external_connection("Frances Allen", "Foodhub", cx);
    let search_action = app
        .root
        .slack_aux_panel(cx)
        .as_ref()
        .and_then(|aux| aux.sections.first())
        .and_then(|section| section.rows.first())
        .and_then(|row| row.action.clone())
        .expect("expected external connection action");
    app.root.activate_slack_aux_panel_action(search_action, cx);
    assert_eq!(
        app.root
            .slack_aux_panel(cx)
            .as_ref()
            .and_then(|aux| aux.query.as_deref()),
        Some("Frances Allen")
    );
}

fn slack_exercise_attachment_controls(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    let workspace = app
        .root
        .slack_workspace(cx)
        .expect("slack workspace should remain available");
    let message = workspace
        .messages
        .first()
        .expect("expected first slack test message");
    let attachment = message
        .attachments
        .first()
        .expect("expected first slack test attachment")
        .title
        .clone();
    let reaction_count = message
        .reactions
        .first()
        .expect("expected first slack test reaction")
        .count;
    let message_id = message.id.clone();
    app.root.toggle_slack_media_playback(&attachment, cx);
    app.root.toggle_slack_media_muted(&attachment, cx);
    app.root.toggle_slack_media_captions(&attachment, cx);
    app.root.cycle_slack_media_speed(&attachment, cx);
    app.root.toggle_slack_attachment_transcript(&attachment, cx);
    app.root.open_slack_attachment_menu(&attachment, cx);
    app.root
        .toggle_slack_reaction(&message_id, "white_check_mark", cx);
    let media_state = app.root.slack_media_state(&attachment, cx);
    assert!(media_state.playing);
    assert!(media_state.muted);
    assert!(media_state.captions_enabled);
    assert!(media_state.transcript_visible);
    assert_eq!(
        media_state.playback_speed,
        SlackPlaybackSpeed::OnePointFiveX
    );
    assert_eq!(
        app.root
            .slack_aux_panel(cx)
            .as_ref()
            .map(|panel| panel.title.as_str()),
        Some("Attachment actions")
    );
    let updated_workspace = app
        .root
        .slack_workspace(cx)
        .expect("slack workspace should remain available");
    let updated_reaction = updated_workspace
        .messages
        .first()
        .and_then(|message| message.reactions.first())
        .expect("expected updated reaction");
    assert!(updated_reaction.active);
    assert_eq!(updated_reaction.count, reaction_count + 1);
}
