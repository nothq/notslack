use crate::ui::surface::message::SlackMessageHoverActionsContext;
use crate::ui::surface::{
    slack_base_icon_radius, slack_message_body_block_in_document, slack_palette,
    SlackMessageDocumentPosition, SlackMessageRenderContext, SlackMessageRow,
    SlackMessageSelectionContext, SurfaceState,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, img, px, rgb, AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Styled,
};
use std::cell::Cell;

/// An Activity detail row in the selection document; each rendered message claims the next index.
#[derive(Clone, Copy)]
struct SlackActivityDetailDocumentRow<'a> {
    document_id: &'a gpui::SharedString,
    row_index: usize,
    next_message_index: &'a Cell<usize>,
}

impl SurfaceState {
    pub(super) fn render_slack_activity_detail_message_in_document(
        &self,
        row: &SlackMessageRow,
        anchor_timestamp: &str,
        position: SlackMessageDocumentPosition,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let SlackMessageDocumentPosition {
            document_id,
            row_index,
            message_index,
        } = position;
        self.render_slack_activity_detail_message_at_document_position(
            row,
            anchor_timestamp,
            SlackActivityDetailDocumentRow {
                document_id: &document_id,
                row_index,
                next_message_index: &Cell::new(message_index),
            },
            cx,
        )
    }

    fn render_slack_activity_detail_message_at_document_position(
        &self,
        row: &SlackMessageRow,
        anchor_timestamp: &str,
        document_row: SlackActivityDetailDocumentRow<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let highlighted = row.id == anchor_timestamp;
        let palette = slack_palette(self.appearance_mode);
        let hover_group = format!("slack-activity-detail-message-hover-{}", row.id);
        let selection_context = slack_activity_detail_selection_context(row, document_row);
        let replies = self.render_slack_activity_detail_message_replies(
            row,
            anchor_timestamp,
            document_row,
            cx,
        );
        let actions = SlackMessageHoverActionsContext::for_message(
            self,
            row,
            SlackMessageRenderContext::Activity,
        );
        div()
            .w_full()
            .min_w(px(0.0))
            .px(px(20.0))
            .py(px(8.0))
            .when(highlighted, |this| this.bg(rgb(palette.topic_bg)))
            .when_some(row.divider.as_ref(), |this, divider| {
                this.child(slack_activity_detail_divider(
                    divider.label.clone(),
                    palette.main_secondary_text,
                ))
            })
            // The same buttons as in the channel, over the message itself
            // rather than the date line above it.
            .child(
                div()
                    .relative()
                    .group(hover_group.clone())
                    .child(self.render_slack_activity_detail_message_row(
                        row,
                        &selection_context,
                        cx,
                    ))
                    .when(actions.any_supported(), |this| {
                        this.child(self.render_slack_message_hover_actions(
                            row,
                            hover_group,
                            actions,
                            cx,
                        ))
                    }),
            )
            .children(replies)
            .into_any_element()
    }

    fn render_slack_activity_detail_message_row(
        &self,
        row: &SlackMessageRow,
        selection_context: &SlackMessageSelectionContext,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .w_full()
            .flex()
            .items_start()
            .gap(px(9.0))
            .child(self.render_slack_activity_detail_avatar(row))
            .child(self.render_slack_activity_detail_message_content(row, selection_context, cx))
    }

    fn render_slack_activity_detail_message_replies(
        &self,
        row: &SlackMessageRow,
        anchor_timestamp: &str,
        document_row: SlackActivityDetailDocumentRow<'_>,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        row.replies
            .iter()
            .map(|reply| {
                self.render_slack_activity_detail_message_at_document_position(
                    reply,
                    anchor_timestamp,
                    document_row,
                    cx,
                )
            })
            .collect()
    }

    fn render_slack_activity_detail_message_content(
        &self,
        row: &SlackMessageRow,
        selection_context: &SlackMessageSelectionContext,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .child(div().h(px(21.0)).child(self.render_slack_message_author(
                row,
                SlackMessageRenderContext::Activity,
                Some(selection_context),
                cx,
            )))
            .when(
                row.body.has_renderable_content() || !row.table_rows.is_empty(),
                |this| {
                    this.child(slack_message_body_block_in_document(
                        self,
                        row,
                        SlackMessageRenderContext::Activity,
                        selection_context,
                        cx,
                    ))
                },
            )
            .when(!row.attachments.is_empty(), |this| {
                this.child(self.render_slack_message_attachments(
                    row,
                    SlackMessageRenderContext::Activity,
                    cx,
                ))
            })
            .when(!row.reactions.is_empty(), |this| {
                this.child(self.render_slack_message_reactions(
                    row,
                    SlackMessageRenderContext::Activity,
                    cx,
                ))
            })
    }

    fn render_slack_activity_detail_avatar(&self, row: &SlackMessageRow) -> Div {
        if let Some(image) = row
            .avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            return div()
                .size(px(36.0))
                .flex_none()
                .rounded(slack_base_icon_radius(36.0))
                .overflow_hidden()
                .child(img(image).size_full().rounded(slack_base_icon_radius(36.0)));
        }
        div()
            .size(px(36.0))
            .flex_none()
            .rounded(slack_base_icon_radius(36.0))
            .bg(rgb(row.avatar_fill))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(12.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(row.avatar_text.clone())
    }
}

fn slack_activity_detail_selection_context(
    row: &SlackMessageRow,
    document_row: SlackActivityDetailDocumentRow<'_>,
) -> SlackMessageSelectionContext {
    let SlackActivityDetailDocumentRow {
        document_id: selection_document_id,
        row_index,
        next_message_index,
    } = document_row;
    let message_index = next_message_index.get();
    next_message_index.set(message_index + 1);
    SlackMessageSelectionContext::new(
        selection_document_id.clone(),
        row_index,
        message_index,
        row.id.clone(),
    )
}

fn slack_activity_detail_divider(label: gpui::SharedString, color: u32) -> Div {
    div()
        .h(px(34.0))
        .pb(px(8.0))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(color))
        .child(label)
}
