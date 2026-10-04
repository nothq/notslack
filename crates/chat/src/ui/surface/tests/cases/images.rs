use crate::ui::surface::tests::cases::*;

#[gpui::test]
fn slack_preview_images_are_cached_on_workspace_load() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api();
    let mut workspace = slack_test_workspace("C_DESIGN", "design", false);
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
        bytes: base64::Engine::decode(
            &base64::prelude::BASE64_STANDARD,
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jc1EAAAAASUVORK5CYII=",
            )
            .unwrap(),
        mimetype: "image/png".to_string(),
    };
    let (api, _) = slack_test_api_with_remote_images(HashMap::from([
        (logo_url.clone(), remote_image.clone()),
        (avatar_url.clone(), remote_image),
    ]));
    let mut workspace = slack_test_workspace("C_DESIGN", "design", false);
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
        bytes: base64::Engine::decode(
            &base64::prelude::BASE64_STANDARD,
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jc1EAAAAASUVORK5CYII=",
            )
            .unwrap(),
        mimetype: "image/png".to_string(),
    };
    let (api, _) =
        slack_test_api_with_remote_images(HashMap::from([(preview_url.clone(), remote_image)]));
    let mut workspace = slack_test_workspace("C_DESIGN", "design", false);
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
fn slack_bottom_attachment_preview_images_prefetch_on_initial_render() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let preview_url = "https://files.slack.com/bottom-attachment-preview.png".to_string();
    let remote_image = RemoteImageData {
        bytes: base64::Engine::decode(
            &base64::prelude::BASE64_STANDARD,
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jc1EAAAAASUVORK5CYII=",
            )
            .unwrap(),
        mimetype: "image/png".to_string(),
    };
    let (api, _) =
        slack_test_api_with_remote_images(HashMap::from([(preview_url.clone(), remote_image)]));
    let mut workspace = slack_test_workspace_with_message_count("C_DESIGN", "design", false, 16);
    workspace.messages[14].attachments = vec![SlackAttachment {
        title: "Visible bottom preview".to_string(),
        source: Default::default(),
        mimetype: "image/png".to_string(),
        description: "Attachment preview near the scrolled end should load.".to_string(),
        link_url: "https://example.com/builds/visible-bottom".to_string(),
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
        .expect("failed to open slack bottom attachment preview test window");
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack bottom attachment preview root");
    root.update(&mut cx, |app, cx| {
        assert!(
            app.root.slack_remote_images(cx).contains_key(&preview_url),
            "message view should prefetch attachment previews near the scrolled end"
        );
    });
}

#[gpui::test]
fn slack_large_preview_images_are_downscaled_on_workspace_load() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let (api, _) = slack_test_api();
    let mut workspace = slack_test_workspace("C_DESIGN", "design", false);
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
