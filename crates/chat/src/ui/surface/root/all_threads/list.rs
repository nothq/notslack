use super::super::super::{
    div, list, px, rgb, slack_palette, AnyElement, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, ListSizingBehavior, ParentElement,
    SlackAllThreadRow, SlackMessageDocumentPosition, SlackMessageRenderContext,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use super::all_threads_action_key;
use crate::ui::surface::channel::slack_members_presence_dot;
use gpui::{Role, SharedString};
use gpui_components::selectable_text::SelectableTextDocument;

impl SurfaceState {
    pub(super) fn render_slack_all_threads_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.entity();
        let selection_document_id = self.slack_all_threads_selection_document_id();
        let row_document_id = selection_document_id.clone();
        let list_state = self.slack_all_threads_list_state.clone();
        let messages = list(
            self.slack_all_threads_list_state.clone(),
            move |index, _window, cx| {
                view.update(cx, |this, cx| {
                    let target = {
                        let row = this
                            .slack_all_threads_rows
                            .get(index)
                            .expect("Slack All Threads row index should exist");
                        this.slack_all_threads_composer_target(row)
                    };
                    this.ensure_slack_all_threads_composer(target, cx);
                    let row = this
                        .slack_all_threads_rows
                        .get(index)
                        .expect("Slack All Threads row index should exist");
                    this.render_slack_all_thread_row(row, index, row_document_id.clone(), cx)
                })
            },
        )
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .size_full();
        div()
            .id("slack-all-threads-list")
            .role(Role::ListBox)
            .aria_label(slack_all_threads_list_label(self))
            .flex_grow(1.0)
            .min_h(px(0.0))
            .relative()
            .child(
                SelectableTextDocument::new(
                    "slack-all-threads-selection-document",
                    selection_document_id,
                    messages,
                )
                .on_autoscroll(move |distance, window, _cx| {
                    list_state.scroll_by(distance);
                    window.refresh();
                }),
            )
            .when(
                self.slack_all_threads_loading || self.slack_all_threads_error.is_some(),
                |this| this.child(self.render_slack_all_threads_loading_indicator()),
            )
            .into_any_element()
    }

    fn slack_all_threads_selection_document_id(&self) -> SharedString {
        let team_id = self
            .slack_all_threads_team_id
            .as_deref()
            .expect("Slack All Threads rows require a team id");
        format!(
            "slack-selection:{team_id}:all-threads:{}:{}",
            self.slack_all_threads_generation,
            self.slack_all_threads_rows.len(),
        )
        .into()
    }

    fn render_slack_all_threads_loading_indicator(&self) -> Div {
        div()
            .absolute()
            .left(px(0.0))
            .right(px(0.0))
            .top(px(8.0))
            .h(px(20.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .size(px(16.0))
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(slack_palette(self.appearance_mode).main_secondary_text)),
            )
    }

    fn render_slack_all_thread_row(
        &self,
        row: &SlackAllThreadRow,
        index: usize,
        selection_document_id: SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let count = self.slack_all_threads_rows.len();
        let keyboard_conversation_id = row.conversation_id.clone();
        let keyboard_thread_timestamp = row.parent.id.clone();
        div()
            .id(row.key.clone())
            .role(Role::ListBoxOption)
            .aria_label(row.accessibility_label.clone())
            .aria_position_in_set(index + 1)
            .aria_size_of_set(count)
            .focusable()
            .tab_stop(true)
            .w_full()
            .pt(px(if index == 0 { 16.0 } else { 12.0 }))
            .pb(px(12.0))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if all_threads_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_all_threads_thread(
                        keyboard_conversation_id.as_ref(),
                        &keyboard_thread_timestamp,
                        cx,
                    );
                }
            }))
            .when(row.starts_read_section, |this| {
                this.child(self.render_slack_all_threads_up_to_date_transition())
            })
            .child(self.render_slack_all_thread_heading(row, cx))
            .child(self.render_slack_all_thread_card(row, index, selection_document_id, cx))
            .into_any_element()
    }

    fn render_slack_all_thread_card(
        &self,
        row: &SlackAllThreadRow,
        row_index: usize,
        selection_document_id: SharedString,
        cx: &mut Context<Self>,
    ) -> Div {
        let composer_popover = self.render_slack_all_threads_composer_popover(row, cx);
        let parent_document_id = selection_document_id.clone();
        let replies_document_id = selection_document_id.clone();
        div()
            .relative()
            .mx(px(16.0))
            .child(
                div()
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(rgb(slack_palette(self.appearance_mode).main_border))
                    .overflow_hidden()
                    .bg(rgb(slack_palette(self.appearance_mode).main_bg))
                    .child(div().px(px(16.0)).pt(px(8.0)).child(
                        self.render_slack_message_in_document(
                            &row.parent,
                            SlackMessageRenderContext::AllThreads,
                            SlackMessageDocumentPosition::row(parent_document_id, row_index),
                            cx,
                        ),
                    ))
                    .when(row.hidden_reply_count > 0, |this| {
                        this.child(self.render_slack_all_threads_hidden_replies(row, cx))
                    })
                    .children(row.replies.iter().enumerate().map(|(reply_index, reply)| {
                        div()
                            .w_full()
                            .when(reply.unread_boundary_before, |this| {
                                this.child(self.render_slack_message_unread_divider())
                            })
                            .child(
                                div()
                                    .px(px(16.0))
                                    .child(self.render_slack_message_in_document(
                                        reply,
                                        SlackMessageRenderContext::AllThreads,
                                        SlackMessageDocumentPosition {
                                            document_id: replies_document_id.clone(),
                                            row_index,
                                            message_index: reply_index + 1,
                                        },
                                        cx,
                                    )),
                            )
                    }))
                    .child(self.render_slack_all_threads_composer(row, cx)),
            )
            .when_some(composer_popover, |this, popover| this.child(popover))
    }

    fn render_slack_all_threads_up_to_date_transition(&self) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div().w_full().h(px(56.0)).flex_none().child(
            div()
                .w_full()
                .h(px(20.0))
                .mt(px(6.0))
                .flex()
                .items_center()
                .child(div().h(px(1.0)).flex_grow(1.0).bg(rgb(palette.main_border)))
                .child(
                    div()
                        .mx(px(12.0))
                        .text_size(px(14.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(palette.main_secondary_text))
                        .child("You’re up to date"),
                )
                .child(div().h(px(1.0)).flex_grow(1.0).bg(rgb(palette.main_border))),
        )
    }

    fn render_slack_all_thread_heading(
        &self,
        row: &SlackAllThreadRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let conversation_id = row.conversation_id.clone();
        let keyboard_conversation_id = conversation_id.clone();
        div()
            .h(px(40.0))
            .mb(px(12.0))
            .px(px(27.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .id(format!("slack-all-threads-channel-{}", row.key))
                    .role(Role::Link)
                    .aria_label(format!("Open {}", row.conversation_label))
                    .focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_slack_conversation(conversation_id.as_ref(), cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if all_threads_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.select_slack_conversation(keyboard_conversation_id.as_ref(), cx);
                        }
                    }))
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .text_size(px(15.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .when_some(row.direct_message_presence, |this, presence| {
                        this.child(slack_members_presence_dot(presence, palette.main_bg))
                    })
                    .child(row.conversation_label.clone()),
            )
            .when(!row.participant_label.is_empty(), |this| {
                this.child(slack_all_thread_participants(
                    row.participant_label.clone(),
                    palette.main_secondary_text,
                ))
            })
    }

    fn render_slack_all_threads_hidden_replies(
        &self,
        row: &SlackAllThreadRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let conversation_id = row.conversation_id.clone();
        let keyboard_conversation_id = conversation_id.clone();
        let thread_timestamp = row.parent.id.clone();
        let keyboard_thread_timestamp = thread_timestamp.clone();
        div()
            .id(format!("slack-all-threads-more-{}", row.key))
            .role(Role::Button)
            .aria_label(format!("Show {} more replies", row.hidden_reply_count))
            .focusable()
            .tab_stop(true)
            .h(px(32.0))
            .px(px(16.0))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_slack_all_threads_thread(conversation_id.as_ref(), &thread_timestamp, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if all_threads_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_all_threads_thread(
                        keyboard_conversation_id.as_ref(),
                        &keyboard_thread_timestamp,
                        cx,
                    );
                }
            }))
            .flex()
            .items_center()
            .text_size(px(13.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(palette.link))
            .child(format!("Show {} more replies", row.hidden_reply_count))
            .into_any_element()
    }
}

fn slack_all_threads_list_label(state: &SurfaceState) -> String {
    let unread_replies = state
        .slack_all_threads_snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.total_unread_replies)
        .unwrap_or(0);
    if unread_replies == 0 {
        "Threads".to_string()
    } else {
        format!("Threads, {unread_replies} new replies")
    }
}

fn slack_all_thread_participants(label: gpui::SharedString, text_color: u32) -> Div {
    div()
        .h(px(18.0))
        .flex()
        .items_center()
        .text_size(px(12.0))
        .text_color(rgb(text_color))
        .child(label)
}
