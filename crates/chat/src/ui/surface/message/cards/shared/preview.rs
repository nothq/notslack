use std::sync::Arc;

use super::super::video::SlackInlineVideoFrame;
use crate::ui::surface::{
    slack_icon, slack_palette, SlackAttachmentSelection, SlackMediaHostId,
    SlackMessageRenderContext, SlackSharedMessageFileRow, SlackShellIcon, SurfaceState,
};
use crate::ui::{
    div, img, px, rgb, AnyElement, Context, Div, FluentBuilder, Image, InteractiveElement,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement,
    SlackAttachmentMediaKind, StatefulInteractiveElement, Styled,
};
use gpui::{ObjectFit, Role, Stateful, StyledImage};

use self::controls::{
    slack_shared_message_action_key, slack_shared_message_file_title,
    slack_shared_message_media_action,
};

mod controls;

struct SlackSharedMessageFilePresentation {
    preview_image: Option<Arc<Image>>,
    has_preview: bool,
    collapsed: bool,
}

struct SlackSharedFilePreviewInteraction {
    selection: Option<SlackAttachmentSelection>,
    link_url: gpui::SharedString,
}

struct SlackSharedFilePreviewBinding<'a> {
    file: &'a SlackSharedMessageFileRow,
    interaction: SlackSharedFilePreviewInteraction,
    interactive: bool,
    focus_color: u32,
}

