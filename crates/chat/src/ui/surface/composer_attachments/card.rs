use std::sync::Arc;

use gpui::{Image, ObjectFit, Role, StyledImage};

use super::status::SlackComposerAttachmentStatusContext;
use super::SlackComposerAttachmentOwner;
use crate::ui::surface::{
    alpha, div, img, point, px, rgb, slack_icon, slack_palette, slack_spinner, AnyElement,
    BoxShadow, Context, Div, FluentBuilder, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, SlackComposerAttachmentKind, SlackComposerAttachmentPresentation,
    SlackComposerAttachmentPreview, SlackComposerAttachmentStatus, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState,
};

struct SlackComposerAttachmentVisuals {
    image: Option<Arc<Image>>,
    preview_pending: bool,
    show_fallback: bool,
    has_visual_preview: bool,
}

impl SurfaceState {
    pub(super) fn render_slack_composer_attachment_card(
        &self,
        presentation: SlackComposerAttachmentPresentation,
        owner: SlackComposerAttachmentOwner,
        context_id: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let file_id = presentation.file_id;
        let card_hover_group = format!("slack-composer-attachment-card-{context_id}-{file_id}");
        let visuals = self.slack_composer_attachment_visuals(&presentation);
        let has_visual_preview = visuals.has_visual_preview;
        let keyboard_owner = owner.clone();
        div()
            .id(format!(
                "slack-composer-attachment-card-{context_id}-{file_id}"
            ))
            .group(card_hover_group.clone())
            .role(Role::Group)
            .aria_label(slack_composer_attachment_accessibility_label(&presentation))
            .focusable()
            .tab_stop(true)
            .relative()
            .size(px(62.0))
            .flex_none()
            .rounded(px(12.0))
            .border_1()
            .border_color(alpha(palette.main_text, 0.13))
            .bg(if has_visual_preview {
                alpha(palette.main_text, 0.04)
            } else {
                alpha(palette.main_bg, 1.0)
            })
            .focus_visible(move |style| style.border_2().border_color(rgb(palette.link)))
            .hover(move |style| {
                style.shadow(slack_composer_attachment_card_shadow(has_visual_preview))
            })
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !slack_composer_attachment_remove_key(event) {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.remove_slack_composer_attachment(&keyboard_owner, file_id, cx);
            }))
            .child(self.render_slack_composer_attachment_preview(&presentation, visuals, cx))
            .child(self.render_slack_composer_attachment_status_button(
                &presentation,
                SlackComposerAttachmentStatusContext::new(owner, context_id, &card_hover_group),
                cx,
            ))
            .into_any_element()
    }

    fn slack_composer_attachment_visuals(
        &self,
        presentation: &SlackComposerAttachmentPresentation,
    ) -> SlackComposerAttachmentVisuals {
        let image = match &presentation.preview {
            Some(SlackComposerAttachmentPreview::Local(image)) => Some(image.clone()),
            Some(SlackComposerAttachmentPreview::Remote(url)) => {
                self.slack_remote_images.get(url.as_ref()).cloned()
            }
            None => None,
        };
        let preview_pending = match &presentation.preview {
            Some(SlackComposerAttachmentPreview::Remote(url)) if image.is_none() => {
                self.slack_active_remote_image_urls.contains(url.as_ref())
                    || self
                        .slack_pending_remote_image_urls
                        .iter()
                        .any(|pending| pending == url.as_ref())
            }
            Some(SlackComposerAttachmentPreview::Local(_))
            | Some(SlackComposerAttachmentPreview::Remote(_))
            | None => false,
        };
        SlackComposerAttachmentVisuals {
            show_fallback: image.is_none() && !preview_pending,
            has_visual_preview: presentation.preview.is_some(),
            image,
            preview_pending,
        }
    }

    fn render_slack_composer_attachment_preview(
        &self,
        presentation: &SlackComposerAttachmentPresentation,
        visuals: SlackComposerAttachmentVisuals,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .size_full()
            .rounded(px(12.0))
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .when_some(visuals.image, |this, image| {
                this.child(img(image).size_full().object_fit(ObjectFit::Cover))
            })
            .when(visuals.show_fallback, |this| {
                this.child(slack_composer_attachment_fallback(
                    presentation.kind,
                    palette.main_text,
                    cx,
                ))
            })
            .when(visuals.preview_pending, |this| {
                this.child(slack_spinner(
                    &self.slack_spinner_frame_cache,
                    format!(
                        "slack-composer-attachment-preview-spinner-{}",
                        presentation.file_id
                    ),
                    palette.main_secondary_text,
                    24.0,
                    cx,
                ))
            })
            .when(
                presentation.preview.is_some()
                    && !matches!(presentation.status, SlackComposerAttachmentStatus::Ready),
                |this| this.bg(alpha(palette.main_text, 0.08)),
            )
    }
}

fn slack_composer_attachment_card_shadow(has_visual_preview: bool) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: alpha(0x000000, if has_visual_preview { 0.06 } else { 0.05 }),
        offset: point(px(0.0), px(1.0)),
        blur_radius: px(if has_visual_preview { 1.0 } else { 2.0 }),
        spread_radius: px(0.0),
        inset: false,
    }]
}

fn slack_composer_attachment_accessibility_label(
    presentation: &SlackComposerAttachmentPresentation,
) -> String {
    let status = match presentation.status {
        SlackComposerAttachmentStatus::Loading => "uploading",
        SlackComposerAttachmentStatus::Ready => "ready",
        SlackComposerAttachmentStatus::Error => "upload failed",
    };
    format!("{}, {status}", presentation.title)
}

fn slack_composer_attachment_fallback(
    kind: SlackComposerAttachmentKind,
    foreground: u32,
    cx: &mut Context<SurfaceState>,
) -> Div {
    let (background, icon, label) = match kind {
        SlackComposerAttachmentKind::Image => (0x2eb67d, Some(SlackShellIcon::Attach), None),
        SlackComposerAttachmentKind::Video => (0x1264a3, Some(SlackShellIcon::Video), None),
        SlackComposerAttachmentKind::Audio => (0x611f69, Some(SlackShellIcon::Mic), None),
        SlackComposerAttachmentKind::Pdf => (0xe01e5a, None, Some("PDF")),
        SlackComposerAttachmentKind::Text => (0x1264a3, None, Some("TXT")),
        SlackComposerAttachmentKind::File => (foreground, Some(SlackShellIcon::Attach), None),
    };
    div()
        .w(px(36.0))
        .h(px(42.0))
        .rounded(px(5.0))
        .bg(rgb(background))
        .flex()
        .items_center()
        .justify_center()
        .when_some(icon, |this, icon| {
            this.child(slack_icon(icon, 0xffffff, 20.0, cx))
        })
        .when_some(label, |this, label| {
            this.text_size(px(9.0))
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child(label)
        })
}

fn slack_composer_attachment_remove_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "backspace" | "delete")
}
