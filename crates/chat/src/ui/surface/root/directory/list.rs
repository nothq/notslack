use gpui::prelude::FluentBuilder;
use gpui::{
    div, img, px, rgb, uniform_list, AnyElement, Context, Div, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, ListSizingBehavior, ParentElement, Role, Stateful,
    StatefulInteractiveElement, Styled,
};

use super::{slack_directory_action_key, SLACK_DIRECTORY_ROW_HEIGHT};
use crate::ui::surface::{
    slack_base_icon_radius, slack_palette, SlackNewMessageCandidateRow, SurfaceState,
};
use crate::ui::SlackUserPresence;

/// A directory card's place in the People listbox.
#[derive(Clone, Copy)]
struct SlackDirectoryCardPosition {
    visible_index: usize,
    row_count: usize,
    selected: bool,
}

impl SurfaceState {
    pub(super) fn render_slack_directory_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.slack_directory_rows.clone();
        let visible_indices = self.slack_directory_visible_row_indices.clone();
        let row_count = visible_indices.len();
        let selected_index = self.slack_directory_selected_index;
        let scroll_handle = self.slack_directory_scroll_handle.clone();
        let view = cx.entity();
        div()
            .id("slack-people-listbox")
            .role(Role::ListBox)
            .aria_label("People")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_hidden()
            .child(
                uniform_list("slack-people-rows", row_count, move |range, _window, cx| {
                    let visible_start = range.start;
                    let visible_end = range.end;
                    let rows = rows.clone();
                    let visible_indices = visible_indices.clone();
                    view.update(cx, move |this, cx| {
                        this.queue_slack_directory_visible_images(visible_start, visible_end, cx);
                        range
                            .map(|visible_index| {
                                let row_index = visible_indices[visible_index];
                                this.render_slack_directory_card(
                                    &rows[row_index],
                                    SlackDirectoryCardPosition {
                                        visible_index,
                                        row_count,
                                        selected: selected_index == Some(visible_index),
                                    },
                                    cx,
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .track_scroll(&scroll_handle)
                .size_full(),
            )
            .into_any_element()
    }

    fn render_slack_directory_card(
        &self,
        row: &SlackNewMessageCandidateRow,
        position: SlackDirectoryCardPosition,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let visible_index = position.visible_index;
        let profile_enabled = self.slack_workspace_api_capabilities.load_profile;
        let card = self.render_slack_directory_card_body(row, position, profile_enabled);
        let wrapper = div()
            .h(px(SLACK_DIRECTORY_ROW_HEIGHT))
            .px(px(28.0))
            .py(px(5.0));
        if !profile_enabled {
            return wrapper.child(card).into_any_element();
        }
        wrapper
            .child(
                card.on_click(cx.listener(move |this, _, _, cx| {
                    this.open_slack_directory_person(visible_index, cx);
                }))
                .on_key_down(cx.listener(
                    move |this, event: &KeyDownEvent, window, cx| {
                        if slack_directory_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.open_slack_directory_person(visible_index, cx);
                        }
                    },
                )),
            )
            .into_any_element()
    }

    fn render_slack_directory_card_body(
        &self,
        row: &SlackNewMessageCandidateRow,
        position: SlackDirectoryCardPosition,
        profile_enabled: bool,
    ) -> Stateful<Div> {
        let SlackDirectoryCardPosition {
            visible_index,
            row_count,
            selected,
        } = position;
        let palette = slack_palette(self.appearance_mode);
        div()
            .id(row.element_id.clone())
            .role(Role::ListBoxOption)
            .aria_label(row.accessibility_label.clone())
            .aria_selected(selected)
            .aria_position_in_set(visible_index + 1)
            .aria_size_of_set(row_count)
            .focusable()
            .tab_stop(profile_enabled && selected)
            .h(px(66.0))
            .w_full()
            .rounded(px(9.0))
            .border_1()
            .border_color(rgb(if selected {
                0x1264a3
            } else {
                palette.main_border
            }))
            .bg(rgb(if selected {
                directory_selected_background(self.appearance_mode)
            } else {
                palette.main_bg
            }))
            .px(px(14.0))
            .flex()
            .items_center()
            .gap(px(13.0))
            .when(profile_enabled, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(rgb(directory_hover_background(self.appearance_mode))))
                    .focus_visible(|style| style.border_2().border_color(rgb(0x1264a3)))
            })
            .child(self.render_slack_directory_avatar(row))
            .child(self.render_slack_directory_labels(row))
    }

    fn render_slack_directory_labels(&self, row: &SlackNewMessageCandidateRow) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .gap(px(3.0))
            .child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(15.0))
                    .line_height(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child(row.label.clone()),
            )
            .when_some(row.secondary_label.clone(), |this, secondary| {
                this.child(
                    div()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(13.0))
                        .line_height(px(18.0))
                        .text_color(rgb(palette.main_secondary_text))
                        .child(secondary),
                )
            })
    }

    fn render_slack_directory_avatar(&self, row: &SlackNewMessageCandidateRow) -> Div {
        let avatar = if let Some(image) = row
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            div()
                .size(px(42.0))
                .rounded(slack_base_icon_radius(42.0))
                .overflow_hidden()
                .child(img(image).size_full().rounded(slack_base_icon_radius(42.0)))
        } else {
            self.render_slack_directory_avatar_fallback(row)
        };
        avatar.relative().when_some(row.presence, |this, presence| {
            this.child(slack_directory_presence_badge(
                presence,
                slack_palette(self.appearance_mode).main_bg,
            ))
        })
    }

    fn render_slack_directory_avatar_fallback(&self, row: &SlackNewMessageCandidateRow) -> Div {
        div()
            .size(px(42.0))
            .rounded(slack_base_icon_radius(42.0))
            .bg(rgb(row.avatar_fill))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(row.avatar_initials.clone())
    }
}

fn slack_directory_presence_badge(presence: SlackUserPresence, background: u32) -> Div {
    match presence {
        SlackUserPresence::Active => slack_directory_presence_dot(background, 0x20a271),
        SlackUserPresence::Away => slack_directory_presence_dot(0x616061, background),
    }
}

fn slack_directory_presence_dot(border: u32, background: u32) -> Div {
    div()
        .absolute()
        .right(px(-1.0))
        .bottom(px(-1.0))
        .size(px(11.0))
        .rounded_full()
        .border_2()
        .border_color(rgb(border))
        .bg(rgb(background))
}

fn directory_hover_background(appearance: crate::ui::AppearanceMode) -> u32 {
    match appearance {
        crate::ui::AppearanceMode::Dark => 0x24262a,
        crate::ui::AppearanceMode::Light => 0xf8f8f8,
    }
}

fn directory_selected_background(appearance: crate::ui::AppearanceMode) -> u32 {
    match appearance {
        crate::ui::AppearanceMode::Dark => 0x202b33,
        crate::ui::AppearanceMode::Light => 0xeaf5fb,
    }
}
