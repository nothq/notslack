use crate::ui::surface::{
    alpha, slack_activity_palette, SlackActivityPalette, SlackActivityRow, SurfaceState,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, list, point, px, rgb, AnyElement, BoxShadow, Context, Div, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, ListSizingBehavior, ParentElement, Role, StatefulInteractiveElement,
    Styled, Window,
};

mod actions;
mod visuals;

use visuals::{slack_activity_selected_shadow, slack_activity_unread_strip};

#[derive(Clone, Copy)]
struct SlackActivityRowPosition {
    index: usize,
    count: usize,
}

impl SurfaceState {
    pub(super) fn render_slack_activity_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        div()
            .id("slack-activity-list")
            .role(Role::ListBox)
            .aria_label("Slack activity")
            .size_full()
            .min_h(px(0.0))
            .child(
                list(
                    self.slack_activity_list_state.clone(),
                    move |visible_index, _window, cx| {
                        view.update(cx, |this, cx| {
                            let row_index = this.slack_activity_visible_row_indices[visible_index];
                            let row = &this.slack_activity_rows[row_index];
                            this.render_slack_activity_row(
                                row,
                                visible_index,
                                this.slack_activity_visible_row_indices.len(),
                                cx,
                            )
                        })
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .size_full(),
            )
            .into_any_element()
    }

    fn render_slack_activity_row(
        &self,
        row: &SlackActivityRow,
        visible_index: usize,
        visible_count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.slack_activity_selected_key.as_deref() == Some(row.key.as_ref());
        div()
            .h(px(row.card_height))
            .w_full()
            .flex()
            .flex_col()
            .when_some(row.divider_label.clone(), |this, label| {
                this.child(self.render_slack_activity_divider(label))
            })
            .child(
                self.slack_activity_row_card_shell(
                    row,
                    SlackActivityRowPosition {
                        index: visible_index,
                        count: visible_count,
                    },
                    selected,
                    cx,
                )
                .flex()
                .items_start()
                .gap(px(9.0))
                .child(self.render_slack_activity_avatar(row, cx))
                .child(self.render_slack_activity_row_content(row))
                .when(self.slack_activity_row_actions_supported(row), |this| {
                    this.child(self.render_slack_activity_row_actions(row, selected, cx))
                }),
            )
            .into_any_element()
    }

    fn slack_activity_row_card_shell(
        &self,
        row: &SlackActivityRow,
        position: SlackActivityRowPosition,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let click_key = row.key.clone();
        let keyboard_key = row.key.clone();
        let palette = slack_activity_palette(self.appearance_mode);
        let background = if row.unread {
            palette.unread_card_bg
        } else {
            palette.card_bg
        };
        div()
            .id(row.element_id.clone())
            .relative()
            .group(row.actions_hover_group.clone())
            .role(Role::ListBoxOption)
            .aria_label(row.accessibility_label.clone())
            .aria_selected(selected)
            .aria_position_in_set(position.index + 1)
            .aria_size_of_set(position.count)
            .focusable()
            .tab_stop(true)
            .flex_grow(1.0)
            .min_h(px(0.0))
            .mx(px(16.0))
            .mb(px(8.0))
            .px(px(12.0))
            .py(px(12.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(alpha(palette.card_border, palette.card_border_alpha))
            .bg(rgb(background))
            .cursor_pointer()
            .hover(move |style| {
                style.border_color(alpha(palette.card_border, palette.card_hover_border_alpha))
            })
            .focus_visible(move |style| style.border_color(rgb(palette.focus_border)))
            .when(selected, |this| {
                this.shadow(slack_activity_selected_shadow(palette))
            })
            .when(row.unread, |this| {
                this.overflow_hidden()
                    .child(slack_activity_unread_strip(palette))
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_activity_row(click_key.as_ref(), cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                this.handle_slack_activity_row_key(event, keyboard_key.as_ref(), window, cx);
            }))
    }

    fn handle_slack_activity_row_key(
        &mut self,
        event: &KeyDownEvent,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let key_name = event.keystroke.key.as_str();
        if !matches!(key_name, "enter" | "space" | "up" | "down" | "u" | "c") {
            return;
        }
        window.prevent_default();
        cx.stop_propagation();
        match key_name {
            "enter" | "space" => self.activate_selected_slack_activity(cx),
            "up" => self.move_slack_activity_selection(-1, cx),
            "down" => self.move_slack_activity_selection(1, cx),
            "u" => self.toggle_slack_activity_item_read(key, cx),
            "c" => self.toggle_slack_activity_item_archive(
                key,
                "clear_notification_shortcut",
                "restore_notification_shortcut",
                cx,
            ),
            _ => unreachable!("recognized Slack activity shortcut"),
        }
    }

    fn render_slack_activity_context(&self, row: &SlackActivityRow) -> Div {
        let palette = slack_activity_palette(self.appearance_mode);
        let text = slack_activity_row_text(row, palette);
        div()
            .h(px(21.0))
            .min_w(px(0.0))
            .flex()
            .items_center()
            .gap(px(5.0))
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .text_size(px(14.0))
            .text_color(rgb(text))
            .child(row.kind.context_prefix())
            .when_some(
                row.channel_label.clone().filter(|_| !row.kind.is_dm()),
                |this, channel| {
                    this.child(
                        div()
                            .h(px(20.0))
                            .max_w(px(240.0))
                            .px(px(6.0))
                            .rounded(px(4.0))
                            .bg(alpha(palette.primary_text, 0.06))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(13.0))
                            .text_color(rgb(text))
                            .child(channel),
                    )
                },
            )
    }

    fn render_slack_activity_row_content(&self, row: &SlackActivityRow) -> Div {
        let mutation_pending = self.slack_activity_item_mutation_is_pending(row.key.as_ref());
        let palette = slack_activity_palette(self.appearance_mode);
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .child(slack_activity_row_header(row, mutation_pending, palette))
            .child(self.render_slack_activity_context(row))
            .child(slack_activity_row_body(row, palette))
            .when_some(row.reaction_label.clone(), |this, reaction| {
                this.child(slack_activity_reaction(row, reaction, palette))
            })
    }
}

fn slack_activity_row_header(
    row: &SlackActivityRow,
    mutation_pending: bool,
    palette: SlackActivityPalette,
) -> Div {
    let text = slack_activity_row_text(row, palette);
    div()
        .h(px(21.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.0))
        .child(
            div()
                .flex_grow(1.0)
                .min_w(px(0.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_size(px(15.0))
                .font_weight(if row.unread {
                    FontWeight::BOLD
                } else {
                    FontWeight::NORMAL
                })
                .text_color(rgb(text))
                .child(row.actor_label.clone()),
        )
        .child(
            div()
                .flex_none()
                .text_size(px(13.0))
                .text_color(rgb(text))
                .child(if mutation_pending {
                    "Updating…".into()
                } else {
                    row.timestamp_label.clone()
                }),
        )
}

fn slack_activity_row_body(row: &SlackActivityRow, palette: SlackActivityPalette) -> Div {
    div()
        .max_h(px(44.0))
        .overflow_hidden()
        .whitespace_normal()
        .line_clamp(2)
        .text_ellipsis()
        .line_height(px(22.0))
        .text_size(px(14.0))
        .text_color(rgb(slack_activity_row_text(row, palette)))
        .child(row.body.text.clone())
}

fn slack_activity_reaction(
    row: &SlackActivityRow,
    reaction: gpui::SharedString,
    palette: SlackActivityPalette,
) -> gpui::Stateful<Div> {
    div()
        .id(format!("{}-reaction", row.element_id.as_ref()))
        .mt(px(5.0))
        .h(px(28.0))
        .self_start()
        .px(px(10.0))
        .rounded_full()
        .bg(alpha(palette.primary_text, 0.06))
        .hover(move |style| {
            style
                .bg(alpha(palette.primary_text, 0.0))
                .shadow(vec![BoxShadow {
                    color: alpha(palette.reaction_hover_border, 1.0),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(1.0),
                    inset: true,
                }])
        })
        .flex()
        .items_center()
        .text_size(px(13.0))
        .text_color(rgb(palette.primary_text))
        .child(reaction)
}

fn slack_activity_row_text(row: &SlackActivityRow, palette: SlackActivityPalette) -> u32 {
    if row.unread {
        palette.primary_text
    } else {
        palette.secondary_text
    }
}
