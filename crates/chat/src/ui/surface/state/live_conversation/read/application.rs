use super::super::{build_slack_dm_rows, Context, SurfaceState};
use crate::model::{SlackConversationReadReceipt, SlackMessageTimestamp};

#[derive(Default)]
struct SlackConversationReadChanges {
    conversation: bool,
    workspace_conversation: bool,
    workspace_sidebar: bool,
    sidebar: bool,
    dm: bool,
}

impl SlackConversationReadChanges {
    fn any(&self) -> bool {
        self.conversation
            || self.workspace_conversation
            || self.workspace_sidebar
            || self.sidebar
            || self.dm
    }
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn apply_slack_conversation_read_receipt(
        &mut self,
        receipt: &SlackConversationReadReceipt,
        cx: &mut Context<Self>,
    ) -> bool {
        let authoritative_dm_latest = self.slack_authoritative_dm_latest(receipt);
        let mut changes =
            self.apply_slack_backing_read_receipt(receipt, authoritative_dm_latest.as_ref());
        changes.conversation = self.apply_current_slack_conversation_read_receipt(receipt);
        if changes.conversation || changes.workspace_conversation {
            self.refresh_slack_message_read_boundary();
        }
        if changes.workspace_sidebar || changes.sidebar {
            self.refresh_slack_sidebar_rows();
        }
        self.apply_slack_dm_conversation_read_change(changes.dm);
        if changes.any() {
            cx.notify();
        }
        changes.any()
    }

    fn slack_authoritative_dm_latest(
        &self,
        receipt: &SlackConversationReadReceipt,
    ) -> Option<SlackMessageTimestamp> {
        let sidebar_authoritative_dm_latest = self
            .slack_sidebar_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.team_id == receipt.team_id)
            .and_then(|snapshot| {
                snapshot
                    .direct_message_unread_states
                    .iter()
                    .find(|state| state.conversation_id == receipt.conversation_id)
            })
            .or_else(|| {
                self.slack_workspace()
                    .filter(|workspace| workspace.team_id == receipt.team_id)
                    .and_then(|workspace| {
                        workspace
                            .direct_message_unread_states
                            .iter()
                            .find(|state| state.conversation_id == receipt.conversation_id)
                    })
            })
            .and_then(|state| state.latest_message_timestamp.clone());
        let inbox_authoritative_dm_latest = self
            .slack_dm_inbox_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.team_id == receipt.team_id)
            .and_then(|snapshot| {
                snapshot
                    .items
                    .iter()
                    .find(|item| item.conversation_id == receipt.conversation_id)
            })
            .map(|item| item.effective_unread_latest_timestamp().clone());
        match (
            sidebar_authoritative_dm_latest,
            inbox_authoritative_dm_latest,
        ) {
            (Some(sidebar), Some(inbox)) if sidebar.sort_key() >= inbox.sort_key() => Some(sidebar),
            (_, Some(inbox)) => Some(inbox),
            (sidebar, None) => sidebar,
        }
    }

    fn apply_slack_backing_read_receipt(
        &mut self,
        receipt: &SlackConversationReadReceipt,
        authoritative_dm_latest: Option<&SlackMessageTimestamp>,
    ) -> SlackConversationReadChanges {
        let workspace_conversation_matches = self.slack_workspace().is_some_and(|workspace| {
            workspace.team_id == receipt.team_id
                && workspace.conversation_id == receipt.conversation_id
        });
        let workspace_sidebar = self
            .slack_workspace_mut()
            .filter(|workspace| workspace.team_id == receipt.team_id)
            .is_some_and(|workspace| {
                workspace.apply_conversation_read_receipt(
                    &receipt.conversation_id,
                    &receipt.last_read,
                    authoritative_dm_latest,
                )
            });
        let sidebar = self
            .slack_sidebar_snapshot
            .as_mut()
            .filter(|snapshot| snapshot.team_id == receipt.team_id)
            .is_some_and(|snapshot| {
                snapshot.apply_conversation_read_receipt(
                    &receipt.conversation_id,
                    &receipt.last_read,
                    authoritative_dm_latest,
                )
            });
        let dm = self
            .slack_dm_inbox_snapshot
            .as_mut()
            .filter(|snapshot| snapshot.team_id == receipt.team_id)
            .is_some_and(|snapshot| {
                snapshot
                    .apply_conversation_read_receipt(&receipt.conversation_id, &receipt.last_read)
            });
        SlackConversationReadChanges {
            workspace_conversation: workspace_conversation_matches && workspace_sidebar,
            workspace_sidebar,
            sidebar,
            dm,
            ..SlackConversationReadChanges::default()
        }
    }

    fn apply_current_slack_conversation_read_receipt(
        &mut self,
        receipt: &SlackConversationReadReceipt,
    ) -> bool {
        self.slack_conversation_snapshot
            .as_mut()
            .filter(|snapshot| snapshot.team_id == receipt.team_id)
            .is_some_and(|snapshot| {
                snapshot
                    .apply_conversation_read_receipt(&receipt.conversation_id, &receipt.last_read)
            })
    }

    fn apply_slack_dm_conversation_read_change(&mut self, changed: bool) {
        if !changed {
            return;
        }
        self.slack_dm_rows = build_slack_dm_rows(
            self.slack_dm_inbox_snapshot
                .as_ref()
                .expect("changed Slack DM read state requires an inbox snapshot"),
        );
        {
            let (authority, data) = (&mut self.slack_presence_authority, &mut self.data);
            authority.overlay_dm_rows(&mut data.slack_dm_rows);
        }
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_dm(data);
        }
        self.refresh_slack_dm_visible_rows();
    }

    pub(in crate::ui::surface::state) fn slack_conversation_live_request_is_current(
        &self,
        generation: u64,
        target: &super::super::SlackConversationLiveTarget,
    ) -> bool {
        self.slack_conversation_live_generation == generation
            && self.slack_conversation_live_target.as_ref() == Some(target)
            && self
                .current_slack_conversation_live_target_ids()
                .is_some_and(|(team_id, conversation_id)| {
                    target.team_id == team_id && target.conversation_id == conversation_id
                })
    }
}
