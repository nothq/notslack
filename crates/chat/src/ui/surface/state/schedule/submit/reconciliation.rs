use super::super::{Context, SurfaceState};
use super::execution::{
    append_refresh_error, execute_slack_schedule_reconcile_work, schedule_remote_request,
};
use super::{
    SlackScheduleReconcileResult, SlackScheduleReconcileWork, SLACK_SCHEDULE_RECONCILE_BASE_DELAY,
    SLACK_SCHEDULE_RECONCILE_MAX_ATTEMPTS,
};
use crate::ui::surface::{
    SlackScheduleDraftOwner, SlackScheduledMutationIdentity, SlackScheduledPendingPhase,
};

struct SlackScheduleReconciliationContext {
    identity: SlackScheduledMutationIdentity,
    attempt: u8,
    owner: SlackScheduleDraftOwner,
    mirrors_active_composer: bool,
    refresh_error: Option<String>,
}

type SlackScheduleReconciliationOutcome =
    Result<Option<crate::ui::SlackScheduledDraftReceipt>, String>;

impl SurfaceState {
    pub(super) fn schedule_slack_unknown_reconciliation(
        &mut self,
        identity: SlackScheduledMutationIdentity,
        attempt: u8,
        cx: &mut Context<Self>,
    ) {
        let exponent = u32::from(attempt.saturating_sub(1));
        let delay = SLACK_SCHEDULE_RECONCILE_BASE_DELAY
            .checked_mul(1_u32 << exponent)
            .expect("Slack scheduled-draft reconciliation delay overflowed");
        self.spawn_timer_task((identity, attempt), delay, cx, |this, request, cx| {
            this.begin_slack_unknown_reconciliation(request.0, request.1, cx);
        });
    }

    fn begin_slack_unknown_reconciliation(
        &mut self,
        identity: SlackScheduledMutationIdentity,
        attempt: u8,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = self.slack_schedule_pending.as_ref() else {
            return;
        };
        if pending.identity != identity
            || !matches!(
                &pending.phase,
                SlackScheduledPendingPhase::ReconcilingUnknown {
                    attempt: active_attempt,
                    ..
                } if *active_attempt == attempt
            )
        {
            return;
        }
        let refresh = Self::slack_scheduled_list_refresh(pending);
        let work = SlackScheduleReconcileWork {
            request: schedule_remote_request(pending),
            refresh,
            attempt,
        };
        self.spawn_background_task(
            work,
            cx,
            |work: SlackScheduleReconcileWork| execute_slack_schedule_reconcile_work(work),
            |this, result, cx| {
                this.finish_slack_unknown_reconciliation(result, cx);
            },
        );
    }

    fn finish_slack_unknown_reconciliation(
        &mut self,
        result: SlackScheduleReconcileResult,
        cx: &mut Context<Self>,
    ) {
        let Some((context, outcome)) =
            self.prepare_slack_schedule_reconciliation_completion(result)
        else {
            return;
        };
        self.apply_slack_schedule_reconciliation_outcome(context, outcome, cx);
    }

    fn prepare_slack_schedule_reconciliation_completion(
        &mut self,
        result: SlackScheduleReconcileResult,
    ) -> Option<(
        SlackScheduleReconciliationContext,
        SlackScheduleReconciliationOutcome,
    )> {
        if !self.slack_schedule_reconciliation_is_current(&result) {
            return None;
        }
        let SlackScheduleReconcileResult {
            identity,
            attempt,
            outcome,
            refreshed,
        } = result;
        let (mirrors_active_composer, owner) = self
            .slack_schedule_pending
            .as_ref()
            .map(|pending| {
                (
                    self.slack_schedule_pending_origin_is_current(pending),
                    pending.owner.clone(),
                )
            })
            .expect("validated reconciliation must retain its pending owner");
        let refresh_error = self.apply_slack_schedule_refresh(&identity, refreshed);
        Some((
            SlackScheduleReconciliationContext {
                identity,
                attempt,
                owner,
                mirrors_active_composer,
                refresh_error,
            },
            outcome,
        ))
    }

