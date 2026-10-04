use std::time::Duration;

use gpui::{KeyDownEvent, Role, Window};

use super::{
    alpha, div, px, rgb, slack_icon, slack_palette, Context, FluentBuilder, InteractiveElement,
    IntoElement, ParentElement, SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::surface::SlackAudioClipCaptureState;

struct SlackAudioClipCapturePresentation {
    label: String,
    recording: bool,
    show_stop: bool,
    show_cancel: bool,
    show_dismiss: bool,
}

impl SurfaceState {
    pub(super) fn render_slack_audio_clip_button(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-composer-audio-clip")
            .role(Role::Button)
            .aria_label("Record audio clip")
            .focusable()
            .tab_stop(true)
            .size(px(28.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xffffff, 0.08)))
            .focus_visible(|style| style.bg(alpha(0xffffff, 0.08)))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::Mic,
                palette.composer_icon,
                18.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.start_slack_audio_clip_from_control(window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !slack_audio_clip_key_activates(event) {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.start_slack_audio_clip_from_control(window, cx);
            }))
    }

    pub(super) fn render_slack_audio_clip_capture_status(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let capture = self
            .slack_composer_capture
            .audio()
            .expect("only an audio capture may render the audio capture status");
        let presentation = slack_audio_clip_capture_presentation(capture);
        div()
            .id("slack-composer-audio-clip-status")
            .role(Role::Status)
            .aria_label(presentation.label.clone())
            .h(px(super::SLACK_AUDIO_CLIP_STATUS_HEIGHT))
            .px(px(10.0))
            .border_t_1()
            .border_color(rgb(palette.composer_border))
            .flex()
            .items_center()
            .justify_between()
            .child(self.render_slack_audio_clip_capture_summary(&presentation, cx))
            .child(self.render_slack_audio_clip_capture_actions(&presentation, cx))
    }

    fn render_slack_audio_clip_capture_summary(
        &self,
        presentation: &SlackAudioClipCapturePresentation,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .min_w(px(0.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .when(presentation.recording, |this| {
                this.child(div().size(px(8.0)).rounded(px(4.0)).bg(rgb(0xe01e5a)))
            })
            .child(slack_icon(
                SlackShellIcon::Mic,
                if presentation.recording {
                    0xe01e5a
                } else {
                    palette.composer_icon
                },
                16.0,
                cx,
            ))
            .child(
                div()
                    .min_w(px(0.0))
                    .text_ellipsis()
                    .text_size(px(12.0))
                    .text_color(rgb(palette.main_text))
                    .child(presentation.label.clone()),
            )
    }

    fn render_slack_audio_clip_capture_actions(
        &self,
        presentation: &SlackAudioClipCapturePresentation,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(6.0))
            .when(presentation.show_stop, |this| {
                this.child(self.render_slack_audio_clip_stop_button(cx))
            })
            .when(presentation.show_cancel, |this| {
                this.child(self.render_slack_audio_clip_cancel_button(cx))
            })
            .when(presentation.show_dismiss, |this| {
                this.child(self.render_slack_audio_clip_dismiss_button(cx))
            })
    }

    fn render_slack_audio_clip_stop_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("slack-composer-audio-clip-stop")
            .role(Role::Button)
            .aria_label("Stop and attach audio clip")
            .focusable()
            .tab_stop(true)
            .h(px(24.0))
            .px(px(9.0))
            .rounded(px(4.0))
            .bg(rgb(0xe01e5a))
            .text_size(px(12.0))
            .text_color(rgb(0xffffff))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0xc0184b)))
            .focus_visible(|style| style.bg(rgb(0xc0184b)))
            .flex()
            .items_center()
            .child("Stop")
            .on_click(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.stop_slack_audio_clip_from_control(window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !slack_audio_clip_key_activates(event) {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.stop_slack_audio_clip_from_control(window, cx);
            }))
    }

    fn render_slack_audio_clip_cancel_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-composer-audio-clip-cancel")
            .role(Role::Button)
            .aria_label("Cancel audio clip")
            .focusable()
            .tab_stop(true)
            .h(px(24.0))
            .px(px(8.0))
            .rounded(px(4.0))
            .text_size(px(12.0))
            .text_color(rgb(palette.main_text))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xffffff, 0.08)))
            .focus_visible(|style| style.bg(alpha(0xffffff, 0.08)))
            .flex()
            .items_center()
            .child("Cancel")
            .on_click(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.cancel_slack_audio_clip_from_control(window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !slack_audio_clip_key_activates(event) {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.cancel_slack_audio_clip_from_control(window, cx);
            }))
    }

    fn render_slack_audio_clip_dismiss_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-composer-audio-clip-dismiss")
            .role(Role::Button)
            .aria_label("Dismiss audio clip error")
            .focusable()
            .tab_stop(true)
            .h(px(24.0))
            .px(px(8.0))
            .rounded(px(4.0))
            .text_size(px(12.0))
            .text_color(rgb(palette.main_text))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xffffff, 0.08)))
            .focus_visible(|style| style.bg(alpha(0xffffff, 0.08)))
            .flex()
            .items_center()
            .child("Dismiss")
            .on_click(cx.listener(|this, _, window, cx| {
                cx.stop_propagation();
                this.dismiss_slack_audio_clip_capture_status(cx);
                this.refocus_slack_composer_after_audio_clip_control(window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !slack_audio_clip_key_activates(event) {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.dismiss_slack_audio_clip_capture_status(cx);
                this.refocus_slack_composer_after_audio_clip_control(window, cx);
            }))
    }

    fn start_slack_audio_clip_from_control(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.start_slack_audio_clip_capture(cx) {
            self.slack_error = Some(error);
            cx.notify();
        }
        self.refocus_slack_composer_after_audio_clip_control(window, cx);
    }

    fn stop_slack_audio_clip_from_control(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.stop_slack_audio_clip_capture(cx) {
            self.slack_error = Some(error);
            cx.notify();
        }
        self.refocus_slack_composer_after_audio_clip_control(window, cx);
    }

    fn cancel_slack_audio_clip_from_control(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.cancel_slack_audio_clip_capture(cx) {
            self.slack_error = Some(error);
            cx.notify();
        }
        self.refocus_slack_composer_after_audio_clip_control(window, cx);
    }

    fn refocus_slack_composer_after_audio_clip_control(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_slack_composer(cx);
        let focus = self.slack_composer_input.read(cx).focus_handle_clone();
        window.focus(&focus, cx);
    }
}

