use crate::ui::surface::tests::cases::*;

#[gpui::test]
fn slack_attachment_menu_and_external_connections_are_actionable() {
    let _guard = acquire_headless_test_lock();
    let mut cx = headless_test_context();
    let window = open_slack_attachment_action_window(&mut cx);
    cx.run_until_parked();

    let root = window
        .root(&mut cx)
        .expect("failed to access slack attachment action root");
    root.update(&mut cx, assert_slack_attachment_and_external_actions);
}

fn open_slack_attachment_action_window(
    cx: &mut HeadlessAppContext,
) -> gpui::WindowHandle<SlackTestApp> {
    let (api, _) = slack_test_api();
    let workspace = slack_attachment_action_workspace();
    cx.open_window(app_size(), |_, cx| {
        let workspace = slack_test_board_with_workspace(workspace);
        let api = api.clone();
        cx.new(move |_| SlackTestApp::from_slack_workspace(workspace, api))
    })
    .expect("failed to open slack attachment action test window")
}

fn slack_attachment_action_workspace() -> SlackWorkspace {
    let mut workspace = slack_test_workspace("C_DESIGN", "design", false);
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

fn assert_slack_attachment_and_external_actions(
    app: &mut SlackTestApp,
    cx: &mut Context<SlackTestApp>,
) {
    let attachment = app
        .root
        .slack_workspace(cx)
        .expect("slack workspace should remain available")
        .messages[0]
        .attachments
        .first()
        .expect("expected recording attachment")
        .title
        .clone();

    app.root.open_slack_attachment_menu(&attachment, cx);
    activate_first_slack_attachment_actions(app, cx);
    assert_slack_attachment_action_state(app, &attachment, cx);

    app.root
        .open_slack_external_connection("Frances Allen", "Northwind", cx);
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

fn activate_first_slack_attachment_actions(app: &mut SlackTestApp, cx: &mut Context<SlackTestApp>) {
    let panel = app
        .root
        .slack_aux_panel(cx)
        .expect("expected attachment action panel");
    for row in &panel.sections[0].rows[0..3] {
        let action = row.action.clone().expect("expected attachment action");
        app.root.activate_slack_aux_panel_action(action, cx);
    }
}

fn assert_slack_attachment_action_state(
    app: &mut SlackTestApp,
    attachment: &str,
    cx: &mut Context<SlackTestApp>,
) {
    let media_state = app.root.slack_media_state(attachment, cx);
    assert!(media_state.playing);
    assert!(media_state.transcript_visible);
    assert!(media_state.transcript_generated);
    assert_eq!(
        media_state.playback_speed,
        SlackPlaybackSpeed::OnePointFiveX
    );
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

    app.update(&mut cx, assert_slack_attachment_controls_and_reactions);
}

fn assert_slack_attachment_controls_and_reactions(
    app: &mut SlackTestApp,
    cx: &mut Context<SlackTestApp>,
) {
    let (attachment, reaction_count, message_id) = slack_first_attachment_and_reaction(app, cx);

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

    let workspace = app
        .root
        .slack_workspace(cx)
        .expect("slack workspace should remain available");
    let updated_reaction = workspace
        .messages
        .first()
        .and_then(|message| message.reactions.first())
        .expect("expected updated reaction");
    assert!(updated_reaction.active);
    assert_eq!(updated_reaction.count, reaction_count + 1);
}

fn slack_first_attachment_and_reaction(
    app: &mut SlackTestApp,
    cx: &mut Context<SlackTestApp>,
) -> (String, u32, String) {
    let workspace = app
        .root
        .slack_workspace(cx)
        .expect("slack workspace should remain available")
        .clone();
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
    (attachment, reaction_count, message_id)
}
