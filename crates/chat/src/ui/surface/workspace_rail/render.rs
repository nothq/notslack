use gpui::{
    prelude::FluentBuilder, uniform_list, AppContext, Context, Div, FocusHandle, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, ListSizingBehavior, MouseButton, MouseDownEvent,
    ObjectFit, Orientation, ParentElement, Render, Role, SharedString, Stateful,
    StatefulInteractiveElement, Styled, StyledImage, Window,
};

use super::super::{alpha, div, img, px, rgb};
use super::{
    attention::{slack_workspace_accessibility_label, slack_workspace_attention_badge},
    shortcuts::slack_workspace_item_key_target,
    SlackWorkspaceRail, WORKSPACE_RING_SIZE, WORKSPACE_ROW_HEIGHT, WORKSPACE_TILE_SIZE,
};

impl SlackWorkspaceRail {
    fn render_item(&mut self, index: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let item = &self.items[index];
        let team_id = item.team_id.clone();
        let workspace_name = item.workspace_name.clone();
        let selected = team_id == self.selected_team_id;
        let attention = self.attention[index];
        let focus_handle = item.focus_handle.clone();
        let tooltip_name = workspace_name.clone();
        let shortcut = (index < 9).then(|| format!("Command-{}", index + 1));
        let item_count = self.items.len();
        let accessibility_label = slack_workspace_accessibility_label(
            &workspace_name,
            index,
            item_count,
            attention,
            shortcut.as_deref(),
        );
        workspace_item_base(
            index,
            selected,
            item_count,
            accessibility_label,
            &focus_handle,
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, window, cx| {
                this.request_selection(index, window, cx);
                cx.stop_propagation();
            }),
        )
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
            if let Some(target) = slack_workspace_item_key_target(event, index, this.items.len()) {
                window.prevent_default();
                cx.stop_propagation();
                this.request_selection(target, window, cx);
            }
        }))
        .tooltip(move |_, cx| {
            cx.new(|_| SlackWorkspaceTooltip {
                workspace_name: tooltip_name.clone(),
                ordinal: (index < 9).then_some(index + 1),
            })
            .into()
        })
        .child(self.render_workspace_ring(index, selected))
    }

    fn render_workspace_ring(&self, index: usize, selected: bool) -> Div {
        div()
            .size(px(WORKSPACE_RING_SIZE))
            .relative()
            .flex_none()
            .p(px(2.0))
            .rounded(px(12.0))
            .border_2()
            .border_color(if selected {
                alpha(0xe5e5e5, 0.9)
            } else {
                alpha(0xe5e5e5, 0.0)
            })
            .child(self.render_workspace_tile(index))
            .child(slack_workspace_attention_badge(self.attention[index]))
    }

    fn render_workspace_tile(&self, index: usize) -> Div {
        let item = &self.items[index];
        let fallback = slack_workspace_logo_fallback(item.initials.clone(), item.fallback_fill);
        let Some(url) = item.workspace_logo_url.clone() else {
            return fallback;
        };
        let loading_initials = item.initials.clone();
        let loading_fill = item.fallback_fill;
        let fallback_initials = item.initials.clone();
        let fallback_fill = item.fallback_fill;
        div()
            .size(px(WORKSPACE_TILE_SIZE))
            .rounded(px(8.0))
            .overflow_hidden()
            .child(
                img(url)
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .with_loading(move || {
                        slack_workspace_logo_fallback(loading_initials.clone(), loading_fill)
                            .into_any_element()
                    })
                    .with_fallback(move || {
                        slack_workspace_logo_fallback(fallback_initials.clone(), fallback_fill)
                            .into_any_element()
                    }),
            )
    }
}

fn workspace_item_base(
    index: usize,
    selected: bool,
    item_count: usize,
    accessibility_label: String,
    focus_handle: &FocusHandle,
) -> Stateful<Div> {
    div()
        .id(("slack-workspace", index))
        .role(Role::Tab)
        .aria_label(accessibility_label)
        .aria_selected(selected)
        .aria_position_in_set(index + 1)
        .aria_size_of_set(item_count)
        .focusable()
        .track_focus(focus_handle)
        .tab_stop(selected)
        .w_full()
        .h(px(WORKSPACE_ROW_HEIGHT))
        .flex_none()
        .flex()
        .justify_center()
        .items_start()
        .cursor_pointer()
}

impl Render for SlackWorkspaceRail {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.rendered_window_id = Some(window.window_handle().window_id());
        if !self.expanded {
            return div().size_full();
        }
        if std::mem::take(&mut self.add_menu_focus_pending) {
            window.focus(&self.add_menu_focus_handle, cx);
        }
        let view = cx.entity();
        let count = self.items.len() + 1;
        let scroll_handle = self.scroll_handle.clone();
        let viewport_size = window.viewport_size();
        div()
            .size_full()
            .relative()
            .bg(rgb(0x0e0e0e))
            .child(
                div()
                    .id("slack-workspace-tabs")
                    .role(Role::TabList)
                    .aria_label("Slack workspaces")
                    .aria_orientation(Orientation::Vertical)
                    .size_full()
                    .pt(px(10.0))
                    .overflow_hidden()
                    .occlude()
                    .child(
                        uniform_list("slack-workspace-tiles", count, move |range, _window, cx| {
                            view.update(cx, |this, cx| {
                                range
                                    .map(|index| {
                                        if index < this.items.len() {
                                            this.render_item(index, cx).into_any_element()
                                        } else {
                                            this.render_add_row(cx).into_any_element()
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            })
                        })
                        .with_sizing_behavior(ListSizingBehavior::Auto)
                        .track_scroll(&scroll_handle)
                        .size_full(),
                    ),
            )
            .when(self.add_menu_open, |rail| {
                rail.child(self.render_add_menu_layer(
                    viewport_size.width,
                    viewport_size.height,
                    cx,
                ))
            })
    }
}

struct SlackWorkspaceTooltip {
    workspace_name: SharedString,
    ordinal: Option<usize>,
}

impl Render for SlackWorkspaceTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.0))
            .py(px(7.0))
            .rounded(px(6.0))
            .bg(rgb(0x1d1c1d))
            .flex()
            .items_center()
            .gap(px(10.0))
            .text_size(px(13.0))
            .line_height(px(18.0))
            .text_color(rgb(0xffffff))
            .child(self.workspace_name.clone())
            .when_some(self.ordinal, |this, ordinal| {
                this.child(
                    div()
                        .px(px(5.0))
                        .rounded(px(4.0))
                        .bg(alpha(0xffffff, 0.10))
                        .text_size(px(11.0))
                        .text_color(rgb(0xd1d2d3))
                        .child(format!("⌘{ordinal}")),
                )
            })
    }
}

fn slack_workspace_logo_fallback(initials: SharedString, fill: u32) -> Div {
    div()
        .size(px(WORKSPACE_TILE_SIZE))
        .rounded(px(8.0))
        .bg(rgb(fill))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .child(initials)
}
