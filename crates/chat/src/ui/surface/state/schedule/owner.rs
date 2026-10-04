use super::{
    slack_schedule_draft_is_ready, SharedString, SlackComposerDestination, SlackComposerDraft,
    SlackComposerDraftKey, SlackMainComposerDraftOwner, SlackMainRoute, SlackScheduleAnchor,
    SlackScheduleDraftIdentity, SlackScheduleDraftOwner, SlackScheduleDraftRestore,
    SlackThreadDraftHandle, SlackThreadPanelState, SurfaceState,
};
use crate::ui::surface::SlackScheduledPendingPhase;

impl SurfaceState {
    pub(crate) fn current_slack_main_schedule_owner(&self) -> Option<SlackScheduleDraftOwner> {
        let context = self.slack_active_main_composer_context.as_ref()?;
        Some(SlackScheduleDraftOwner::Main {
            handle: self.current_slack_main_composer_draft_handle()?,
            source: context.source.clone(),
        })
    }

    pub(crate) fn slack_thread_panel_schedule_owner(
        &self,
        panel: &SlackThreadPanelState,
    ) -> SlackScheduleDraftOwner {
        SlackScheduleDraftOwner::ThreadPanel {
            handle: SlackThreadDraftHandle::new(
                panel.reply_draft_key.clone(),
                panel.reply_draft.borrow().id,
            ),
            panel_generation: panel.generation,
        }
    }

    pub(crate) fn slack_all_threads_schedule_owner(
        &self,
        draft_key: &SlackComposerDraftKey,
        thread_key: SharedString,
    ) -> Option<SlackScheduleDraftOwner> {
        let draft = self.slack_composer_drafts.get(draft_key)?;
        Some(SlackScheduleDraftOwner::AllThreads {
            handle: SlackThreadDraftHandle::new(draft_key.clone(), draft.id),
            thread_key,
        })
    }

    pub(super) fn slack_schedule_owner_surface_is_current(
        &self,
        owner: &SlackScheduleDraftOwner,
    ) -> bool {
        match owner {
            SlackScheduleDraftOwner::Main { handle, source } => self
                .slack_active_main_composer_context
                .as_ref()
                .is_some_and(|context| context.owner == handle.owner && context.source == *source),
            SlackScheduleDraftOwner::ThreadPanel {
                handle,
                panel_generation,
            } => self.slack_thread_panel.as_ref().is_some_and(|panel| {
                panel.generation == *panel_generation
                    && panel.reply_draft_key == *handle.key()
                    && self.slack_thread_panel_origin_is_current(panel)
            }),
            SlackScheduleDraftOwner::AllThreads { handle, thread_key } => {
                self.slack_main_route == SlackMainRoute::AllThreads
                    && self
                        .slack_all_threads_composers
                        .get(thread_key)
                        .is_some_and(|composer| composer.target.draft_key() == handle.key())
            }
        }
    }

    fn slack_schedule_owner_draft_is_current(&self, owner: &SlackScheduleDraftOwner) -> bool {
        if !self.slack_schedule_owner_surface_is_current(owner) {
            return false;
        }
        match owner {
            SlackScheduleDraftOwner::Main { handle, .. } => {
                self.current_slack_main_composer_draft_handle().as_ref() == Some(handle)
            }
            SlackScheduleDraftOwner::ThreadPanel { handle, .. } => self
                .slack_thread_panel
                .as_ref()
                .is_some_and(|panel| panel.reply_draft.borrow().id == handle.draft_id()),
            SlackScheduleDraftOwner::AllThreads { handle, .. } => self
                .slack_composer_drafts
                .get(handle.key())
                .is_some_and(|draft| draft.id == handle.draft_id()),
        }
    }

    pub(super) fn slack_schedule_draft_identity(
        &self,
        owner: &SlackScheduleDraftOwner,
    ) -> Option<SlackScheduleDraftIdentity> {
        if !self.slack_schedule_owner_surface_is_current(owner) {
            return None;
        }
        let (key, destination, team_id, self_user_id) = match owner {
            SlackScheduleDraftOwner::Main { handle, .. } => {
                let context = self.slack_active_main_composer_context.as_ref()?;
                match &handle.owner {
                    SlackMainComposerDraftOwner::Conversation(key) => (
                        Some(key.clone()),
                        key.destination.clone(),
                        key.team_id.clone(),
                        key.self_user_id.clone(),
                    ),
                    SlackMainComposerDraftOwner::NewMessage(_) => (
                        None,
                        SlackComposerDestination::Conversation {
                            conversation_id: context.target.conversation_id.clone(),
                        },
                        context.target.team_id.clone(),
                        context.target.self_user_id.clone(),
                    ),
                }
            }
            SlackScheduleDraftOwner::ThreadPanel { handle, .. }
            | SlackScheduleDraftOwner::AllThreads { handle, .. } => (
                Some(handle.key().clone()),
                handle.key().destination.clone(),
                handle.key().team_id.clone(),
                handle.key().self_user_id.clone(),
            ),
        };
        let conversation_id = match &destination {
            SlackComposerDestination::Conversation { conversation_id }
            | SlackComposerDestination::Thread {
                conversation_id, ..
            } => conversation_id.clone(),
        };
        let workspace = self.slack_workspace()?;
        (workspace.team_id == team_id
            && workspace.self_user_id.as_deref() == Some(self_user_id.as_str()))
        .then_some(SlackScheduleDraftIdentity {
            team_id,
            self_user_id,
            conversation_id,
            destination,
            key,
        })
    }

