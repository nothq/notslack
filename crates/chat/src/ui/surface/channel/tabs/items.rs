use crate::ui::surface::{
    slack_icon, slack_palette, PreparedSlackConversationTab, SlackConversationTabAction,
    SlackMainTab, SlackRailView, SlackShellIcon, SurfaceState, SLACK_CONVERSATION_TAB_MORE_KEY,
};
use crate::ui::{
    div, px, rgb, Context, Div, FluentBuilder, FocusHandle, FontWeight, InteractiveElement,
    KeyDownEvent, ParentElement, StatefulInteractiveElement, Styled, Window, SLACK_TABS_HEIGHT,
};
use gpui::{Role, SharedString, Stateful};

use super::{
    SLACK_CONVERSATION_TABS_MORE_HORIZONTAL_PADDING, SLACK_CONVERSATION_TABS_MORE_ICON_SIZE,
    SLACK_CONVERSATION_TAB_FONT_SIZE, SLACK_CONVERSATION_TAB_GAP,
    SLACK_CONVERSATION_TAB_HORIZONTAL_PADDING, SLACK_CONVERSATION_TAB_ICON_SIZE,
    SLACK_CONVERSATION_TAB_LINE_HEIGHT,
};

#[derive(Clone, Copy)]
struct SlackConversationTabVisual {
    color: u32,
    underline: u32,
    hover: u32,
}

#[derive(Clone, Copy)]
struct SlackConversationTabButtonState {
    active: bool,
    tab_stop: bool,
    enabled: bool,
    visual: SlackConversationTabVisual,
}

#[derive(Clone, Copy)]
struct SlackConversationTabsMoreState {
    width: f32,
    active: bool,
    expanded: bool,
    tab_stop: bool,
    visual: SlackConversationTabVisual,
}

impl SurfaceState {
    pub(super) fn render_slack_conversation_tab(
        &self,
        spec: &PreparedSlackConversationTab,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let active = self.slack_conversation_tab_is_active(spec);
        let visual = self.slack_conversation_tab_visual(active);
        let key = spec.key.clone();
        let click_key = key.clone();
        let key_action = spec.action.clone();
        let click_action = key_action.clone();
        let focus_handle = self
            .slack_conversation_tab_focus_handles
            .get(&key)
            .expect("prepared Slack conversation tab requires a focus handle");
        let button = slack_conversation_tab_button(
            spec,
            SlackConversationTabButtonState {
                active,
                tab_stop: self.slack_conversation_tab_roving_key == key,
                enabled: spec.enabled,
                visual,
            },
            focus_handle,
            cx,
        );
        if !spec.enabled {
            return button.on_key_down(cx.listener(move |this, event, window, cx| {
                this.handle_slack_conversation_tab_roving_key(key.as_ref(), event, window, cx);
            }));
        }
        button
            .on_click(cx.listener(move |this, _, _, cx| {
                this.slack_conversation_tab_roving_key = click_key.clone();
                this.activate_slack_conversation_tab_action(click_action.clone(), cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if this.handle_slack_conversation_tab_roving_key(key.as_ref(), event, window, cx) {
                    return;
                }
                if slack_conversation_tab_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.activate_slack_conversation_tab_action(key_action.clone(), cx);
                }
            }))
    }

