use gpui::prelude::FluentBuilder;
use gpui::{
    div, img, point, px, rgb, uniform_list, AnyElement, BoxShadow, Context, Div, FontWeight,
    InteractiveElement, IntoElement, ListSizingBehavior, MouseButton, MouseDownEvent,
    ParentElement, Role, StatefulInteractiveElement, Styled,
};

use super::{
    slack_message_forward_action_key, SLACK_MESSAGE_FORWARD_RESULTS_LEFT,
    SLACK_MESSAGE_FORWARD_RESULTS_MAX_HEIGHT, SLACK_MESSAGE_FORWARD_RESULTS_WIDTH,
    SLACK_MESSAGE_FORWARD_RESULT_HEIGHT,
};
use crate::ui::surface::{
    slack_base_icon_radius, SlackNewMessageCandidateKind, SlackNewMessageCandidateRow, SurfaceState,
};
use crate::ui::{alpha, SlackUserPresence};

#[derive(Clone, Copy)]
struct SlackMessageForwardResultContext {
    visible_index: usize,
    selected: bool,
    count: usize,
}

impl SurfaceState {
    pub(super) fn render_slack_message_forward_results(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let modal = self
            .slack_message_forward_modal
            .as_ref()
            .expect("forward results require modal state");
        let result_count = modal.visible_row_indices.len();
        let height = if result_count == 0 {
            56.0
        } else {
            (result_count as f32 * SLACK_MESSAGE_FORWARD_RESULT_HEIGHT + 12.0)
                .min(SLACK_MESSAGE_FORWARD_RESULTS_MAX_HEIGHT)
        };
        div()
            .id("slack-message-forward-results")
            .role(Role::ListBox)
            .aria_label("Forward message destinations")
            .absolute()
            .top(px(114.0))
            .left(px(SLACK_MESSAGE_FORWARD_RESULTS_LEFT))
            .w(px(SLACK_MESSAGE_FORWARD_RESULTS_WIDTH))
            .h(px(height))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(0x565856))
            .bg(rgb(0x1a1d21))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.45),
                offset: point(px(0.0), px(8.0)),
                blur_radius: px(24.0),
                spread_radius: px(-6.0),
                inset: false,
            }])
            .occlude()
            .overflow_hidden()
            .when(result_count == 0, |this| {
                this.child(slack_message_forward_empty(
                    self.slack_message_forward_empty_label(),
                ))
            })
            .when(result_count > 0, |this| {
                this.py(px(6.0))
                    .child(self.render_slack_message_forward_result_list(cx))
            })
            .into_any_element()
    }

    fn slack_message_forward_empty_label(&self) -> String {
        let modal = self
            .slack_message_forward_modal
            .as_ref()
            .expect("forward empty label requires modal state");
        if modal.directory_loading {
            "Loading people and conversations…".to_string()
        } else if let Some(error) = modal.error.as_ref() {
            error.clone()
        } else if modal.normalized_query.is_empty() {
            "No destinations available".to_string()
        } else {
            "No people or conversations match your search".to_string()
        }
    }

    fn render_slack_message_forward_result_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let modal = self
            .slack_message_forward_modal
            .as_ref()
            .expect("forward result list requires modal state");
        let rows = modal.rows.clone();
        let result_indices = modal.visible_row_indices.clone();
        let result_count = result_indices.len();
        let selected_index = modal.selected_index;
        let scroll_handle = modal.scroll_handle.clone();
        let view = cx.entity();
        uniform_list(
            "slack-message-forward-destination-results",
            result_count,
            move |range, _window, cx| {
                let visible_start = range.start;
                let visible_end = range.end;
                let rows = rows.clone();
                let result_indices = result_indices.clone();
                view.update(cx, move |this, cx| {
                    this.queue_slack_message_forward_visible_images(visible_start, visible_end, cx);
                    range
                        .map(|visible_index| {
                            let row_index = result_indices[visible_index];
                            let row = &rows[row_index];
                            this.render_slack_message_forward_result_row(
                                row,
                                SlackMessageForwardResultContext {
                                    visible_index,
                                    selected: selected_index == Some(visible_index),
                                    count: result_count,
                                },
                                cx,
                            )
                        })
                        .collect::<Vec<_>>()
                })
            },
        )
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .track_scroll(&scroll_handle)
        .size_full()
        .into_any_element()
    }

    fn render_slack_message_forward_result_row(
        &self,
        row: &SlackNewMessageCandidateRow,
        context: SlackMessageForwardResultContext,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let background = if context.selected { 0x1264a3 } else { 0x1a1d21 };
        slack_message_forward_result_row_shell(row, context, background)
            .child(self.render_slack_message_forward_candidate_icon(row, background, cx))
            .child(slack_message_forward_result_labels(row, context.selected))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.select_slack_message_forward_destination(context.visible_index, cx);
                }),
            )
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_message_forward_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.select_slack_message_forward_destination(context.visible_index, cx);
                }
            }))
            .into_any_element()
    }

    fn render_slack_message_forward_candidate_icon(
        &self,
        row: &SlackNewMessageCandidateRow,
        row_background: u32,
        cx: &mut Context<Self>,
    ) -> Div {
        match row.kind {
            SlackNewMessageCandidateKind::Channel
            | SlackNewMessageCandidateKind::PrivateChannel
            | SlackNewMessageCandidateKind::GroupMessage => self
                .render_slack_message_forward_destination_icon(row.kind, 0xb9babd, cx)
                .bg(rgb(row_background)),
            SlackNewMessageCandidateKind::DirectMessage | SlackNewMessageCandidateKind::Person => {
                slack_message_forward_avatar(self, row, row_background)
            }
        }
    }
}