    pub(super) fn snapshot_slack_schedule_draft(
        &self,
        owner: &SlackScheduleDraftOwner,
    ) -> Option<SlackComposerDraft> {
        if !self.slack_schedule_owner_draft_is_current(owner) {
            return None;
        }
        match owner {
            SlackScheduleDraftOwner::Main { .. } => Some(self.snapshot_slack_send_draft()),
            SlackScheduleDraftOwner::ThreadPanel { .. } => self
                .slack_thread_panel
                .as_ref()
                .map(|panel| panel.reply_draft.borrow().clone()),
            SlackScheduleDraftOwner::AllThreads { handle, .. } => {
                self.slack_composer_drafts.get(handle.key()).cloned()
            }
        }
    }

    pub(super) fn take_slack_schedule_draft(
        &mut self,
        owner: &SlackScheduleDraftOwner,
    ) -> Option<SlackComposerDraft> {
        if !self.slack_schedule_owner_draft_is_current(owner) {
            return None;
        }
        let draft = match owner {
            SlackScheduleDraftOwner::Main { .. } => {
                let draft = self.take_slack_send_draft();
                self.restore_slack_send_draft(None);
                draft
            }
            SlackScheduleDraftOwner::ThreadPanel { .. } => {
                let replacement_id = self.next_slack_composer_draft_id();
                let replacement_token = self.next_slack_composer_draft_token();
                let mut replacement = SlackComposerDraft::new(replacement_id);
                replacement.token = replacement_token;
                let panel = self
                    .slack_thread_panel
                    .as_mut()
                    .expect("validated Slack thread schedule owner must remain active");
                std::mem::replace(panel.reply_draft.get_mut(), replacement)
            }
            SlackScheduleDraftOwner::AllThreads { handle, .. } => self
                .slack_composer_drafts
                .remove(handle.key())
                .expect("validated Slack All Threads schedule draft must remain stored"),
        };
        assert_eq!(
            draft.id,
            owner.draft_id(),
            "Slack scheduling must take the exact typed draft owner"
        );
        Some(draft)
    }

    pub(super) fn restore_slack_schedule_draft(
        &mut self,
        owner: &SlackScheduleDraftOwner,
        draft: SlackComposerDraft,
    ) -> Result<SlackScheduleDraftRestore, Box<SlackComposerDraft>> {
        assert_eq!(
            draft.id,
            owner.draft_id(),
            "Slack schedule recovery must restore the exact accepted draft"
        );
        match owner {
            SlackScheduleDraftOwner::Main { .. } => {
                if self.slack_schedule_owner_surface_is_current(owner)
                    && self.slack_active_scheduled_edit.is_none()
                    && self.snapshot_slack_send_draft().is_empty()
                {
                    self.restore_slack_send_draft(Some(draft));
                    Ok(SlackScheduleDraftRestore::Active)
                } else {
                    Err(Box::new(draft))
                }
            }
            SlackScheduleDraftOwner::ThreadPanel { handle, .. } => {
                let panel_is_current = self.slack_schedule_owner_surface_is_current(owner);
                if panel_is_current {
                    let panel = self
                        .slack_thread_panel
                        .as_mut()
                        .expect("validated Slack thread schedule owner must remain active");
                    if panel.reply_draft.borrow().is_empty() {
                        *panel.reply_draft.get_mut() = draft;
                        return Ok(SlackScheduleDraftRestore::Active);
                    }
                    return Err(Box::new(draft));
                }
                self.restore_slack_stored_thread_schedule_draft(handle.key(), draft)
            }
            SlackScheduleDraftOwner::AllThreads { handle, .. } => {
                let active = self.slack_schedule_owner_surface_is_current(owner);
                self.restore_slack_stored_thread_schedule_draft(handle.key(), draft)
                    .map(|_| {
                        if active {
                            SlackScheduleDraftRestore::Active
                        } else {
                            SlackScheduleDraftRestore::Stored
                        }
                    })
            }
        }
    }

