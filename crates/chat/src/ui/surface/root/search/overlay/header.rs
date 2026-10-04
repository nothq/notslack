use super::super::super::super::{
    div, px, rgb, slack_icon, Context, Div, FluentBuilder, InteractiveElement, ParentElement,
    SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};
use super::{
    super::{consume_slack_search_action_key, slack_search_overlay_palette},
    SLACK_SEARCH_DIALOG_RIGHT_LANE_WIDTH, SLACK_SEARCH_HEADER_HEIGHT,
};
use gpui::Role;

impl SurfaceState {
    pub(super) fn render_slack_search_dialog_header(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        div()
            .h(px(SLACK_SEARCH_HEADER_HEIGHT))
            .flex_none()
            .border_b_1()
            .border_color(rgb(palette.border))
            .flex()
            .items_center()
            .child(slack_search_header_icon(palette.secondary_text, cx))
            .child(
                div()
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .h_full()
                    .mr(px(8.0))
                    .flex()
                    .flex_col()
                    .justify_center()
                    .child(self.slack_search_input_entity(cx)),
            )
            .when(!self.slack_search_query.trim().is_empty(), |this| {
                this.child(self.render_slack_search_clear(
                    palette.hover_bg,
                    palette.secondary_text,
                    cx,
                ))
            })
            .child(
                div()
                    .w(px(SLACK_SEARCH_DIALOG_RIGHT_LANE_WIDTH))
                    .h_full()
                    .flex_none(),
            )
    }

    fn render_slack_search_clear(
        &self,
        hover_background: u32,
        text_color: u32,
        cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        div()
            .id("slack-search-clear")
            .role(Role::Button)
            .aria_label("Clear search")
            .focusable()
            .tab_stop(true)
            .h_full()
            .w(px(63.0))
            .flex_none()
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(hover_background)))
            .focus_visible(move |style| style.bg(rgb(hover_background)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.clear_slack_search(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if consume_slack_search_action_key(event, window, cx) {
                    this.clear_slack_search(cx);
                }
            }))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .text_color(rgb(text_color))
            .child("Clear")
    }
}

fn slack_search_header_icon(color: u32, cx: &mut Context<SurfaceState>) -> Div {
    div()
        .w(px(56.0))
        .h_full()
        .pl(px(20.0))
        .flex_none()
        .flex()
        .items_center()
        .child(slack_icon(SlackShellIcon::Search, color, 20.0, cx))
}
