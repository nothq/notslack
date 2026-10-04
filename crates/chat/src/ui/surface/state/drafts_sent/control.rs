use crate::model::{
    ChatScheduleConversationRecovery, ChatScheduleEditState, ChatSchedulePendingMutation,
    ChatSchedulePendingPhase, ChatScheduleState, ChatScheduledItem,
};

use super::rows::slack_drafts_sent_tab_index;
use super::{Context, SlackDraftsSentRowTarget, SlackDraftsSentTab, SurfaceState};
use crate::ui::surface::SlackDraftsSentRow;

struct SlackSchedulePendingControl {
    mutation: Option<ChatSchedulePendingMutation>,
    phase: Option<ChatSchedulePendingPhase>,
}

impl SurfaceState {
    pub(crate) fn control_slack_schedule_state(&self) -> Result<ChatScheduleState, String> {
        let pending = self.control_slack_schedule_pending();
        let conversation_recovery =
            self.slack_composer_schedule_recovery
                .as_ref()
                .map(|recovery| ChatScheduleConversationRecovery {
                    team_id: recovery.identity.team_id.clone(),
                    self_user_id: recovery.identity.self_user_id.clone(),
                    conversation_id: recovery.identity.conversation_id.clone(),
                    file_ids: recovery.file_ids.iter().map(ToString::to_string).collect(),
                    failure_diagnostic: recovery.failure_diagnostic.clone(),
                });
        Ok(ChatScheduleState {
            pending_create_or_update: pending.mutation,
            pending_phase: pending.phase,
            pending_delete_draft_id: self
                .slack_scheduled_delete_pending
                .as_ref()
                .map(|pending| pending.target.draft_id().to_string()),
            conversation_recovery,
            edit: self.control_slack_schedule_edit(),
            submission_error: self.slack_schedule_submission_error.clone(),
            scheduled_list_error: self.slack_drafts_sent_error.clone(),
            items: self.control_slack_scheduled_items()?,
        })
    }

    fn control_slack_schedule_pending(&self) -> SlackSchedulePendingControl {
        let mutation =
            self.slack_schedule_pending
                .as_ref()
                .map(|pending| match &pending.mutation {
                    crate::ui::surface::SlackScheduledPendingMutation::Update { edit, .. } => {
                        ChatSchedulePendingMutation::Update {
                            conversation_id: pending.identity.conversation_id.clone(),
                            draft_id: edit.target.draft_id().to_string(),
                            file_ids: pending.file_ids.iter().map(ToString::to_string).collect(),
                            post_at_unix_seconds: pending.post_at_unix_seconds,
                        }
                    }
                    crate::ui::surface::SlackScheduledPendingMutation::Create { .. }
                    | crate::ui::surface::SlackScheduledPendingMutation::Promote { .. } => {
                        ChatSchedulePendingMutation::Create {
                            conversation_id: pending.identity.conversation_id.clone(),
                            file_ids: pending.file_ids.iter().map(ToString::to_string).collect(),
                            post_at_unix_seconds: pending.post_at_unix_seconds,
                        }
                    }
                });
        let phase = self
            .slack_schedule_pending
            .as_ref()
            .map(|pending| match &pending.phase {
                crate::ui::surface::SlackScheduledPendingPhase::Submitting => {
                    ChatSchedulePendingPhase::Submitting
                }
                crate::ui::surface::SlackScheduledPendingPhase::ReconcilingUnknown {
                    attempt,
                    diagnostic,
                } => ChatSchedulePendingPhase::ReconcilingUnknown {
                    attempt: *attempt,
                    diagnostic: diagnostic.clone(),
                },
                crate::ui::surface::SlackScheduledPendingPhase::ProtectedUnknown { diagnostic } => {
                    ChatSchedulePendingPhase::ProtectedUnknown {
                        diagnostic: diagnostic.clone(),
                    }
                }
            });
        SlackSchedulePendingControl { mutation, phase }
    }

    fn control_slack_schedule_edit(&self) -> Option<ChatScheduleEditState> {
        self.slack_active_scheduled_edit
            .as_ref()
            .map(|active| ChatScheduleEditState::Active {
                conversation_id: active.edit.conversation_id.clone(),
                draft_id: active.edit.target.draft_id().to_string(),
            })
            .or_else(|| {
                let recovery = self.slack_scheduled_edit_recovery.as_ref()?;
                Some(ChatScheduleEditState::PendingNavigation {
                    conversation_id: recovery.edit.conversation_id.clone(),
                    draft_id: recovery.edit.target.draft_id().to_string(),
                    failure_diagnostic: Some(recovery.failure_diagnostic.clone()),
                })
            })
            .or_else(|| {
                let pending = self
                    .slack_pending_draft_restore
                    .as_ref()?
                    .scheduled_edit
                    .as_ref()?;
                Some(ChatScheduleEditState::PendingNavigation {
                    conversation_id: pending.conversation_id.clone(),
                    draft_id: pending.target.draft_id().to_string(),
                    failure_diagnostic: None,
                })
            })
    }

