use std::sync::Arc;

use crate::ui::surface::{
    slack_palette, SlackAttachmentSelection, SlackMediaHostId, SlackMediaPlayback, SurfaceState,
};
use crate::ui::{
    alpha, div, img, px, rgb, AnyElement, Context, Div, Entity, FluentBuilder, FontWeight, Image,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement,
    StatefulInteractiveElement, Styled, VideoPlayer,
};
use gpui::{ObjectFit, Role, Stateful, StyledImage};
use gpui_components::backdrop::blocking_backdrop;

pub(super) enum SlackInlineVideoState {
    Inactive,
    Loading,
    Ready(Entity<VideoPlayer>),
    Failed,
}

pub(crate) struct SlackInlineVideoFrame {
    pub(crate) id: String,
    pub(crate) selection: SlackAttachmentSelection,
    pub(crate) host: SlackMediaHostId,
    pub(crate) preview_image: Option<Arc<Image>>,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) radius: f32,
    pub(crate) object_fit: ObjectFit,
    pub(crate) duration_label: Option<gpui::SharedString>,
}

struct SlackInlineVideoInteraction {
    selection: SlackAttachmentSelection,
    host: SlackMediaHostId,
    can_prepare: bool,
    interactive: bool,
    focus_color: u32,
}

struct SlackInlineVideoContent<'a> {
    state: &'a SlackInlineVideoState,
    preview_image: Option<Arc<Image>>,
    object_fit: ObjectFit,
    duration_label: Option<gpui::SharedString>,
    placeholder_title: gpui::SharedString,
    can_prepare: bool,
}

fn slack_inline_video_frame_shell(
    id: String,
    width: f32,
    height: f32,
    radius: f32,
    background: u32,
) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .w(px(width))
        .h(px(height))
        .max_w_full()
        .flex_none()
        .rounded(px(radius))
        .overflow_hidden()
        .bg(rgb(background))
}

impl SurfaceState {
    pub(super) fn slack_inline_video_state(
        &self,
        attachment_id: &str,
        file_id: &str,
        host: &SlackMediaHostId,
        cx: &Context<Self>,
    ) -> SlackInlineVideoState {
        let Some(playback) = self.slack_media_playback.as_ref().filter(|playback| {
            playback.target().matches(attachment_id, file_id) && playback.host() == host
        }) else {
            return SlackInlineVideoState::Inactive;
        };
        match playback {
            SlackMediaPlayback::Loading { .. } => SlackInlineVideoState::Loading,
            SlackMediaPlayback::Video { player, .. }
                if player.read(cx).playback_error_message().is_some() =>
            {
                SlackInlineVideoState::Failed
            }
            SlackMediaPlayback::Video { player, .. } => {
                SlackInlineVideoState::Ready(player.clone())
            }
            SlackMediaPlayback::Failed { .. } => SlackInlineVideoState::Failed,
            SlackMediaPlayback::Audio { .. } => SlackInlineVideoState::Inactive,
        }
    }

    pub(crate) fn render_slack_inline_video_frame(
        &self,
        frame: SlackInlineVideoFrame,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let SlackInlineVideoFrame {
            id,
            selection,
            host,
            preview_image,
            width,
            height,
            radius,
            object_fit,
            duration_label,
        } = frame;
        let media = slack_inline_video_media(&selection);
        debug_assert_eq!(media.kind(), crate::ui::SlackAttachmentMediaKind::Video);
        let state = self.slack_inline_video_state(
            selection.attachment_id.as_ref(),
            media.file_id(),
            &host,
            cx,
        );
        let can_prepare = self
            .slack_workspace_api_capabilities
            .prepare_attachment_media;
        let interactive = !matches!(
            &state,
            SlackInlineVideoState::Loading | SlackInlineVideoState::Ready(_)
        );
        let palette = slack_palette(self.appearance_mode);
        let placeholder_title = selection.title.clone();
        let frame =
            slack_inline_video_frame_shell(id, width, height, radius, palette.attachment_bg);
        let interaction = SlackInlineVideoInteraction {
            selection,
            host,
            can_prepare,
            interactive,
            focus_color: palette.link,
        };
        self.bind_slack_inline_video_interaction(frame, &state, interaction, cx)
            .child(self.render_slack_inline_video_content(
                SlackInlineVideoContent {
                    state: &state,
                    preview_image,
                    object_fit,
                    duration_label,
                    placeholder_title,
                    can_prepare,
                },
                cx,
            ))
    }

