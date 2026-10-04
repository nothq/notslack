use std::time::Duration;

use gpui::{Context, KeyDownEvent, Window};

use crate::ui::spawn_timer_task_for_entity;

use super::{
    SlackVideoClipCountdown, SlackVideoClipModal, SlackVideoClipModalPhase,
    SLACK_VIDEO_CLIP_COUNTDOWN_START,
};

const COUNTDOWN_TICK: Duration = Duration::from_secs(1);

impl SlackVideoClipModal {
    pub(crate) fn start_countdown(&mut self, cx: &mut Context<Self>) {
        if self.phase != SlackVideoClipModalPhase::Previewing || self.countdown.is_some() {
            return;
        }
        let generation = self.generation;
        let current = self
            .surface
            .update(cx, move |surface, _cx| {
                surface.slack_video_clip_is_ready_to_record(generation)
            })
            .unwrap_or(false);
        if !current {
            self.set_interaction_error(
                "The video preview no longer belongs to the active draft.",
                cx,
            );
            return;
        }
        self.countdown_serial = self
            .countdown_serial
            .checked_add(1)
            .expect("video clip countdown serial must not overflow");
        let countdown = SlackVideoClipCountdown {
            serial: self.countdown_serial,
            seconds_remaining: SLACK_VIDEO_CLIP_COUNTDOWN_START,
        };
        self.countdown = Some(countdown);
        self.diagnostic = None;
        cx.notify();
        self.schedule_countdown_tick(countdown.serial, cx);
    }

    fn schedule_countdown_tick(&self, serial: u64, cx: &mut Context<Self>) {
        spawn_timer_task_for_entity(serial, COUNTDOWN_TICK, cx, |this, serial, cx| {
            this.advance_countdown(serial, cx);
        });
    }

