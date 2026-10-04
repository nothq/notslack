mod preview;

use crate::ui::surface::{
    slack_icon, slack_palette, SlackAttachmentRow, SlackShellIcon, SurfaceState,
};
use crate::ui::{
    alpha, div, px, rgb, AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, StatefulInteractiveElement, Styled,
};
use gpui::{Role, Stateful};

impl SurfaceState {
    fn render_slack_attachment_card_header(
        &self,
        attachment_row: &SlackAttachmentRow,
        collapsible_preview: bool,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let mut lines = Vec::<AnyElement>::new();
        if attachment_row.shows_title {
            lines.push(if collapsible_preview {
                self.render_slack_collapsible_attachment_header(attachment_row, collapsed, cx)
            } else {
                self.render_slack_link_attachment_header(attachment_row, cx)
            });
        }
        if !attachment_row.attachment.description.is_empty() {
            lines.push(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(
                        slack_palette(self.appearance_mode).attachment_muted_text
                    ))
                    .child(attachment_row.attachment.description.clone())
                    .into_any_element(),
            );
        }
        (!lines.is_empty()).then(|| div().flex().flex_col().gap(px(3.0)).children(lines))
    }

    fn render_slack_collapsible_attachment_header(
        &self,
        attachment_row: &SlackAttachmentRow,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let color = slack_palette(self.appearance_mode).attachment_muted_text;
        div()
            .w(px(936.0))
            .max_w_full()
            .min_w(px(0.0))
            .h(px(22.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .child(slack_attachment_header_title(
                attachment_row.title.clone(),
                color,
            ))
            .child(self.render_slack_attachment_collapse_button(
                attachment_row,
                collapsed,
                color,
                cx,
            ))
            .into_any_element()
    }

    fn render_slack_attachment_collapse_button(
        &self,
        attachment_row: &SlackAttachmentRow,
        collapsed: bool,
        color: u32,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let clicked_attachment_id = attachment_row.attachment_id.clone();
        let keyboard_attachment_id = clicked_attachment_id.clone();
        div()
            .id(format!(
                "slack-attachment-collapse-{}",
                attachment_row.attachment_id
            ))
            .role(Role::Button)
            .aria_label("Toggle file")
            .aria_expanded(!collapsed)
            .focusable()
            .tab_stop(true)
            .size(px(20.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.toggle_slack_attachment_collapsed(clicked_attachment_id.as_ref(), cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.toggle_slack_attachment_collapsed(keyboard_attachment_id.as_ref(), cx);
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

    fn render_slack_link_attachment_header(
        &self,
        attachment_row: &SlackAttachmentRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let attachment = &attachment_row.attachment;
        let color = slack_palette(self.appearance_mode).attachment_muted_text;
        let link_url = attachment.link_url.clone();
        let keyboard_link_url = link_url.clone();
        let row = div()
            .h(px(22.0))
            .flex()
            .items_center()
            .gap(px(4.0))
            .text_size(px(13.0))
            .text_color(rgb(color))
            .child(attachment.title.clone())
            .child(slack_icon(SlackShellIcon::ChevronDown, color, 12.0, cx));
        if link_url.is_empty() {
            return row.into_any_element();
        }
        row.id(format!("slack-attachment-header-{}", attachment.link_url))
            .role(Role::Link)
            .aria_label(format!("Open {}", attachment.title))
            .focusable()
            .tab_stop(true)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_slack_link(&link_url, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.open_slack_link(&keyboard_link_url, cx);
            }))
            .into_any_element()
    }
}

fn slack_attachment_header_title(title: gpui::SharedString, color: u32) -> Div {
    div()
        .min_w(px(0.0))
        .flex_shrink(1.0)
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(13.0))
        .line_height(px(19.0668))
        .font_weight(FontWeight::NORMAL)
        .text_color(rgb(color))
        .child(title)
}

fn slack_attachment_without_preview(
    attachment_row: &SlackAttachmentRow,
    appearance_mode: crate::ui::AppearanceMode,
) -> Div {
    let palette = slack_palette(appearance_mode);
    div()
        .rounded(px(10.0))
        .border_l_1()
        .border_color(alpha(0x36c5f0, 0.92))
        .bg(rgb(palette.attachment_bg))
        .px(px(12.0))
        .py(px(10.0))
        .text_size(px(13.0))
        .text_color(rgb(palette.attachment_text))
        .child(attachment_row.title.clone())
}
