use super::super::super::{
    alpha, div, img, px, rgb, slack_base_icon_radius, slack_icon, slack_palette, AnyElement,
    Context, FluentBuilder, FontWeight, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, StatefulInteractiveElement, Styled, SurfaceState,
    SLACK_TOP_NAV_LEFT_INSET, SLACK_TOP_NAV_RIGHT_INSET,
};
use super::slack_history_action_key;
use crate::ui::surface::{SlackHistoryEntry, SLACK_TOP_NAV_LEFT_FLEX_BASIS};
use crate::ui::AppearanceMode;
use crate::ui::SLACK_TOP_NAV_RAIL_WIDTH;
use gpui::{point, BoxShadow, Div, Role, Stateful};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

struct SlackHistoryMenuLayout {
    background: u32,
    heading_color: gpui::Hsla,
    left: f32,
    first_index: usize,
}

impl SurfaceState {
    pub(in super::super) fn render_slack_history_menu_layer(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let layout = slack_history_menu_layout(self);
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_history_menu(cx);
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
            .child(self.render_slack_history_menu(layout, cx))
            .into_any_element()
    }

    fn render_slack_history_menu(
        &self,
        layout: SlackHistoryMenuLayout,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id("slack-history-menu")
            .role(Role::Menu)
            .aria_label("History")
            .absolute()
            .left(px(layout.left))
            .top(px(33.0))
            .w(px(360.0))
            .py(px(12.0))
            .rounded(px(4.0))
            .bg(rgb(layout.background))
            .shadow(slack_history_menu_shadow())
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(slack_history_menu_heading(layout.heading_color))
            .children(
                (layout.first_index..self.slack_conversation_history.len())
                    .rev()
                    .map(|index| {
                        self.render_slack_history_menu_item(
                            index,
                            &self.slack_conversation_history[index],
                            cx,
                        )
                    }),
            )
    }

    fn render_slack_history_menu_item(
        &self,
        index: usize,
        entry: &SlackHistoryEntry,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let selected = self.slack_history_menu_selected_index == Some(index);
        div()
            .id(("slack-history-menu-item", index))
            .role(Role::MenuItem)
            .aria_label(entry.accessibility_label.clone())
            .focusable()
            .tab_stop(true)
            .h(px(32.0))
            .px(px(16.0))
            .cursor_pointer()
            .when(selected, |this| {
                this.bg(rgb(0x1264a3)).text_color(rgb(0xffffff))
            })
            .hover(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_history_menu_index(index, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_history_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.activate_slack_history_menu_index(index, cx);
                }
            }))
            .flex()
            .items_center()
            .text_size(px(15.0))
            .line_height(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(palette.main_text))
            .child(self.render_slack_history_menu_icon(entry, palette.main_text, cx))
            .child(slack_history_menu_label(entry))
            .into_any_element()
    }

    fn render_slack_history_menu_icon(
        &self,
        entry: &SlackHistoryEntry,
        text_color: u32,
        cx: &mut Context<Self>,
    ) -> Div {
        let icon = entry
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
            .map_or_else(
                || slack_icon(entry.icon, text_color, 20.0, cx),
                |image| {
                    img(image)
                        .size(px(20.0))
                        .rounded(slack_base_icon_radius(20.0))
                        .into_any_element()
                },
            );
        div()
            .w(px(32.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(icon)
    }
}

fn slack_history_menu_layout(state: &SurfaceState) -> SlackHistoryMenuLayout {
    let available_width = (state.preview_width
        - SLACK_TOP_NAV_RAIL_WIDTH
        - SLACK_TOP_NAV_LEFT_INSET
        - SLACK_TOP_NAV_RIGHT_INSET)
        .max(0.0);
    let unclamped_left = SLACK_TOP_NAV_RAIL_WIDTH
        + SLACK_TOP_NAV_LEFT_INSET
        + available_width * SLACK_TOP_NAV_LEFT_FLEX_BASIS
        - 53.0;
    SlackHistoryMenuLayout {
        background: match state.appearance_mode {
            AppearanceMode::Dark => 0x222529,
            AppearanceMode::Light => 0xffffff,
        },
        heading_color: match state.appearance_mode {
            AppearanceMode::Dark => alpha(0xe8e8e8, 0.7),
            AppearanceMode::Light => alpha(0x1d1c1d, 0.7),
        },
        left: unclamped_left.clamp(4.0, (state.preview_width - 364.0).max(4.0)),
        first_index: state.slack_conversation_history.len().saturating_sub(11),
    }
}

fn slack_history_menu_heading(color: gpui::Hsla) -> Div {
    div()
        .h(px(26.0))
        .px(px(24.0))
        .flex()
        .items_center()
        .text_size(px(13.0))
        .line_height(px(18.0))
        .text_color(color)
        .child("Recent")
}

fn slack_history_menu_label(entry: &SlackHistoryEntry) -> Div {
    div()
        .min_w(px(0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .child(entry.label.clone())
}

fn slack_history_menu_shadow() -> Vec<BoxShadow> {
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
