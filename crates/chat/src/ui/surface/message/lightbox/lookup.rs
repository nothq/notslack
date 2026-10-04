#[cfg(test)]
use crate::ui::surface::SlackAttachmentRow;
use crate::ui::surface::{
    SlackActivityDetailState, SlackAttachmentSelection, SlackMainRoute, SlackMainTab,
    SlackMessageRow, SlackRailView, SlackThreadPanelState, SurfaceState,
};

pub(super) fn find_current_slack_attachment_selection_by_id(
    state: &SurfaceState,
    attachment_id: &str,
) -> Option<SlackAttachmentSelection> {
    if state.slack_search_results_open {
        if let Some(selection) = find_slack_search_attachment_selection_by_id(state, attachment_id)
        {
            return Some(selection);
        }
    }
    match state.slack_main_route {
        SlackMainRoute::Directory | SlackMainRoute::NewMessage => None,
        SlackMainRoute::AllThreads => state.slack_all_threads_rows.iter().find_map(|thread| {
            find_slack_attachment_selection_in_rows_by_id(
                std::slice::from_ref(&thread.parent),
                attachment_id,
            )
            .or_else(|| {
                find_slack_attachment_selection_in_rows_by_id(&thread.replies, attachment_id)
            })
        }),
        SlackMainRoute::Conversation => {
            find_current_slack_conversation_attachment_selection_by_id(state, attachment_id)
        }
    }
}

fn find_current_slack_conversation_attachment_selection_by_id(
    state: &SurfaceState,
    attachment_id: &str,
) -> Option<SlackAttachmentSelection> {
    match state.slack_active_rail_view {
        SlackRailView::Activity => match &state.slack_activity_detail {
            SlackActivityDetailState::Loaded { rows, .. } => {
                find_slack_attachment_selection_in_rows_by_id(rows, attachment_id)
            }
            _ => None,
        },
        SlackRailView::Later => {
            find_current_slack_later_attachment_selection_by_id(state, attachment_id)
        }
        SlackRailView::Home | SlackRailView::Dms => {
            find_current_slack_primary_attachment_selection_by_id(state, attachment_id)
        }
        SlackRailView::Files
        | SlackRailView::DraftsSent
        | SlackRailView::More
        | SlackRailView::Admin => None,
    }
}

fn find_current_slack_primary_attachment_selection_by_id(
    state: &SurfaceState,
    attachment_id: &str,
) -> Option<SlackAttachmentSelection> {
    match state.slack_active_tab {
        SlackMainTab::Messages | SlackMainTab::Canvas => {
            find_conversation_thread_attachment_selection_by_id(state, attachment_id).or_else(
                || {
                    find_slack_attachment_selection_in_rows_by_id(
                        &state.slack_message_rows,
                        attachment_id,
                    )
                },
            )
        }
        SlackMainTab::Pins => state.slack_pins_rows.iter().find_map(|pin| {
            find_slack_attachment_selection_in_rows_by_id(
                std::slice::from_ref(&pin.message),
                attachment_id,
            )
        }),
        SlackMainTab::BookmarkFolder | SlackMainTab::FilesLinks => None,
    }
}

fn find_slack_search_attachment_selection_by_id(
    state: &SurfaceState,
    attachment_id: &str,
) -> Option<SlackAttachmentSelection> {
    state
        .slack_search_rows
        .iter()
        .flat_map(|row| row.attachments.iter())
        .find_map(|attachment| {
            find_slack_attachment_selection_in_row_by_id(&attachment.attachment, attachment_id)
        })
}

fn find_current_slack_later_attachment_selection_by_id(
    state: &SurfaceState,
    attachment_id: &str,
) -> Option<SlackAttachmentSelection> {
    let detail = state.selected_slack_later_row()?.thread_target()?;
    if !state.slack_workspace_api_capabilities.load_thread {
        return find_slack_attachment_selection_in_rows_by_id(
            std::slice::from_ref(&detail.selected_message_row),
            attachment_id,
        );
    }
    let panel = state.slack_thread_panel.as_ref().filter(|panel| {
        panel.origin.later_item_key() == Some(&detail.item_key)
            && panel.conversation_id == detail.conversation_id
            && panel.parent_message_id == detail.thread_timestamp
    })?;
    find_slack_attachment_selection_in_thread_panel_by_id(panel, attachment_id)
}

fn find_conversation_thread_attachment_selection_by_id(
    state: &SurfaceState,
    attachment_id: &str,
) -> Option<SlackAttachmentSelection> {
    state
        .slack_thread_panel
        .as_ref()
        .filter(|panel| panel.origin.is_conversation())
        .and_then(|panel| {
            find_slack_attachment_selection_in_thread_panel_by_id(panel, attachment_id)
        })
}

fn find_slack_attachment_selection_in_thread_panel_by_id(
    panel: &SlackThreadPanelState,
    attachment_id: &str,
) -> Option<SlackAttachmentSelection> {
    find_slack_attachment_selection_in_rows_by_id(
        std::slice::from_ref(&panel.parent_row),
        attachment_id,
    )
    .or_else(|| find_slack_attachment_selection_in_rows_by_id(&panel.reply_rows, attachment_id))
}

fn find_slack_attachment_selection_in_rows_by_id(
    rows: &[SlackMessageRow],
    attachment_id: &str,
) -> Option<SlackAttachmentSelection> {
    for message in rows {
        for attachment in &message.attachments {
            if let Some(selection) =
                find_slack_attachment_selection_in_row_by_id(attachment, attachment_id)
            {
                return Some(selection);
            }
        }
        if let Some(selection) =
            find_slack_attachment_selection_in_rows_by_id(&message.replies, attachment_id)
        {
            return Some(selection);
        }
    }
    None
}

fn find_slack_attachment_selection_in_row_by_id(
    attachment: &crate::ui::surface::SlackAttachmentRow,
    attachment_id: &str,
) -> Option<SlackAttachmentSelection> {
    if attachment.attachment_id == attachment_id {
        return Some(SlackAttachmentSelection::from_row(attachment));
    }
    attachment
        .shared_message
        .as_ref()
        .and_then(|shared| {
            shared
                .files
                .iter()
                .find(|file| file.attachment_id == attachment_id)
        })
        .map(SlackAttachmentSelection::from_shared_file)
}

#[cfg(test)]
pub(super) fn find_slack_attachment_in_rows_by_title<'a>(
    rows: &'a [SlackMessageRow],
    attachment_title: &str,
) -> Option<&'a SlackAttachmentRow> {
    for message in rows {
        if let Some(attachment) = message
            .attachments
            .iter()
            .find(|attachment| attachment.title == attachment_title)
        {
            return Some(attachment);
        }
        if let Some(attachment) =
            find_slack_attachment_in_rows_by_title(&message.replies, attachment_title)
        {
            return Some(attachment);
        }
    }
    None
}