fn slack_audio_clip_capture_presentation(
    capture: &SlackAudioClipCaptureState,
) -> SlackAudioClipCapturePresentation {
    match capture {
        SlackAudioClipCaptureState::RequestingPermission { .. } => {
            SlackAudioClipCapturePresentation {
                label: "Waiting for microphone access…".to_string(),
                recording: false,
                show_stop: false,
                show_cancel: true,
                show_dismiss: false,
            }
        }
        SlackAudioClipCaptureState::CancellingPermission { .. } => {
            idle_slack_audio_clip_capture_presentation("Cancelling microphone request…")
        }
        SlackAudioClipCaptureState::Recording { started_at, .. } => {
            SlackAudioClipCapturePresentation {
                label: format!(
                    "Recording audio clip · {}",
                    format_slack_audio_clip_duration(started_at.elapsed())
                ),
                recording: true,
                show_stop: true,
                show_cancel: true,
                show_dismiss: false,
            }
        }
        SlackAudioClipCaptureState::Finalizing { .. } => {
            idle_slack_audio_clip_capture_presentation("Finishing audio clip…")
        }
        SlackAudioClipCaptureState::Cancelling { .. } => {
            idle_slack_audio_clip_capture_presentation("Cancelling audio clip…")
        }
        SlackAudioClipCaptureState::Failed { diagnostic, .. } => {
            SlackAudioClipCapturePresentation {
                label: diagnostic.clone(),
                recording: false,
                show_stop: false,
                show_cancel: false,
                show_dismiss: true,
            }
        }
        SlackAudioClipCaptureState::Cancelled { .. } => {
            panic!("only a visible audio clip capture state may render its status")
        }
    }
}

fn idle_slack_audio_clip_capture_presentation(label: &str) -> SlackAudioClipCapturePresentation {
    SlackAudioClipCapturePresentation {
        label: label.to_string(),
        recording: false,
        show_stop: false,
        show_cancel: false,
        show_dismiss: false,
    }
}

fn slack_audio_clip_key_activates(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}

fn format_slack_audio_clip_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
