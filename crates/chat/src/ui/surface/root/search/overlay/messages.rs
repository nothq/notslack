use gpui::{HighlightStyle, Role, StyledText, TextStyle};

use super::super::super::super::{
    alpha, div, img, px, rgb, slack_base_icon_radius, slack_icon, AnyElement, Context,
    FluentBuilder, FontWeight, InteractiveElement, IntoElement, ParentElement,
    SlackQuickSearchMessageRow, SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};
use super::super::{consume_slack_search_action_key, slack_search_overlay_palette};
use super::{
    SlackSearchOptionPosition, SLACK_QUICK_SEARCH_MESSAGE_BOTTOM_SPACE,
    SLACK_QUICK_SEARCH_MESSAGE_DIVIDER_HEIGHT, SLACK_QUICK_SEARCH_MESSAGE_HEADING_HEIGHT,
    SLACK_QUICK_SEARCH_MESSAGE_ROW_HEIGHT,
};
use crate::ui::{AppearanceMode, SlackConversationKind};

impl SurfaceState {
    pub(super) fn render_slack_quick_search_message_results(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if self.slack_quick_search_message_rows.is_empty() {
            return div().into_any_element();
        }
        let rows = self.slack_quick_search_message_rows.clone();
        let option_start = self.slack_search_action_count() + self.slack_quick_search_rows.len();
        let option_count = self.slack_search_option_count();
        div()
            .flex_none()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(SLACK_QUICK_SEARCH_MESSAGE_DIVIDER_HEIGHT))
                    .flex_none()
                    .flex()
                    .items_center()
                    .child(div().w_full().h(px(1.0)).bg(rgb(palette_border(self)))),
            )
            .child(
                div()
                    .h(px(SLACK_QUICK_SEARCH_MESSAGE_HEADING_HEIGHT))
                    .flex_none()
                    .pb(px(4.0))
                    .child(
                        div()
                            .h(px(32.0))
                            .px(px(16.0))
                            .flex()
                            .items_center()
                            .text_size(px(13.0))
                            .text_color(rgb(slack_quick_search_metadata_color(
                                self.appearance_mode,
                            )))
                            .child("Recent messages"),
                    ),
            )
            .children(rows.iter().enumerate().map(|(row_index, row)| {
                let option_index = option_start + row_index;
                self.render_slack_quick_search_message_row(
                    row,
                    row_index,
                    SlackSearchOptionPosition {
                        option_index,
                        option_count,
                        selected: self.slack_search_selected_option == option_index,
                    },
                    cx,
                )
            }))
            .child(
                div()
                    .h(px(SLACK_QUICK_SEARCH_MESSAGE_BOTTOM_SPACE))
                    .flex_none(),
            )
            .into_any_element()
    }

    fn render_slack_quick_search_message_row(
        &self,
        row: &SlackQuickSearchMessageRow,
        row_index: usize,
        option: SlackSearchOptionPosition,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let SlackSearchOptionPosition {
            option_index,
            option_count,
            selected,
        } = option;
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
            .h(px(SLACK_QUICK_SEARCH_MESSAGE_ROW_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .py(px(6.0))
            .cursor_pointer()
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.select_slack_search_option(option_index, cx);
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_quick_search_message(row_index, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if consume_slack_search_action_key(event, window, cx) {
                    this.activate_slack_quick_search_message(row_index, cx);
                }
            }))
            .child(
                div()
                    .size_full()
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(alpha(0x797c81, 0.30))
                    .when(selected, |this| this.bg(rgb(0x1264a3)))
                    .pl(px(12.0))
                    .pr(px(14.0))
                    .py(px(11.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(self.render_slack_quick_search_message_avatar(row))
                    .child(self.render_slack_quick_search_message_content(row, selected, cx)),
            )
    }

    fn render_slack_quick_search_message_avatar(
        &self,
        row: &SlackQuickSearchMessageRow,
    ) -> AnyElement {
        if let Some(image) = row
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return img(image)
                .size(px(36.0))
                .rounded(slack_base_icon_radius(36.0))
                .into_any_element();
        }
        div()
            .size(px(36.0))
            .rounded(slack_base_icon_radius(36.0))
            .bg(rgb(row.avatar_fill))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(row.avatar_initials.clone())
            .into_any_element()
    }

    fn render_slack_quick_search_message_content(
        &self,
        row: &SlackQuickSearchMessageRow,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        let strong_color = if selected {
            0xffffff
        } else {
            palette.strong_text
        };
        let metadata_color = if selected {
            0xffffff
        } else {
            slack_quick_search_metadata_color(self.appearance_mode)
        };
        div()
            .min_w(px(0.0))
            .flex_grow(1.0)
            .flex()
            .flex_col()
            .child(self.render_slack_quick_search_message_header(
                row,
                strong_color,
                metadata_color,
                cx,
            ))
            .child(
                div()
                    .min_w(px(0.0))
                    .h(px(22.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .child(self.styled_slack_quick_search_excerpt(row, selected)),
            )
    }

    fn render_slack_quick_search_message_header(
        &self,
        row: &SlackQuickSearchMessageRow,
        strong_color: u32,
        metadata_color: u32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .min_w(px(0.0))
            .h(px(18.0))
            .flex_none()
            .flex()
            .items_center()
            .overflow_hidden()
            .child(
                div()
                    .flex_none()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::BLACK)
                    .text_color(rgb(strong_color))
                    .child(row.author_label.clone()),
            )
            .child(
                div()
                    .ml(px(6.0))
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(13.0))
                    .text_color(rgb(metadata_color))
                    .when(row.is_thread, |this| this.child("Thread in "))
                    .when_some(
                        slack_quick_search_conversation_icon(
                            row.conversation_kind,
                            metadata_color,
                            cx,
                        ),
                        |this, icon| this.child(icon).child(" "),
                    )
                    .child(row.conversation_label.clone()),
            )
            .child(
                div()
                    .ml_auto()
                    .pl(px(8.0))
                    .flex_none()
                    .whitespace_nowrap()
                    .text_size(px(13.0))
                    .text_color(rgb(metadata_color))
                    .child(row.timestamp_label.clone()),
            )
    }

    fn styled_slack_quick_search_excerpt(
        &self,
        row: &SlackQuickSearchMessageRow,
        selected: bool,
    ) -> StyledText {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        let (text_color, match_color, match_background) = match (selected, self.appearance_mode) {
            (true, _) => (0xffffff, 0xffffff, 0xffffff),
            (false, AppearanceMode::Light) => (palette.text, 0x1d1c1d, 0xffe7a0),
            (false, AppearanceMode::Dark) => (palette.text, 0xdea700, 0xa07300),
        };
        let default_style = TextStyle {
            color: rgb(text_color).into(),
            font_family: "Lato".into(),
            font_size: px(15.0).into(),
            line_height: px(22.0).into(),
            ..Default::default()
        };
        StyledText::new(row.excerpt.clone()).with_default_highlights(
            &default_style,
            row.highlights.iter().cloned().map(|range| {
                (
                    range,
                    HighlightStyle {
                        color: Some(rgb(match_color).into()),
                        background_color: Some(alpha(match_background, 0.18)),
                        ..Default::default()
                    },
                )
            }),
        )
    }
}

fn slack_quick_search_conversation_icon<T: 'static>(
    kind: SlackConversationKind,
    color: u32,
    cx: &mut Context<T>,
) -> Option<AnyElement> {
    let icon = match kind {
        SlackConversationKind::Channel => SlackShellIcon::HashSmall,
        SlackConversationKind::PrivateChannel => SlackShellIcon::LockSmall,
        SlackConversationKind::GroupMessage => SlackShellIcon::People,
        SlackConversationKind::DirectMessage | SlackConversationKind::Unknown => return None,
    };
    Some(slack_icon(icon, color, 12.0, cx))
}

fn slack_quick_search_metadata_color(appearance_mode: AppearanceMode) -> u32 {
    match appearance_mode {
        AppearanceMode::Light => 0x5e5d60,
        AppearanceMode::Dark => 0x9a9b9e,
    }
}

fn palette_border(state: &SurfaceState) -> u32 {
    slack_search_overlay_palette(state.appearance_mode).border
}
