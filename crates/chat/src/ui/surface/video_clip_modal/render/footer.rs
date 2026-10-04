use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, relative, rgb, Context, Div, FontWeight, InteractiveElement, MouseButton,
    MouseDownEvent, ParentElement, Role, Stateful, StatefulInteractiveElement, Styled,
};

use crate::ui::alpha;

use super::{
    super::{SlackVideoClipModal, SlackVideoClipModalPhase},
    SlackVideoClipButtonSpec, RECORDER_FOOTER_HEIGHT, RECORDER_FOOTER_PADDING_X,
    RECORDING_LIMIT_SECONDS,
};

impl SlackVideoClipModal {
    pub(super) fn render_footer(&self, cx: &mut Context<Self>) -> Div {
        let inline_diagnostic = self.diagnostic.as_ref().filter(|_| {
            matches!(
                self.phase,
                SlackVideoClipModalPhase::Preparing
                    | SlackVideoClipModalPhase::Previewing
                    | SlackVideoClipModalPhase::Recording
                    | SlackVideoClipModalPhase::Reviewing
            )
        });
        div()
            .h(px(RECORDER_FOOTER_HEIGHT))
            .px(px(RECORDER_FOOTER_PADDING_X))
            .flex_none()
            .flex()
            .flex_col()
            .justify_center()
            .when_some(inline_diagnostic, |this, diagnostic| {
                this.child(
                    div()
                        .id("slack-video-clip-diagnostic")
                        .role(Role::Alert)
                        .aria_label(diagnostic.clone())
                        .w_full()
                        .mb(px(4.0))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .text_color(rgb(0xe01e5a))
                        .child(diagnostic.clone()),
                )
            })
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(self.render_footer_left(cx))
                    .child(self.render_footer_right(cx)),
            )
    }

    fn render_footer_left(&self, cx: &mut Context<Self>) -> Div {
        match self.phase {
            SlackVideoClipModalPhase::Preparing | SlackVideoClipModalPhase::Previewing => div()
                .child(self.secondary_button(
                    SlackVideoClipButtonSpec::new(
                        "slack-video-clip-upload",
                        "Upload Video",
                        "Upload Video",
                        20,
                        |this, window, cx| this.request_upload_video(window, cx),
                    ),
                    &self.upload_focus_handle,
                    cx,
                )),
            SlackVideoClipModalPhase::Recording => div()
                .w(px(360.0))
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .w(px(44.0))
                        .whitespace_nowrap()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xf8f8f8))
                        .child(format_duration(self.duration)),
                )
                .child(
                    div()
                        .h(px(4.0))
                        .flex_grow(1.0)
                        .rounded(px(2.0))
                        .bg(alpha(0xffffff, 0.18))
                        .child(
                            div()
                                .h_full()
                                .w(relative(
                                    (self.duration.as_secs_f32() / RECORDING_LIMIT_SECONDS)
                                        .clamp(0.0, 1.0),
                                ))
                                .rounded(px(2.0))
                                .bg(rgb(0xe01e5a)),
                        ),
                ),
            SlackVideoClipModalPhase::Reviewing => div().child(self.secondary_button(
                SlackVideoClipButtonSpec::new(
                    "slack-video-clip-start-over",
                    "Start Over",
                    "Start Over",
                    20,
                    |this, window, cx| this.request_start_over(window, cx),
                ),
                &self.start_over_focus_handle,
                cx,
            )),
            _ => div().child(self.footer_status_label()),
        }
    }

    fn render_footer_right(&self, cx: &mut Context<Self>) -> Div {
        if self.countdown.is_some() {
            return self.render_countdown_controls(cx);
        }
        match self.phase {
            SlackVideoClipModalPhase::Previewing => self.render_preview_controls(cx),
            SlackVideoClipModalPhase::Starting => self.render_starting_controls(cx),
            SlackVideoClipModalPhase::Recording => self.render_recording_controls(cx),
            SlackVideoClipModalPhase::Reviewing => self.render_review_controls(cx),
            _ => footer_controls(),
        }
    }

    fn render_countdown_controls(&self, cx: &mut Context<Self>) -> Div {
        footer_controls().child(self.danger_button(
            SlackVideoClipButtonSpec::new(
                "slack-video-clip-countdown-stop",
                "Stop countdown",
                "Stop",
                30,
                |this, _, cx| {
                    this.cancel_countdown(cx);
                },
            ),
            &self.record_action_focus_handle,
            cx,
        ))
    }

    fn render_preview_controls(&self, cx: &mut Context<Self>) -> Div {
        footer_controls().child(self.danger_button(
            SlackVideoClipButtonSpec::new(
                "slack-video-clip-record",
                "Record video clip",
                "Record",
                30,
                |this, _, cx| this.start_countdown(cx),
            ),
            &self.record_action_focus_handle,
            cx,
        ))
    }

    fn render_starting_controls(&self, cx: &mut Context<Self>) -> Div {
        footer_controls().child(self.danger_button(
            SlackVideoClipButtonSpec::new(
                "slack-video-clip-starting-stop",
                "Cancel video clip recording",
                "Stop",
                30,
                |this, window, cx| this.request_close(window, cx),
            ),
            &self.record_action_focus_handle,
            cx,
        ))
    }

    fn render_recording_controls(&self, cx: &mut Context<Self>) -> Div {
        footer_controls().child(self.danger_button(
            SlackVideoClipButtonSpec::new(
                "slack-video-clip-stop",
                "Stop video clip recording",
                "Stop",
                30,
                |this, _, cx| this.request_stop(cx),
            ),
            &self.record_action_focus_handle,
            cx,
        ))
    }

    fn render_review_controls(&self, cx: &mut Context<Self>) -> Div {
        let download_label = if self.download_pending {
            "Downloading…"
        } else {
            "Download"
        };
        footer_controls()
            .child(self.secondary_button(
                SlackVideoClipButtonSpec::new(
                    "slack-video-clip-download",
                    "Download video clip",
                    download_label,
                    30,
                    |this, window, cx| this.request_download(window, cx),
                ),
                &self.download_focus_handle,
                cx,
            ))
            .child(self.primary_button(
                SlackVideoClipButtonSpec::new(
                    "slack-video-clip-done",
                    "Attach video clip",
                    "Done",
                    40,
                    |this, window, cx| this.request_done(window, cx),
                ),
                &self.done_focus_handle,
                cx,
            ))
    }

    fn footer_status_label(&self) -> Div {
        let label = self
            .diagnostic
            .as_ref()
            .map_or_else(|| self.media_status_label().into(), Clone::clone);
        div()
            .max_w(px(500.0))
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap()
            .text_size(px(13.0))
            .text_color(rgb(if self.diagnostic.is_some() {
                0xe01e5a
            } else {
                0xd1d2d3
            }))
            .child(label)
    }

    pub(super) fn render_discard_dialog(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("slack-video-clip-discard-dialog")
            .role(Role::Dialog)
            .aria_label("Discard video clip")
            .w(px(420.0))
            .max_w(relative(0.92))
            .rounded(px(8.0))
            .bg(rgb(0x1a1d21))
            .shadow_lg()
            .occlude()
            .p(px(24.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(
                div()
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xf8f8f8))
                    .child("Discard video clip?"),
            )
            .child(
                div()
                    .mt(px(12.0))
                    .text_size(px(15.0))
                    .line_height(px(22.0))
                    .text_color(rgb(0xd1d2d3))
                    .child("Your recorded video will not be attached to this message."),
            )
            .child(self.render_discard_actions(cx))
    }

    fn render_discard_actions(&self, cx: &mut Context<Self>) -> Div {
        div()
            .mt(px(24.0))
            .flex()
            .justify_end()
            .gap(px(8.0))
            .child(self.secondary_button(
                SlackVideoClipButtonSpec::new(
                    "slack-video-clip-keep",
                    "Keep video clip",
                    "Cancel",
                    10,
                    |this, _, cx| this.cancel_discard_confirmation(cx),
                ),
                &self.keep_focus_handle,
                cx,
            ))
            .child(self.danger_button(
                SlackVideoClipButtonSpec::new(
                    "slack-video-clip-discard",
                    "Discard video clip",
                    "Discard",
                    20,
                    |this, window, cx| this.confirm_discard(window, cx),
                ),
                &self.discard_focus_handle,
                cx,
            ))
    }
}

fn footer_controls() -> Div {
    div().flex().items_center().gap(px(8.0))
}

fn format_duration(duration: std::time::Duration) -> String {
    let seconds = duration.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