    fn control_slack_scheduled_items(&self) -> Result<Vec<ChatScheduledItem>, String> {
        let index = slack_drafts_sent_tab_index(SlackDraftsSentTab::Scheduled);
        self.slack_drafts_sent_snapshots[index]
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .items
                    .iter()
                    .map(|item| {
                        let destination = item.primary_destination().ok_or_else(|| {
                            format!("Slack scheduled draft {} omitted its destination", item.id)
                        })?;
                        Ok(ChatScheduledItem {
                            draft_id: item.id.to_string(),
                            revision: item.revision.to_string(),
                            conversation_id: destination.conversation_id.clone(),
                            body: item.body.clone(),
                            file_ids: item.file_ids.iter().map(ToString::to_string).collect(),
                            post_at_unix_seconds: i64::try_from(item.scheduled_unix_seconds)
                                .map_err(|_| {
                                    format!(
                                        "Slack scheduled draft {} timestamp overflowed",
                                        item.id
                                    )
                                })?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()
            })
            .transpose()
            .map(Option::unwrap_or_default)
    }

    pub(crate) fn control_edit_slack_scheduled_draft(
        &mut self,
        draft_id: &str,
        cx: &mut Context<Self>,
    ) -> Result<ChatScheduleState, String> {
        if let Some(diagnostic) = self.slack_schedule_blocking_owner_diagnostic() {
            return Err(diagnostic);
        }
        if self.slack_schedule_pending.is_some() {
            return Err("a Slack scheduled draft create or update is already pending".to_string());
        }
        if self.slack_scheduled_delete_pending.is_some() {
            return Err("a Slack scheduled draft cancellation is already pending".to_string());
        }
        let target = self.control_slack_scheduled_row_target(draft_id)?;
        let SlackDraftsSentRowTarget::Scheduled { edit, .. } = &target else {
            return Err(format!(
                "cached Slack scheduled row {draft_id} is not editable"
            ));
        };
        let expected_target = edit.target.clone();
        self.activate_slack_drafts_sent_target(target, cx);
        let edit_active = self
            .slack_active_scheduled_edit
            .as_ref()
            .is_some_and(|active| active.edit.target == expected_target);
        let edit_pending_navigation = self
            .slack_pending_draft_restore
            .as_ref()
            .and_then(|restore| restore.scheduled_edit.as_ref())
            .is_some_and(|pending| pending.target == expected_target);
        if !edit_active && !edit_pending_navigation {
            return Err(format!(
                "Slack scheduled draft {draft_id} edit could not be initiated"
            ));
        }
        self.control_slack_schedule_state()
    }

    pub(crate) fn control_cancel_slack_scheduled_draft(
        &mut self,
        draft_id: &str,
        cx: &mut Context<Self>,
    ) -> Result<ChatScheduleState, String> {
        if let Some(diagnostic) = self.slack_schedule_blocking_owner_diagnostic() {
            return Err(diagnostic);
        }
        if self.slack_schedule_pending.is_some() {
            return Err("a Slack scheduled draft create or update is already pending".to_string());
        }
        if let Some(pending) = self.slack_scheduled_delete_pending.as_ref() {
            if pending.target.draft_id().as_str() == draft_id {
                return self.control_slack_schedule_state();
            }
            return Err(
                "a different Slack scheduled draft cancellation is already pending".to_string(),
            );
        }
        let target = self.control_slack_scheduled_row_target(draft_id)?;
        let SlackDraftsSentRowTarget::Scheduled { edit, .. } = target else {
            return Err(format!(
                "cached Slack scheduled row {draft_id} is not cancelable"
            ));
        };
        if !self
            .slack_workspace_api_capabilities
            .delete_scheduled_message
        {
            return Err("Slack scheduled draft cancellation is unavailable".to_string());
        }
        self.delete_slack_scheduled_draft(edit, cx);
        if self
            .slack_scheduled_delete_pending
            .as_ref()
            .is_none_or(|pending| pending.target.draft_id().as_str() != draft_id)
        {
            return Err(self.slack_drafts_sent_error.clone().unwrap_or_else(|| {
                format!("Slack scheduled draft {draft_id} cancellation could not be initiated")
            }));
        }
        self.control_slack_schedule_state()
    }

    fn control_slack_scheduled_row_target(
        &self,
        draft_id: &str,
    ) -> Result<SlackDraftsSentRowTarget, String> {
        let index = slack_drafts_sent_tab_index(SlackDraftsSentTab::Scheduled);
        self.slack_drafts_sent_cached_rows[index]
            .iter()
            .find_map(|row| match row {
                SlackDraftsSentRow::Item(row) if row.id.as_ref() == draft_id => {
                    Some(row.target.clone())
                }
                SlackDraftsSentRow::Item(_) | SlackDraftsSentRow::DateDivider { .. } => None,
            })
            .ok_or_else(|| {
                format!("cached authoritative Slack scheduled items contain no draft {draft_id}")
            })
    }
}
