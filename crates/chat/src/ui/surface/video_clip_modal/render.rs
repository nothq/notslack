use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, relative, rgb, Context, Div, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Render, Role, Stateful,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use crate::ui::alpha;

use super::{preview::SlackVideoClipPreviewElement, SlackVideoClipModal, SlackVideoClipModalPhase};
use crate::ui::surface::{slack_icon, SlackShellIcon};

mod buttons;
mod focus;
mod footer;

const RECORDER_DIALOG_WIDTH: f32 = 600.0;
const RECORDER_DIALOG_HEIGHT: f32 = 440.0;
const RECORDER_HEADER_HEIGHT: f32 = 72.0;
const RECORDER_FOOTER_HEIGHT: f32 = 72.0;
const RECORDER_FOOTER_PADDING_X: f32 = 24.0;
const CAMERA_PREVIEW_WIDTH: f32 = 525.0;
const CAMERA_PREVIEW_HEIGHT: f32 = 296.0;
const REVIEW_MEDIA_MAX_HEIGHT: f32 = 338.0;
const RECORDING_LIMIT_SECONDS: f32 = 5.0 * 60.0;

type SlackVideoClipButtonAction =
    fn(&mut SlackVideoClipModal, &mut Window, &mut Context<SlackVideoClipModal>);

#[derive(Clone, Copy)]
struct SlackVideoClipButtonSpec {
    id: &'static str,
    aria_label: &'static str,
    label: &'static str,
    tab_index: isize,
    action: SlackVideoClipButtonAction,
}

impl SlackVideoClipButtonSpec {
    const fn new(
        id: &'static str,
        aria_label: &'static str,
        label: &'static str,
        tab_index: isize,
        action: SlackVideoClipButtonAction,
    ) -> Self {
        Self {
            id,
            aria_label,
            label,
            tab_index,
            action,
        }
    }
}

impl Render for SlackVideoClipModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.initialize_focus_containment(window, cx);
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .bg(alpha(0x000000, 0.56)),
            BackdropDismissal::new(|this: &mut Self, _, window, cx| {
                this.request_close(window, cx);
            }),
            cx,
        );
        div()
            .id("slack-video-clip-layer")
            .absolute()
            .top(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .focusable()
            .track_focus(&self.focus_handle)
            .tab_index(0)
            .tab_group()
            .tab_stop(false)
            .on_key_down(cx.listener(Self::handle_key_down))
            .child(backdrop)
            .child(self.render_start_focus_guard())
            .child(if self.discard_confirmation {
                self.render_discard_dialog(cx)
            } else {
                self.render_recorder_dialog(cx)
            })
            .child(self.render_end_focus_guard())
    }
}

impl SlackVideoClipModal {
    fn render_recorder_dialog(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("slack-video-clip-dialog")
            .role(Role::Dialog)
            .aria_label(self.title())
            .relative()
            .w(px(RECORDER_DIALOG_WIDTH))
            .h(px(RECORDER_DIALOG_HEIGHT))
            .max_w(relative(0.94))
            .max_h(px(640.0))
            .rounded(px(8.0))
            .overflow_hidden()
            .bg(rgb(0x1a1d21))
            .shadow_lg()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(self.render_header(cx))
            .child(self.render_media(cx))
            .child(self.render_footer(cx))
    }

