use std::sync::Arc;

use super::{
    build_slack_remote_image_from_parts, div, img, px, rgb, slack_base_icon_radius, Context, Div,
    FluentBuilder, FontWeight, Image, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SlackAuxPanelRow, SlackAuxPanelState, Styled, SurfaceState,
};
use crate::ui::initials;
use crate::ui::slack_avatar_fill;

impl SurfaceState {
    pub(crate) fn render_slack_mention_picker_popover(
        &self,
        panel: &SlackAuxPanelState,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex_grow(1.0)
                    .overflow_hidden()
                    .px(px(4.0))
                    .pt(px(6.0))
                    .pb(px(6.0))
                    .flex()
                    .flex_col()
                    .children(
                        self.slack_visible_mention_picker_rows(panel)
                            .enumerate()
                            .map(|(index, row)| {
                                self.render_slack_mention_picker_row(row, index == 0, cx)
                            }),
                    ),
            )
            .child(self.render_slack_mention_picker_footer())
    }

    fn render_slack_mention_picker_row(
        &self,
        row: &SlackAuxPanelRow,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let base = div()
            .h(px(28.0))
            .rounded(px(6.0))
            .px(px(8.0))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(10.0))
            .bg(rgb(if selected { 0x2b2f34 } else { 0x1f2226 }))
            .when(row.action.is_some(), |this| this.cursor_pointer())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(self.render_slack_mention_picker_avatar(row))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(1.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .text_color(rgb(0xf3f4f6))
                                    .child(row.label.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(rgb(0x8f959c))
                                    .child(self.slack_mention_picker_secondary_text(row)),
                            ),
                    ),
            )
            .when(selected, |this| {
                this.child(self.render_slack_mention_picker_enter_badge())
            });
        let Some(action) = row.action.clone() else {
            return base;
        };
        base.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                this.activate_slack_aux_panel_action(action.clone(), cx);
            }),
        )
    }

    fn render_slack_mention_picker_avatar(&self, row: &SlackAuxPanelRow) -> Div {
        let avatar_image = self.slack_picker_avatar_image(&row.label);
        let avatar_text = self.slack_mention_picker_avatar_text(&row.label);
        div()
            .size(px(18.0))
            .rounded(slack_base_icon_radius(18.0))
            .overflow_hidden()
            .bg(rgb(slack_avatar_fill(&row.label)))
            .flex()
            .items_center()
            .justify_center()
            .child(avatar_image.map_or_else(
                || {
                    div()
                        .text_size(px(8.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(0xffffff))
                        .child(avatar_text)
                        .into_any_element()
                },
                |image| {
                    img(image)
                        .w_full()
                        .h_full()
                        .rounded(slack_base_icon_radius(18.0))
                        .into_any_element()
                },
            ))
    }

    fn slack_mention_picker_avatar_text(&self, label: &str) -> String {
        if label.starts_with('@') {
            return "@".to_string();
        }
        let initials = initials(label);
        if initials.is_empty() {
            label
                .chars()
                .next()
                .map(|character| character.to_string())
                .unwrap_or_default()
        } else {
            initials
        }
    }

    fn slack_mention_picker_secondary_text(&self, row: &SlackAuxPanelRow) -> String {
        row.detail
            .clone()
            .filter(|detail| !detail.contains("Visible") && !detail.contains("Signed in"))
            .unwrap_or_else(|| row.label.clone())
    }

    fn render_slack_mention_picker_enter_badge(&self) -> Div {
        div()
            .h(px(18.0))
            .px(px(6.0))
            .rounded(px(5.0))
            .bg(rgb(0x444a50))
            .text_size(px(10.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xf3f4f6))
            .child("Enter")
    }

    fn render_slack_mention_picker_footer(&self) -> Div {
        div()
            .h(px(32.0))
            .px(px(10.0))
            .border_t_1()
            .border_color(rgb(0x30353a))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(
                div()
                    .size(px(18.0))
                    .rounded_full()
                    .bg(rgb(0x2f3439))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(10.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xd9dde2))
                    .child("@"),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0x8f959c))
                    .child("Mention someone"),
            )
    }

    fn slack_picker_avatar_image(&self, label: &str) -> Option<Arc<Image>> {
        let workspace = self.slack_workspace()?;
        if let Some(image) = workspace
            .mention_suggestions
            .iter()
            .find(|suggestion| suggestion.label == label)
            .and_then(|suggestion| suggestion.avatar_image_url.as_deref())
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return Some(image);
        }
        if let Some(image) = workspace
            .mention_suggestions
            .iter()
            .find(|suggestion| suggestion.label == label)
            .and_then(|suggestion| {
                suggestion
                    .avatar_image_base64
                    .as_deref()
                    .zip(suggestion.avatar_image_mimetype.as_deref())
            })
            .and_then(|(base64, mimetype)| build_slack_remote_image_from_parts(base64, mimetype))
            .map(Arc::new)
        {
            return Some(image);
        }
        if let Some(image) = workspace
            .self_display_name
            .as_deref()
            .filter(|display_name| *display_name == label)
            .and(workspace.self_avatar_image_url.as_deref())
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return Some(image);
        }
        if let Some(image) = workspace
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .find(|item| item.label == label)
            .and_then(|item| item.avatar_image_url.as_deref())
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return Some(image);
        }
        workspace
            .messages
            .iter()
            .find(|message| message.author == label)
            .and_then(|message| message.avatar_image_url.as_deref())
            .and_then(|url| self.slack_remote_images.get(url).cloned())
    }
}