    fn restore_slack_stored_thread_schedule_draft(
        &mut self,
        key: &SlackComposerDraftKey,
        draft: SlackComposerDraft,
    ) -> Result<SlackScheduleDraftRestore, Box<SlackComposerDraft>> {
        if self
            .slack_thread_panel
            .as_ref()
            .is_some_and(|panel| panel.reply_draft_key == *key)
        {
            return Err(Box::new(draft));
        }
        if self
            .slack_composer_drafts
            .get(key)
            .is_some_and(|current| !current.is_empty())
        {
            return Err(Box::new(draft));
        }
        self.slack_composer_drafts.insert(key.clone(), draft);
        Ok(SlackScheduleDraftRestore::Stored)
    }

    pub(crate) fn has_slack_schedule_target(&self, owner: &SlackScheduleDraftOwner) -> bool {
        self.slack_schedule_controls_visible()
            && self.slack_schedule_draft_identity(owner).is_some()
    }

    pub(crate) fn slack_schedule_controls_visible(&self) -> bool {
        if !self.slack_workspace_api_capabilities.schedule_message {
            return false;
        }
        self.slack_workspace().is_some_and(|workspace| {
            workspace
                .self_timezone_id
                .as_deref()
                .is_some_and(|timezone| !timezone.trim().is_empty())
                && workspace
                    .self_timezone_label
                    .as_deref()
                    .is_some_and(|label| !label.trim().is_empty())
        })
    }

    pub(crate) fn slack_schedule_keyboard_anchor(&self) -> SlackScheduleAnchor {
        SlackScheduleAnchor {
            x: (self.preview_width - 16.0).max(8.0),
            y: (self.viewport_height - 48.0).max(8.0),
        }
    }

    pub(crate) fn can_schedule_slack_draft(&self, owner: &SlackScheduleDraftOwner) -> bool {
        if self.slack_schedule_create_or_update_is_blocked()
            || owner
                .draft_key()
                .is_some_and(|key| self.slack_thread_reply_is_pending(key))
        {
            return false;
        }
        match owner {
            SlackScheduleDraftOwner::AllThreads { handle, .. } => {
                let has_target = self.slack_schedule_controls_visible()
                    && self.slack_main_route == SlackMainRoute::AllThreads
                    && self.slack_workspace().is_some_and(|workspace| {
                        workspace.team_id == handle.key().team_id
                            && workspace.self_user_id.as_deref()
                                == Some(handle.key().self_user_id.as_str())
                    });
                has_target
                    && self
                        .slack_composer_drafts
                        .get(handle.key())
                        .filter(|draft| draft.id == handle.draft_id())
                        .is_some_and(slack_schedule_draft_is_ready)
            }
            SlackScheduleDraftOwner::ThreadPanel { handle, .. } => {
                self.has_slack_schedule_target(owner)
                    && self.slack_thread_panel.as_ref().is_some_and(|panel| {
                        let draft = panel.reply_draft.borrow();
                        draft.id == handle.draft_id() && slack_schedule_draft_is_ready(&draft)
                    })
            }
            SlackScheduleDraftOwner::Main { .. } => {
                self.has_slack_schedule_target(owner)
                    && !self.slack_composer_capture_blocks_current_draft()
                    && self
                        .snapshot_slack_schedule_draft(owner)
                        .as_ref()
                        .is_some_and(slack_schedule_draft_is_ready)
            }
        }
    }

    pub(crate) fn slack_schedule_create_or_update_is_blocked(&self) -> bool {
        self.slack_schedule_pending.is_some()
            || self.slack_scheduled_delete_pending.is_some()
            || self.slack_composer_schedule_recovery.is_some()
            || self.slack_scheduled_edit_recovery.is_some()
            || self
                .slack_pending_draft_restore
                .as_ref()
                .is_some_and(|restore| restore.scheduled_edit.is_some())
    }

    pub(in crate::ui::surface::state) fn slack_schedule_blocking_owner_diagnostic(
        &self,
    ) -> Option<String> {
        self.slack_schedule_pending
            .as_ref()
            .and_then(|pending| match &pending.phase {
                SlackScheduledPendingPhase::Submitting => None,
                SlackScheduledPendingPhase::ReconcilingUnknown { diagnostic, .. }
                | SlackScheduledPendingPhase::ProtectedUnknown { diagnostic } => {
                    Some(diagnostic.clone())
                }
            })
            .or_else(|| self.slack_composer_schedule_recovery_diagnostic())
            .or_else(|| {
                self.slack_scheduled_edit_recovery
                    .as_ref()
                    .map(|recovery| recovery.failure_diagnostic.clone())
            })
    }
}
