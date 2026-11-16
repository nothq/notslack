//! A draft attachment's image opened large over the window, like Slack's
//! preview: the file name at the top, the image fitted in the middle.

use gpui::{
    div, img, px, rgb, AnyElement, Context, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ObjectFit, ParentElement, Role, StatefulInteractiveElement, Styled, StyledImage,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use crate::ui::{
    alpha,
    surface::{slack_icon, SlackShellIcon, SurfaceState},
};

impl SurfaceState {
    pub(super) fn render_slack_attachment_preview_layer(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (title, image) = self.slack_attachment_preview.clone()?;
        let backdrop = dismissible_backdrop(
            div().absolute().inset_0().bg(alpha(0x000000, 0.85)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.slack_attachment_preview = None;
                cx.notify();
            }),
            cx,
        );
        Some(
            div()
                .id("slack-attachment-preview")
                .role(Role::Dialog)
                .aria_label(title.clone())
                .absolute()
                .inset_0()
                .occlude()
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        this.slack_attachment_preview = None;
                        cx.notify();
                    }
                }))
                .child(backdrop)
                .child(
                    div()
                        .absolute()
                        .top(px(0.0))
                        .left(px(0.0))
                        .right(px(0.0))
                        .h(px(60.0))
                        .px(px(24.0))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(15.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(0xffffff))
                                .child(title),
                        )
                        .child(
                            div()
                                .id("slack-attachment-preview-close")
                                .role(Role::Button)
                                .aria_label("Close preview")
                                .size(px(36.0))
                                .rounded(px(8.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .hover(|style| style.bg(alpha(0xffffff, 0.10)))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.slack_attachment_preview = None;
                                    cx.notify();
                                }))
                                .child(slack_icon(SlackShellIcon::Close, 0xffffff, 22.0, cx)),
                        ),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(72.0))
                        .bottom(px(40.0))
                        .left(px(60.0))
                        .right(px(60.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(img(image).size_full().object_fit(ObjectFit::Contain)),
                )
                .into_any_element(),
        )
    }
}
