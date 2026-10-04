use super::super::{
    alpha, div, px, relative, rgb, slack_activity_palette, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, ParentElement, SlackActivityFilter, SlackActivityPalette,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use gpui::Role;

mod detail;
mod detail_message;
mod rows;
mod states;
mod toolbar;

const SLACK_ACTIVITY_HEADER_HEIGHT: f32 = 49.0;
const SLACK_ACTIVITY_TABS_HEIGHT: f32 = 38.0;
const SLACK_ACTIVITY_LIST_TOP_GAP: f32 = 8.0;

impl SurfaceState {
    pub(super) fn render_slack_activity_surface(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_activity_palette(self.appearance_mode);
        div()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .bg(rgb(palette.surface_bg))
            .child(self.render_slack_activity_feed_pane(cx))
            .child(self.render_slack_activity_detail_pane(cx))
    }

    fn render_slack_activity_feed_pane(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(relative(0.5))
            .h_full()
            .min_w(px(0.0))
            .flex_none()
            .flex()
            .flex_col()
            .child(self.render_slack_activity_header(cx))
            .child(self.render_slack_activity_tabs(cx))
            .child(self.render_slack_activity_toolbar(cx))
            .child(self.render_slack_activity_feed(cx))
    }

    fn render_slack_activity_header(&self, _cx: &mut Context<Self>) -> Div {
        let palette = slack_activity_palette(self.appearance_mode);
        div()
            .h(px(SLACK_ACTIVITY_HEADER_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .flex()
            .items_center()
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.primary_text))
                    .child("Activity"),
            )
    }

    fn render_slack_activity_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = slack_activity_palette(self.appearance_mode);
        div()
            .id("slack-activity-tabs")
            .role(Role::TabList)
            .aria_label("Activity filters")
            .h(px(SLACK_ACTIVITY_TABS_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .border_b_1()
            .border_color(alpha(palette.card_border, palette.card_border_alpha))
            .flex()
            .items_end()
            .gap(px(18.0))
            .children(
                SlackActivityFilter::ALL
                    .into_iter()
                    .map(|filter| self.render_slack_activity_tab(filter, cx)),
            )
    }

    fn render_slack_activity_tab(
        &self,
        filter: SlackActivityFilter,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.slack_activity_filter == filter;
        let count = self.slack_activity_filter_count(filter);
        let palette = slack_activity_palette(self.appearance_mode);
        div()
            .id(format!(
                "slack-activity-tab-{}",
                filter.label().to_ascii_lowercase()
            ))
            .role(Role::Tab)
            .aria_label(if count > 0 {
                format!("{} activity, {count} unread", filter.label())
            } else {
                format!("{} activity", filter.label())
            })
            .aria_selected(selected)
            .focusable()
            .tab_stop(true)
            .h_full()
            .relative()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_activity_filter(filter, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if activity_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.select_slack_activity_filter(filter, cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(7.0))
            .text_size(px(14.0))
            .font_weight(if selected {
                FontWeight::BOLD
            } else {
                FontWeight::MEDIUM
            })
            .text_color(rgb(if selected {
                palette.primary_text
            } else {
                palette.tertiary_text
            }))
            .child(filter.label())
            .when(count > 0, |this| {
                this.child(slack_activity_tab_count(count, selected, palette))
            })
            .when(selected, |this| {
                this.child(slack_activity_tab_indicator(palette))
            })
    }

    fn render_slack_activity_feed(&self, cx: &mut Context<Self>) -> Div {
        let content = if self.slack_activity_rows.is_empty() && self.slack_activity_loading {
            self.render_slack_activity_skeleton()
        } else if self.slack_activity_rows.is_empty() {
            if let Some(error) = self.slack_activity_error.as_deref() {
                self.render_slack_activity_error(error, cx)
            } else {
                self.render_slack_activity_empty()
            }
        } else {
            self.render_slack_activity_list(cx)
        };
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .pt(px(SLACK_ACTIVITY_LIST_TOP_GAP))
            .child(content)
    }
}

fn slack_activity_tab_count(count: u32, selected: bool, palette: SlackActivityPalette) -> Div {
    div()
        .min_w(px(20.0))
        .h(px(20.0))
        .px(px(6.0))
        .rounded_full()
        .when(selected, |this| this.bg(rgb(palette.importance_bg)))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(if selected {
            palette.importance_text
        } else {
            palette.tertiary_text
        }))
        .child(count.to_string())
}

fn slack_activity_tab_indicator(palette: SlackActivityPalette) -> Div {
    div()
        .absolute()
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .h(px(2.0))
        .bg(rgb(palette.primary_text))
}

fn activity_action_key(event: &gpui::KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
