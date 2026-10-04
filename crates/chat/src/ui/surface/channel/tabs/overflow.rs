use crate::ui::surface::{slack_icon, slack_palette, PreparedSlackConversationTab, SurfaceState};
use crate::ui::{
    alpha, div, point, px, rgb, AnyElement, BoxShadow, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement,
    StatefulInteractiveElement, Styled, Window, SLACK_TABS_HEIGHT,
};
use gpui::{Role, Stateful};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use super::{
    SLACK_CONVERSATION_TABS_MENU_ROW_HEIGHT, SLACK_CONVERSATION_TABS_MENU_WIDTH,
    SLACK_CONVERSATION_TABS_RIGHT_PADDING, SLACK_CONVERSATION_TAB_FONT_SIZE,
    SLACK_CONVERSATION_TAB_LINE_HEIGHT,
};

#[derive(Clone, Copy)]
struct SlackConversationTabsOverflowLayout {
    left: f32,
    top: f32,
    background: u32,
}

impl SurfaceState {
    pub(crate) fn render_slack_conversation_tabs_overflow_layer(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(bounds) = self.slack_conversation_tabs_cache.bounds else {
            return div().into_any_element();
        };
        let left = (bounds.origin.x.as_f32() + bounds.size.width.as_f32()
            - SLACK_CONVERSATION_TABS_RIGHT_PADDING
            - SLACK_CONVERSATION_TABS_MENU_WIDTH)
            .clamp(
                4.0,
                (self.preview_width - SLACK_CONVERSATION_TABS_MENU_WIDTH - 4.0).max(4.0),
            );
        let layout = SlackConversationTabsOverflowLayout {
            left,
            top: bounds.origin.y.as_f32() + SLACK_TABS_HEIGHT,
            background: match self.appearance_mode {
                crate::ui::AppearanceMode::Dark => 0x222529,
                crate::ui::AppearanceMode::Light => 0xffffff,
            },
        };
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_conversation_tabs_overflow(cx);
            }),
            cx,
        );
        div()
            .absolute()
            .top(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .occlude()
            .child(backdrop)
            .child(self.render_slack_conversation_tabs_overflow_menu(layout, cx))
            .into_any_element()
    }

    fn render_slack_conversation_tabs_overflow_menu(
        &self,
        layout: SlackConversationTabsOverflowLayout,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let mut menu = self.slack_conversation_tabs_overflow_menu_shell(layout, cx);
        for (index, spec) in self
            .slack_conversation_tabs_cache
            .tabs
            .iter()
            .skip(self.slack_conversation_tabs_cache.visible_count)
            .enumerate()
        {
            menu = menu.child(self.render_slack_conversation_tabs_overflow_item(index, spec, cx));
        }
        menu
    }

    fn slack_conversation_tabs_overflow_menu_shell(
        &self,
        layout: SlackConversationTabsOverflowLayout,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id("slack-conversation-tabs-overflow-menu")
            .role(Role::Menu)
            .aria_label("More conversation views")
            .track_focus(&self.slack_conversation_tabs_overflow_focus_handle)
            .absolute()
            .left(px(layout.left))
            .top(px(layout.top))
            .w(px(SLACK_CONVERSATION_TABS_MENU_WIDTH))
            .py(px(8.0))
            .rounded(px(8.0))
            .bg(rgb(layout.background))
            .shadow(vec![
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
            ])
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|this, event, window, cx| {
                this.handle_slack_conversation_tabs_overflow_key(event, window, cx);
            }))
    }

    fn render_slack_conversation_tabs_overflow_item(
        &self,
        index: usize,
        spec: &PreparedSlackConversationTab,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let selected = self.slack_conversation_tabs_overflow_selected_index == Some(index);
        let active = self.slack_conversation_tab_is_active(spec);
        let action = spec.action.clone();
        let item = self.slack_conversation_tabs_overflow_item_base(spec, selected, active, cx);
        if !spec.enabled {
            return item;
        }
        item.cursor_pointer()
            .hover(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.hover_slack_conversation_tabs_overflow_item(index, cx);
                }
            }))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_conversation_tab_action(action.clone(), cx);
            }))
    }

    fn slack_conversation_tabs_overflow_item_base(
        &self,
        spec: &PreparedSlackConversationTab,
        selected: bool,
        active: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        let text_color = if selected {
            0xffffff
        } else {
            palette.main_text
        };
        let accessibility_label = if spec.enabled {
            spec.label.clone()
        } else {
            format!("{}, unavailable", spec.label).into()
        };
        let font_weight = if active {
            FontWeight::BOLD
        } else {
            FontWeight::NORMAL
        };
        div()
            .id(spec.overflow_element_id.clone())
            .role(Role::MenuItem)
            .aria_label(accessibility_label)
            .aria_selected(active)
            .when(selected, |this| this.aria_active_descendant())
            .h(px(SLACK_CONVERSATION_TABS_MENU_ROW_HEIGHT))
            .px(px(12.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .text_size(px(SLACK_CONVERSATION_TAB_FONT_SIZE))
            .line_height(px(SLACK_CONVERSATION_TAB_LINE_HEIGHT))
            .font_weight(font_weight)
            .text_color(rgb(text_color))
            .when(selected, |this| this.bg(rgb(0x1264a3)))
            .child(
                div()
                    .flex_none()
                    .size(px(20.0))
                    .child(slack_icon(spec.icon, text_color, 20.0, cx)),
            )
            .child(
                div()
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .truncate()
                    .child(spec.label.clone()),
            )
    }

    pub(super) fn toggle_slack_conversation_tabs_overflow(&mut self, cx: &mut Context<Self>) {
        if self.slack_conversation_tabs_overflow_open {
            self.close_slack_conversation_tabs_overflow(cx);
        } else {
            self.open_slack_conversation_tabs_overflow(cx);
        }
    }

    fn open_slack_conversation_tabs_overflow(&mut self, cx: &mut Context<Self>) {
        if !self.slack_conversation_tabs_cache.has_overflow() {
            return;
        }
        self.slack_history_menu_open = false;
        self.slack_history_menu_selected_index = None;
        self.slack_channel_menu_open = false;
        self.slack_channel_menu_selected_index = None;
        self.slack_conversation_tabs_overflow_open = true;
        self.slack_conversation_tabs_overflow_selected_index = Some(0);
        self.slack_conversation_tabs_overflow_focus_pending = true;
        cx.notify();
    }

    fn close_slack_conversation_tabs_overflow(&mut self, cx: &mut Context<Self>) {
        if !self.slack_conversation_tabs_overflow_open {
            return;
        }
        self.slack_conversation_tabs_overflow_open = false;
        self.slack_conversation_tabs_overflow_selected_index = None;
        self.slack_conversation_tabs_overflow_focus_pending = false;
        cx.notify();
    }

    fn hover_slack_conversation_tabs_overflow_item(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.slack_conversation_tabs_overflow_selected_index == Some(index) {
            return;
        }
        self.slack_conversation_tabs_overflow_selected_index = Some(index);
        cx.notify();
    }

    fn handle_slack_conversation_tabs_overflow_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let overflow_count = self
            .slack_conversation_tabs_cache
            .tabs
            .len()
            .saturating_sub(self.slack_conversation_tabs_cache.visible_count);
        if overflow_count == 0 {
            self.close_slack_conversation_tabs_overflow(cx);
            return;
        }
        if self.handle_slack_conversation_tabs_overflow_navigation(event, overflow_count, cx) {
            window.prevent_default();
            cx.stop_propagation();
            return;
        }
        match event.keystroke.key.as_str() {
            "escape" => {
                window.prevent_default();
                cx.stop_propagation();
                self.close_slack_conversation_tabs_overflow(cx);
                let focus = self.slack_conversation_tabs_more_focus_handle.clone();
                window.focus(&focus, cx);
            }
            "enter" | "space" => {
                let index = self
                    .slack_conversation_tabs_overflow_selected_index
                    .unwrap_or(0);
                let tab = &self.slack_conversation_tabs_cache.tabs
                    [self.slack_conversation_tabs_cache.visible_count + index];
                window.prevent_default();
                cx.stop_propagation();
                if tab.enabled {
                    self.activate_slack_conversation_tab_action(tab.action.clone(), cx);
                }
            }
            _ => {}
        }
    }

    fn handle_slack_conversation_tabs_overflow_navigation(
        &mut self,
        event: &KeyDownEvent,
        overflow_count: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        let selected = match event.keystroke.key.as_str() {
            "up" | "arrowup" => {
                let current = self
                    .slack_conversation_tabs_overflow_selected_index
                    .unwrap_or(0);
                Some((current + overflow_count - 1) % overflow_count)
            }
            "down" | "arrowdown" => {
                let current = self
                    .slack_conversation_tabs_overflow_selected_index
                    .unwrap_or(overflow_count - 1);
                Some((current + 1) % overflow_count)
            }
            "home" => Some(0),
            "end" => Some(overflow_count - 1),
            _ => None,
        };
        let Some(selected) = selected else {
            return false;
        };
        self.slack_conversation_tabs_overflow_selected_index = Some(selected);
        cx.notify();
        true
    }
}
