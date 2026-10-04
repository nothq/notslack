mod control;
mod file_metadata;
mod paging;
mod rows;
mod scheduled;

use rows::next_slack_drafts_sent_generation;
use std::sync::Arc;

use super::{Context, SlackRailView, SurfaceState};
use crate::ui::surface::{SlackDraftRestore, SlackDraftsSentRowTarget};
use crate::ui::SlackDraftsSentTab;

const SLACK_DRAFTS_SENT_PAGINATION_THRESHOLD: usize = 5;

impl SurfaceState {
    pub(crate) fn activate_slack_drafts_sent(&mut self, cx: &mut Context<Self>) {
        if !self.is_slack_workspace() || !self.slack_workspace_api_capabilities.load_drafts_sent {
            return;
        }
        let Some((team_id, self_user_id)) = self.slack_workspace().and_then(|workspace| {
            let self_user_id = workspace.self_user_id.clone()?;
            (!workspace.team_id.is_empty() && !self_user_id.is_empty())
                .then(|| (workspace.team_id.clone(), self_user_id))
        }) else {
            self.slack_drafts_sent_error = Some(
                "Slack Drafts & sent requires an authenticated workspace identity.".to_string(),
            );
            cx.notify();
            return;
        };
        self.leave_slack_directory(cx);
        self.reset_slack_schedule_context(cx);
        if self.slack_drafts_sent_team_id.as_deref() != Some(team_id.as_str())
            || self.slack_drafts_sent_self_user_id.as_deref() != Some(self_user_id.as_str())
        {
            self.reset_slack_drafts_sent_context();
            self.slack_drafts_sent_team_id = Some(team_id);
            self.slack_drafts_sent_self_user_id = Some(self_user_id);
        }
        self.leave_slack_activity(cx);
        self.leave_slack_later();
        self.leave_slack_files();
        self.slack_active_rail_view = SlackRailView::DraftsSent;
        self.slack_dms_peek_visible = false;
        self.slack_aux_panel = None;
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_composer_focused = false;
        self.slack_expanded_attachment = None;
        self.show_cached_slack_drafts_sent_tab();
        self.slack_drafts_sent_list_state.remeasure();
        cx.notify();
        self.refresh_current_slack_drafts_sent_tab(cx);
    }

    pub(crate) fn leave_slack_drafts_sent(&mut self) {
        if self.slack_active_rail_view != SlackRailView::DraftsSent {
            return;
        }
        self.slack_drafts_sent_generation =
            next_slack_drafts_sent_generation(self.slack_drafts_sent_generation);
        self.slack_drafts_sent_loading = false;
    }

    pub(crate) fn reset_slack_drafts_sent_context(&mut self) {
        self.slack_drafts_sent_generation =
            next_slack_drafts_sent_generation(self.slack_drafts_sent_generation);
        self.slack_drafts_sent_team_id = None;
        self.slack_drafts_sent_self_user_id = None;
        self.slack_drafts_sent_tab = SlackDraftsSentTab::Drafts;
        self.slack_drafts_sent_snapshots = [None, None, None];
        self.slack_drafts_sent_cached_rows = std::array::from_fn(|_| Arc::default());
        self.slack_drafts_sent_rows = Arc::default();
        self.slack_drafts_sent_list_state.reset(0);
        self.slack_drafts_sent_loading = false;
        self.slack_drafts_sent_error = None;
        self.slack_drafts_sent_file_metadata.reset();
        self.slack_pending_draft_restore = None;
        self.slack_scheduled_mutation_generation = self
            .slack_scheduled_mutation_generation
            .checked_add(1)
            .expect("Slack scheduled mutation generation overflowed");
        self.slack_scheduled_delete_pending = None;
    }