impl SurfaceState {
    pub(super) fn render_slack_shared_message_file(
        &self,
        file: &SlackSharedMessageFileRow,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Div {
        let presentation = SlackSharedMessageFilePresentation {
            preview_image: self
                .slack_attachment_preview_image_by_key(file.preview_cache_key.as_deref()),
            has_preview: file.preview_cache_key.is_some()
                && file.preview_width > 0
                && file.preview_height > 0,
            collapsed: self
                .slack_collapsed_attachment_ids
                .contains(file.attachment_id.as_ref()),
        };
        div()
            .w_full()
            .max_w(px(572.0))
            .flex()
            .flex_col()
            .child(self.render_slack_shared_message_file_header(
                file,
                &presentation,
                render_context,
                cx,
            ))
            .when(
                !presentation.collapsed
                    && (presentation.has_preview
                        || file
                            .attachment
                            .media
                            .as_ref()
                            .is_some_and(|media| media.kind() == SlackAttachmentMediaKind::Video)),
                |this| {
                    this.child(self.render_slack_shared_message_file_preview(
                        file,
                        presentation.preview_image,
                        render_context,
                        cx,
                    ))
                },
            )
    }

    fn render_slack_shared_message_file_header(
        &self,
        file: &SlackSharedMessageFileRow,
        presentation: &SlackSharedMessageFilePresentation,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Div {
        let color = slack_palette(self.appearance_mode).attachment_muted_text;
        div()
            .w_full()
            .min_w(px(0.0))
            .h(px(22.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(slack_shared_message_file_title(file.title.clone(), color))
            .when(file.attachment.media.is_some(), |this| {
                this.child(self.render_slack_shared_message_file_play_button(
                    file,
                    render_context,
                    color,
                    cx,
                ))
            })
            .when(presentation.has_preview, |this| {
                this.child(self.render_slack_shared_message_file_toggle(
                    file,
                    presentation.collapsed,
                    color,
                    cx,
                ))
            })
    }

    fn render_slack_shared_message_file_play_button(
        &self,
        file: &SlackSharedMessageFileRow,
        render_context: SlackMessageRenderContext,
        color: u32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selection = SlackAttachmentSelection::from_shared_file(file);
        let keyboard_selection = selection.clone();
        let host = SlackMediaHostId::message(render_context, file.attachment_id.clone());
        let keyboard_host = host.clone();
        let media_kind = file
            .attachment
            .media
            .as_ref()
            .expect("shared-message media action requires shaped media")
            .kind();
        let (icon, media_label) = slack_shared_message_media_action(media_kind);
        let control_background = slack_palette(self.appearance_mode).composer_chip_bg;
        div()
            .id(format!(
                "slack-shared-attachment-play-{}",
                file.attachment_id
            ))
            .role(Role::Button)
            .aria_label(format!("Play {media_label} {}", file.title))
            .focusable()
            .tab_stop(true)
            .size(px(20.0))
            .rounded(px(4.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(control_background)))
            .focus_visible(move |style| style.bg(rgb(control_background)))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.activate_slack_shared_message_file(media_kind, &selection, host.clone(), cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if slack_shared_message_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.activate_slack_shared_message_file(
                        media_kind,
                        &keyboard_selection,
                        keyboard_host.clone(),
                        cx,
                    );
                }
            }))
            .child(slack_icon(icon, color, 16.0, cx))
    }

    fn activate_slack_shared_message_file(
        &mut self,
        media_kind: SlackAttachmentMediaKind,
        selection: &SlackAttachmentSelection,
        host: SlackMediaHostId,
        cx: &mut Context<Self>,
    ) {
        if media_kind == SlackAttachmentMediaKind::Video {
            self.activate_slack_attachment_media(selection, host, cx);
        } else {
            self.toggle_slack_attachment_expanded(selection, cx);
        }
    }

    fn render_slack_shared_message_file_toggle(
        &self,
        file: &SlackSharedMessageFileRow,
        collapsed: bool,
        color: u32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let clicked_attachment_id = file.attachment_id.clone();
        let keyboard_attachment_id = clicked_attachment_id.clone();
        let control_background = slack_palette(self.appearance_mode).composer_chip_bg;
        div()
            .id(format!(
                "slack-shared-attachment-collapse-{}",
                file.attachment_id
            ))
            .role(Role::Button)
            .aria_label(format!(
                "{} preview for {}",
                if collapsed { "Expand" } else { "Collapse" },
                file.title
            ))
            .aria_expanded(!collapsed)
            .focusable()
            .tab_stop(true)
            .size(px(20.0))
            .rounded(px(4.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(control_background)))
            .focus_visible(move |style| style.bg(rgb(control_background)))
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.toggle_slack_attachment_collapsed(clicked_attachment_id.as_ref(), cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if slack_shared_message_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_attachment_collapsed(keyboard_attachment_id.as_ref(), cx);
                }
            }))
            .child(slack_icon(
                if collapsed {
                    SlackShellIcon::AttachmentCaretRight
                } else {
                    SlackShellIcon::AttachmentCaretDown
                },
                color,
                20.0,
                cx,
            ))
    }

    fn render_slack_shared_message_file_preview(
        &self,
        file: &SlackSharedMessageFileRow,
        preview_image: Option<Arc<Image>>,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(video) = self.render_slack_shared_message_video_preview(
            file,
            preview_image.clone(),
            render_context,
            cx,
        ) {
            return video;
        }
        let palette = slack_palette(self.appearance_mode);
        let interaction = SlackSharedFilePreviewInteraction {
            selection: file
                .attachment
                .media
                .as_ref()
                .map(|_| SlackAttachmentSelection::from_shared_file(file)),
            link_url: file.link_url.clone(),
        };
        let interactive = interaction.selection.is_some() || !interaction.link_url.is_empty();
        let preview = div()
            .id(format!(
                "slack-shared-attachment-preview-{}",
                file.attachment_id
            ))
            .w(px(file.preview_width as f32))
            .h(px(file.preview_height as f32))
            .rounded(px(4.0))
            .overflow_hidden()
            .bg(rgb(palette.attachment_bg));
        self.bind_slack_shared_file_preview(
            preview,
            SlackSharedFilePreviewBinding {
                file,
                interaction,
                interactive,
                focus_color: palette.composer_focused_border,
            },
            cx,
        )
        .when_some(preview_image, |this, image| {
            this.child(
                img(image)
                    .size_full()
                    .object_fit(ObjectFit::Contain)
                    .rounded(px(4.0)),
            )
        })
        .into_any_element()
    }

    fn render_slack_shared_message_video_preview(
        &self,
        file: &SlackSharedMessageFileRow,
        preview_image: Option<Arc<Image>>,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let media = file.attachment.media.as_ref()?;
        if media.kind() != SlackAttachmentMediaKind::Video {
            return None;
        }
        let (width, height) = if file.preview_width > 0 && file.preview_height > 0 {
            (file.preview_width as f32, file.preview_height as f32)
        } else {
            (360.0, 202.0)
        };
        Some(
            self.render_slack_inline_video_frame(
                SlackInlineVideoFrame {
                    id: format!("slack-shared-attachment-preview-{}", file.attachment_id),
                    selection: SlackAttachmentSelection::from_shared_file(file),
                    host: SlackMediaHostId::message(render_context, file.attachment_id.clone()),
                    preview_image,
                    width,
                    height,
                    radius: 4.0,
                    object_fit: ObjectFit::Contain,
                    duration_label: file.attachment.duration_millis.map(|duration| {
                        let total_seconds = duration.get() / 1_000;
                        format!("{}:{:02}", total_seconds / 60, total_seconds % 60).into()
                    }),
                },
                cx,
            )
            .into_any_element(),
        )
    }

    fn bind_slack_shared_file_preview(
        &self,
        preview: Stateful<Div>,
        binding: SlackSharedFilePreviewBinding<'_>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let SlackSharedFilePreviewBinding {
            file,
            interaction,
            interactive,
            focus_color,
        } = binding;
        let mouse_selection = interaction.selection.clone();
        let keyboard_selection = interaction.selection;
        let mouse_link_url = interaction.link_url.clone();
        let keyboard_link_url = interaction.link_url;
        preview.when(interactive, |this| {
            this.role(Role::Button)
                .aria_label(format!("Open preview for {}", file.title))
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .focus_visible(move |style| style.border_2().border_color(rgb(focus_color)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if let Some(selection) = mouse_selection.as_ref() {
                            this.toggle_slack_attachment_expanded(selection, cx);
                        } else {
                            this.open_slack_link(mouse_link_url.as_ref(), cx);
                        }
                    }),
                )
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    if slack_shared_message_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        if let Some(selection) = keyboard_selection.as_ref() {
                            this.toggle_slack_attachment_expanded(selection, cx);
                        } else {
                            this.open_slack_link(keyboard_link_url.as_ref(), cx);
                        }
                    }
                }))
        })
    }
}
