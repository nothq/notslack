use gpui::{KeyDownEvent, Role};

use super::{
    alpha, div, point, px, rgb, slack_dm_peer_local_time_label, slack_error_banner, slack_icon,
    slack_palette, AnyElement, BoxShadow, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    SlackComposerFormatAction, SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
    Window, SLACK_COMPOSER_ATTACHMENTS_HEIGHT, SLACK_COMPOSER_INPUT_HEIGHT,
    SLACK_COMPOSER_TOOLBAR_HEIGHT,
};
use crate::ui::surface::SlackMainComposerPresentation;

use self::notice::{prepare_slack_main_composer_notice, SlackPreparedMainComposerNotice};

mod audio_clip;
mod input;
mod link;
mod notice;
mod schedule;
mod toolbar;
mod video_clip;

const SLACK_COMPOSER_LINE_HEIGHT: f32 = 22.0;
const SLACK_COMPOSER_VERTICAL_PADDING: f32 = 16.0;
const SLACK_AUDIO_CLIP_STATUS_HEIGHT: f32 = 36.0;

struct SlackComposerShellProps<'a> {
    composer_placeholder: &'a str,
    composer_notice: Option<&'a SlackPreparedMainComposerNotice>,
    has_draft_attachments: bool,
    composer_shell_height: f32,
}

impl SurfaceState {
    pub(crate) fn render_slack_composer_panel(
        &self,
        presentation: &SlackMainComposerPresentation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let has_draft_attachments = !self.slack_composer_files.is_empty();
        let composer_notice = presentation
            .notice
            .as_ref()
            .and_then(prepare_slack_main_composer_notice);
        let composer_popover = self.slack_aux_panel.as_ref().filter(|panel| {
            self.slack_composer_aux_target.as_ref() == Some(&super::SlackComposerTarget::Main)
                && matches!(
                    panel.query_behavior,
                    Some(
                        super::SlackAuxPanelQueryBehavior::Emoji
                            | super::SlackAuxPanelQueryBehavior::Mention
                    )
                )
        });
        let composer_shell_height =
            self.slack_composer_shell_height(composer_notice.is_some(), has_draft_attachments, cx);
        (div()
            .relative()
            .h(px(composer_shell_height + 24.0))
            .flex_none()
            .px(px(20.0))
            .flex()
            .flex_col()
            .child(self.render_slack_composer_shell(
                SlackComposerShellProps {
                    composer_placeholder: presentation.placeholder.as_ref(),
                    composer_notice: composer_notice.as_ref(),
                    has_draft_attachments,
                    composer_shell_height,
                },
                presentation,
                cx,
            ))
            .when_some(composer_popover, |this, panel| {
                this.child(self.render_slack_composer_popover(panel, cx))
            })
            .child(self.render_slack_composer_hint()))
        .into_any_element()
    }

    fn slack_composer_shell_height(
        &self,
        has_notice: bool,
        has_draft_attachments: bool,
        cx: &Context<Self>,
    ) -> f32 {
        80.0 + self.slack_composer_input_extra_height(cx)
            + if has_notice { 42.0 } else { 0.0 }
            + if has_draft_attachments {
                SLACK_COMPOSER_ATTACHMENTS_HEIGHT
            } else {
                0.0
            }
            + if self.slack_formatting_enabled {
                38.0
            } else {
                0.0
            }
            + if self.slack_audio_clip_capture_visible_for_current_draft() {
                SLACK_AUDIO_CLIP_STATUS_HEIGHT
            } else {
                0.0
            }
    }

    fn slack_composer_input_extra_height(&self, cx: &Context<Self>) -> f32 {
        let visible_lines = self
            .slack_composer_input
            .read(cx)
            .measured_line_count()
            .min(self.slack_composer_max_visible_lines());
        visible_lines.saturating_sub(1) as f32 * SLACK_COMPOSER_LINE_HEIGHT
    }

    fn slack_composer_max_visible_lines(&self) -> usize {
        let max_editor_height = self.viewport_height * 0.60 - 80.0;
        ((max_editor_height - SLACK_COMPOSER_VERTICAL_PADDING) / SLACK_COMPOSER_LINE_HEIGHT)
            .floor()
            .max(1.0) as usize
    }

    fn render_slack_composer_shell(
        &self,
        props: SlackComposerShellProps<'_>,
        presentation: &SlackMainComposerPresentation,
        cx: &mut Context<Self>,
    ) -> Div {
        let notice_height = if props.composer_notice.is_some() {
            42.0
        } else {
            0.0
        };
        div()
            .h(px(props.composer_shell_height))
            .flex()
            .flex_col()
            .when_some(props.composer_notice, |this, notice| {
                this.child(self.render_slack_composer_notice(notice, cx))
            })
            .child(self.render_slack_composer_editor_shell(
                &props,
                presentation,
                props.composer_shell_height - notice_height,
                cx,
            ))
    }

