use super::super::super::{
    alpha, div, px, relative, rgb, slack_icon, Context, Div, InteractiveElement, MouseButton,
    MouseDownEvent, ParentElement, SlackShellIcon, StatefulInteractiveElement, Styled,
    SurfaceState, SLACK_TOP_SEARCH_HEIGHT, SLACK_TOP_SEARCH_MAX_WIDTH, SLACK_TOP_SEARCH_MIN_WIDTH,
};
use gpui::Role;

impl SurfaceState {
    pub(super) fn render_slack_history_search(&self, cx: &mut Context<Self>) -> Div {
        let search = slack_history_search_box();
        if self.slack_search_open || !self.slack_workspace_api_capabilities.search_messages {
            return search;
        }
        if self.slack_search_results_open {
            return self.render_slack_history_committed_search(search, cx);
        }
        self.render_slack_history_idle_search(search, cx)
    }

    fn render_slack_history_committed_search(&self, search: Div, cx: &mut Context<Self>) -> Div {
        search
            .rounded(px(6.0))
            .bg(alpha(0xf6f6f6, 0.16))
            .flex()
            .items_center()
            .child(self.render_slack_history_committed_query(cx))
            .child(self.render_slack_history_committed_clear(cx))
    }

    fn render_slack_history_committed_query(
        &self,
        cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        div()
            .id("slack-search-committed-query")
            .role(Role::Button)
            .aria_label(format!(
                "Edit search for {}",
                self.slack_search_committed_query
            ))
            .focusable()
            .tab_stop(true)
            .flex_grow(1.0)
            .min_w(px(0.0))
            .h_full()
            .cursor_pointer()
            .focus_visible(|style| style.bg(alpha(0xf6f6f6, 0.12)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.open_slack_search(cx);
                }),
            )
            .flex()
            .items_center()
            .child(slack_history_search_icon(cx))
            .child(slack_history_committed_query_label(
                self.slack_search_committed_query.as_ref(),
            ))
    }

    fn render_slack_history_committed_clear(
        &self,
        cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        div()
            .id("slack-search-committed-clear")
            .role(Role::Button)
            .aria_label("Clear search and return to Slack home")
            .focusable()
            .tab_stop(true)
            .size(px(28.0))
            .flex_none()
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xf6f6f6, 0.18)))
            .focus_visible(|style| style.bg(alpha(0xf6f6f6, 0.18)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.clear_slack_search_results(cx);
                }),
            )
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(SlackShellIcon::Close, 0xf8f8f8, 16.0, cx))
    }

    fn render_slack_history_idle_search(&self, search: Div, cx: &mut Context<Self>) -> Div {
        search.child(
            div()
                .id("slack-top-search")
                .role(Role::Button)
                .aria_label("Search Slack messages")
                .focusable()
                .tab_stop(true)
                .size_full()
                .rounded(px(6.0))
                .bg(alpha(0xf6f6f6, 0.16))
                .cursor_pointer()
                .hover(|style| style.bg(alpha(0xf6f6f6, 0.22)))
                .focus_visible(|style| style.bg(alpha(0xf6f6f6, 0.22)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.open_slack_search(cx);
                    }),
                )
                .on_key_down(cx.listener(|this, event, window, cx| {
                    if super::slack_history_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.open_slack_search(cx);
                    }
                }))
                .flex()
                .items_center()
                .child(slack_history_search_icon(cx))
                .child(slack_history_idle_search_label(
                    "Describe what you are looking for",
                )),
        )
    }
}

fn slack_history_search_box() -> Div {
    div()
        .flex_grow(2.0)
        .flex_shrink(1.0)
        .flex_basis(relative(0.0))
        .min_w(px(SLACK_TOP_SEARCH_MIN_WIDTH))
        .max_w(px(SLACK_TOP_SEARCH_MAX_WIDTH))
        .h(px(SLACK_TOP_SEARCH_HEIGHT))
}

fn slack_history_search_icon(cx: &mut Context<SurfaceState>) -> Div {
    div()
        .w(px(35.0))
        .h_full()
        .pl(px(12.0))
        .flex()
        .items_center()
        .child(slack_icon(SlackShellIcon::Search, 0xf8f8f8, 15.0, cx))
}

fn slack_history_committed_query_label(query: &str) -> Div {
    div()
        .flex_grow(1.0)
        .min_w(px(0.0))
        .h_full()
        .flex()
        .items_center()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(13.0))
        .text_color(rgb(0xf8f8f8))
        .child(format!("Search: {query}"))
}

fn slack_history_idle_search_label(label: &'static str) -> Div {
    div()
        .flex_grow(1.0)
        .min_w(px(0.0))
        .h_full()
        .mr(px(5.0))
        .flex()
        .items_center()
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(13.0))
        .line_height(px(18.0))
        .text_color(rgb(0xf8f8f8))
        .child(label)
}
