use super::super::{
    alpha, div, list, px, rgb, AnyElement, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, ListSizingBehavior, ParentElement, SlackDraftsSentRow,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::SlackDraftsSentTab;
use gpui::Role;

mod empty;
mod item;

const SLACK_DRAFTS_SENT_HEADER_HEIGHT: f32 = 49.0;
const SLACK_DRAFTS_SENT_TABS_HEIGHT: f32 = 36.0;
const SLACK_DRAFTS_SENT_EMPTY_WIDTH: f32 = 420.0;

impl SurfaceState {
    pub(super) fn render_slack_drafts_sent_surface(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(0x222529))
            .child(self.render_slack_drafts_sent_header())
            .child(self.render_slack_drafts_sent_tabs(cx))
            .child(self.render_slack_drafts_sent_content(cx))
    }

    fn render_slack_drafts_sent_header(&self) -> Div {
        div()
            .h(px(SLACK_DRAFTS_SENT_HEADER_HEIGHT))
            .flex_none()
            .px(px(18.0))
            .flex()
            .items_center()
            .text_size(px(18.0))
            .line_height(px(24.0))
            .font_weight(FontWeight::BLACK)
            .text_color(rgb(0xd1d2d3))
            .child("Drafts & sent")
    }

    fn render_slack_drafts_sent_tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("slack-drafts-sent-tabs")
            .role(Role::TabList)
            .aria_label("Drafts and sent message views")
            .h(px(SLACK_DRAFTS_SENT_TABS_HEIGHT))
            .flex_none()
            .px(px(24.0))
            .bg(rgb(0x222529))
            .flex()
            .items_end()
            .gap(px(28.0))
            .children(
                SlackDraftsSentTab::ALL
                    .into_iter()
                    .map(|tab| self.render_slack_drafts_sent_tab(tab, cx)),
            )
    }

    fn render_slack_drafts_sent_tab(
        &self,
        tab: SlackDraftsSentTab,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.slack_drafts_sent_tab == tab;
        let label = tab.label();
        div()
            .id(format!(
                "slack-drafts-sent-tab-{}",
                label.to_ascii_lowercase()
            ))
            .role(Role::Tab)
            .aria_label(label)
            .aria_selected(selected)
            .focusable()
            .tab_stop(true)
            .h_full()
            .relative()
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_drafts_sent_tab(tab, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if drafts_sent_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.select_slack_drafts_sent_tab(tab, cx);
                }
            }))
            .flex()
            .items_center()
            .text_size(px(13.0))
            .line_height(px(18.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(if selected { 0xf8f8f8 } else { 0xb9babd }))
            .child(label)
            .when(selected, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(0.0))
                        .right(px(0.0))
                        .bottom(px(0.0))
                        .h(px(2.0))
                        .bg(rgb(0xe5e5e5)),
                )
            })
    }

    fn render_slack_drafts_sent_content(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.slack_drafts_sent_rows.is_empty() {
            if self.slack_drafts_sent_loading {
                return self.render_slack_drafts_sent_loading().into_any_element();
            }
            if let Some(error) = self.slack_drafts_sent_error.as_deref() {
                return self
                    .render_slack_drafts_sent_error(error, cx)
                    .into_any_element();
            }
            return self.render_slack_drafts_sent_empty(cx).into_any_element();
        }
        if let Some(error) = self.slack_drafts_sent_error.as_deref() {
            return div()
                .flex_grow(1.0)
                .min_h(px(0.0))
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex_none()
                        .min_h(px(36.0))
                        .px(px(16.0))
                        .py(px(8.0))
                        .bg(rgb(0x3d1f24))
                        .text_size(px(13.0))
                        .line_height(px(18.0))
                        .text_color(rgb(0xf2c0bd))
                        .child(error.to_string()),
                )
                .child(self.render_slack_drafts_sent_list(cx))
                .into_any_element();
        }
        self.render_slack_drafts_sent_list(cx)
    }

    fn render_slack_drafts_sent_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let background = if self.slack_drafts_sent_tab == SlackDraftsSentTab::Sent {
            0x222529
        } else {
            0x1a1d21
        };
        div()
            .id("slack-drafts-sent-list")
            .role(Role::ListBox)
            .aria_label(self.slack_drafts_sent_tab.label())
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_hidden()
            .bg(rgb(background))
            .child(
                list(
                    self.slack_drafts_sent_list_state.clone(),
                    move |index, _window, cx| {
                        view.update(cx, |this, cx| {
                            let row = this
                                .slack_drafts_sent_rows
                                .get(index)
                                .expect("Slack Drafts & sent row index must exist");
                            this.render_slack_drafts_sent_row(row, index, cx)
                        })
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .size_full(),
            )
            .into_any_element()
    }

    fn render_slack_drafts_sent_row(
        &self,
        row: &SlackDraftsSentRow,
        index: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match row {
            SlackDraftsSentRow::DateDivider { element_id, label } => div()
                .id(element_id.clone())
                .h(px(row.height()))
                .mx(px(20.0))
                .px(px(16.0))
                .pt(px(24.0))
                .pb(px(12.0))
                .text_size(px(13.0))
                .line_height(px(18.0))
                .font_weight(FontWeight::BOLD)
                .text_color(alpha(0xe8e8e8, 0.7))
                .child(label.clone())
                .into_any_element(),
            SlackDraftsSentRow::Item(item) => self.render_slack_drafts_sent_item(item, index, cx),
        }
    }
}

fn drafts_sent_action_key(event: &gpui::KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