    fn advance_countdown(&mut self, serial: u64, cx: &mut Context<Self>) {
        let Some(mut countdown) = self.countdown else {
            return;
        };
        if countdown.serial != serial {
            return;
        }
        let generation = self.generation;
        let current = self
            .surface
            .update(cx, move |surface, _cx| {
                surface.slack_video_clip_is_ready_to_record(generation)
            })
            .unwrap_or(false);
        if !current {
            self.countdown = None;
            self.set_interaction_error(
                "The active draft changed before video recording began.",
                cx,
            );
            return;
        }
        if countdown.seconds_remaining > 1 {
            countdown.seconds_remaining -= 1;
            self.countdown = Some(countdown);
            cx.notify();
            self.schedule_countdown_tick(serial, cx);
            return;
        }
        self.countdown = None;
        self.phase = SlackVideoClipModalPhase::Starting;
        cx.notify();
        let result = self.surface.update(cx, move |surface, cx| {
            surface.begin_slack_video_clip_recording_after_countdown(generation, cx)
        });
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => self.set_interaction_error(error, cx),
            Err(_) => self.set_interaction_error("The Chat surface closed.", cx),
        }
    }

    pub(super) fn request_stop(&mut self, cx: &mut Context<Self>) {
        if self.cancel_countdown(cx) {
            return;
        }
        if self.phase != SlackVideoClipModalPhase::Recording {
            return;
        }
        self.phase = SlackVideoClipModalPhase::Finalizing;
        self.focus_pending = true;
        cx.notify();
        let generation = self.generation;
        let result = self.surface.update(cx, move |surface, cx| {
            surface.stop_slack_video_clip_capture_for_generation(generation, cx)
        });
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => self.set_interaction_error(error, cx),
            Err(_) => self.set_interaction_error("The Chat surface closed.", cx),
        }
    }

    pub(super) fn request_done(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.phase != SlackVideoClipModalPhase::Reviewing {
            return;
        }
        self.retire_review_player(cx);
        self.phase = SlackVideoClipModalPhase::Attaching;
        self.diagnostic = None;
        self.focus_pending = true;
        cx.notify();
        let generation = self.generation;
        let result = self.surface.update(cx, move |surface, cx| {
            surface.attach_reviewed_slack_video_clip_for_generation(generation, cx)
        });
        match result {
            Ok(Ok(_)) => self.refocus_composer(window, cx),
            Ok(Err(error)) => {
                self.set_interaction_error(error, cx);
            }
            Err(_) => self.set_interaction_error("The Chat surface closed.", cx),
        }
    }

    pub(super) fn request_start_over(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.phase != SlackVideoClipModalPhase::Reviewing {
            return;
        }
        self.retire_review_player(cx);
        let generation = self.generation;
        let result = self.surface.update(cx, move |surface, cx| {
            surface.restart_slack_video_clip_capture_for_generation(generation, cx)
        });
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(error)) => self.set_interaction_error(error, cx),
            Err(_) => {
                self.set_interaction_error("The Chat surface closed.", cx);
                self.refocus_composer(window, cx);
            }
        }
    }

    pub(super) fn request_download(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.phase != SlackVideoClipModalPhase::Reviewing || self.download_pending {
            return;
        }
        self.download_pending = true;
        self.diagnostic = None;
        cx.notify();
        let generation = self.generation;
        let result = self.surface.update(cx, move |surface, cx| {
            surface.prompt_download_reviewed_slack_video_clip(generation, window, cx)
        });
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => self.finish_download(Err(error), cx),
            Err(_) => self.finish_download(Err("The Chat surface closed.".to_string()), cx),
        }
    }

    pub(super) fn request_upload_video(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(
            self.phase,
            SlackVideoClipModalPhase::Preparing | SlackVideoClipModalPhase::Previewing
        ) {
            return;
        }
        self.diagnostic = None;
        cx.notify();
        let generation = self.generation;
        let result = self.surface.update(cx, move |surface, cx| {
            surface.upload_video_from_slack_clip_modal(generation, window, cx)
        });
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => self.set_interaction_error(error, cx),
            Err(_) => self.set_interaction_error("The Chat surface closed.", cx),
        }
    }

    pub(super) fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(
            self.phase,
            SlackVideoClipModalPhase::Finalizing
                | SlackVideoClipModalPhase::Attaching
                | SlackVideoClipModalPhase::Cancelling
        ) {
            return;
        }
        if matches!(
            self.phase,
            SlackVideoClipModalPhase::Recording | SlackVideoClipModalPhase::Reviewing
        ) {
            self.discard_confirmation = true;
            self.focus_pending = true;
            cx.notify();
            return;
        }
        self.cancel_and_refocus(window, cx);
    }

    pub(super) fn cancel_discard_confirmation(&mut self, cx: &mut Context<Self>) {
        self.discard_confirmation = false;
        self.focus_pending = true;
        cx.notify();
    }

    pub(super) fn confirm_discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.discard_confirmation = false;
        self.cancel_and_refocus(window, cx);
    }

    fn cancel_and_refocus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.retire_review_player(cx);
        let generation = self.generation;
        let result = self.surface.update(cx, move |surface, cx| {
            surface.cancel_slack_video_clip_capture_for_generation(generation, cx)
        });
        match result {
            Ok(Ok(_)) => self.refocus_composer(window, cx),
            Ok(Err(error)) => self.set_interaction_error(error, cx),
            Err(_) => self.set_interaction_error("The Chat surface closed.", cx),
        }
    }

    pub(super) fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        if event.keystroke.key.as_str() == "escape" {
            window.prevent_default();
            cx.stop_propagation();
            if self.discard_confirmation {
                self.cancel_discard_confirmation(cx);
            } else {
                self.request_close(window, cx);
            }
        }
    }

    fn refocus_composer(&self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = self
            .surface
            .update(cx, |surface, cx| {
                surface.focus_slack_composer(cx);
                surface.slack_composer_input.read(cx).focus_handle_clone()
            })
            .ok();
        if let Some(focus) = focus {
            window.focus(&focus, cx);
        }
    }

    pub(crate) fn set_interaction_error(
        &mut self,
        error: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        self.diagnostic = Some(error.into().into());
        cx.notify();
    }
}
