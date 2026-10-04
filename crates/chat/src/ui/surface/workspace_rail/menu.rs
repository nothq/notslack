use gpui::{
    point, prelude::FluentBuilder, BoxShadow, Context, Div, FontWeight, InteractiveElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, Point, Role, Stateful,
    StatefulInteractiveElement, Styled, Window,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use super::super::{alpha, div, px, rgb, slack_icon, SlackShellIcon, SLACK_HISTORY_HEIGHT};
use super::{
    SlackWorkspaceRail, WORKSPACE_ADD_BUTTON_SIZE, WORKSPACE_ADD_BUTTON_TOP_IN_ROW,
    WORKSPACE_ADD_MENU_HEIGHT, WORKSPACE_ADD_MENU_LEFT, WORKSPACE_ADD_MENU_ROW_HEIGHT,
    WORKSPACE_ADD_MENU_WIDTH, WORKSPACE_ROW_HEIGHT,
};
use crate::ui::SLACK_WORKSPACE_RAIL_WIDTH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlackWorkspaceAddAction {
    SignIn,
    Create,
    Find,
}

impl SlackWorkspaceAddAction {
    const ALL: [Self; 3] = [Self::SignIn, Self::Create, Self::Find];

    fn element_id(self) -> &'static str {
        match self {
            Self::SignIn => "slack-workspace-add-sign-in",
            Self::Create => "slack-workspace-add-create",
            Self::Find => "slack-workspace-add-find",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::SignIn => "Sign in to another workspace",
            Self::Create => "Create a new workspace",
            Self::Find => "Find workspaces",
        }
    }

    fn url(self) -> &'static str {
        match self {
            Self::SignIn => "https://app.slack.com/ssb/add?s=1",
            Self::Create => "https://slack.com/ssb/get-started?entry_point=tab_rail_ssb#Create",
            Self::Find => "https://slack.com/ssb/get-started?entry_point=tab_rail_ssb#Find",
        }
    }
}

