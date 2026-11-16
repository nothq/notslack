//! The profile panel's Message button: opens the direct message with the
//! person, starting one when there is none yet.

use std::sync::Arc;

use super::{Context, SurfaceState, WorkspaceApi};
use crate::ui::{SlackConversationOpenReceipt, SlackConversationOpenRequest};

impl SurfaceState {
    pub(crate) fn open_slack_dm_with_user(&mut self, user_id: &str, cx: &mut Context<Self>) {
        if let Some(conversation_id) = self.slack_existing_dm_with_user(user_id) {
            self.close_slack_profile_panel(cx);
            self.select_slack_conversation(&conversation_id, cx);
            return;
        }
        let Some(team_id) = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone())
            .filter(|team_id| !team_id.is_empty())
        else {
            return;
        };
        let Ok(request) =
            SlackConversationOpenRequest::new(team_id.clone(), vec![user_id.to_string()])
        else {
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_error = Some("Connect to Slack to message this person.".to_string());
            cx.notify();
            return;
        };
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackConversationOpenRequest)| {
                workspace_api.open_slack_conversation(request)
            },
            move |this, result: Result<SlackConversationOpenReceipt, String>, cx| match result {
                Ok(receipt) if receipt.team_id == team_id => {
                    this.close_slack_profile_panel(cx);
                    this.select_slack_conversation(&receipt.conversation_id, cx);
                }
                Ok(_) => {}
                Err(error) => {
                    this.slack_error = Some(error);
                    cx.notify();
                }
            },
        );
    }

    fn slack_existing_dm_with_user(&self, user_id: &str) -> Option<String> {
        self.slack_workspace()?
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .find(|item| item.user_id.as_deref() == Some(user_id) && !item.target_id.is_empty())
            .map(|item| item.target_id.clone())
    }
}
