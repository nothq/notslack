use gpui::prelude::FluentBuilder;
use gpui::{
    div, img, point, px, rgb, uniform_list, AnyElement, BoxShadow, Context, Div, FontWeight,
    InteractiveElement, IntoElement, ListSizingBehavior, MouseButton, MouseDownEvent,
    ParentElement, Role, StatefulInteractiveElement, Styled,
};

use super::{slack_new_message_action_key, SLACK_NEW_MESSAGE_HEADER_HEIGHT};
use crate::ui::surface::{
    slack_base_icon_radius, slack_icon, slack_palette, SlackNewMessageCandidateKind,
    SlackNewMessageCandidateRow, SlackShellIcon, SurfaceState,
};
use crate::ui::{alpha, SlackUserPresence};

const SLACK_NEW_MESSAGE_RESULT_ROW_HEIGHT: f32 = 32.0;
const SLACK_NEW_MESSAGE_RESULTS_MAX_HEIGHT: f32 = 264.0;
const SLACK_NEW_MESSAGE_RESULTS_VERTICAL_INSET: f32 = 12.0;

#[derive(Clone, Copy)]
struct SlackNewMessageRowContext {
    visible_index: usize,
    selected: bool,
    count: usize,
}

impl SurfaceState {
    pub(super) fn slack_new_message_suggestions_visible(&self) -> bool {
        self.slack_new_message_to_focused
            && (self.slack_new_message_destination.is_none()
                || !self.slack_new_message_selected_people.is_empty()
                || !self.slack_new_message_query.is_empty())
    }

    pub(super) fn render_slack_new_message_suggestions(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let result_count = self.slack_new_message_visible_row_indices.len();
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-new-message-suggestions")
            .role(Role::ListBox)
            .aria_label("New message destinations")
            .absolute()
            .top(px(SLACK_NEW_MESSAGE_HEADER_HEIGHT + 40.0))
            .left(px(27.0))
            .right(px(10.0))
            .h(px(slack_new_message_suggestions_height(result_count)))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.composer_chip_border))
            .bg(rgb(palette.main_bg))
            .shadow(slack_new_message_suggestions_shadow())
            .occlude()
            .overflow_hidden()
            .when(result_count > 0, |this| {
                this.py(px(SLACK_NEW_MESSAGE_RESULTS_VERTICAL_INSET))
            })
            .when(result_count == 0, |this| {
                this.child(slack_new_message_empty_results(
                    self.slack_new_message_empty_label(),
                    palette.main_muted_text,
                ))
            })
            .when(result_count > 0, |this| {
                this.child(self.render_slack_new_message_suggestion_list(cx))
            })
            .into_any_element()
    }

    fn slack_new_message_empty_label(&self) -> String {
        if self.slack_new_message_directory_loading {
            "Loading people and conversations…".to_string()
        } else if let Some(error) = self.slack_new_message_error.as_ref() {
            error.clone()
        } else if self.slack_new_message_normalized_query.is_empty() {
            "No destinations available".to_string()
        } else {
            "No people or conversations match your search".to_string()
        }
    }

    fn render_slack_new_message_suggestion_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.slack_new_message_rows.clone();
        let result_indices = self.slack_new_message_visible_row_indices.clone();
        let result_count = result_indices.len();
        let selected_index = self.slack_new_message_selected_index;
        let scroll_handle = self.slack_new_message_scroll_handle.clone();
        let view = cx.entity();
        uniform_list(
            "slack-new-message-destination-results",
            result_count,
            move |range, _window, cx| {
                let visible_start = range.start;
                let visible_end = range.end;
                let rows = rows.clone();
                let result_indices = result_indices.clone();
                view.update(cx, move |this, cx| {
                    this.queue_slack_new_message_visible_images(visible_start, visible_end, cx);
                    range
                        .map(|visible_index| {
                            let row_index = result_indices[visible_index];
                            this.render_slack_new_message_suggestion_row(
                                &rows[row_index],
                                SlackNewMessageRowContext {
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

    fn render_slack_new_message_suggestion_row(
        &self,
        row: &SlackNewMessageCandidateRow,
        context: SlackNewMessageRowContext,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let background = palette.main_bg;
        div()
            .id(row.element_id.clone())
            .role(Role::ListBoxOption)
            .aria_label(row.accessibility_label.clone())
            .aria_selected(context.selected)
            .aria_position_in_set(context.visible_index + 1)
            .aria_size_of_set(context.count)
            .focusable()
            .tab_stop(context.selected)
            .w_full()
            .h(px(SLACK_NEW_MESSAGE_RESULT_ROW_HEIGHT))
            .flex_none()
            .px(px(24.0))
            .bg(rgb(background))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(9.0))
            .hover(|style| {
                style.bg(rgb(slack_new_message_hover_background(
                    self.appearance_mode,
                )))
            })
            .child(self.render_slack_new_message_candidate_icon(row, background, cx))
            .child(slack_new_message_candidate_labels(
                row,
                context.selected,
                palette.main_text,
                palette.main_muted_text,
            ))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.select_slack_new_message_candidate(context.visible_index, cx);
                }),
            )
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_new_message_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.select_slack_new_message_candidate(context.visible_index, cx);
                }
            }))
            .into_any_element()
    }

    fn render_slack_new_message_candidate_icon(
        &self,
        row: &SlackNewMessageCandidateRow,
        row_background: u32,
        cx: &mut Context<Self>,
    ) -> Div {
        match row.kind {
            SlackNewMessageCandidateKind::Channel => {
                slack_new_message_candidate_glyph(SlackShellIcon::HashSmall, 0xb9babd, 18.0, cx)
            }
            SlackNewMessageCandidateKind::PrivateChannel => {
                slack_new_message_candidate_glyph(SlackShellIcon::LockSmall, 0xb9babd, 17.0, cx)
            }
            SlackNewMessageCandidateKind::GroupMessage => div()
                .size(px(20.0))
                .rounded(px(5.0))
                .bg(rgb(0xd1d2d3))
                .flex()
                .items_center()
                .justify_center()
                .child(slack_icon(SlackShellIcon::People, 0x1d1c1d, 14.0, cx)),
            SlackNewMessageCandidateKind::DirectMessage | SlackNewMessageCandidateKind::Person => {
                self.render_slack_new_message_candidate_avatar(row)
                    .relative()
                    .when_some(row.presence, |this, presence| {
                        this.child(slack_new_message_presence_badge(presence, row_background))
                    })
            }
        }
    }

    fn render_slack_new_message_candidate_avatar(&self, row: &SlackNewMessageCandidateRow) -> Div {
        if let Some(image) = row
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return div()
                .size(px(20.0))
                .rounded(slack_base_icon_radius(20.0))
                .overflow_hidden()
                .child(img(image).size_full().rounded(slack_base_icon_radius(20.0)));
        }
        div()
            .size(px(20.0))
            .rounded(slack_base_icon_radius(20.0))
            .bg(rgb(row.avatar_fill))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(8.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(row.avatar_initials.clone())
    }
}