    pub(crate) fn select_slack_drafts_sent_tab(
        &mut self,
        tab: SlackDraftsSentTab,
        cx: &mut Context<Self>,
    ) {
        if self.slack_active_rail_view != SlackRailView::DraftsSent
            || self.slack_drafts_sent_tab == tab
        {
            return;
        }
        self.slack_drafts_sent_generation =
            next_slack_drafts_sent_generation(self.slack_drafts_sent_generation);
        self.slack_drafts_sent_loading = false;
        self.slack_drafts_sent_tab = tab;
        self.slack_drafts_sent_error = None;
        self.show_cached_slack_drafts_sent_tab();
        self.record_slack_drafts_sent_history(tab);
        cx.notify();
        self.refresh_current_slack_drafts_sent_tab(cx);
    }

    pub(crate) fn retry_slack_drafts_sent(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_rail_view != SlackRailView::DraftsSent
            || self.slack_drafts_sent_loading
        {
            return;
        }
        let cursor = self
            .current_slack_drafts_sent_snapshot()
            .and_then(|snapshot| snapshot.next_cursor.clone());
        self.begin_slack_drafts_sent_page(cursor, cx);
    }

    pub(crate) fn handle_slack_drafts_sent_scroll(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) {
        self.queue_slack_drafts_sent_file_metadata(visible_start, visible_end, cx);
        self.queue_slack_drafts_sent_images(visible_start, visible_end, cx);
        if visible_end.saturating_add(SLACK_DRAFTS_SENT_PAGINATION_THRESHOLD) >= count {
            self.maybe_load_more_slack_drafts_sent(cx);
        }
    }

    pub(crate) fn activate_slack_drafts_sent_target(
        &mut self,
        target: SlackDraftsSentRowTarget,
        cx: &mut Context<Self>,
    ) {
        if self.slack_drafts_sent_draft_activation_is_blocked()
            && matches!(
                &target,
                SlackDraftsSentRowTarget::Draft { .. } | SlackDraftsSentRowTarget::Scheduled { .. }
            )
        {
            return;
        }
        match target {
            SlackDraftsSentRowTarget::Draft {
                conversation_id,
                document,
            } => {
                self.select_slack_conversation(conversation_id.as_ref(), cx);
                if self.slack_pending_conversation_id.as_deref() == Some(conversation_id.as_ref()) {
                    self.slack_pending_draft_restore = Some(SlackDraftRestore {
                        conversation_id,
                        document,
                        scheduled_edit: None,
                    });
                } else if self.slack_conversation_id() == Some(conversation_id.as_ref()) {
                    self.replace_slack_send_draft_document(document);
                    self.slack_composer_focused = true;
                    self.slack_error = None;
                    cx.notify();
                }
            }
            SlackDraftsSentRowTarget::Scheduled { edit, document } => {
                let conversation_id = edit.conversation_id.clone();
                self.select_slack_conversation(&conversation_id, cx);
                if self.slack_pending_conversation_id.as_deref() == Some(conversation_id.as_str()) {
                    self.slack_pending_draft_restore = Some(SlackDraftRestore {
                        conversation_id: conversation_id.into(),
                        document,
                        scheduled_edit: Some(edit),
                    });
                } else if self.slack_conversation_id() == Some(conversation_id.as_str()) {
                    self.begin_slack_scheduled_edit(edit, document, cx);
                    self.slack_composer_focused = true;
                    self.slack_error = None;
                    cx.notify();
                }
            }
            SlackDraftsSentRowTarget::Conversation(conversation_id) => {
                self.select_slack_conversation(conversation_id.as_ref(), cx);
            }
            SlackDraftsSentRowTarget::Message(permalink) => {
                self.open_slack_link(permalink.as_ref(), cx);
            }
        }
    }

    pub(crate) fn slack_drafts_sent_draft_activation_is_blocked(&self) -> bool {
        self.slack_schedule_pending.is_some()
            || self.slack_scheduled_delete_pending.is_some()
            || self.slack_composer_schedule_recovery.is_some()
            || self.slack_scheduled_edit_recovery.is_some()
            || self.slack_active_scheduled_edit.is_some()
            || self
                .slack_pending_draft_restore
                .as_ref()
                .is_some_and(|restore| restore.scheduled_edit.is_some())
    }
}
