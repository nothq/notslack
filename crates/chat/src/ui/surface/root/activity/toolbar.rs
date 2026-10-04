use super::activity_action_key;
use crate::ui::surface::{
    alpha, div, px, rgb, slack_activity_palette, slack_icon, slack_palette, Context, Div,
    FluentBuilder, FontWeight, InteractiveElement, IntoElement, ParentElement, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use gpui::Role;

const SLACK_ACTIVITY_TOOLBAR_TOP_GAP: f32 = 16.0;
const SLACK_ACTIVITY_TOOLBAR_HEIGHT: f32 = 32.0;

impl SurfaceState {
    pub(super) fn render_slack_activity_toolbar(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_activity_palette(self.appearance_mode);
        let mutation_pending = self.slack_activity_mutation_is_pending();
        let mutation_error = (!mutation_pending)
            .then(|| self.slack_activity_mutation_error.clone())
            .flatten();
        let has_mutation_error = mutation_error.is_some();
        let has_rows = !self.slack_activity_rows.is_empty();
        let show_loading_more =
            !mutation_pending && !has_mutation_error && self.slack_activity_loading && has_rows;
        let show_page_retry = !mutation_pending
            && !has_mutation_error
            && self.slack_activity_error.is_some()
            && has_rows;
        div()
            .h(px(
                SLACK_ACTIVITY_TOOLBAR_TOP_GAP + SLACK_ACTIVITY_TOOLBAR_HEIGHT
            ))
            .flex_none()
            .pt(px(SLACK_ACTIVITY_TOOLBAR_TOP_GAP))
            .px(px(16.0))
            .flex()
            .items_center()
            .justify_between()
            .child(self.render_slack_activity_toolbar_actions(cx))
            .when(mutation_pending, |this| {
                this.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(palette.tertiary_text))
                        .child("Updating Activity…"),
                )
            })
            .when_some(mutation_error, |this, error| {
                this.child(
                    div()
                        .max_w(px(210.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(12.0))
                        .text_color(rgb(palette.error_text))
                        .child(error),
                )
            })
            .when(show_loading_more, |this| {
                this.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(palette.tertiary_text))
                        .child("Loading more…"),
                )
            })
            .when(show_page_retry, |this| {
                this.child(self.render_slack_activity_page_retry(cx))
            })
    }

    fn render_slack_activity_toolbar_actions(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(self.render_slack_activity_unread_button(cx))
            .child(self.render_slack_activity_search_button(cx))
    }

    fn render_slack_activity_page_retry(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = slack_activity_palette(self.appearance_mode);
        div()
            .id("slack-activity-page-retry")
            .role(Role::Button)
            .aria_label("Retry loading more Slack activity")
            .focusable()
            .tab_stop(true)
            .h(px(28.0))
            .px(px(10.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(alpha(palette.card_border, palette.card_border_alpha))
            .bg(rgb(palette.card_bg))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
                this.retry_slack_activity(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if activity_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.retry_slack_activity(cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(8.0))
            .text_size(px(12.0))
            .text_color(rgb(palette.error_text))
            .child("Couldn’t load more")
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.primary_text))
                    .child("Retry"),
            )
    }

    fn render_slack_activity_unread_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.slack_activity_unread_only;
        let palette = slack_palette(self.appearance_mode);
        let activity_palette = slack_activity_palette(self.appearance_mode);
        div()
            .id("slack-activity-unreads")
            .role(Role::Button)
            .aria_label(if selected {
                "Show all activity"
            } else {
                "Show unread activity"
            })
            .aria_selected(selected)
            .focusable()
            .tab_stop(true)
            .h(px(SLACK_ACTIVITY_TOOLBAR_HEIGHT))
            .px(px(12.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(alpha(
                activity_palette.card_border,
                activity_palette.card_border_alpha,
            ))
            .bg(rgb(activity_palette.surface_bg))
            .cursor_pointer()
            .hover(move |style| {
                style.bg(alpha(
                    activity_palette.interaction_hover,
                    activity_palette.interaction_hover_alpha,
                ))
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_activity_unread_only(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if activity_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_activity_unread_only(cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(7.0))
            .text_size(px(14.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(activity_palette.primary_text))
            .child(slack_activity_unread_checkbox(
                selected,
                activity_palette.tertiary_text,
                palette.link,
            ))
            .child("Unreads")
    }

    fn render_slack_activity_search_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = slack_activity_palette(self.appearance_mode);
        div()
            .id("slack-activity-search")
            .role(Role::Button)
            .aria_label("Search Slack messages")
            .focusable()
            .tab_stop(true)
            .size(px(SLACK_ACTIVITY_TOOLBAR_HEIGHT))
            .rounded(px(8.0))
            .border_1()
            .border_color(alpha(palette.card_border, palette.card_border_alpha))
            .cursor_pointer()
            .hover(move |style| {
                style.bg(alpha(
                    palette.interaction_hover,
                    palette.interaction_hover_alpha,
                ))
            })
            .on_click(cx.listener(|this, _, _, cx| {
                this.open_slack_search(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if activity_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_search(cx);
                }
            }))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::Search,
                palette.primary_text,
                18.0,
                cx,
            ))
    }
}

fn slack_activity_unread_checkbox(selected: bool, border_color: u32, selected_bg: u32) -> Div {
    div()
        .size(px(14.0))
        .rounded(px(3.0))
        .border_1()
        .border_color(rgb(border_color))
        .when(selected, |this| {
            this.bg(rgb(selected_bg))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.0))
                .text_color(rgb(0xffffff))
                .child("✓")
        })
}
