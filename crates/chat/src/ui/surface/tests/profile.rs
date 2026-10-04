use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use gpui::{px, AppContext, TestAppContext};

use crate::ui::{
    test_support::{
        slack_test_board_with_workspace, slack_test_workspace_with_channel_count,
        slack_test_workspace_with_message_count, MockSlackSendOutcome, MockSlackWorkspaceApi,
    },
    SlackAttachment, SlackWorkspace, SlackWorkspaceApi, SurfaceInput, SurfaceState, SurfaceTheme,
};

use super::super::{ChatStartup, SurfaceStateConfig};

#[gpui::test]
#[ignore]
fn profile_slack_scroll_render_loop(cx: &mut TestAppContext) {
    profile_slack_scroll_loop(cx, true);
}

#[gpui::test]
#[ignore]
fn profile_slack_scroll_update_loop(cx: &mut TestAppContext) {
    profile_slack_scroll_loop(cx, false);
}

#[gpui::test]
#[ignore]
fn profile_slack_sidebar_scroll_update_loop(cx: &mut TestAppContext) {
    profile_slack_sidebar_scroll_loop(cx, false);
}

#[gpui::test]
#[ignore]
fn profile_slack_sidebar_scroll_render_loop(cx: &mut TestAppContext) {
    profile_slack_sidebar_scroll_loop(cx, true);
}

fn profile_slack_sidebar_scroll_loop(cx: &mut TestAppContext, capture_frames: bool) {
    let frame_count = slack_profile_frame_count();
    let workspace = slack_test_board_with_workspace(slack_test_workspace_with_channel_count(2_000));
    let label = if capture_frames {
        "slack-sidebar-scroll-render"
    } else {
        "slack-sidebar-scroll-update"
    };

    eprintln!("profiling slack sidebar with 2000 channels");

    let (surface, cx) = cx.add_window_view(|_, cx| slack_profile_surface_state(workspace, cx));
    cx.refresh().expect("refresh slack sidebar profile window");

    let start = Instant::now();
    let mut frame_durations = Vec::with_capacity(frame_count);
    for step in 0..frame_count {
        let direction = if step % 40 < 20 { 1.0 } else { -1.0 };
        let frame_start = Instant::now();
        cx.update_entity(&surface, |surface, cx| {
            surface
                .slack_sidebar_list_state
                .scroll_by(px(96.0 * direction));
            cx.notify();
        });
        cx.run_until_parked();
        if capture_frames {
            cx.refresh().expect("refresh Slack sidebar profile frame");
        }
        frame_durations.push(frame_start.elapsed());
    }
    let elapsed = start.elapsed();

    eprintln!(
        "{}",
        summarize_profile_frames(label, &frame_durations, elapsed)
    );
}

#[gpui::test]
#[ignore]
fn profile_slack_composer_render_loop(cx: &mut TestAppContext) {
    let message_count = 2_000usize;
    let frame_count = slack_profile_frame_count();
    let workspace =
        slack_test_workspace_with_message_count("C_AICRAZE", "design", false, message_count);
    let workspace = slack_test_board_with_workspace(workspace);
    let (surface, cx) = cx.add_window_view(|_, cx| slack_profile_surface_state(workspace, cx));
    cx.refresh().expect("refresh Slack composer profile window");

    let start = Instant::now();
    let mut frame_durations = Vec::with_capacity(frame_count);
    for step in 0..frame_count {
        let frame_start = Instant::now();
        cx.update_entity(&surface, |surface, cx| {
            surface.replace_slack_send_draft_text(format!(
                "Profiling Slack composer input frame {step:04}"
            ));
            cx.notify();
        });
        cx.run_until_parked();
        cx.refresh().expect("refresh Slack composer profile frame");
        frame_durations.push(frame_start.elapsed());
    }
    let elapsed = start.elapsed();

    eprintln!(
        "{}",
        summarize_profile_frames("slack-composer-render", &frame_durations, elapsed)
    );
}