    fn apply_slack_schedule_reconciliation_outcome(
        &mut self,
        context: SlackScheduleReconciliationContext,
        outcome: SlackScheduleReconciliationOutcome,
        cx: &mut Context<Self>,
    ) {
        let attempt = context.attempt;
        match outcome {
            Ok(Some(_)) => self.confirm_reconciled_slack_schedule(context.refresh_error, cx),
            Ok(None) if attempt < SLACK_SCHEDULE_RECONCILE_MAX_ATTEMPTS => self
                .retry_slack_schedule_reconciliation(
                    context,
                    "Slack has not yet confirmed the scheduled draft mutation.".to_string(),
                    cx,
                ),
            Err(error) if attempt < SLACK_SCHEDULE_RECONCILE_MAX_ATTEMPTS => self
                .retry_slack_schedule_reconciliation(
                    context,
                    format!("Slack scheduled-draft reconciliation failed: {error}"),
                    cx,
                ),
            Ok(None) => self.protect_unknown_slack_schedule(
                context,
                "Slack could not confirm whether the scheduled draft mutation was accepted. The draft and its files remain protected.".to_string(),
                cx,
            ),
            Err(error) => self.protect_unknown_slack_schedule(
                context,
                format!(
                    "Slack could not resolve the scheduled draft mutation: {error}. The draft and its files remain protected."
                ),
                cx,
            ),
        }
    }

    fn slack_schedule_reconciliation_is_current(
        &self,
        result: &SlackScheduleReconcileResult,
    ) -> bool {
        self.slack_schedule_pending.as_ref().is_some_and(|pending| {
            pending.identity == result.identity
                && matches!(
                    &pending.phase,
                    SlackScheduledPendingPhase::ReconcilingUnknown {
                        attempt: active_attempt,
                        ..
                    } if *active_attempt == result.attempt
                )
        })
    }

    fn confirm_reconciled_slack_schedule(
        &mut self,
        refresh_error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let pending = self
            .slack_schedule_pending
            .take()
            .expect("reconciled schedule must retain its pending owner");
        self.confirm_slack_schedule_owner(pending, refresh_error, cx);
    }

    fn retry_slack_schedule_reconciliation(
        &mut self,
        context: SlackScheduleReconciliationContext,
        diagnostic: String,
        cx: &mut Context<Self>,
    ) {
        let next_attempt = context.attempt + 1;
        let pending = self
            .slack_schedule_pending
            .as_mut()
            .expect("retrying reconciliation must retain its pending owner");
        pending.phase = SlackScheduledPendingPhase::ReconcilingUnknown {
            attempt: next_attempt,
            diagnostic: diagnostic.clone(),
        };
        self.publish_slack_schedule_reconciliation_diagnostic(&context, diagnostic);
        cx.notify();
        self.schedule_slack_unknown_reconciliation(context.identity, next_attempt, cx);
    }

    fn protect_unknown_slack_schedule(
        &mut self,
        context: SlackScheduleReconciliationContext,
        diagnostic: String,
        cx: &mut Context<Self>,
    ) {
        let pending = self
            .slack_schedule_pending
            .as_mut()
            .expect("protected reconciliation must retain its pending owner");
        pending.phase = SlackScheduledPendingPhase::ProtectedUnknown {
            diagnostic: diagnostic.clone(),
        };
        self.publish_slack_schedule_reconciliation_diagnostic(&context, diagnostic);
        cx.notify();
    }

    fn publish_slack_schedule_reconciliation_diagnostic(
        &mut self,
        context: &SlackScheduleReconciliationContext,
        diagnostic: String,
    ) {
        self.slack_schedule_submission_error = Some(diagnostic.clone());
        if context.mirrors_active_composer {
            let diagnostic = append_refresh_error(diagnostic, context.refresh_error.clone());
            self.set_slack_schedule_owner_error(&context.owner, &diagnostic);
        }
    }

    pub(in crate::ui::surface::state) fn resume_protected_slack_schedule_reconciliation(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some(pending) = self.slack_schedule_pending.as_mut() else {
            return;
        };
        if !matches!(
            pending.phase,
            SlackScheduledPendingPhase::ProtectedUnknown { .. }
        ) {
            return;
        }
        pending.phase = SlackScheduledPendingPhase::ReconcilingUnknown {
            attempt: 1,
            diagnostic: "Rechecking the protected Slack scheduled draft mutation.".to_string(),
        };
        let identity = pending.identity.clone();
        self.schedule_slack_unknown_reconciliation(identity, 1, cx);
    }
}
