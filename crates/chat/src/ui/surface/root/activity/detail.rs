use super::{activity_action_key, SLACK_ACTIVITY_HEADER_HEIGHT, SLACK_ACTIVITY_TABS_HEIGHT};
use crate::ui::surface::{
    alpha, slack_base_icon_radius, slack_icon, slack_palette, SlackActivityDetailState,
    SlackMessageDocumentPosition, SlackMessageRow, SlackShellIcon, SurfaceState,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, list, px, relative, rgb, AnyElement, Context, Div, FontWeight, InteractiveElement,
    IntoElement, ListSizingBehavior, ParentElement, Role, SharedString, StatefulInteractiveElement,
    Styled,
};
use gpui_components::selectable_text::SelectableTextDocument;

impl SurfaceState {
    pub(super) fn render_slack_activity_detail_pane(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .h_full()
            .flex()
            .flex_col()
            .child(self.render_slack_activity_detail_header(cx))
            .child(self.render_slack_activity_detail_tabs())
            .child(self.render_slack_activity_detail_content(cx))
    }

    fn render_slack_activity_detail_header(&self, cx: &mut Context<Self>) -> Div {
        let has_detail = !matches!(self.slack_activity_detail, SlackActivityDetailState::Empty);
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_ACTIVITY_HEADER_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child(
                        self.slack_activity_detail
                            .channel_label()
                            .unwrap_or("Activity details")
                            .to_string(),
                    ),
            )
            .when(has_detail, |this| {
                this.child(
                    div()
                        .id("slack-activity-detail-close")
                        .role(Role::Button)
                        .aria_label("Close activity conversation")
                        .focusable()
                        .tab_stop(true)
                        .size(px(32.0))
                        .rounded(px(6.0))
                        .cursor_pointer()
                        .hover(move |style| style.bg(alpha(palette.main_secondary_text, 0.06)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.close_slack_activity_detail(cx);
                        }))
                        .on_key_down(cx.listener(|this, event, window, cx| {
                            if activity_action_key(event) {
                                window.prevent_default();
                                cx.stop_propagation();
                                this.close_slack_activity_detail(cx);
                            }
                        }))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(slack_icon(
                            SlackShellIcon::Close,
                            palette.main_secondary_text,
                            20.0,
                            cx,
                        )),
                )
            })
    }

    fn render_slack_activity_detail_tabs(&self) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-activity-detail-tabs")
            .role(Role::TabList)
            .aria_label("Activity conversation views")
            .h(px(SLACK_ACTIVITY_TABS_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .border_b_1()
            .border_color(alpha(palette.main_secondary_text, 0.13))
            .flex()
            .items_end()
            .child(
                div()
                    .id("slack-activity-detail-messages-tab")
                    .role(Role::Tab)
                    .aria_label("Messages")
                    .aria_selected(true)
                    .h_full()
                    .relative()
                    .px(px(4.0))
                    .flex()
                    .items_center()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child("Messages")
                    .child(
                        div()
                            .absolute()
                            .left(px(0.0))
                            .right(px(0.0))
                            .bottom(px(0.0))
                            .h(px(2.0))
                            .bg(rgb(palette.main_text)),
                    ),
            )
    }

    fn render_slack_activity_detail_content(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let content = match &self.slack_activity_detail {
            SlackActivityDetailState::Empty => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(14.0))
                .text_color(rgb(palette.main_muted_text))
                .child("Select an activity item to view its conversation")
                .into_any_element(),
            SlackActivityDetailState::Loading { .. } => self.render_slack_activity_detail_loading(),
            SlackActivityDetailState::Error { message, .. } => {
                self.render_slack_activity_detail_error(message, cx)
            }
            SlackActivityDetailState::Loaded {
                key,
                rows,
                message_timestamp,
                ..
            } => self.render_slack_activity_loaded_content(
                key.clone(),
                rows.clone(),
                message_timestamp.clone(),
                cx,
            ),
        };
        div().flex_grow(1.0).min_h(px(0.0)).child(content)
    }

    fn render_slack_activity_detail_loading(&self) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .size_full()
            .px(px(20.0))
            .pt(px(24.0))
            .children((0..5).map(|index| {
                div()
                    .h(px(if index % 2 == 0 { 74.0 } else { 52.0 }))
                    .flex()
                    .gap(px(9.0))
                    .child(
                        div()
                            .size(px(36.0))
                            .rounded(slack_base_icon_radius(36.0))
                            .bg(alpha(palette.main_secondary_text, 0.15)),
                    )
                    .child(
                        div()
                            .flex_grow(1.0)
                            .pt(px(3.0))
                            .flex()
                            .flex_col()
                            .gap(px(9.0))
                            .child(
                                div()
                                    .w(px(112.0))
                                    .h(px(10.0))
                                    .rounded(px(4.0))
                                    .bg(alpha(palette.main_secondary_text, 0.15)),
                            )
                            .child(
                                div()
                                    .w(relative(if index % 2 == 0 { 0.85 } else { 0.58 }))
                                    .h(px(10.0))
                                    .rounded(px(4.0))
                                    .bg(alpha(palette.main_secondary_text, 0.10)),
                            ),
                    )
            }))
            .into_any_element()
    }

    fn render_slack_activity_detail_error(
        &self,
        message: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .size_full()
            .px(px(28.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .text_size(px(13.0))
            .text_color(rgb(palette.main_text))
            .child(message.to_string())
            .child(
                div()
                    .id("slack-activity-detail-retry")
                    .role(Role::Button)
                    .aria_label("Retry loading activity conversation")
                    .focusable()
                    .tab_stop(true)
                    .h(px(32.0))
                    .px(px(14.0))
                    .rounded(px(8.0))
                    .bg(rgb(palette.link))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.retry_slack_activity_detail(cx);
                    }))
                    .on_key_down(cx.listener(|this, event, window, cx| {
                        if activity_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.retry_slack_activity_detail(cx);
                        }
                    }))
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child("Retry"),
            )
            .into_any_element()
    }

    fn render_slack_activity_detail_list(
        &self,
        rows: std::sync::Arc<[SlackMessageRow]>,
        message_timestamp: crate::ui::SlackMessageTimestamp,
        selection_document_id: SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let local_rows = self.slack_activity_local_delivery_rows.clone();
        let view = cx.entity();
        let row_document_id = selection_document_id.clone();
        let list_state = self.slack_activity_detail_list_state.clone();
        let messages = list(
            self.slack_activity_detail_list_state.clone(),
            move |index, _window, cx| {
                let rows = rows.clone();
                let local_rows = local_rows.clone();
                let message_timestamp = message_timestamp.clone();
                view.update(cx, |this, cx| {
                    let row = rows
                        .get(index)
                        .or_else(|| local_rows.get(index.saturating_sub(rows.len())))
                        .expect("Slack Activity detail list index must match its cached rows");
                    this.render_slack_activity_detail_message_in_document(
                        row,
                        message_timestamp.as_str(),
                        SlackMessageDocumentPosition::row(row_document_id.clone(), index),
                        cx,
                    )
                })
            },
        )
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .size_full();
        SelectableTextDocument::new(
            "slack-activity-detail-selection-document",
            selection_document_id,
            messages,
        )
        .on_autoscroll(move |distance, window, _cx| {
            list_state.scroll_by(distance);
            window.refresh();
        })
        .into_any_element()
    }

    fn render_slack_activity_loaded_content(
        &self,
        detail_key: SharedString,
        rows: std::sync::Arc<[SlackMessageRow]>,
        message_timestamp: crate::ui::SlackMessageTimestamp,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let presentation = self
            .slack_activity_detail
            .composer()
            .map(|composer| composer.presentation.clone());
        let local_rows = self.slack_activity_local_delivery_rows.clone();
        let team_id = self
            .slack_activity_team_id
            .as_deref()
            .expect("Slack Activity detail requires a team id");
        let selection_document_id: SharedString = format!(
            "slack-selection:{team_id}:activity-detail:{detail_key}:{}:{}:{}:{}",
            message_timestamp.as_str(),
            self.slack_activity_detail_generation,
            rows.len(),
            local_rows.len(),
        )
        .into();
        div()
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(div().flex_grow(1.0).min_h(px(0.0)).child(
                self.render_slack_activity_detail_list(
                    rows,
                    message_timestamp,
                    selection_document_id,
                    cx,
                ),
            ))
            .when_some(
                presentation.filter(|_| {
                    self.slack_activity_detail
                        .composer()
                        .is_some_and(|composer| {
                            self.slack_active_main_composer_context.as_ref() == Some(composer)
                        })
                        && self.has_current_slack_send_target()
                }),
                |this, presentation| {
                    this.child(self.render_slack_composer_panel(&presentation, cx))
                },
            )
            .into_any_element()
    }
}
