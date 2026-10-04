use gpui::prelude::FluentBuilder;
use gpui::{
    div, list, point, px, rgb, AnyElement, BoxShadow, Context, Div, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, ListSizingBehavior, MouseButton, MouseDownEvent, ParentElement,
    Role, StatefulInteractiveElement, Styled,
};

use super::super::rows::{
    slack_sidebar_item_badge, slack_sidebar_item_badge_count, slack_sidebar_item_body,
    slack_sidebar_item_row_with_active, SlackSidebarItemBodySpec,
};
use super::{
    alpha, slack_icon, slack_palette, SlackShellIcon, SlackSidebarRow, SlackSidebarRowKind,
    SlackSidebarSectionIndicator, SurfaceState,
};

const SLACK_HOME_FINDER_BAND_HEIGHT: f32 = 32.0;
const SLACK_HOME_FINDER_FIELD_HEIGHT: f32 = 28.0;
const SLACK_HOME_FINDER_RESULT_TOP_GAP: f32 = 6.0;
const SLACK_HOME_FINDER_ROW_HEIGHT: f32 = 28.0;

impl SurfaceState {
    pub(super) fn render_slack_home_finder_search(&self, cx: &mut Context<Self>) -> Div {
        let focused = self.slack_home_finder_focused;
        div()
            .h(px(SLACK_HOME_FINDER_BAND_HEIGHT))
            .flex_none()
            .px(px(8.0))
            .pb(px(4.0))
            .child(self.render_slack_home_finder_field(focused, cx))
    }