fn slack_new_message_suggestions_height(result_count: usize) -> f32 {
    if result_count == 0 {
        return 56.0;
    }
    (result_count as f32 * SLACK_NEW_MESSAGE_RESULT_ROW_HEIGHT
        + SLACK_NEW_MESSAGE_RESULTS_VERTICAL_INSET * 2.0)
        .min(SLACK_NEW_MESSAGE_RESULTS_MAX_HEIGHT)
}

fn slack_new_message_suggestions_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: alpha(0x000000, 0.40),
        offset: point(px(0.0), px(8.0)),
        blur_radius: px(24.0),
        spread_radius: px(-8.0),
        inset: false,
    }]
}

fn slack_new_message_empty_results(label: String, text_color: u32) -> Div {
    div()
        .h_full()
        .px(px(16.0))
        .flex()
        .items_center()
        .text_size(px(15.0))
        .text_color(rgb(text_color))
        .child(label)
}

fn slack_new_message_hover_background(appearance: crate::ui::AppearanceMode) -> u32 {
    match appearance {
        crate::ui::AppearanceMode::Dark => 0x27292d,
        crate::ui::AppearanceMode::Light => 0xf1f2f3,
    }
}

fn slack_new_message_candidate_labels(
    row: &SlackNewMessageCandidateRow,
    selected: bool,
    text_color: u32,
    muted_text_color: u32,
) -> Div {
    let foreground = if selected { 0x1264a3 } else { text_color };
    div()
        .flex_grow(1.0)
        .min_w(px(0.0))
        .flex()
        .items_baseline()
        .gap(px(7.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .child(slack_new_message_candidate_label(
            row.label.clone(),
            foreground,
        ))
        .when_some(row.secondary_label.clone(), |this, secondary_label| {
            this.child(slack_new_message_candidate_secondary_label(
                secondary_label,
                if selected { 0x1264a3 } else { muted_text_color },
            ))
        })
}

fn slack_new_message_candidate_label(label: gpui::SharedString, text_color: u32) -> Div {
    div()
        .min_w(px(0.0))
        .overflow_hidden()
        .text_ellipsis()
        .text_size(px(15.0))
        .line_height(px(20.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(text_color))
        .child(label)
}

fn slack_new_message_candidate_secondary_label(label: gpui::SharedString, text_color: u32) -> Div {
    div()
        .min_w(px(0.0))
        .overflow_hidden()
        .text_ellipsis()
        .text_size(px(13.0))
        .line_height(px(18.0))
        .text_color(rgb(text_color))
        .child(label)
}

fn slack_new_message_candidate_glyph(
    icon: SlackShellIcon,
    color: u32,
    size: f32,
    cx: &mut Context<SurfaceState>,
) -> Div {
    div()
        .size(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .child(slack_icon(icon, color, size, cx))
}

fn slack_new_message_presence_badge(presence: SlackUserPresence, row_background: u32) -> Div {
    let fill = match presence {
        SlackUserPresence::Active => 0x2bac76,
        SlackUserPresence::Away => row_background,
    };
    div()
        .absolute()
        .right(px(-1.0))
        .bottom(px(-1.0))
        .size(px(6.0))
        .rounded_full()
        .border_1()
        .border_color(rgb(row_background))
        .bg(rgb(fill))
}
