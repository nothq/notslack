use std::sync::Arc;

use super::super::{SlackMessageRow, SurfaceState};
use crate::ui::surface::{SlackLaterRowContent, SlackMessageActionTarget};
use crate::ui::SlackReaction;

type SlackReactionSource = (Arc<SlackMessageActionTarget>, Arc<[SlackReaction]>);

impl SurfaceState {
    pub(in crate::ui::surface::state) fn slack_message_row_for_action_target(
        &self,
        target: &SlackMessageActionTarget,
    ) -> Option<&SlackMessageRow> {
        find_slack_message_row(&self.slack_message_rows, target)
            .or_else(|| {
                self.slack_thread_panel.as_ref().and_then(|panel| {
                    (panel.parent_row.action_target.as_deref() == Some(target))
                        .then_some(&panel.parent_row)
                        .or_else(|| find_slack_message_row(&panel.reply_rows, target))
                })
            })
            .or_else(|| {
                self.slack_all_threads_rows.iter().find_map(|thread| {
                    (thread.parent.action_target.as_deref() == Some(target))
                        .then_some(&thread.parent)
                        .or_else(|| find_slack_message_row(&thread.replies, target))
                })
            })
            .or_else(|| {
                self.slack_later_rows.iter().find_map(|later| {
                    let SlackLaterRowContent::Message { detail } = &later.content else {
                        return None;
                    };
                    (detail.selected_message_row.action_target.as_deref() == Some(target))
                        .then_some(&detail.selected_message_row)
                })
            })
            .or_else(|| {
                self.slack_pins_rows
                    .iter()
                    .map(|pin| &pin.message)
                    .find(|row| row.action_target.as_deref() == Some(target))
            })
            .or_else(|| {
                let crate::ui::surface::SlackActivityDetailState::Loaded { rows, .. } =
                    &self.slack_activity_detail
                else {
                    return None;
                };
                find_slack_message_row(rows.as_ref(), target)
            })
    }

    pub(super) fn slack_reaction_state_for_action_target(
        &self,
        target: &SlackMessageActionTarget,
    ) -> Option<&Arc<[SlackReaction]>> {
        self.slack_message_row_for_action_target(target)
            .map(|row| &row.reaction_state)
            .or_else(|| {
                self.slack_search_rows
                    .iter()
                    .find(|row| row.action_target.as_ref() == target)
                    .map(|row| &row.reaction_state)
            })
    }

    pub(super) fn slack_reaction_source(&self, message_id: &str) -> Option<SlackReactionSource> {
        let search_row = self.slack_search_rows.iter().find(|row| {
            row.result_id.as_ref() == message_id
                || (self.slack_search_results_open
                    && row.action_target.message_timestamp().as_str() == message_id)
        });
        if let Some(row) = search_row {
            return Some((row.action_target.clone(), row.reaction_state.clone()));
        }
        let target = self.slack_message_action_target(message_id)?;
        let reactions = self
            .slack_reaction_state_for_action_target(&target)?
            .clone();
        Some((target, reactions))
    }

    pub(in crate::ui::surface::state) fn slack_message_action_target(
        &self,
        message_id: &str,
    ) -> Option<Arc<SlackMessageActionTarget>> {
        self.slack_message_rows
            .iter()
            .find(|row| row.id == message_id)
            .and_then(|row| row.action_target.clone())
            .or_else(|| {
                self.slack_thread_panel.as_ref().and_then(|panel| {
                    std::iter::once(&panel.parent_row)
                        .chain(panel.reply_rows.iter())
                        .find(|row| row.id == message_id)
                        .and_then(|row| row.action_target.clone())
                })
            })
            .or_else(|| {
                self.slack_all_threads_rows.iter().find_map(|thread| {
                    std::iter::once(&thread.parent)
                        .chain(thread.replies.iter())
                        .find(|row| row.id == message_id)
                        .and_then(|row| row.action_target.clone())
                })
            })
            .or_else(|| {
                self.slack_later_rows.iter().find_map(|later| {
                    let SlackLaterRowContent::Message { detail } = &later.content else {
                        return None;
                    };
                    (detail.selected_message_row.id == message_id)
                        .then(|| detail.selected_message_row.action_target.clone())
                        .flatten()
                })
            })
            .or_else(|| {
                self.slack_pins_rows
                    .iter()
                    .map(|pin| &pin.message)
                    .find(|row| row.id == message_id)
                    .and_then(|row| row.action_target.clone())
            })
            .or_else(|| {
                let crate::ui::surface::SlackActivityDetailState::Loaded { rows, .. } =
                    &self.slack_activity_detail
                else {
                    return None;
                };
                find_slack_message_row_by_id(rows.as_ref(), message_id)
                    .and_then(|row| row.action_target.clone())
            })
            .or_else(|| {
                self.slack_search_rows
                    .iter()
                    .find(|row| {
                        row.result_id.as_ref() == message_id
                            || row.action_target.message_timestamp().as_str() == message_id
                    })
                    .map(|row| row.action_target.clone())
            })
    }
}

pub(super) fn find_slack_message_row<'a>(
    rows: &'a [SlackMessageRow],
    target: &SlackMessageActionTarget,
) -> Option<&'a SlackMessageRow> {
    rows.iter().find_map(|row| {
        (row.action_target.as_deref() == Some(target))
            .then_some(row)
            .or_else(|| find_slack_message_row(&row.replies, target))
    })
}

pub(super) fn top_level_slack_message_row_index(
    rows: &[SlackMessageRow],
    target: &SlackMessageActionTarget,
) -> Option<usize> {
    rows.iter()
        .position(|row| find_slack_message_row(std::slice::from_ref(row), target).is_some())
}

fn find_slack_message_row_by_id<'a>(
    rows: &'a [SlackMessageRow],
    message_id: &str,
) -> Option<&'a SlackMessageRow> {
    rows.iter().find_map(|row| {
        (row.id == message_id)
            .then_some(row)
            .or_else(|| find_slack_message_row_by_id(&row.replies, message_id))
    })
}