    fn render_header(&self, cx: &mut Context<Self>) -> Div {
        div()
            .h(px(RECORDER_HEADER_HEIGHT))
            .px(px(24.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(20.0))
                    .line_height(px(28.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xf8f8f8))
                    .child(self.title()),
            )
            .when(self.close_available(), |this| {
                this.child(
                    div()
                        .id("slack-video-clip-close")
                        .role(Role::Button)
                        .aria_label("Close video clip recorder")
                        .size(px(36.0))
                        .rounded(px(8.0))
                        .cursor_pointer()
                        .hover(|style| style.bg(alpha(0xffffff, 0.08)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .focusable()
                        .track_focus(&self.close_focus_handle)
                        .tab_index(10)
                        .tab_stop(true)
                        .focus_visible(|style| style.border_2().border_color(rgb(0xffffff)))
                        .child(slack_icon(SlackShellIcon::Close, 0xb9babd, 24.0, cx))
                        .on_click(cx.listener(|this, _, window, cx| {
                            cx.stop_propagation();
                            this.request_close(window, cx);
                        }))
                        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                            if event.keystroke.modifiers.modified()
                                || !matches!(event.keystroke.key.as_str(), "enter" | "space")
                            {
                                return;
                            }
                            window.prevent_default();
                            cx.stop_propagation();
                            this.request_close(window, cx);
                        })),
                )
            })
    }

    fn render_media(&self, _cx: &mut Context<Self>) -> Div {
        let media = div()
            .id("slack-video-clip-media")
            .relative()
            .w(px(CAMERA_PREVIEW_WIDTH))
            .h(px(CAMERA_PREVIEW_HEIGHT))
            .max_h(px(REVIEW_MEDIA_MAX_HEIGHT))
            .rounded(px(8.0))
            .overflow_hidden()
            .bg(rgb(0x000000))
            .flex()
            .items_center()
            .justify_center();
        let media = if self.phase == SlackVideoClipModalPhase::Reviewing {
            match self.review_player.as_ref() {
                Some(player) => media.child(player.clone()),
                None => media.child(self.render_media_status("Preparing review…")),
            }
        } else {
            match self.preview.as_ref() {
                Some(preview) => media.child(SlackVideoClipPreviewElement::new(
                    preview.clone(),
                    "slack-video-clip-preview-surface",
                )),
                None => media.child(self.render_media_status(self.media_status_label())),
            }
        };
        div()
            .h(px(CAMERA_PREVIEW_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(media)
            .when_some(self.countdown, |this, countdown| {
                this.child(
                    div()
                        .absolute()
                        .w(px(CAMERA_PREVIEW_WIDTH))
                        .h(px(CAMERA_PREVIEW_HEIGHT))
                        .rounded(px(8.0))
                        .bg(alpha(0x1d1c1d, 0.70))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(22.0))
                        .line_height(px(30.0))
                        .font_weight(FontWeight::BLACK)
                        .text_color(rgb(0xffffff))
                        .child(format!(
                            "Recording begins in {}…",
                            countdown.seconds_remaining
                        )),
                )
            })
    }

    fn render_media_status(&self, label: &'static str) -> Stateful<Div> {
        div()
            .id("slack-video-clip-media-status")
            .role(Role::Status)
            .aria_label(label)
            .px(px(18.0))
            .text_size(px(14.0))
            .line_height(px(20.0))
            .text_color(rgb(0xd1d2d3))
            .focusable()
            .track_focus(&self.status_focus_handle)
            .tab_index(50)
            .tab_stop(!self.close_available())
            .focus_visible(|style| style.border_2().border_color(rgb(0xffffff)))
            .child(label)
    }

    fn title(&self) -> &'static str {
        match self.phase {
            SlackVideoClipModalPhase::Reviewing | SlackVideoClipModalPhase::Attaching => {
                "Review video clip"
            }
            _ => "Record video clip",
        }
    }

    fn media_status_label(&self) -> &'static str {
        match self.phase {
            SlackVideoClipModalPhase::Preparing => "Preparing camera and microphone…",
            SlackVideoClipModalPhase::Previewing => "Camera preview",
            SlackVideoClipModalPhase::Starting => "Starting recording…",
            SlackVideoClipModalPhase::Recording => "Recording video clip",
            SlackVideoClipModalPhase::Finalizing => "Finishing video clip…",
            SlackVideoClipModalPhase::Reviewing => "Preparing review…",
            SlackVideoClipModalPhase::Attaching => "Attaching video clip…",
            SlackVideoClipModalPhase::Cancelling => "Discarding video clip…",
            SlackVideoClipModalPhase::Failed => "Video clip recording failed",
        }
    }
}