    pub(super) fn render_slack_conversation_tabs_more(
        &self,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let active = self
            .slack_conversation_tabs_cache
            .tabs
            .iter()
            .skip(self.slack_conversation_tabs_cache.visible_count)
            .any(|tab| self.slack_conversation_tab_is_active(tab));
        let visual = self.slack_conversation_tab_visual(active);
        slack_conversation_tabs_more_button(
            SlackConversationTabsMoreState {
                width: self.slack_conversation_tabs_cache.more_width,
                active,
                expanded: self.slack_conversation_tabs_overflow_open,
                tab_stop: self.slack_conversation_tab_roving_key.as_ref()
                    == SLACK_CONVERSATION_TAB_MORE_KEY,
                visual,
            },
            &self.slack_conversation_tabs_more_focus_handle,
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.slack_conversation_tab_roving_key = SLACK_CONVERSATION_TAB_MORE_KEY.into();
            this.toggle_slack_conversation_tabs_overflow(cx);
        }))
        .on_key_down(cx.listener(|this, event, window, cx| {
            if this.handle_slack_conversation_tab_roving_key(
                SLACK_CONVERSATION_TAB_MORE_KEY,
                event,
                window,
                cx,
            ) {
                return;
            }
            if slack_conversation_tab_action_key(event) {
                window.prevent_default();
                cx.stop_propagation();
                this.toggle_slack_conversation_tabs_overflow(cx);
            }
        }))
    }

    pub(super) fn slack_conversation_tab_is_active(
        &self,
        spec: &PreparedSlackConversationTab,
    ) -> bool {
        match spec.action.as_ref() {
            SlackConversationTabAction::Messages => {
                self.slack_active_tab == SlackMainTab::Messages
                    && matches!(
                        self.slack_active_rail_view,
                        SlackRailView::Home | SlackRailView::Dms
                    )
            }
            SlackConversationTabAction::AddCanvas => false,
            SlackConversationTabAction::CanvasLink { .. } => false,
            SlackConversationTabAction::BookmarkFolder(tab) => {
                self.slack_active_tab == SlackMainTab::BookmarkFolder
                    && self
                        .slack_active_bookmark_folder
                        .as_ref()
                        .is_some_and(|identity| identity.matches_tab(tab))
            }
            SlackConversationTabAction::Files => {
                self.slack_active_tab == SlackMainTab::FilesLinks
                    && self.slack_active_rail_view == SlackRailView::Home
            }
            SlackConversationTabAction::Pins => {
                self.slack_active_tab == SlackMainTab::Pins
                    && self.slack_active_rail_view == SlackRailView::Home
            }
        }
    }

    fn slack_conversation_tab_visual(&self, active: bool) -> SlackConversationTabVisual {
        let palette = slack_palette(self.appearance_mode);
        let (underline, hover) = match self.appearance_mode {
            crate::ui::AppearanceMode::Dark => (0xe5e5e5, 0x25272b),
            crate::ui::AppearanceMode::Light => (palette.main_text, 0xf4f4f4),
        };
        SlackConversationTabVisual {
            color: if active {
                palette.sidebar_header_text
            } else {
                palette.main_secondary_text
            },
            underline,
            hover,
        }
    }

    fn handle_slack_conversation_tab_roving_key(
        &mut self,
        current_key: &str,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.keystroke.modifiers.modified() {
            return false;
        }
        let visible_count = self.slack_conversation_tabs_cache.visible_count;
        let has_overflow = self.slack_conversation_tabs_cache.has_overflow();
        let item_count = visible_count + usize::from(has_overflow);
        let Some(current_index) =
            self.slack_conversation_tab_roving_index(current_key, visible_count, has_overflow)
        else {
            return false;
        };
        let next_index = match event.keystroke.key.as_str() {
            "left" | "arrowleft" => (current_index + item_count - 1) % item_count,
            "right" | "arrowright" => (current_index + 1) % item_count,
            "home" => 0,
            "end" => item_count - 1,
            _ => return false,
        };
        let (next_key, focus_handle) =
            self.slack_conversation_tab_roving_target(next_index, visible_count, has_overflow);
        self.slack_conversation_tab_roving_key = next_key;
        window.prevent_default();
        cx.stop_propagation();
        window.focus(&focus_handle, cx);
        cx.notify();
        true
    }

    fn slack_conversation_tab_roving_index(
        &self,
        current_key: &str,
        visible_count: usize,
        has_overflow: bool,
    ) -> Option<usize> {
        if visible_count == 0 && !has_overflow {
            return None;
        }
        if current_key == SLACK_CONVERSATION_TAB_MORE_KEY && has_overflow {
            return Some(visible_count);
        }
        self.slack_conversation_tabs_cache
            .tabs
            .iter()
            .take(visible_count)
            .position(|tab| tab.key.as_ref() == current_key)
    }

    fn slack_conversation_tab_roving_target(
        &self,
        next_index: usize,
        visible_count: usize,
        has_overflow: bool,
    ) -> (SharedString, FocusHandle) {
        if has_overflow && next_index == visible_count {
            return (
                SharedString::from(SLACK_CONVERSATION_TAB_MORE_KEY),
                self.slack_conversation_tabs_more_focus_handle.clone(),
            );
        }
        let tab = &self.slack_conversation_tabs_cache.tabs[next_index];
        (
            tab.key.clone(),
            self.slack_conversation_tab_focus_handles
                .get(&tab.key)
                .expect("visible Slack conversation tab requires a focus handle")
                .clone(),
        )
    }
}