    fn render_slack_home_finder_field(&self, focused: bool, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_HOME_FINDER_FIELD_HEIGHT))
            .w_full()
            .rounded(px(7.0))
            .border_1()
            .border_color(rgb(if focused {
                0x1d9bd1
            } else {
                palette.sidebar_border
            }))
            .bg(rgb(palette.sidebar_bg))
            .flex()
            .items_center()
            .overflow_hidden()
            .when(focused, |this| {
                this.shadow(vec![BoxShadow {
                    color: alpha(0x1d9bd1, 0.48),
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(4.0),
                    spread_radius: px(2.0),
                    inset: false,
                }])
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    this.focus_slack_home_finder(cx);
                    let focus = this.slack_home_finder_input.read(cx).focus_handle_clone();
                    window.focus(&focus, cx);
                }),
            )
            .child(
                div()
                    .w(px(25.0))
                    .h_full()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(
                        SlackShellIcon::ListSearch,
                        palette.sidebar_icon,
                        18.0,
                        cx,
                    )),
            )
            .child(
                div()
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .h_full()
                    .child(self.slack_home_finder_input_entity(cx)),
            )
            .when(!self.slack_home_finder_query.is_empty(), |this| {
                this.child(self.render_slack_home_finder_clear(cx))
            })
    }

    fn render_slack_home_finder_clear(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("slack-home-finder-clear")
            .role(Role::Button)
            .aria_label("Clear text")
            .focusable()
            .tab_stop(true)
            .w(px(32.0))
            .h_full()
            .flex_none()
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .size(px(16.0))
                    .rounded_full()
                    .bg(rgb(0xf8f8f8))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(SlackShellIcon::Close, 0x1d1c1d, 12.0, cx)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.clear_slack_home_finder_query(cx);
                    let focus = this.slack_home_finder_input.read(cx).focus_handle_clone();
                    window.focus(&focus, cx);
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    this.clear_slack_home_finder_query(cx);
                    let focus = this.slack_home_finder_input.read(cx).focus_handle_clone();
                    window.focus(&focus, cx);
                }
            }))
            .into_any_element()
    }

    pub(super) fn render_slack_home_finder_results(&self, cx: &mut Context<Self>) -> AnyElement {
        let rows = self.slack_sidebar_rows.clone();
        let result_indices = self.slack_home_finder_row_indices.clone();
        let result_count = result_indices.len();
        let selected_display_index = self.slack_home_finder_selected_index.and_then(|index| {
            self.slack_home_finder_selectable_indices
                .get(index)
                .copied()
        });
        let list_state = self.slack_home_finder_list_state.clone();
        let view = cx.entity();

        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_hidden()
            .px(px(8.0))
            .pt(px(SLACK_HOME_FINDER_RESULT_TOP_GAP))
            .when(result_count == 0, |this| {
                this.child(self.render_slack_home_finder_empty())
            })
            .when(result_count > 0, |this| {
                this.child(
                    div()
                        .id("slack-home-finder-listbox")
                        .role(Role::ListBox)
                        .aria_label("Conversation search results")
                        .size_full()
                        .overflow_hidden()
                        .child(
                            list(list_state, move |display_index, _window, cx| {
                                let rows = rows.clone();
                                let result_indices = result_indices.clone();
                                view.update(cx, move |this, cx| {
                                    let source_index = *result_indices
                                        .get(display_index)
                                        .expect("Slack Home finder source index should exist");
                                    let row = rows
                                        .get(source_index)
                                        .expect("prepared Slack Home finder row should exist");
                                    this.render_slack_home_finder_row(
                                        row,
                                        display_index,
                                        selected_display_index == Some(display_index),
                                        cx,
                                    )
                                })
                            })
                            .with_sizing_behavior(ListSizingBehavior::Auto)
                            .size_full(),
                        ),
                )
            })
            .into_any_element()
    }

    fn render_slack_home_finder_empty(&self) -> Div {
        div()
            .h(px(56.0))
            .px(px(24.0))
            .flex()
            .items_center()
            .text_size(px(15.0))
            .text_color(rgb(slack_palette(self.appearance_mode).sidebar_muted_text))
            .child("No conversations match your search")
    }

    fn render_slack_home_finder_row(
        &self,
        row: &SlackSidebarRow,
        display_index: usize,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match &row.kind {
            SlackSidebarRowKind::SectionHeader {
                label, indicator, ..
            } => self.render_slack_home_finder_section(*indicator, label, cx),
            SlackSidebarRowKind::Item { .. } => {
                self.render_slack_home_finder_item(row, display_index, selected, cx)
            }
            _ => div().h(px(SLACK_HOME_FINDER_ROW_HEIGHT)).into_any_element(),
        }
    }

    fn render_slack_home_finder_section(
        &self,
        indicator: SlackSidebarSectionIndicator,
        label: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label: gpui::SharedString = label.to_string().into();
        div()
            .w_full()
            .h(px(SLACK_HOME_FINDER_ROW_HEIGHT))
            .px(px(8.0))
            .flex()
            .items_center()
            .gap(px(8.0))
            .text_size(px(15.0))
            .line_height(px(SLACK_HOME_FINDER_ROW_HEIGHT))
            .font_weight(FontWeight::NORMAL)
            .text_color(rgb(slack_palette(self.appearance_mode).sidebar_section_text))
            .child(self.render_slack_section_header_indicator(indicator, &label, false, cx))
            .child(label.clone())
            .into_any_element()
    }

    fn render_slack_home_finder_item(
        &self,
        source: &SlackSidebarRow,
        display_index: usize,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let SlackSidebarRowKind::Item {
            item,
            label,
            secondary_context,
            finder_element_id,
            finder_accessibility_label,
            finder_group_count_label,
            ..
        } = &source.kind
        else {
            unreachable!("Slack Home finder item renderer requires an item row");
        };
        let row = slack_sidebar_item_row_with_active(item, self.appearance_mode, selected)
            .id(finder_element_id.clone())
            .role(Role::ListBoxOption)
            .aria_label(finder_accessibility_label.clone())
            .aria_selected(selected)
            .child(slack_sidebar_item_body(
                SlackSidebarItemBodySpec {
                    item,
                    active: selected,
                    label: label.clone(),
                    secondary_context: secondary_context.clone(),
                    group_count_label: finder_group_count_label.clone(),
                    unread: item.unread || selected,
                    peer_notifications_paused: false,
                    appearance_mode: self.appearance_mode,
                    remote_images: &self.slack_remote_images,
                },
                cx,
            ))
            .when_some(slack_sidebar_item_badge_count(item), |this, count| {
                this.child(slack_sidebar_item_badge(item, count, self.appearance_mode))
            });
        if !self.slack_workspace_api_capabilities.load_conversation {
            return row.into_any_element();
        }
        row.cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    this.select_slack_home_finder_result(display_index, cx);
                }),
            )
            .into_any_element()
    }
}