fn slack_message_forward_empty(label: String) -> Div {
    div()
        .size_full()
        .px(px(16.0))
        .flex()
        .items_center()
        .text_size(px(14.0))
        .line_height(px(20.0))
        .text_color(rgb(0x9a9b9e))
        .child(label)
}

fn slack_message_forward_result_row_shell(
    row: &SlackNewMessageCandidateRow,
    context: SlackMessageForwardResultContext,
    background: u32,
) -> gpui::Stateful<Div> {
    div()
        .id(format!(
            "slack-message-forward-destination-{}",
            row.target.stable_id()
        ))
        .role(Role::ListBoxOption)
        .aria_label(row.accessibility_label.clone())
        .aria_selected(context.selected)
        .aria_position_in_set(context.visible_index + 1)
        .aria_size_of_set(context.count)
        .focusable()
        .tab_stop(context.selected)
        .w_full()
        .h(px(SLACK_MESSAGE_FORWARD_RESULT_HEIGHT))
        .flex_none()
        .px(px(14.0))
        .bg(rgb(background))
        .cursor_pointer()
        .flex()
        .items_center()
        .gap(px(10.0))
        .hover(|style| style.bg(rgb(0x27292d)))
}

fn slack_message_forward_result_labels(row: &SlackNewMessageCandidateRow, selected: bool) -> Div {
    div()
        .flex_grow(1.0)
        .min_w(px(0.0))
        .flex()
        .items_baseline()
        .gap(px(7.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .child(
            div()
                .min_w(px(0.0))
                .overflow_hidden()
                .text_ellipsis()
                .text_size(px(15.0))
                .line_height(px(20.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xf8f8f8))
                .child(row.label.clone()),
        )
        .when_some(row.secondary_label.clone(), |this, secondary_label| {
            this.child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_size(px(14.0))
                    .line_height(px(20.0))
                    .text_color(rgb(if selected { 0xffffff } else { 0xb9babd }))
                    .child(secondary_label),
            )
        })
}

fn slack_message_forward_avatar(
    surface: &SurfaceState,
    row: &SlackNewMessageCandidateRow,
    row_background: u32,
) -> Div {
    let avatar = if let Some(image) = row
        .avatar_image_url
        .as_deref()
        .and_then(|url| surface.slack_remote_images.get(url).cloned())
    {
        div()
            .size(px(24.0))
            .rounded(slack_base_icon_radius(24.0))
            .overflow_hidden()
            .child(img(image).size_full().rounded(slack_base_icon_radius(24.0)))
    } else {
        div()
            .size(px(24.0))
            .rounded(slack_base_icon_radius(24.0))
            .bg(rgb(row.avatar_fill))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(9.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(row.avatar_initials.clone())
    };
    avatar.relative().when_some(row.presence, |this, presence| {
        this.child(slack_message_forward_presence_badge(
            presence,
            row_background,
        ))
    })
}

fn slack_message_forward_presence_badge(presence: SlackUserPresence, row_background: u32) -> Div {
    let (fill, border, size) = match presence {
        SlackUserPresence::Active => (0x2bac76, row_background, 8.0),
        SlackUserPresence::Away => (row_background, 0x9a9b9e, 8.0),
    };
    div()
        .absolute()
        .right(px(-1.0))
        .bottom(px(-1.0))
        .size(px(size))
        .rounded_full()
        .border_2()
        .border_color(rgb(border))
        .bg(rgb(fill))
}