    fn render_slack_composer_editor_shell(
        &self,
        props: &SlackComposerShellProps<'_>,
        presentation: &SlackMainComposerPresentation,
        editor_height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-main-composer-group")
            .role(Role::Group)
            .aria_label("composer")
            .h(px(editor_height))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(if self.slack_composer_focused {
                palette.composer_focused_border
            } else {
                palette.composer_border
            }))
            .bg(rgb(palette.composer_bg))
            .flex()
            .flex_col()
            .when_some(self.slack_error.clone(), |this, message| {
                this.child(slack_error_banner(&message))
            })
            .when(self.slack_error.is_none(), |this| {
                this.when_some(self.slack_main_draft_autosave_error(), |this, message| {
                    this.child(slack_error_banner(message))
                })
            })
            .when(props.has_draft_attachments, |this| {
                this.child(self.render_slack_main_draft_attachments("main", cx))
            })
            .when(
                self.slack_formatting_enabled && self.has_current_slack_send_target(),
                |this| this.child(self.render_slack_composer_format_bar(cx)),
            )
            .child(self.render_slack_composer_field(props.composer_placeholder, presentation, cx))
            .when(
                self.slack_audio_clip_capture_visible_for_current_draft(),
                |this| this.child(self.render_slack_audio_clip_capture_status(cx)),
            )
            .child(self.render_slack_composer_toolbar(cx))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_slack_composer_shell_key_down(event, window, cx);
            }))
    }

    fn handle_slack_composer_shell_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.handle_slack_composer_link_shortcut(event, cx) {
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        if event.keystroke.key != "escape"
            || event.keystroke.modifiers.modified()
            || !self.slack_formatting_enabled
        {
            return;
        }
        let composer_focus = self.slack_composer_input.read(cx).focus_handle_clone();
        self.slack_formatting_enabled = false;
        self.slack_composer_focused = true;
        window.focus(&composer_focus, cx);
        cx.stop_propagation();
        cx.notify();
    }

    pub(in crate::ui::surface) fn render_slack_composer_hint(&self) -> Div {
        div()
            .h(px(24.0))
            .pt(px(4.0))
            .pr(px(8.0))
            .flex()
            .items_start()
            .justify_end()
            .text_size(px(10.5))
            .line_height(px(16.0))
            .text_color(rgb(0xABABAD))
            .opacity(0.0)
            .child("Shift + Return to add a new line.")
    }

    fn render_slack_composer_field(
        &self,
        composer_placeholder: &str,
        presentation: &SlackMainComposerPresentation,
        cx: &mut Context<Self>,
    ) -> Div {
        let private_channel = presentation.private_channel;
        let private_channel_label = (private_channel && self.slack_composer_text.is_empty())
            .then(|| presentation.conversation_label.clone());
        let props = self.slack_composer_input_props(
            composer_placeholder,
            presentation.conversation_label.as_ref(),
            private_channel,
            cx,
        );
        self.slack_composer_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        let palette = slack_palette(self.appearance_mode);
        div()
            .relative()
            .w_full()
            .min_w(px(0.0))
            .flex_none()
            .flex()
            .flex_col()
            .child(self.slack_composer_input.clone())
            .when_some(private_channel_label, |this, channel_label| {
                this.child(
                    div()
                        .absolute()
                        .top(px(8.0))
                        .left(px(12.0))
                        .h(px(SLACK_COMPOSER_LINE_HEIGHT))
                        .flex()
                        .items_center()
                        .text_size(px(15.0))
                        .line_height(px(SLACK_COMPOSER_LINE_HEIGHT))
                        .text_color(rgb(palette.composer_placeholder))
                        .child("Message ")
                        .child(
                            div()
                                .w(px(14.0))
                                .h(px(SLACK_COMPOSER_LINE_HEIGHT))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(slack_icon(
                                    SlackShellIcon::LockSmall,
                                    palette.composer_placeholder,
                                    14.0,
                                    cx,
                                )),
                        )
                        .child(channel_label),
                )
            })
    }

    pub(crate) fn ensure_slack_composer_blur_observer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = self.slack_composer_input.read(cx).focus_handle_clone();
        if !self.slack_composer_blur_observer_registered {
            cx.on_blur(&focus, window, |surface, _, cx| {
                if surface.slack_composer_focused {
                    surface.slack_composer_focused = false;
                    cx.notify();
                }
            })
            .detach();
            self.slack_composer_blur_observer_registered = true;
        }
        if !self.slack_composer_focused && focus.is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }
    }
}
