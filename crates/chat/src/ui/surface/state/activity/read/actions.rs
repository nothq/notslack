use gpui::Context;

use crate::ui::surface::{SlackActivityItemMutation, SlackActivityRow, SurfaceState};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn queue_slack_activity_item_read(
        &mut self,
        row: &SlackActivityRow,
        cx: &mut Context<Self>,
    ) {
        if !row.unread
            || !self
                .slack_workspace_api_capabilities
                .mark_activity_item_read
        {
            return;
        }
        let Some(target) = row.read_target.clone() else {
            return;
        };
        self.queue_slack_activity_item_mutation(SlackActivityItemMutation::MarkRead(target), cx);
    }

    pub(crate) fn toggle_slack_activity_item_read(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(row) = self
            .slack_activity_rows
            .iter()
            .find(|row| row.key.as_ref() == key)
            .cloned()
        else {
            return;
        };
        let Some(target) = row.read_target else {
            return;
        };
        let mutation = if row.unread {
            if !self
                .slack_workspace_api_capabilities
                .mark_activity_item_read
            {
                return;
            }
            SlackActivityItemMutation::MarkRead(target)
        } else {
            if !self
                .slack_workspace_api_capabilities
                .mark_activity_item_unread
            {
                return;
            }
            SlackActivityItemMutation::MarkUnread(target)
        };
        self.queue_slack_activity_item_mutation(mutation, cx);
    }

    pub(crate) fn toggle_slack_activity_item_archive(
        &mut self,
        key: &str,
        archive_reason: &'static str,
        unarchive_reason: &'static str,
        cx: &mut Context<Self>,
    ) {
        let Some(row) = self
            .slack_activity_rows
            .iter()
            .find(|row| row.key.as_ref() == key)
            .cloned()
        else {
            return;
        };
        let Some(target) = row.archive_target else {
            return;
        };
        let mutation = if row.archived {
            if !self
                .slack_workspace_api_capabilities
                .unarchive_activity_item
            {
                return;
            }
            SlackActivityItemMutation::Unarchive {
                target,
                reason: unarchive_reason,
            }
        } else {
            if !self.slack_workspace_api_capabilities.archive_activity_item {
                return;
            }
            SlackActivityItemMutation::Archive {
                target,
                reason: archive_reason,
            }
        };
        self.queue_slack_activity_item_mutation(mutation, cx);
    }
}
