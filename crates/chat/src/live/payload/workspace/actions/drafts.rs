use crate::model::{
    SlackDraftContent, SlackDraftFileDeletion, SlackDraftReceipt, SlackDraftTarget,
    SlackDraftUpdateTarget, SlackDraftWriteTarget, SlackFileId, SlackScheduledDraftCreateTarget,
    SlackScheduledDraftLocalFile, SlackScheduledDraftMutationFailure, SlackScheduledDraftReceipt,
    SlackScheduledDraftReconcileTarget, SlackScheduledDraftUpdateTarget,
};

use crate::live::payload::workspace::{
    file_staging::SlackFileDraftReservation, ops, SlackLiveWorkspaceLoader, SlackMessageDraft,
};

impl SlackLiveWorkspaceLoader {
    pub fn create_scheduled_draft(
        &self,
        target: SlackScheduledDraftCreateTarget<'_>,
        content: SlackDraftContent<'_, SlackMessageDraft>,
        file_ids: &[SlackFileId],
    ) -> Result<SlackScheduledDraftReceipt, SlackScheduledDraftMutationFailure> {
        let reservation = self
            .reserve_staged_file_draft(file_ids)
            .map_err(|failure| SlackScheduledDraftMutationFailure::NotSent {
                diagnostic: failure.to_string(),
            })?;
        let outcome = ops::create_scheduled_draft(&self.api, target, content, file_ids);
        self.finish_scheduled_draft_mutation(reservation, outcome, || {
            self.reconcile_scheduled_draft_create(
                target.client_message_id(),
                target.write_target(),
                target.post_at_unix_seconds(),
                file_ids,
            )
        })
    }

    pub fn create_draft(
        &self,
        client_message_id: &crate::model::SlackMessageClientId,
        write_target: &SlackDraftWriteTarget,
        content: SlackDraftContent<'_, SlackMessageDraft>,
        file_ids: &[SlackFileId],
    ) -> Result<SlackDraftReceipt, String> {
        ops::create_draft(
            &self.api,
            client_message_id,
            write_target,
            content,
            file_ids,
        )
    }

    pub fn update_draft(
        &self,
        update_target: SlackDraftUpdateTarget<'_>,
        write_target: &SlackDraftWriteTarget,
        content: SlackDraftContent<'_, SlackMessageDraft>,
        file_ids: &[SlackFileId],
    ) -> Result<SlackDraftReceipt, String> {
        ops::update_draft(&self.api, update_target, write_target, content, file_ids)
    }

    pub fn delete_draft(
        &self,
        target: &SlackDraftTarget,
        file_deletion: SlackDraftFileDeletion,
    ) -> Result<(), String> {
        ops::delete_draft(&self.api, target, file_deletion)
    }

    pub fn update_scheduled_draft(
        &self,
        target: SlackScheduledDraftUpdateTarget<'_>,
        content: SlackDraftContent<'_, SlackMessageDraft>,
        file_ids: &[SlackFileId],
    ) -> Result<SlackScheduledDraftReceipt, SlackScheduledDraftMutationFailure> {
        let reservation = self
            .reserve_staged_file_draft(file_ids)
            .map_err(|failure| SlackScheduledDraftMutationFailure::NotSent {
                diagnostic: failure.to_string(),
            })?;
        let outcome = ops::update_scheduled_draft(&self.api, target, content, file_ids);
        self.finish_scheduled_draft_mutation(reservation, outcome, || {
            self.reconcile_scheduled_draft_update(
                target.update_target(),
                target.write_targets(),
                target.post_at_unix_seconds(),
                file_ids,
            )
        })
    }