    fn bind_slack_inline_video_interaction(
        &self,
        frame: Stateful<Div>,
        state: &SlackInlineVideoState,
        interaction: SlackInlineVideoInteraction,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let intercepts_mouse_down = !matches!(state, SlackInlineVideoState::Ready(_));
        let interactive = interaction.interactive;
        let can_prepare = interaction.can_prepare;
        let focus_color = interaction.focus_color;
        let mouse_selection = interaction.selection;
        let mouse_host = interaction.host;
        let title = mouse_selection.title.clone();
        let keyboard_selection = mouse_selection.clone();
        let keyboard_host = mouse_host.clone();
        frame
            .when(intercepts_mouse_down, |this| {
                this.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if interactive {
                            this.activate_slack_inline_video(
                                &mouse_selection,
                                mouse_host.clone(),
                                can_prepare,
                                cx,
                            );
                        }
                    }),
                )
            })
            .when(interactive, |this| {
                this.role(Role::Button)
                    .aria_label(slack_inline_video_action_label(title.as_ref(), can_prepare))
                    .focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .focus_visible(move |style| style.border_2().border_color(rgb(focus_color)))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if slack_inline_video_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.activate_slack_inline_video(
                                &keyboard_selection,
                                keyboard_host.clone(),
                                can_prepare,
                                cx,
                            );
                        }
                    }))
            })
    }

    fn activate_slack_inline_video(
        &mut self,
        selection: &SlackAttachmentSelection,
        host: SlackMediaHostId,
        can_prepare: bool,
        cx: &mut Context<Self>,
    ) {
        if can_prepare {
            self.activate_slack_attachment_media(selection, host, cx);
        } else {
            self.toggle_slack_attachment_expanded(selection, cx);
        }
    }

    fn render_slack_inline_video_content(
        &self,
        content: SlackInlineVideoContent<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let SlackInlineVideoContent {
            state,
            preview_image,
            object_fit,
            duration_label,
            placeholder_title,
            can_prepare,
        } = content;
        if let SlackInlineVideoState::Ready(player) = state {
            return player.clone().into_any_element();
        }
        let preview_missing = preview_image.is_none();
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .when(preview_missing, |this| {
                this.child(self.render_slack_attachment_preview_placeholder(placeholder_title, cx))
            })
            .when_some(preview_image, |this, image| {
                this.child(img(image).size_full().object_fit(object_fit))
            })
            .when(
                can_prepare && matches!(state, SlackInlineVideoState::Inactive),
                |this| this.child(slack_inline_video_play_overlay(cx)),
            )
            .when(matches!(state, SlackInlineVideoState::Loading), |this| {
                this.child(blocking_backdrop(
                    slack_inline_video_status_overlay("Preparing playback…", false),
                    cx,
                ))
            })
            .when(matches!(state, SlackInlineVideoState::Failed), |this| {
                this.child(slack_inline_video_status_overlay(
                    "Playback failed — retry",
                    true,
                ))
            })
            .when_some(
                duration_label.filter(|_| matches!(state, SlackInlineVideoState::Inactive)),
                |this, duration| this.child(slack_inline_video_duration(duration)),
            )
            .into_any_element()
    }
}

fn slack_inline_video_media(
    selection: &SlackAttachmentSelection,
) -> &crate::ui::SlackAttachmentMedia {
    selection
        .attachment
        .media
        .as_ref()
        .expect("inline Slack video frame must have typed media")
}

fn slack_inline_video_play_overlay(cx: &mut Context<SurfaceState>) -> Div {
    div()
        .absolute()
        .left(px(0.0))
        .right(px(0.0))
        .top(px(0.0))
        .bottom(px(0.0))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .size(px(48.0))
                .rounded(px(24.0))
                .bg(alpha(0x000000, 0.72))
                .flex()
                .items_center()
                .justify_center()
                .child(crate::ui::surface::slack_icon(
                    crate::ui::surface::SlackShellIcon::Play,
                    0xffffff,
                    22.0,
                    cx,
                )),
        )
}

fn slack_inline_video_duration(duration: gpui::SharedString) -> Div {
    div()
        .absolute()
        .left(px(12.0))
        .bottom(px(13.0))
        .h(px(30.0))
        .rounded(px(6.0))
        .bg(alpha(0x000000, 0.72))
        .px(px(10.0))
        .flex()
        .items_center()
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(rgb(0xffffff))
        .child(duration)
}

fn slack_inline_video_action_label(title: &str, can_prepare: bool) -> String {
    if can_prepare {
        format!("Play video {title}")
    } else {
        format!("Open preview for {title}")
    }
}

fn slack_inline_video_action_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}

fn slack_inline_video_status_overlay(label: &'static str, retry: bool) -> Div {
    div()
        .absolute()
        .left(px(0.0))
        .right(px(0.0))
        .top(px(0.0))
        .bottom(px(0.0))
        .bg(alpha(0x000000, if retry { 0.62 } else { 0.48 }))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .rounded(px(6.0))
                .bg(alpha(0x000000, 0.72))
                .px(px(12.0))
                .py(px(7.0))
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(0xffffff))
                .child(label),
        )
}