impl SlackWorkspaceRail {
    pub(super) fn render_add_row(&self, cx: &mut Context<Self>) -> Div {
        let focus_handle = self.add_button_focus_handle.clone();
        div()
            .w_full()
            .h(px(WORKSPACE_ROW_HEIGHT))
            .flex_none()
            .pt(px(WORKSPACE_ADD_BUTTON_TOP_IN_ROW))
            .flex()
            .items_start()
            .justify_center()
            .child(
                div()
                    .id("slack-workspace-add")
                    .role(Role::Button)
                    .aria_label("Add workspaces")
                    .aria_expanded(self.add_menu_open)
                    .track_focus(&focus_handle)
                    .focusable()
                    .tab_stop(true)
                    .size(px(WORKSPACE_ADD_BUTTON_SIZE))
                    .rounded(px(8.0))
                    .cursor_pointer()
                    .hover(|style| style.bg(alpha(0xf6f6f6, 0.25)))
                    .active(|style| style.bg(alpha(0xf6f6f6, 0.32)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_: &mut Self, _: &MouseDownEvent, window, cx| {
                            // Slack keeps the existing message selection while this control opens.
                            // Suppress implicit focus here; the menu receives focus after rendering.
                            window.prevent_default();
                            cx.stop_propagation();
                        }),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.toggle_add_menu(true, window, cx);
                        cx.stop_propagation();
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space")
                            && !event.keystroke.modifiers.modified()
                        {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.toggle_add_menu(true, window, cx);
                        }
                    }))
                    .child(slack_icon(SlackShellIcon::Plus, 0xe5e5e5, 20.0, cx)),
            )
    }

    pub(super) fn toggle_add_menu(
        &mut self,
        restore_button_focus: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.add_menu_open {
            self.close_add_menu(window, cx);
        } else {
            self.add_menu_open = true;
            self.add_menu_selected_index = None;
            self.add_menu_focus_pending = true;
            self.add_menu_restore_button_focus = restore_button_focus;
            cx.notify();
        }
    }

    fn close_add_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.add_menu_open {
            return;
        }
        self.add_menu_open = false;
        self.add_menu_selected_index = None;
        self.add_menu_focus_pending = false;
        if self.add_menu_restore_button_focus {
            window.focus(&self.add_button_focus_handle, cx);
        } else {
            window.blur();
        }
        self.add_menu_restore_button_focus = false;
        cx.notify();
    }

    pub(super) fn add_button_contains(&self, position: Point<Pixels>) -> bool {
        let left = px((SLACK_WORKSPACE_RAIL_WIDTH - WORKSPACE_ADD_BUTTON_SIZE) / 2.0);
        let top = px(SLACK_HISTORY_HEIGHT
            + 10.0
            + self.items.len() as f32 * WORKSPACE_ROW_HEIGHT
            + WORKSPACE_ADD_BUTTON_TOP_IN_ROW)
            + self.scroll_handle.0.borrow().base_handle.offset().y;
        position.x >= left
            && position.x < left + px(WORKSPACE_ADD_BUTTON_SIZE)
            && position.y >= top
            && position.y < top + px(WORKSPACE_ADD_BUTTON_SIZE)
    }

    pub(super) fn render_add_menu_layer(
        &self,
        viewport_width: Pixels,
        viewport_height: Pixels,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .left(px(0.0))
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, window, cx| {
                this.close_add_menu(window, cx);
            }),
            cx,
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|_: &mut Self, _: &MouseDownEvent, window, _| {
                window.prevent_default();
            }),
        );
        div()
            .id("slack-workspace-add-menu-layer")
            .absolute()
            .left(px(0.0))
            .top(px(-SLACK_HISTORY_HEIGHT))
            .w(viewport_width)
            .h(viewport_height)
            .occlude()
            .child(backdrop)
            .child(self.render_add_menu(cx))
    }

    fn render_add_menu(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let focus_handle = self.add_menu_focus_handle.clone();
        let menu_top = px(SLACK_HISTORY_HEIGHT
            + 10.0
            + self.items.len() as f32 * WORKSPACE_ROW_HEIGHT
            + WORKSPACE_ADD_BUTTON_TOP_IN_ROW)
            + self.scroll_handle.0.borrow().base_handle.offset().y;
        div()
            .id("slack-workspace-add-menu")
            .role(Role::Menu)
            .aria_label("Add workspace")
            .track_focus(&focus_handle)
            .focusable()
            .tab_stop(false)
            .absolute()
            .left(px(WORKSPACE_ADD_MENU_LEFT))
            .top(menu_top)
            .w(px(WORKSPACE_ADD_MENU_WIDTH))
            .h(px(WORKSPACE_ADD_MENU_HEIGHT))
            .py(px(12.0))
            .rounded(px(4.0))
            .bg(rgb(0xffffff))
            .shadow(slack_workspace_add_menu_shadow())
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.handle_add_menu_key(event, window, cx) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .children(
                SlackWorkspaceAddAction::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(index, action)| self.render_add_menu_item(index, action, cx)),
            )
    }

    fn render_add_menu_item(
        &self,
        index: usize,
        action: SlackWorkspaceAddAction,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.add_menu_selected_index == Some(index);
        div()
            .id(action.element_id())
            .role(Role::MenuItem)
            .aria_label(action.label())
            .when(selected, |item| item.aria_active_descendant())
            .w_full()
            .h(px(WORKSPACE_ADD_MENU_ROW_HEIGHT))
            .px(px(24.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .text_size(px(15.0))
            .line_height(px(WORKSPACE_ADD_MENU_ROW_HEIGHT))
            .font_weight(FontWeight::NORMAL)
            .text_color(rgb(if selected { 0xffffff } else { 0x1d1c1d }))
            .when(selected, |item| item.bg(rgb(0x1264a3)))
            .hover(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered && this.add_menu_selected_index != Some(index) {
                    this.add_menu_selected_index = Some(index);
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| {
                window.prevent_default();
                cx.stop_propagation();
                this.activate_add_menu_action(action, window, cx);
            }))
            .child(action.label())
    }

    fn handle_add_menu_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.keystroke.key == "escape" {
            self.close_add_menu(window, cx);
            return true;
        }
        if event.keystroke.key == "tab" {
            return true;
        }
        if event.keystroke.modifiers.modified() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "up" | "arrowup" => self.move_add_menu_selection(-1, cx),
            "down" | "arrowdown" => self.move_add_menu_selection(1, cx),
            "home" => self.select_add_menu_boundary(false, cx),
            "end" => self.select_add_menu_boundary(true, cx),
            "enter" | "space" => {
                if let Some(index) = self.add_menu_selected_index {
                    self.activate_add_menu_action(SlackWorkspaceAddAction::ALL[index], window, cx);
                }
            }
            _ => return false,
        }
        true
    }

    fn move_add_menu_selection(&mut self, direction: isize, cx: &mut Context<Self>) {
        let count = SlackWorkspaceAddAction::ALL.len();
        self.add_menu_selected_index = Some(match self.add_menu_selected_index {
            Some(index) => (index as isize + direction).rem_euclid(count as isize) as usize,
            None if direction < 0 => count - 1,
            None => 0,
        });
        cx.notify();
    }

    fn select_add_menu_boundary(&mut self, end: bool, cx: &mut Context<Self>) {
        self.add_menu_selected_index = Some(if end {
            SlackWorkspaceAddAction::ALL.len() - 1
        } else {
            0
        });
        cx.notify();
    }

    fn activate_add_menu_action(
        &mut self,
        action: SlackWorkspaceAddAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.add_menu_open {
            return;
        }
        self.close_add_menu(window, cx);
        cx.open_url(action.url());
    }
}

fn slack_workspace_add_menu_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: alpha(0x1d1c1d, 0.13),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
            inset: false,
        },
        BoxShadow {
            color: alpha(0x000000, 0.12),
            offset: point(px(0.0), px(4.0)),
            blur_radius: px(12.0),
            spread_radius: px(0.0),
            inset: false,
        },
    ]
}
