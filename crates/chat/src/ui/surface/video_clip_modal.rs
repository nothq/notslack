use std::time::Duration;

use gpui::{App, Context, Entity, FocusHandle, Focusable, SharedString, Subscription, WeakEntity};

use crate::ui::{VideoClipCaptureStatus, VideoClipPreview, VideoPlayer};

use super::{SlackComposerCaptureGeneration, SurfaceState};

mod actions;
mod preview;
mod render;

const SLACK_VIDEO_CLIP_COUNTDOWN_START: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackVideoClipModalPhase {
    Preparing,
    Previewing,
    Starting,
    Recording,
    Finalizing,
    Reviewing,
    Attaching,
    Cancelling,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SlackVideoClipCountdown {
    serial: u64,
    seconds_remaining: u8,
}

pub(crate) struct SlackVideoClipModal {
    surface: WeakEntity<SurfaceState>,
    generation: SlackComposerCaptureGeneration,
    focus_handle: FocusHandle,
    focus_guard_start: FocusHandle,
    focus_guard_end: FocusHandle,
    status_focus_handle: FocusHandle,
    close_focus_handle: FocusHandle,
    upload_focus_handle: FocusHandle,
    record_action_focus_handle: FocusHandle,
    start_over_focus_handle: FocusHandle,
    download_focus_handle: FocusHandle,
    done_focus_handle: FocusHandle,
    keep_focus_handle: FocusHandle,
    discard_focus_handle: FocusHandle,
    focus_subscriptions: Vec<Subscription>,
    focus_pending: bool,
    phase: SlackVideoClipModalPhase,
    duration: Duration,
    displayed_duration_seconds: u64,
    preview: Option<VideoClipPreview>,
    preview_generation: Option<u64>,
    countdown_serial: u64,
    countdown: Option<SlackVideoClipCountdown>,
    discard_confirmation: bool,
    review_player: Option<Entity<VideoPlayer>>,
    diagnostic: Option<SharedString>,
    download_pending: bool,
}

impl SlackVideoClipModal {
    pub(crate) fn new(
        surface: WeakEntity<SurfaceState>,
        generation: SlackComposerCaptureGeneration,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            surface,
            generation,
            focus_handle: cx.focus_handle(),
            focus_guard_start: cx.focus_handle(),
            focus_guard_end: cx.focus_handle(),
            status_focus_handle: cx.focus_handle(),
            close_focus_handle: cx.focus_handle(),
            upload_focus_handle: cx.focus_handle(),
            record_action_focus_handle: cx.focus_handle(),
            start_over_focus_handle: cx.focus_handle(),
            download_focus_handle: cx.focus_handle(),
            done_focus_handle: cx.focus_handle(),
            keep_focus_handle: cx.focus_handle(),
            discard_focus_handle: cx.focus_handle(),
            focus_subscriptions: Vec::new(),
            focus_pending: true,
            phase: SlackVideoClipModalPhase::Preparing,
            duration: Duration::ZERO,
            displayed_duration_seconds: 0,
            preview: None,
            preview_generation: None,
            countdown_serial: 0,
            countdown: None,
            discard_confirmation: false,
            review_player: None,
            diagnostic: None,
            download_pending: false,
        }
    }

    pub(crate) const fn generation(&self) -> SlackComposerCaptureGeneration {
        self.generation
    }

    pub(crate) fn countdown_seconds_remaining(&self) -> Option<u8> {
        self.countdown.map(|countdown| countdown.seconds_remaining)
    }

    pub(crate) fn diagnostic(&self) -> Option<String> {
        self.diagnostic.as_ref().map(ToString::to_string)
    }

    pub(crate) fn apply_capture_snapshot(
        &mut self,
        status: &VideoClipCaptureStatus,
        duration: Duration,
        preview: Option<&VideoClipPreview>,
        cx: &mut Context<Self>,
    ) {
        let phase = match status {
            VideoClipCaptureStatus::RequestingPermissions | VideoClipCaptureStatus::Ready => {
                SlackVideoClipModalPhase::Preparing
            }
            VideoClipCaptureStatus::Previewing => SlackVideoClipModalPhase::Previewing,
            VideoClipCaptureStatus::Recording => SlackVideoClipModalPhase::Recording,
            VideoClipCaptureStatus::DurationLimitReached => SlackVideoClipModalPhase::Finalizing,
            VideoClipCaptureStatus::Failed(_) => SlackVideoClipModalPhase::Failed,
        };
        let preview_generation = preview.map(|preview| preview.generation().get());
        let displayed_duration_seconds = duration.as_secs();
        let changed = self.phase != phase
            || self.displayed_duration_seconds != displayed_duration_seconds
            || self.preview_generation != preview_generation;
        self.phase = phase;
        self.duration = duration;
        self.displayed_duration_seconds = displayed_duration_seconds;
        if self.preview_generation != preview_generation {
            self.preview = preview.cloned();
            self.preview_generation = preview_generation;
        }
        if changed {
            cx.notify();
        }
    }

    pub(crate) fn set_phase(
        &mut self,
        phase: SlackVideoClipModalPhase,
        duration: Duration,
        diagnostic: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if phase != SlackVideoClipModalPhase::Reviewing {
            self.retire_review_player(cx);
        }
        self.phase = phase;
        self.duration = duration;
        self.displayed_duration_seconds = duration.as_secs();
        self.diagnostic = diagnostic.map(Into::into);
        self.countdown = None;
        self.focus_pending = matches!(
            phase,
            SlackVideoClipModalPhase::Finalizing
                | SlackVideoClipModalPhase::Attaching
                | SlackVideoClipModalPhase::Cancelling
                | SlackVideoClipModalPhase::Failed
        );
        if !matches!(
            phase,
            SlackVideoClipModalPhase::Preparing
                | SlackVideoClipModalPhase::Previewing
                | SlackVideoClipModalPhase::Starting
                | SlackVideoClipModalPhase::Recording
        ) {
            self.preview = None;
            self.preview_generation = None;
        }
        cx.notify();
    }

    pub(crate) fn begin_review(
        &mut self,
        player: Entity<VideoPlayer>,
        duration: Duration,
        cx: &mut Context<Self>,
    ) {
        self.retire_review_player(cx);
        self.phase = SlackVideoClipModalPhase::Reviewing;
        self.duration = duration;
        self.displayed_duration_seconds = duration.as_secs();
        self.preview = None;
        self.preview_generation = None;
        self.countdown = None;
        self.review_player = Some(player);
        self.diagnostic = None;
        self.focus_pending = true;
        cx.notify();
    }

    pub(crate) fn cancel_countdown(&mut self, cx: &mut Context<Self>) -> bool {
        if self.countdown.take().is_none() {
            return false;
        }
        self.countdown_serial = self
            .countdown_serial
            .checked_add(1)
            .expect("video clip countdown serial must not overflow");
        self.phase = SlackVideoClipModalPhase::Previewing;
        cx.notify();
        true
    }

    pub(super) fn retire_review_player(&mut self, cx: &mut Context<Self>) {
        let Some(player) = self.review_player.take() else {
            return;
        };
        player.update(cx, |player, cx| player.clear(cx));
    }

    pub(crate) fn finish_download(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.download_pending = false;
        self.diagnostic = result.err().map(Into::into);
        cx.notify();
    }
}

impl Focusable for SlackVideoClipModal {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