#[gpui::test]
#[ignore]
fn profile_slack_ambiguous_send_reconciliation_render_loop(cx: &mut TestAppContext) {
    let message_count = 2_000usize;
    let frame_count = slack_profile_frame_count();
    let workspace =
        slack_test_workspace_with_message_count("C_AICRAZE", "design", false, message_count);
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let workspace_api: Arc<dyn SlackWorkspaceApi> = Arc::new(MockSlackWorkspaceApi {
        conversations: Arc::new(Mutex::new(HashMap::from([(
            workspace.conversation_id.clone(),
            workspace.clone(),
        )]))),
        profiles: HashMap::new(),
        remote_images: HashMap::new(),
        sent_messages,
        load_delay: Duration::ZERO,
        send_delay: Duration::ZERO,
        send_outcome: MockSlackSendOutcome::AcceptedWithError("Slack internal_error".to_string()),
        load_metrics: None,
    });
    let workspace = slack_test_board_with_workspace(workspace);
    let (surface, cx) = cx.add_window_view(|_, cx| {
        slack_profile_surface_state_with_api(workspace, workspace_api, cx)
    });
    cx.refresh()
        .expect("refresh Slack ambiguous-send profile window");

    let start = Instant::now();
    let mut frame_durations = Vec::with_capacity(frame_count);
    for step in 0..frame_count {
        let frame_start = Instant::now();
        cx.update_entity(&surface, |surface, cx| {
            surface.replace_slack_send_draft_with_plain_text(format!(
                "Profiling Slack ambiguous send {step:04}"
            ));
            surface.send_slack_message(cx);
        });
        cx.run_until_parked();
        cx.refresh()
            .expect("refresh Slack ambiguous-send reconciliation frame");
        frame_durations.push(frame_start.elapsed());
    }
    let elapsed = start.elapsed();

    cx.update_entity(&surface, |surface, _cx| {
        assert!(surface.slack_composer_text.is_empty());
        assert!(surface.slack_error.is_none());
        assert_eq!(
            surface.slack_message_rows.len(),
            message_count + frame_count
        );
    });
    eprintln!(
        "{}",
        summarize_profile_frames(
            "slack-ambiguous-send-reconciliation-render",
            &frame_durations,
            elapsed
        )
    );
}

fn profile_slack_scroll_loop(cx: &mut TestAppContext, capture_frames: bool) {
    let message_count = 2_000usize;
    let mut workspace =
        slack_test_workspace_with_message_count("C_AICRAZE", "design", false, message_count);
    seed_slack_profile_attachments(&mut workspace);
    let workspace = slack_test_board_with_workspace(workspace);
    let frame_count = slack_profile_frame_count();
    let label = slack_scroll_profile_label(capture_frames);

    eprintln!("profiling slack conversation with {message_count} messages");

    let (surface, cx) = cx.add_window_view(|_, cx| slack_profile_surface_state(workspace, cx));
    cx.refresh().expect("refresh slack profile window");
    let start = Instant::now();
    let mut frame_durations = Vec::with_capacity(frame_count);
    for step in 0..frame_count {
        let direction = if step % 40 < 20 { 1.0 } else { -1.0 };
        let frame_start = Instant::now();
        cx.update_entity(&surface, |surface, cx| {
            surface
                .slack_message_list_state
                .scroll_by(px(96.0 * direction));
            cx.notify();
        });
        cx.run_until_parked();
        if capture_frames {
            cx.refresh().expect("refresh slack profile frame");
        }
        frame_durations.push(frame_start.elapsed());
    }
    let elapsed = start.elapsed();

    eprintln!(
        "{}",
        summarize_profile_frames(label, &frame_durations, elapsed)
    );
}

fn slack_profile_surface_state(
    workspace: SlackWorkspace,
    cx: &mut gpui::Context<SurfaceState>,
) -> SurfaceState {
    slack_profile_surface_state_with_optional_api(workspace, None, cx)
}

fn slack_profile_surface_state_with_api(
    workspace: SlackWorkspace,
    workspace_api: Arc<dyn SlackWorkspaceApi>,
    cx: &mut gpui::Context<SurfaceState>,
) -> SurfaceState {
    slack_profile_surface_state_with_optional_api(workspace, Some(workspace_api), cx)
}

