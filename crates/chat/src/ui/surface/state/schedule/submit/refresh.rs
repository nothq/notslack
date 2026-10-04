use super::super::{Context, SlackScheduleDraftOwner, SurfaceState};
use super::SlackScheduledListRefresh;
use crate::ui::surface::{
    PreparedSlackDraftsSentSnapshot, SlackSchedulePendingSubmission, SlackScheduledMutationIdentity,
};
use crate::ui::{SlackDraftsSentRequest, SlackDraftsSentTab};

impl SurfaceState {
    pub(super) fn slack_scheduled_list_refresh(
        pending: &SlackSchedulePendingSubmission,
    ) -> SlackScheduledListRefresh {
        SlackScheduledListRefresh {
            request: SlackDraftsSentRequest {
                team_id: pending.identity.team_id.clone(),
                tab: SlackDraftsSentTab::Scheduled,
                cursor: None,
            },
            timezone: pending.timezone,
            authenticated_self_user_id: pending.identity.self_user_id.clone(),
        }
    }

    pub(super) fn apply_slack_schedule_refresh(
        &mut self,
        identity: &SlackScheduledMutationIdentity,
        refreshed: Result<PreparedSlackDraftsSentSnapshot, String>,
    ) -> Option<String> {
        if self.slack_drafts_sent_team_id.as_deref() != Some(identity.team_id.as_str())
            || self.slack_drafts_sent_self_user_id.as_deref()
                != Some(identity.self_user_id.as_str())
            || self.slack_workspace().is_none_or(|workspace| {
                workspace.team_id != identity.team_id
                    || workspace.self_user_id.as_deref() != Some(identity.self_user_id.as_str())
            })
        {
            return None;
        }
        let refresh_error = match refreshed {
            Ok(prepared) => self
                .apply_authoritative_slack_scheduled_snapshot(prepared)
                .err()
                .map(|error| format!("The scheduled list could not refresh: {error}")),
            Err(error) => {
                self.invalidate_slack_scheduled_snapshot();
                Some(format!(
                    "The scheduled list could not refresh after Slack changed it: {error}"
                ))
            }
        };
        self.slack_drafts_sent_error.clone_from(&refresh_error);
        refresh_error
    }

    pub(in crate::ui::surface::state) fn fail_slack_schedule(
        &mut self,
        owner: &SlackScheduleDraftOwner,
        message: &str,
        cx: &mut Context<Self>,
    ) {
        self.slack_schedule_overlay = None;
        self.slack_schedule_submission_error = Some(message.to_string());
        self.set_slack_schedule_owner_error(owner, message);
        self.focus_slack_schedule_owner(owner);
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn show_slack_schedule_error(
        &mut self,
        owner: &SlackScheduleDraftOwner,
        message: &str,
        cx: &mut Context<Self>,
    ) {
        self.slack_schedule_submission_error = Some(message.to_string());
        self.set_slack_schedule_owner_error(owner, message);
        if let Some(custom) = self
            .slack_schedule_overlay
            .as_mut()
            .filter(|overlay| overlay.owner == *owner)
            .and_then(|overlay| overlay.custom_mut())
        {
            custom.error = Some(message.to_string());
        }
        cx.notify();
    }
}
