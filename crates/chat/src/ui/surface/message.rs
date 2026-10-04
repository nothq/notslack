use std::sync::Arc;

use super::{
    alpha, div, img, px, relative, rgb, slack_icon, slack_palette, slack_remote_image_cache_key,
    AnyElement, Context, Div, FluentBuilder, FontWeight, Image, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, SlackAttachmentRenderKind, SlackAttachmentRow,
    SlackMediaPlayback, SlackMessageRenderContext, SlackShellIcon, StatefulInteractiveElement,
    Styled, SurfaceState, SLACK_MESSAGE_GAP,
};

mod attachment_rows;
mod card_helpers;
mod cards;
mod content;
mod dividers;
mod hover_actions;
mod lightbox;
mod reaction_bar;
mod reaction_picker;
mod render_core;
mod reply_summary;
mod rows;

pub(crate) use cards::SlackInlineVideoFrame;
pub(crate) use reaction_bar::SlackReactionBarInput;
pub(crate) use render_core::SlackMessageDocumentPosition;
pub(super) const SLACK_MESSAGE_EDGE_PADDING: f32 = SLACK_MESSAGE_GAP / 2.0;

#[cfg(test)]
pub(crate) use rows::build_slack_message_rows;
pub(crate) use rows::{
    build_slack_appended_message_row_with_local_today,
    build_slack_conversation_message_rows_with_local_today, build_slack_local_delivery_row,
    build_slack_message_chunks, build_slack_message_rows_with_local_today,
    build_slack_pinned_message_row, build_slack_thread_page_reply_rows_in_timezone,
    build_slack_thread_parent_row, build_slack_thread_parent_row_in_timezone,
    prepare_slack_message_body, prepare_slack_message_body_from_message,
    slack_all_threads_timestamp_label, slack_attachment_row_with_identity, slack_local_today,
    slack_message_timezone, slack_reaction_rows, SlackAppendedMessageRowInput,
    SlackLocalDeliveryRowInput,
};
pub(in crate::ui::surface) use rows::{
    slack_message_body_block_in_document, SlackMessageSelectionContext,
};

pub(super) fn slack_message_render_context_id(context: SlackMessageRenderContext) -> &'static str {
    context.id_segment()
}
