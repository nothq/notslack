use crate::ui::surface::{
    div, list, px, rgb, slack_icon, slack_palette, AnyElement, Context, Div, FluentBuilder,
    FontWeight, InteractiveElement, IntoElement, ListSizingBehavior, ParentElement,
    SlackMessageDocumentPosition, SlackMessageRenderContext, SlackMessageRow, SlackPalette,
    SlackShellIcon, SlackThreadPanelState, StatefulInteractiveElement, Styled, SurfaceState,
};
use gpui::{Role, SharedString};
use gpui_components::selectable_text::SelectableTextDocument;

use super::helpers::{later_action_key, slack_later_thread_reply_label};

mod loading;

impl SurfaceState {
    pub(super) fn render_slack_later_thread(
        &self,
        panel: &SlackThreadPanelState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let selection_document_id = self.slack_later_thread_selection_document_id(panel);
        let list_state = panel.list_state.clone();
        let selection_content = self.render_slack_later_thread_selection_content(
            panel,
            &palette,
            selection_document_id.clone(),
            cx,
        );
        div()
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .bg(rgb(palette.main_bg))
            .child(self.render_slack_later_detail_header(
                "Thread",
                Some(panel.conversation_name.clone().into()),
                cx,
            ))
            .child(
                SelectableTextDocument::new(
                    "slack-later-thread-selection-document",
                    selection_document_id,
                    selection_content,
                )
                .on_autoscroll(move |distance, window, _cx| {
                    list_state.scroll_by(distance);
                    window.refresh();
                }),
            )
            .into_any_element()
    }

    fn slack_later_thread_selection_document_id(
        &self,
        panel: &SlackThreadPanelState,
    ) -> SharedString {
        let team_id = &self
            .slack_workspace
            .as_ref()
            .expect("Slack Later thread requires a workspace")
            .team_id;
        format!(
            "slack-selection:{team_id}:{}:later-thread:{}:{}:{}:{}",
            panel.conversation_id,
            panel.parent_message_id,
            panel.generation,
            panel.parent_hydrated,
            panel.reply_rows.len(),
        )
        .into()
    }

    fn render_slack_later_thread_selection_content(
        &self,
        panel: &SlackThreadPanelState,
        palette: &SlackPalette,
        selection_document_id: SharedString,
        cx: &mut Context<Self>,
    ) -> Div {
        let selected_message_id = panel.origin.later_selected_message_id();
        let parent_selected = selected_message_id == Some(panel.parent_row.id.as_str());
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(div().flex_none().pt(px(14.0)).child(
                self.render_slack_later_thread_message_in_document(
                    &panel.parent_row,
                    parent_selected,
                    SlackMessageDocumentPosition::row(selection_document_id.clone(), 0),
                    cx,
                ),
            ))
            .when(panel.expected_reply_count > 0, |this| {
                this.child(
                    self.render_slack_later_thread_reply_divider(
                        panel.expected_reply_count,
                        palette,
                    ),
                )
            })
            .child(self.render_slack_later_thread_reply_region(
                panel,
                palette,
                selection_document_id,
                cx,
            ))
            .when(self.can_send_slack_thread_reply(), |this| {
                this.child(self.render_slack_thread_composer(panel, cx))
            })
    }

    fn render_slack_later_thread_reply_region(
        &self,
        panel: &SlackThreadPanelState,
        palette: &SlackPalette,
        selection_document_id: SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if panel.reply_rows.is_empty() {
            return self.render_slack_later_empty_thread_reply_region(panel, palette, cx);
        }
        self.render_slack_later_thread_reply_list(panel, palette, selection_document_id, cx)
    }

    fn render_slack_later_empty_thread_reply_region(
        &self,
        panel: &SlackThreadPanelState,
        palette: &SlackPalette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(error) = panel.error.as_deref() {
            return self
                .render_slack_later_thread_error(error, palette, false, cx)
                .into_any_element();
        }
        if panel.loading {
            return self
                .render_slack_later_thread_loading_skeleton(palette)
                .into_any_element();
        }
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(13.0))
            .text_color(rgb(palette.main_secondary_text))
            .child("No replies yet")
            .into_any_element()
    }

    fn render_slack_later_thread_reply_list(
        &self,
        panel: &SlackThreadPanelState,
        palette: &SlackPalette,
        selection_document_id: SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = cx.entity();
        let row_document_id = selection_document_id;
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(
                list(panel.list_state.clone(), move |index, _window, cx| {
                    view.update(cx, |this, cx| {
                        let panel = this.slack_thread_panel.as_ref().expect(
                            "Later thread panel disappeared while rendering its reply list",
                        );
                        let row = panel
                            .reply_rows
                            .get(index)
                            .expect("Later thread reply index should exist");
                        let selected =
                            panel.origin.later_selected_message_id() == Some(row.id.as_str());
                        this.render_slack_later_thread_message_in_document(
                            row,
                            selected,
                            SlackMessageDocumentPosition::row(row_document_id.clone(), index + 1),
                            cx,
                        )
                    })
                })
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .size_full()
                .pb(px(12.0)),
            )
            .when(panel.loading, |this| {
                this.child(
                    div()
                        .h(px(36.0))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(13.0))
                        .text_color(rgb(palette.main_secondary_text))
                        .child("Loading more replies…"),
                )
            })
            .when_some(panel.error.as_deref(), |this, error| {
                this.child(self.render_slack_later_thread_error(error, palette, true, cx))
            })
            .into_any_element()
    }

    fn render_slack_later_thread_message(
        &self,
        row: &SlackMessageRow,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .w_full()
            .min_w(px(0.0))
            .px(px(16.0))
            .py(px(10.0))
            .when(selected, |this| {
                this.bg(rgb(0x1d2c36)).child(
                    div()
                        .pb(px(8.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .text_size(px(12.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0x7fc8f1))
                        .child(slack_icon(SlackShellIcon::Later, 0x7fc8f1, 14.0, cx))
                        .child("Saved for later"),
                )
            })
            .child(self.render_slack_message(row, SlackMessageRenderContext::Later, cx))
            .into_any_element()
    }

    fn render_slack_later_thread_message_in_document(
        &self,
        row: &SlackMessageRow,
        selected: bool,
        position: SlackMessageDocumentPosition,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .w_full()
            .min_w(px(0.0))
            .px(px(16.0))
            .py(px(10.0))
            .when(selected, |this| {
                this.bg(rgb(0x1d2c36)).child(
                    div()
                        .pb(px(8.0))
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .text_size(px(12.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0x7fc8f1))
                        .child(slack_icon(SlackShellIcon::Later, 0x7fc8f1, 14.0, cx))
                        .child("Saved for later"),
                )
            })
            .child(self.render_slack_message_in_document(
                row,
                SlackMessageRenderContext::Later,
                position,
                cx,
            ))
            .into_any_element()
    }

    pub(super) fn render_slack_later_saved_message(
        &self,
        row: &SlackMessageRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_slack_later_thread_message(row, true, cx)
    }

    fn render_slack_later_thread_reply_divider(
        &self,
        reply_count: u32,
        palette: &SlackPalette,
    ) -> Div {
        div()
            .h(px(38.0))
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
                    .child(slack_later_thread_reply_label(reply_count)),
            )
            .child(div().h(px(1.0)).flex_grow(1.0).bg(rgb(palette.main_border)))
    }

    fn render_slack_later_thread_error(
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
                    .id("slack-later-thread-retry")
                    .role(Role::Button)
                    .aria_label("Retry loading saved message thread")
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
                    .on_key_down(cx.listener(|this, event, _, cx| {
                        if later_action_key(event) {
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
}