fn slack_profile_surface_state_with_optional_api(
    workspace: SlackWorkspace,
    workspace_api: Option<Arc<dyn SlackWorkspaceApi>>,
    cx: &mut gpui::Context<SurfaceState>,
) -> SurfaceState {
    cx.set_global(app_model::AppearanceMode::Dark);
    SurfaceState::new(
        SurfaceInput {
            workspace: Some(workspace),
            workspace_api,
            local_file_api: None,
            embedded_shell: false,
            initial_thread_message_id: None,
        },
        ChatStartup::Fixture,
        SurfaceStateConfig {
            theme: SurfaceTheme::default(),
            appearance_mode: app_model::AppearanceMode::current(cx),
            active: true,
            preview_width: 1280.0,
            viewport_height: 720.0,
        },
        cx,
    )
}

fn slack_profile_frame_count() -> usize {
    120
}

fn slack_scroll_profile_label(capture_frames: bool) -> &'static str {
    if capture_frames {
        "slack-scroll-render"
    } else {
        "slack-scroll-update"
    }
}

fn seed_slack_profile_attachments(workspace: &mut SlackWorkspace) {
    for (index, message) in workspace.messages.iter_mut().enumerate() {
        if index % 13 == 0 {
            message.attachments = vec![SlackAttachment {
                title: format!(
                    "Screen Recording 2026-04-06 at 1.{:02}.16 PM.mov",
                    index % 60
                ),
                source: Default::default(),
                mimetype: "video/mp4".to_string(),
                description: "Walking through the active Slack thread and the follow-up actions."
                    .to_string(),
                link_url: String::new(),
                source_label: String::new(),
                preview_image_url: None,
                preview_image_base64: None,
                preview_image_mimetype: None,
                ..SlackAttachment::default()
            }];
        } else if index % 5 == 0 {
            message.attachments = vec![SlackAttachment::website_preview(
                format!("Profile run attachment {}", index + 1),
                format!("https://example.com/builds/{}", index + 1),
                "example.com",
                "A longer preview card payload to exercise the conversation render path.",
            )];
        }
    }
}

fn summarize_profile_frames(
    label: &str,
    frame_durations: &[Duration],
    total_elapsed: Duration,
) -> String {
    const SIXTY_HZ_FRAME_BUDGET_MS: f64 = 16.7;
    const THIRTY_HZ_FRAME_BUDGET_MS: f64 = 33.3;

    let mut frame_millis = frame_durations
        .iter()
        .map(Duration::as_secs_f64)
        .map(|seconds| seconds * 1000.0)
        .collect::<Vec<_>>();
    frame_millis.sort_by(|left, right| left.total_cmp(right));
    let frame_count = frame_millis.len();
    let mean_ms = if frame_count == 0 {
        0.0
    } else {
        frame_millis.iter().sum::<f64>() / frame_count as f64
    };
    let spikes_over_60hz_budget = frame_millis
        .iter()
        .filter(|millis| **millis > SIXTY_HZ_FRAME_BUDGET_MS)
        .count();
    let spikes_over_30hz_budget = frame_millis
        .iter()
        .filter(|millis| **millis > THIRTY_HZ_FRAME_BUDGET_MS)
        .count();
    format!(
        "captured {frame_count} {label} frames in {:?} (mean {:.2} ms, p50 {:.2} ms, p95 {:.2} ms, p99 {:.2} ms, max {:.2} ms, spikes >{:.1} ms: {}, >{:.1} ms: {})",
        total_elapsed,
        mean_ms,
        percentile_millis(&frame_millis, 0.50),
        percentile_millis(&frame_millis, 0.95),
        percentile_millis(&frame_millis, 0.99),
        frame_millis.last().copied().unwrap_or(0.0),
        SIXTY_HZ_FRAME_BUDGET_MS,
        spikes_over_60hz_budget,
        THIRTY_HZ_FRAME_BUDGET_MS,
        spikes_over_30hz_budget,
    )
}

fn percentile_millis(sorted_millis: &[f64], percentile: f64) -> f64 {
    if sorted_millis.is_empty() {
        return 0.0;
    }
    let rank = (percentile.clamp(0.0, 1.0) * sorted_millis.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted_millis.len() - 1);
    sorted_millis[index]
}
