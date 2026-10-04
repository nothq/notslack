use super::card_helpers::{
    slack_attachment_source_label, slack_compact_attachment_detail,
    slack_compact_attachment_kind_label,
};
use super::{
    div, img, px, rgb, slack_icon, slack_palette, AnyElement, Context, Div, FluentBuilder,
    FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    SlackAttachmentRenderKind, SlackAttachmentRow, SlackShellIcon, StatefulInteractiveElement,
    Styled, SurfaceState,
};
use crate::ui::surface::{SlackAttachmentSelection, SlackPalette};
use crate::ui::SlackAttachment;
use gpui::{KeyDownEvent, Role, Stateful};

mod audio;
mod file;
mod gallery;
mod legacy;
mod shared;
mod video;

pub(crate) use video::SlackInlineVideoFrame;

pub(super) struct SlackCompactAttachmentShellConfig {
    pub(super) icon: SlackShellIcon,
    pub(super) eyebrow: String,
    pub(super) detail: Option<String>,
    pub(super) trailing_action: Option<Div>,
}

struct SlackCompactAttachmentInteraction {
    expands_locally: bool,
    link_url: String,
    selection: SlackAttachmentSelection,
}

struct SlackCompactAttachmentBinding<'a> {
    attachment: &'a SlackAttachment,
    interaction: SlackCompactAttachmentInteraction,
    palette: SlackPalette,
}

fn slack_compact_attachment_base_shell(
    attachment_id: &str,
    is_expanded: bool,
    palette: SlackPalette,
) -> Stateful<Div> {
    div()
        .id(format!("slack-compact-attachment-{attachment_id}"))
        .w(px(440.0))
        .max_w_full()
        .rounded(px(10.0))
        .border_1()
        .border_color(rgb(if is_expanded {
            palette.composer_focused_border
        } else {
            palette.attachment_border
        }))
        .bg(rgb(if is_expanded {
            palette.topic_bg
        } else {
            palette.attachment_bg
        }))
        .px(px(12.0))
        .py(px(10.0))
        .flex()
        .items_center()
        .gap(px(12.0))
}

pub(super) fn slack_attachment_prefers_full_card(attachment_row: &SlackAttachmentRow) -> bool {
    matches!(
        attachment_row.kind,
        SlackAttachmentRenderKind::SharedMessage | SlackAttachmentRenderKind::WebsitePreview
    ) || attachment_row.preview_cache_key.is_some()
}

impl SurfaceState {
    pub(super) fn render_slack_compact_attachment_card(
        &self,
        attachment_row: &SlackAttachmentRow,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let attachment = &attachment_row.attachment;
        let eyebrow = match attachment_row.kind {
            SlackAttachmentRenderKind::WebsitePreview => {
                let source = slack_attachment_source_label(attachment);
                if source.is_empty() {
                    "Link".to_string()
                } else {
                    source
                }
            }
            SlackAttachmentRenderKind::SharedMessage => "Message".to_string(),
            _ => slack_compact_attachment_kind_label(attachment),
        };
        let detail = slack_compact_attachment_detail(attachment_row);
        let icon = match attachment_row.kind {
            SlackAttachmentRenderKind::WebsitePreview => SlackShellIcon::Search,
            SlackAttachmentRenderKind::SharedMessage => SlackShellIcon::MessageFilled,
            _ => SlackShellIcon::Files,
        };
        self.render_slack_compact_attachment_shell(
            attachment_row,
            SlackCompactAttachmentShellConfig {
                icon,
                eyebrow,
                detail,
                trailing_action: None,
            },
            cx,
        )
    }

    pub(super) fn render_slack_compact_attachment_shell(
        &self,
        attachment_row: &SlackAttachmentRow,
        shell: SlackCompactAttachmentShellConfig,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let attachment = &attachment_row.attachment;
        let SlackCompactAttachmentShellConfig {
            icon,
            eyebrow,
            detail,
            trailing_action,
        } = shell;
        let expands_locally = attachment_row.kind == SlackAttachmentRenderKind::Recording;
        let is_expanded = expands_locally
            && self
                .slack_expanded_attachment
                .as_ref()
                .is_some_and(|selection| selection.attachment_id == attachment_row.attachment_id);
        let interaction = SlackCompactAttachmentInteraction {
            expands_locally,
            link_url: attachment.link_url.clone(),
            selection: SlackAttachmentSelection::from_row(attachment_row),
        };
        let palette = slack_palette(self.appearance_mode);
        let shell = slack_compact_attachment_base_shell(
            attachment_row.attachment_id.as_ref(),
            is_expanded,
            palette,
        );
        self.bind_slack_compact_attachment_interaction(
            shell,
            SlackCompactAttachmentBinding {
                attachment,
                interaction,
                palette,
            },
            cx,
        )
        .child(self.render_slack_compact_attachment_leading(attachment_row, icon, cx))
        .child(self.render_slack_compact_attachment_copy(attachment, eyebrow, detail))
        .when_some(trailing_action, |this, action| this.child(action))
    }

    fn bind_slack_compact_attachment_interaction(
        &self,
        shell: Stateful<Div>,
        binding: SlackCompactAttachmentBinding<'_>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let SlackCompactAttachmentBinding {
            attachment,
            interaction,
            palette,
        } = binding;
        let interaction_enabled = interaction.expands_locally || !interaction.link_url.is_empty();
        let expands_locally = interaction.expands_locally;
        let link_url = interaction.link_url;
        let selection = interaction.selection;
        let keyboard_link_url = link_url.clone();
        let keyboard_selection = selection.clone();
        shell.when(interaction_enabled, |this| {
            this.role(Role::Button)
                .aria_label(format!("Open preview for {}", attachment.title))
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .hover(move |style| style.bg(rgb(palette.composer_chip_bg)))
                .focus_visible(move |style| style.border_color(rgb(palette.link)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        if expands_locally {
                            this.toggle_slack_attachment_expanded(&selection, cx);
                        } else {
                            this.open_slack_link(&link_url, cx);
                        }
                    }),
                )
                .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.modifiers.modified()
                        || !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    {
                        return;
                    }
                    window.prevent_default();
                    cx.stop_propagation();
                    if expands_locally {
                        this.toggle_slack_attachment_expanded(&keyboard_selection, cx);
                    } else {
                        this.open_slack_link(&keyboard_link_url, cx);
                    }
                }))
        })
    }

    fn render_slack_compact_attachment_leading(
        &self,
        attachment_row: &SlackAttachmentRow,
        icon: SlackShellIcon,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        if let Some(image) =
            self.slack_attachment_preview_image_by_key(attachment_row.preview_cache_key.as_deref())
        {
            return div()
                .w(px(56.0))
                .h(px(56.0))
                .rounded(px(8.0))
                .overflow_hidden()
                .child(img(image).w_full().h_full().rounded(px(8.0)))
                .into_any_element();
        }
        div()
            .w(px(56.0))
            .h(px(56.0))
            .rounded(px(8.0))
            .bg(rgb(palette.composer_chip_bg))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(icon, palette.composer_icon, 18.0, cx))
            .into_any_element()
    }

    fn render_slack_compact_attachment_copy(
        &self,
        attachment: &SlackAttachment,
        eyebrow: String,
        detail: Option<String>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(palette.attachment_muted_text))
                    .child(eyebrow),
            )
            .child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(palette.attachment_text))
                    .child(attachment.title.clone()),
            )
            .when_some(detail, |this, detail| {
                this.child(
                    div()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_size(px(12.0))
                        .text_color(rgb(palette.attachment_muted_text))
                        .child(detail),
                )
            })
    }
}
