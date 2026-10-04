use crate::model::{
    SlackFileId, SlackFileStagingCancellationOutcome, SlackFileStagingCleanupOutcome,
    SlackFileStagingOperationFailure, SlackFileStagingOperationId, SlackFileStagingOutcome,
    SlackFileStagingReconcileOutcome, SlackUploadFile,
};

use crate::live::payload::workspace::SlackLiveWorkspaceLoader;

use super::{SlackFileDraftReservation, SlackFileShareReservation};

impl SlackLiveWorkspaceLoader {
    pub fn stage_file(
        &self,
        operation_id: &SlackFileStagingOperationId,
        file: SlackUploadFile,
    ) -> SlackFileStagingOutcome {
        self.file_staging_ledger
            .stage(&self.api, operation_id, file)
    }

    pub fn cancel_file_staging(
        &self,
        operation_id: &SlackFileStagingOperationId,
    ) -> SlackFileStagingCancellationOutcome {
        self.file_staging_ledger.cancel(&self.api, operation_id)
    }

    pub fn reconcile_file_staging(
        &self,
        operation_id: &SlackFileStagingOperationId,
    ) -> SlackFileStagingReconcileOutcome {
        self.file_staging_ledger.reconcile(&self.api, operation_id)
    }

    pub fn cleanup_staged_file(
        &self,
        operation_id: &SlackFileStagingOperationId,
    ) -> SlackFileStagingCleanupOutcome {
        self.file_staging_ledger.cleanup(&self.api, operation_id)
    }

    pub(crate) fn reserve_staged_file_share(
        &self,
        file_ids: &[SlackFileId],
    ) -> Result<SlackFileShareReservation, SlackFileStagingOperationFailure> {
        self.file_staging_ledger.reserve_share(file_ids)
    }

    pub(crate) fn abort_staged_file_share(
        &self,
        reservation: SlackFileShareReservation,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        self.file_staging_ledger.abort_share(&self.api, reservation)
    }

    pub(crate) fn protect_staged_file_share_unknown(
        &self,
        reservation: SlackFileShareReservation,
        diagnostic: String,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        self.file_staging_ledger
            .protect_share_unknown(&self.api, reservation, diagnostic)
    }

    pub(crate) fn confirm_staged_file_share(
        &self,
        reservation: SlackFileShareReservation,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        self.file_staging_ledger
            .confirm_share(&self.api, reservation)
    }

    pub(crate) fn reserve_staged_file_draft(
        &self,
        file_ids: &[SlackFileId],
    ) -> Result<SlackFileDraftReservation, SlackFileStagingOperationFailure> {
        self.file_staging_ledger.reserve_draft(file_ids)
    }

    pub(crate) fn abort_staged_file_draft(
        &self,
        reservation: SlackFileDraftReservation,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        self.file_staging_ledger.abort_draft(&self.api, reservation)
    }

    pub(crate) fn protect_staged_file_draft_unknown(
        &self,
        reservation: SlackFileDraftReservation,
        diagnostic: String,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        self.file_staging_ledger
            .protect_draft_unknown(&self.api, reservation, diagnostic)
    }

    pub(crate) fn confirm_staged_file_draft(
        &self,
        reservation: SlackFileDraftReservation,
    ) -> Result<(), SlackFileStagingOperationFailure> {
        self.file_staging_ledger
            .confirm_draft(&self.api, reservation)
    }

    pub(crate) fn confirm_staged_file_draft_unknown(
        &self,
        local_files: &[crate::model::SlackScheduledDraftLocalFile],
    ) -> Result<(), SlackFileStagingOperationFailure> {
        self.file_staging_ledger.confirm_draft_unknown(local_files)
    }
}