    fn finish_scheduled_draft_mutation(
        &self,
        reservation: SlackFileDraftReservation,
        outcome: ops::SlackScheduledDraftMutationOutcome,
        reconcile: impl FnOnce() -> Result<Option<SlackScheduledDraftReceipt>, String>,
    ) -> Result<SlackScheduledDraftReceipt, SlackScheduledDraftMutationFailure> {
        match outcome {
            ops::SlackScheduledDraftMutationOutcome::Confirmed(receipt) => {
                self.confirm_staged_file_draft(reservation)
                    .map_err(|failure| SlackScheduledDraftMutationFailure::Unknown {
                        diagnostic: failure.to_string(),
                    })?;
                Ok(receipt)
            }
            ops::SlackScheduledDraftMutationOutcome::NotSent { diagnostic } => {
                self.abort_staged_file_draft(reservation)
                    .map_err(|failure| {
                        SlackScheduledDraftMutationFailure::Unknown {
                            diagnostic: format!(
                                "{diagnostic}; failed to release staged-file draft reservation: {failure}"
                            ),
                        }
                    })?;
                Err(SlackScheduledDraftMutationFailure::NotSent { diagnostic })
            }
            ops::SlackScheduledDraftMutationOutcome::Rejected { diagnostic } => {
                self.abort_staged_file_draft(reservation)
                    .map_err(|failure| SlackScheduledDraftMutationFailure::Unknown {
                        diagnostic: format!(
                            "{diagnostic}; failed to release staged-file draft reservation: {failure}"
                        ),
                    })?;
                Err(SlackScheduledDraftMutationFailure::Rejected { diagnostic })
            }
            ops::SlackScheduledDraftMutationOutcome::Unknown { diagnostic } => {
                self.finish_unknown_scheduled_draft_mutation(reservation, diagnostic, reconcile)
            }
        }
    }

    fn finish_unknown_scheduled_draft_mutation(
        &self,
        reservation: SlackFileDraftReservation,
        diagnostic: String,
        reconcile: impl FnOnce() -> Result<Option<SlackScheduledDraftReceipt>, String>,
    ) -> Result<SlackScheduledDraftReceipt, SlackScheduledDraftMutationFailure> {
        match reconcile() {
            Ok(Some(receipt)) => {
                self.confirm_staged_file_draft(reservation)
                    .map_err(|failure| SlackScheduledDraftMutationFailure::Unknown {
                        diagnostic: failure.to_string(),
                    })?;
                Ok(receipt)
            }
            Ok(None) => self.protect_unknown_scheduled_draft(
                reservation,
                format!(
                    "{diagnostic}; Slack drafts.list did not confirm the scheduled draft mutation"
                ),
            ),
            Err(reconciliation_diagnostic) => self.protect_unknown_scheduled_draft(
                reservation,
                format!(
                    "{diagnostic}; Slack scheduled-draft reconciliation failed: {reconciliation_diagnostic}"
                ),
            ),
        }
    }

    fn protect_unknown_scheduled_draft(
        &self,
        reservation: SlackFileDraftReservation,
        diagnostic: String,
    ) -> Result<SlackScheduledDraftReceipt, SlackScheduledDraftMutationFailure> {
        self.protect_staged_file_draft_unknown(reservation, diagnostic.clone())
            .map_err(|failure| SlackScheduledDraftMutationFailure::Unknown {
                diagnostic: format!(
                    "{diagnostic}; failed to protect ambiguous staged-file draft ownership: {failure}"
                ),
            })?;
        Err(SlackScheduledDraftMutationFailure::Unknown { diagnostic })
    }

    pub fn reconcile_scheduled_draft(
        &self,
        target: SlackScheduledDraftReconcileTarget<'_>,
        file_ids: &[SlackFileId],
        local_files: &[SlackScheduledDraftLocalFile],
    ) -> Result<Option<SlackScheduledDraftReceipt>, String> {
        let receipt = match target {
            SlackScheduledDraftReconcileTarget::Create(target) => self
                .reconcile_scheduled_draft_create(
                    target.client_message_id(),
                    target.write_target(),
                    target.post_at_unix_seconds(),
                    file_ids,
                )?,
            SlackScheduledDraftReconcileTarget::Update(target) => self
                .reconcile_scheduled_draft_update(
                    target.update_target(),
                    target.write_targets(),
                    target.post_at_unix_seconds(),
                    file_ids,
                )?,
        };
        if receipt.is_some() {
            self.confirm_staged_file_draft_unknown(local_files)
                .map_err(|failure| failure.to_string())?;
        }
        Ok(receipt)
    }

    pub fn delete_scheduled_draft(&self, target: &SlackDraftTarget) -> Result<(), String> {
        ops::delete_draft(&self.api, target, SlackDraftFileDeletion::Delete)
    }
}
