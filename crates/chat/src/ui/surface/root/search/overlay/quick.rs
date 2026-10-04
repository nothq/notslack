use gpui::{uniform_list, Div, Role};

use super::super::super::super::{
    div, img, px, rgb, slack_base_icon_radius, slack_icon, AnyElement, Context, FluentBuilder,
    FontWeight, InteractiveElement, IntoElement, ListSizingBehavior, ParentElement,
    SlackQuickSearchRow, SlackQuickSearchRowKind, SlackShellIcon, StatefulInteractiveElement,
    Styled, SurfaceState,
};
use super::super::{consume_slack_search_action_key, slack_search_overlay_palette};
use super::{
    SlackSearchOptionPosition, SLACK_QUICK_SEARCH_MAX_VISIBLE_ROWS, SLACK_QUICK_SEARCH_ROW_HEIGHT,
};

impl SurfaceState {
    pub(super) fn render_slack_quick_search_results(&self, cx: &mut Context<Self>) -> AnyElement {
        let height = slack_quick_search_content_height(self);
        if self.slack_quick_search_rows.is_empty() {
            return self.render_empty_slack_quick_search_results(height);
        }

        let rows = self.slack_quick_search_rows.clone();
        let row_count = rows.len();
        let option_start = self.slack_search_action_count();
        let option_count = self.slack_search_option_count();
        let selected_option = self.slack_search_selected_option;
        let scroll_handle = self.slack_quick_search_scroll_handle.clone();
        let view = cx.entity();
        div()
            .h(px(height))
            .flex_none()
            .overflow_hidden()
            .child(
                uniform_list(
                    "slack-quick-search-results",
                    row_count,
                    move |range, _window, cx| {
                        let visible_start = range.start;
                        let visible_end = range.end;
                        let rows = rows.clone();
                        view.update(cx, move |this, cx| {
                            this.queue_slack_quick_search_visible_images(
                                visible_start,
                                visible_end,
                                cx,
                            );
                            range
                                .map(|row_index| {
                                    let row = rows
                                        .get(row_index)
                                        .expect("prepared Slack quick-search row must exist");
                                    this.render_slack_quick_search_row(
                                        row,
                                        row_index,
                                        SlackSearchOptionPosition {
                                            option_index: option_start + row_index,
                                            option_count,
                                            selected: selected_option == option_start + row_index,
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
                .size_full(),
            )
            .into_any_element()
    }

    fn render_empty_slack_quick_search_results(&self, height: f32) -> AnyElement {
        let label = if self.slack_quick_search_loading {
            "Searching people and places…"
        } else if self.slack_quick_search_error.is_some() {
            "People and places are unavailable"
        } else {
            "No people or places found"
        };
        div()
            .h(px(height))
            .flex_none()
            .px(px(16.0))
            .flex()
            .items_center()
            .text_size(px(13.0))
            .text_color(rgb(
                slack_search_overlay_palette(self.appearance_mode).secondary_text
            ))
            .child(label)
            .into_any_element()
    }

    fn render_slack_quick_search_row(
        &self,
        row: &SlackQuickSearchRow,
        row_index: usize,
        option: SlackSearchOptionPosition,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let SlackSearchOptionPosition {
            option_index,
            option_count,
            selected,
        } = option;
        let palette = slack_search_overlay_palette(self.appearance_mode);
        div()
            .id(row.element_id.clone())
            .role(Role::ListBoxOption)
            .aria_label(row.accessibility_label.clone())
            .aria_selected(selected)
            .aria_position_in_set(option_index + 1)
            .aria_size_of_set(option_count)
            .focusable()
            .tab_stop(selected)
            .w_full()
            .h(px(SLACK_QUICK_SEARCH_ROW_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .when(selected, |this| this.bg(rgb(palette.selected_bg)))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(palette.hover_bg)))
            .focus_visible(move |style| style.bg(rgb(palette.hover_bg)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.select_slack_search_option(option_index, cx);
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_quick_search_row(row_index, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if consume_slack_search_action_key(event, window, cx) {
                    this.activate_slack_quick_search_row(row_index, cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(self.render_slack_quick_search_icon(row, cx))
            .child(self.render_slack_quick_search_row_text(row))
    }

    fn render_slack_quick_search_row_text(&self, row: &SlackQuickSearchRow) -> Div {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        div()
            .min_w(px(0.0))
            .flex_grow(1.0)
            .flex()
            .items_baseline()
            .gap(px(8.0))
            .overflow_hidden()
            .child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(palette.strong_text))
                    .child(row.label.clone()),
            )
            .child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(13.0))
                    .text_color(rgb(palette.secondary_text))
                    .child(row.detail.clone()),
            )
    }

    fn render_slack_quick_search_icon(
        &self,
        row: &SlackQuickSearchRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        match row.kind {
            SlackQuickSearchRowKind::Channel => {
                slack_quick_search_glyph(SlackShellIcon::HashSmall, palette.text, cx)
            }
            SlackQuickSearchRowKind::PrivateChannel => {
                slack_quick_search_glyph(SlackShellIcon::LockSmall, palette.text, cx)
            }
            SlackQuickSearchRowKind::GroupMessage => {
                slack_quick_search_glyph(SlackShellIcon::People, palette.text, cx)
            }
            SlackQuickSearchRowKind::DirectMessage | SlackQuickSearchRowKind::Person => {
                if let Some(image) = row
                    .avatar_image_url
                    .as_deref()
                    .and_then(|url| self.slack_remote_images.get(url).cloned())
                {
                    return img(image)
                        .size(px(28.0))
                        .rounded(slack_base_icon_radius(28.0))
                        .into_any_element();
                }
                div()
                    .size(px(28.0))
                    .rounded(slack_base_icon_radius(28.0))
                    .bg(rgb(row.avatar_fill))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(10.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child(row.avatar_initials.clone())
                    .into_any_element()
            }
        }
    }

    pub(crate) fn slack_search_action_count(&self) -> usize {
        1 + usize::from(self.slack_search_has_current_conversation_action())
    }
}

pub(super) fn slack_quick_search_content_height(state: &SurfaceState) -> f32 {
    if state.slack_quick_search_rows.is_empty() && !state.slack_quick_search_message_rows.is_empty()
    {
        return 0.0;
    }
    let row_count = if state.slack_quick_search_rows.is_empty() {
        1
    } else {
        state
            .slack_quick_search_rows
            .len()
            .min(SLACK_QUICK_SEARCH_MAX_VISIBLE_ROWS)
    };
    SLACK_QUICK_SEARCH_ROW_HEIGHT * row_count as f32
}

fn slack_quick_search_glyph(
    icon: SlackShellIcon,
    color: u32,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    div()
        .size(px(28.0))
        .flex()
        .items_center()
        .justify_center()
        .child(slack_icon(icon, color, 18.0, cx))
        .into_any_element()
}
