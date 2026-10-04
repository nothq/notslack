use super::{
    div, list, px, rgb, slack_icon, slack_palette, AnyElement, Context, Div, FluentBuilder,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ListSizingBehavior, ParentElement,
    SlackPalette, SlackShellIcon, SlackThreadPanelState, StatefulInteractiveElement, Styled,
    SurfaceState,
};
use crate::ui::surface::{
    SlackMessageDocumentPosition, SlackMessageRenderContext, SlackMessageRow, SlackThreadListRow,
};
use gpui::{Role, SharedString};
use gpui_components::selectable_text::SelectableTextDocument;

const SLACK_THREAD_PANEL_WIDTH: f32 = 415.0;

impl SurfaceState {
    pub(crate) fn render_slack_thread_panel(
        &self,
        panel: &SlackThreadPanelState,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);

        div()
            .w(px(SLACK_THREAD_PANEL_WIDTH))
            .h_full()
            .flex_none()
            .border_l_1()
            .border_color(rgb(palette.sidebar_border))
            .bg(rgb(palette.main_bg))
            .flex()
            .flex_col()
            .child(self.render_slack_thread_panel_header(panel, &palette, cx))
            .child(self.render_slack_thread_reply_region(panel, &palette, cx))
    }

    fn render_slack_thread_reply_region(
        &self,
        panel: &SlackThreadPanelState,
        palette: &SlackPalette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if panel.is_cold_loading() {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.0))
                .text_color(rgb(palette.main_secondary_text))
                .child("Loading thread…")
                .into_any_element();
        }
        let palette = *palette;
        let view = cx.entity();
        let team_id = &self
            .slack_workspace
            .as_ref()
            .expect("Slack thread panel requires a workspace")
            .team_id;
        let selection_document_id: SharedString = format!(
            "slack-selection:{team_id}:{}:thread:{}:{}:{}:{}",
            panel.conversation_id,
            panel.parent_message_id,
            panel.generation,
            panel.parent_hydrated,
            panel.reply_rows.len(),
        )
        .into();
        let row_document_id = selection_document_id.clone();
        let list_state = panel.list_state.clone();
        let messages = list(panel.list_state.clone(), move |index, _window, cx| {
            view.update(cx, |this, cx| {
                this.render_slack_thread_list_row(index, &palette, row_document_id.clone(), cx)
            })
        })
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .size_full();
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(
                SelectableTextDocument::new(
                    "slack-thread-selection-document",
                    selection_document_id,
                    messages,
                )
                .on_autoscroll(move |distance, window, _cx| {
                    list_state.scroll_by(distance);
                    window.refresh();
                }),
            )
            .into_any_element()
    }

    fn render_slack_thread_list_row(
        &self,
        index: usize,
        palette: &SlackPalette,
        selection_document_id: SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let panel = self
            .slack_thread_panel
            .as_ref()
            .expect("Slack thread panel disappeared while rendering its list");
        let list_row = *panel
            .list_rows
            .get(index)
            .expect("Slack thread list row index should exist");
        match list_row {
            SlackThreadListRow::Parent => div()
                .px(px(16.0))
                .pt(px(8.0))
                .child(self.render_slack_message_in_document(
                    &panel.parent_row,
                    SlackMessageRenderContext::Thread,
                    SlackMessageDocumentPosition::row(selection_document_id, index),
                    cx,
                ))
                .into_any_element(),
            SlackThreadListRow::ReplyDivider => self
                .render_slack_thread_reply_divider(panel.reply_label.clone(), palette)
                .into_any_element(),
            SlackThreadListRow::Reply(reply_index) => div()
                .px(px(16.0))
                .child(self.render_slack_message_in_document(
                    slack_thread_reply_row(panel, reply_index),
                    SlackMessageRenderContext::Thread,
                    SlackMessageDocumentPosition::row(selection_document_id, index),
                    cx,
                ))
                .into_any_element(),
            SlackThreadListRow::Loading => self
                .render_slack_thread_loading_footer(palette)
                .into_any_element(),
            SlackThreadListRow::Error => self
                .render_slack_thread_error(
                    panel
                        .error
                        .as_deref()
                        .expect("Slack thread error row requires an error"),
                    palette,
                    true,
                    cx,
                )
                .into_any_element(),
            SlackThreadListRow::Composer => self
                .render_slack_thread_composer(panel, cx)
                .into_any_element(),
        }
    }

    fn render_slack_thread_reply_divider(
        &self,
        reply_label: gpui::SharedString,
        palette: &SlackPalette,
    ) -> Div {
        div()
            .h(px(24.0))
            .flex_none()
            .px(px(16.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .flex_none()
                    .text_size(px(13.0))
                    .text_color(rgb(palette.main_secondary_text))
                    .child(reply_label),
            )
            .child(div().h(px(1.0)).flex_grow(1.0).bg(rgb(palette.main_border)))
    }

    fn render_slack_thread_loading_footer(&self, palette: &SlackPalette) -> Div {
        div()
            .h(px(36.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .text_color(rgb(palette.main_secondary_text))
            .child("Loading more replies…")
    }

    fn render_slack_thread_error(
        &self,
        error: &str,
        palette: &SlackPalette,
        compact: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .when(!compact, |this| this.flex_grow(1.0).min_h(px(0.0)))
            .when(compact, |this| this.flex_none())
            .px(px(16.0))
            .py(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.0))
            .child(
                div()
                    .min_w(px(0.0))
                    .text_size(px(13.0))
                    .text_color(rgb(palette.main_secondary_text))
                    .child(error.to_string()),
            )
            .child(
                div()
                    .id("slack-thread-retry")
                    .role(Role::Button)
                    .aria_label("Retry loading Slack thread")
                    .focusable()
                    .tab_stop(true)
                    .h(px(30.0))
                    .px(px(12.0))
                    .flex_none()
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(rgb(palette.composer_chip_border))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(palette.composer_chip_bg)))
                    .focus_visible(|style| style.bg(rgb(palette.composer_chip_bg)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.retry_slack_thread_load(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            cx.stop_propagation();
                            this.retry_slack_thread_load(cx);
                        }
                    }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(palette.main_text))
                    .child("Retry"),
            )
    }

    fn render_slack_thread_panel_header(
        &self,
        panel: &SlackThreadPanelState,
        palette: &SlackPalette,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .h(px(49.0))
            .flex_none()
            .pl(px(16.0))
            .pr(px(12.0))
            .border_b_1()
            .border_color(rgb(palette.sidebar_border))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child("Thread"),
            )
            .child(
                div()
                    .id(format!("slack-thread-close-{}", panel.parent_message_id))
                    .role(Role::Button)
                    .aria_label(format!("Close thread in {}", panel.conversation_name))
                    .focusable()
                    .tab_stop(true)
                    .size(px(28.0))
                    .rounded(px(6.0))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(palette.composer_chip_bg)))
                    .focus_visible(|style| style.bg(rgb(palette.composer_chip_bg)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close_slack_thread_panel(cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            cx.stop_propagation();
                            this.close_slack_thread_panel(cx);
                        }
                    }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(
                        SlackShellIcon::Close,
                        palette.main_secondary_text,
                        20.0,
                        cx,
                    )),
            )
    }
}

fn slack_thread_reply_row(panel: &SlackThreadPanelState, reply_index: usize) -> &SlackMessageRow {
    panel
        .reply_rows
        .get(reply_index)
        .expect("Slack thread reply row index should exist")
}
