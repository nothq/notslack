mod attachments;
mod body;
mod construction;
mod reaction_rows;
mod reactions;
mod time;
mod timeline;

#[cfg(test)]
pub(crate) use self::attachments::slack_attachment_row;
pub(crate) use self::attachments::slack_attachment_row_with_identity;
pub(in crate::ui::surface) use self::body::slack_message_body_block;
pub(crate) use self::body::{prepare_slack_message_body, prepare_slack_message_body_from_message};
pub(in crate::ui::surface) use self::body::{
    slack_message_body_block_in_document, SlackMessageSelectionContext,
};
pub(in crate::ui::surface::message) use self::body::{
    slack_prepared_message_body, SlackPreparedMessageBodyStyle,
};
pub(crate) use self::construction::{build_slack_local_delivery_row, SlackLocalDeliveryRowInput};
pub(crate) use self::reaction_rows::slack_reaction_rows;
#[cfg(test)]
use self::reactions::slack_reaction_display;
pub(crate) use self::time::{
    slack_all_threads_timestamp_label, slack_local_today, slack_message_timezone,
};
#[cfg(test)]
pub(crate) use self::timeline::build_slack_message_rows;
pub(crate) use self::timeline::{
    build_slack_appended_message_row_with_local_today,
    build_slack_conversation_message_rows_with_local_today, build_slack_message_chunks,
    build_slack_message_rows_with_local_today, build_slack_pinned_message_row,
    build_slack_thread_page_reply_rows_in_timezone, build_slack_thread_parent_row,
    build_slack_thread_parent_row_in_timezone, SlackAppendedMessageRowInput,
};
use crate::ui::surface::{
    div, px, rgb, slack_palette, AnyElement, AppearanceMode, Context, Div, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, SlackMessageRow, StatefulInteractiveElement, Styled,
    SurfaceState,
};
use gpui::Role;

pub(super) fn bind_slack_profile_click(
    row: Div,
    user_id: Option<String>,
    element_id: String,
    accessibility_label: String,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    let Some(user_id) = user_id else {
        return row.into_any_element();
    };
    let keyboard_user_id = user_id.clone();
    row.id(element_id)
        .role(Role::Button)
        .aria_label(accessibility_label)
        .focusable()
        .tab_stop(true)
        .cursor_pointer()
        .focus_visible(|style| style.border_1().border_color(rgb(0x1d9bd1)))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.open_slack_profile(&user_id, cx);
        }))
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
            if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                || event.keystroke.modifiers.modified()
            {
                return;
            }
            window.prevent_default();
            cx.stop_propagation();
            this.open_slack_profile(&keyboard_user_id, cx);
        }))
        .into_any_element()
}

pub(super) fn slack_message_shows_identity(row: &SlackMessageRow) -> bool {
    !row.compact
        && (!row.author.trim().is_empty()
            || !row.timestamp.trim().is_empty()
            || row.avatar_image_url.is_some()
            || !row.avatar_text.trim().is_empty())
}

pub(super) fn slack_message_meta(
    text: impl Into<gpui::SharedString>,
    appearance_mode: AppearanceMode,
) -> Div {
    let palette = slack_palette(appearance_mode);
    div()
        .text_size(px(12.0))
        .line_height(px(18.0))
        .text_color(rgb(palette.main_muted_text))
        .child(text.into())
}

#[cfg(test)]
mod tests;
