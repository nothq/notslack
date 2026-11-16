use std::sync::Arc;

use super::{
    build_slack_remote_image_from_parts, div, img, px, rgb, slack_base_icon_radius, Context, Div,
    FluentBuilder, FontWeight, Image, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SlackAuxPanelRow, SlackAuxPanelState, Styled, SurfaceState,
};
use crate::ui::initials;

const SLACK_MENTION_PICKER_ROW_HEIGHT: f32 = 36.0;
const SLACK_MENTION_PICKER_PADDING: f32 = 6.0;
use crate::ui::slack_avatar_fill;

impl SurfaceState {
    pub(crate) fn render_slack_mention_picker_popover(
        &self,
        panel: &SlackAuxPanelState,
        cx: &mut Context<Self>,
    ) -> Div {
        let selected = self.slack_aux_panel_selected_index();
        let mut pickable = 0;
        div()
            .h_full()
            .py(px(SLACK_MENTION_PICKER_PADDING))
            .flex()
            .flex_col()
            .children(self.slack_visible_mention_picker_rows(panel).map(|row| {
                let is_selected = row.action.is_some() && pickable == selected;
                if row.action.is_some() {
                    pickable += 1;
                }
                self.render_slack_mention_picker_row(row, is_selected, cx)
            }))
    }

    /// The popover fits its rows, as Slack's does.
    pub(crate) fn slack_mention_picker_height(&self, panel: &SlackAuxPanelState) -> f32 {
        let rows = self.slack_visible_mention_picker_rows(panel).count().max(1);
        rows as f32 * SLACK_MENTION_PICKER_ROW_HEIGHT + SLACK_MENTION_PICKER_PADDING * 2.0 + 2.0
    }

    /// One line per person, like Slack's: avatar, bold name, full name, and
    /// "Enter" on the selected row.
    fn render_slack_mention_picker_row(
        &self,
        row: &SlackAuxPanelRow,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let secondary = self.slack_mention_picker_secondary_text(row);
        let base = div()
            .h(px(SLACK_MENTION_PICKER_ROW_HEIGHT))
            .flex_none()
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .when(selected, |this| this.bg(rgb(0x2c3036)))
            .when(row.action.is_some(), |this| {
                this.cursor_pointer().hover(|style| style.bg(rgb(0x2c3036)))
            })
            .child(self.render_slack_mention_picker_avatar(row))
            .child(
                div()
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(15.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xd1d2d3))
                            .child(row.label.clone()),
                    )
                    .when(secondary != row.label, |this| {
                        this.child(
                            div()
                                .min_w(px(0.0))
                                .overflow_hidden()
                                .text_ellipsis()
                                .text_size(px(15.0))
                                .text_color(rgb(0xababad))
                                .child(secondary),
                        )
                    }),
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
            .size(px(22.0))
            .flex_none()
            .rounded(slack_base_icon_radius(22.0))
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
                        .rounded(slack_base_icon_radius(22.0))
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
            .flex_none()
            .h(px(22.0))
            .px(px(7.0))
            .flex()
            .items_center()
            .rounded(px(5.0))
            .bg(rgb(0x3b3f45))
            .text_size(px(13.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(0xf3f4f6))
            .child("Enter")
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