fn slack_conversation_tab_button(
    spec: &PreparedSlackConversationTab,
    state: SlackConversationTabButtonState,
    focus_handle: &FocusHandle,
    cx: &mut Context<SurfaceState>,
) -> Stateful<Div> {
    let SlackConversationTabButtonState {
        active,
        tab_stop,
        enabled,
        visual,
    } = state;
    div()
        .id(spec.element_id.clone())
        .role(Role::Tab)
        .aria_label(if enabled {
            spec.label.clone()
        } else {
            format!("{}, unavailable", spec.label).into()
        })
        .aria_selected(active)
        .track_focus(focus_handle)
        .focusable()
        .tab_stop(tab_stop)
        .relative()
        .w(px(spec.width))
        .h(px(SLACK_TABS_HEIGHT))
        .flex_none()
        .px(px(SLACK_CONVERSATION_TAB_HORIZONTAL_PADDING))
        .rounded(px(6.0))
        .when(enabled, |this| {
            this.cursor_pointer()
                .hover(move |style| style.bg(rgb(visual.hover)))
                .focus_visible(move |style| style.bg(rgb(visual.hover)))
        })
        .flex()
        .items_center()
        .gap(px(SLACK_CONVERSATION_TAB_GAP))
        .text_size(px(SLACK_CONVERSATION_TAB_FONT_SIZE))
        .line_height(px(SLACK_CONVERSATION_TAB_LINE_HEIGHT))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(visual.color))
        .when(active, |this| {
            this.child(slack_conversation_tab_underline(visual.underline))
        })
        .child(slack_conversation_tab_icon(spec, visual.color, cx))
        .child(slack_conversation_tab_label(spec))
}

fn slack_conversation_tabs_more_button(
    state: SlackConversationTabsMoreState,
    focus_handle: &FocusHandle,
    cx: &mut Context<SurfaceState>,
) -> Stateful<Div> {
    let SlackConversationTabsMoreState {
        width,
        active,
        expanded,
        tab_stop,
        visual,
    } = state;
    div()
        .id("slack-conversation-tabs-more")
        .role(Role::Tab)
        .aria_label("More conversation views")
        .aria_selected(active)
        .aria_expanded(expanded)
        .track_focus(focus_handle)
        .focusable()
        .tab_stop(tab_stop)
        .relative()
        .w(px(width))
        .h(px(SLACK_TABS_HEIGHT))
        .flex_none()
        .px(px(SLACK_CONVERSATION_TABS_MORE_HORIZONTAL_PADDING))
        .rounded(px(6.0))
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(visual.hover)))
        .focus_visible(move |style| style.bg(rgb(visual.hover)))
        .flex()
        .items_center()
        .gap(px(SLACK_CONVERSATION_TAB_GAP))
        .text_size(px(SLACK_CONVERSATION_TAB_FONT_SIZE))
        .line_height(px(SLACK_CONVERSATION_TAB_LINE_HEIGHT))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(visual.color))
        .when(active, |this| {
            this.child(slack_conversation_tab_underline(visual.underline))
        })
        .child("More")
        .child(slack_icon(
            SlackShellIcon::ChevronDown,
            visual.color,
            SLACK_CONVERSATION_TABS_MORE_ICON_SIZE,
            cx,
        ))
}

fn slack_conversation_tab_underline(color: u32) -> Div {
    div()
        .absolute()
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .h(px(2.0))
        .bg(rgb(color))
}

fn slack_conversation_tab_icon(
    spec: &PreparedSlackConversationTab,
    color: u32,
    cx: &mut Context<SurfaceState>,
) -> Div {
    div()
        .flex_none()
        .size(px(SLACK_CONVERSATION_TAB_ICON_SIZE))
        .child(slack_icon(
            spec.icon,
            color,
            SLACK_CONVERSATION_TAB_ICON_SIZE,
            cx,
        ))
}

fn slack_conversation_tab_label(spec: &PreparedSlackConversationTab) -> Div {
    div()
        .min_w(px(0.0))
        .overflow_hidden()
        .truncate()
        .child(spec.label.clone())
}

fn slack_conversation_tab_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
