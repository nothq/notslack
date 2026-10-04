use super::{Context, SurfaceState};
use crate::ui::surface::{
    SlackComposerDestination, SlackComposerDraft, SlackComposerDraftKey,
    SlackPendingThreadReplySend,
};

impl SurfaceState {
    pub(crate) fn slack_thread_reply_is_pending(&self, key: &SlackComposerDraftKey) -> bool {
        self.slack_pending_thread_replies.contains_key(key)
    }

    pub(crate) fn slack_thread_reply_mutation_is_blocked(
        &self,
        key: &SlackComposerDraftKey,
    ) -> bool {
        self.slack_thread_reply_is_pending(key)
            || self
                .slack_schedule_pending
                .as_ref()
                .and_then(|pending| pending.owner.draft_key())
                == Some(key)
    }

    pub(in crate::ui::surface::state) fn slack_thread_reply_panel_is_current(
        &self,
        key: &SlackComposerDraftKey,
    ) -> bool {
        self.slack_thread_panel.as_ref().is_some_and(|panel| {
            panel.reply_draft_key == *key && self.slack_thread_panel_origin_is_current(panel)
        })
    }

    pub(in crate::ui::surface::state) fn next_slack_thread_reply_send_generation(&mut self) -> u64 {
        self.slack_thread_reply_generation = self
            .slack_thread_reply_generation
            .checked_add(1)
            .expect("Slack thread reply request generation overflowed");
        self.slack_thread_reply_generation
    }

    pub(in crate::ui::surface::state) fn start_slack_thread_reply_send(
        &mut self,
        key: SlackComposerDraftKey,
        pending: SlackPendingThreadReplySend,
    ) -> bool {
        if self.slack_thread_reply_mutation_is_blocked(&key) {
            return false;
        }
        self.slack_pending_thread_replies.insert(key, pending);
        true
    }

    pub(in crate::ui::surface::state) fn slack_thread_reply_send_is_pending(
        &self,
        key: &SlackComposerDraftKey,
        pending: SlackPendingThreadReplySend,
    ) -> bool {
        self.slack_pending_thread_replies.get(key) == Some(&pending)
    }

    pub(in crate::ui::surface::state) fn finish_slack_thread_reply_pending_send(
        &mut self,
        key: &SlackComposerDraftKey,
        pending: SlackPendingThreadReplySend,
    ) -> bool {
        if !self.slack_thread_reply_send_is_pending(key, pending) {
            return false;
        }
        self.slack_pending_thread_replies.remove(key);
        true
    }

    pub(in crate::ui::surface::state) fn clear_submitted_slack_thread_reply_draft(
        &mut self,
        key: &SlackComposerDraftKey,
        pending: SlackPendingThreadReplySend,
        cx: &mut Context<Self>,
    ) -> bool {
        let active_matches = self.slack_thread_panel.as_ref().is_some_and(|panel| {
            panel.reply_draft_key == *key
                && slack_composer_draft_matches_pending(&panel.reply_draft.borrow(), pending)
        });
        if active_matches {
            let token = self.next_slack_composer_draft_token();
            let draft_id = self.next_slack_composer_draft_id();
            let Some(panel) = self
                .slack_thread_panel
                .as_mut()
                .filter(|panel| panel.reply_draft_key == *key)
            else {
                return false;
            };
            let submitted = std::mem::replace(
                panel.reply_draft.get_mut(),
                SlackComposerDraft::new(draft_id),
            );
            panel.reply_draft.get_mut().token = token;
            self.discard_slack_composer_draft(
                crate::ui::surface::SlackFileStagingDraftOwner::Thread(key.clone()),
                submitted,
                cx,
            );
            return true;
        }

        let stored_matches = self
            .slack_composer_drafts
            .get(key)
            .is_some_and(|draft| slack_composer_draft_matches_pending(draft, pending));
        if !stored_matches {
            return false;
        }
        let Some(submitted) = self.slack_composer_drafts.remove(key) else {
            return false;
        };
        self.discard_slack_composer_draft(
            crate::ui::surface::SlackFileStagingDraftOwner::Thread(key.clone()),
            submitted,
            cx,
        );
        true
    }

    pub(in crate::ui::surface::state) fn reconcile_successful_slack_thread_reply(
        &mut self,
        key: &SlackComposerDraftKey,
        all_threads_applied: bool,
        cx: &mut Context<Self>,
    ) {
        self.reconcile_slack_all_threads_after_reply(all_threads_applied, cx);
        let SlackComposerDestination::Thread {
            conversation_id, ..
        } = &key.destination
        else {
            unreachable!("Slack thread reply pending key must target a thread");
        };
        if self
            .slack_conversation_live_target
            .as_ref()
            .is_some_and(|target| {
                target.team_id == key.team_id && target.conversation_id == *conversation_id
            })
        {
            self.queue_slack_conversation_reconciliation(cx);
        }
    }
}

fn slack_composer_draft_matches_pending(
    draft: &SlackComposerDraft,
    pending: SlackPendingThreadReplySend,
) -> bool {
    draft.token == pending.draft_token
        && draft.document.revision() == pending.draft_document_revision
}
