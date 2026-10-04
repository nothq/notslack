use super::{Arc, Context, SurfaceState, WorkspaceApi};
use crate::ui::{SlackConversationOpenReceipt, SlackConversationOpenRequest};

const SLACKBOT_USER_ID: &str = "USLACK";
const LEGACY_SLACKBOT_USER_ID: &str = "USLACKBOT";

impl SurfaceState {
    pub(crate) fn slack_slackbot_control_available(&self) -> bool {
        self.slack_workspace_api_capabilities.open_conversation
            && self.slack_workspace_api_capabilities.load_conversation
            && self
                .slack_workspace()
                .is_some_and(|workspace| !workspace.team_id.is_empty())
    }

    pub(crate) fn open_slack_slackbot(&mut self, cx: &mut Context<Self>) {
        if !self.slack_slackbot_control_available() || self.slack_slackbot_opening {
            return;
        }
        if let Some(conversation_id) = self.slack_slackbot_conversation_id() {
            self.select_slack_conversation(&conversation_id, cx);
            return;
        }

        let team_id = self
            .slack_workspace()
            .expect("available Slackbot control requires a Slack workspace")
            .team_id
            .clone();
        let request =
            SlackConversationOpenRequest::new(team_id.clone(), vec![SLACKBOT_USER_ID.to_string()])
                .expect("canonical Slackbot identity must form a valid conversation request");
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_error = Some("missing Slack workspace api".to_string());
            cx.notify();
            return;
        };

        self.slack_slackbot_opening = true;
        self.slack_error = None;
        cx.notify();
        self.spawn_background_task(
            (workspace_api, request),
            cx,
            |(workspace_api, request): (Arc<dyn WorkspaceApi>, SlackConversationOpenRequest)| {
                let result = workspace_api.open_slack_conversation(request.clone());
                (request, result)
            },
            move |this, (request, result), cx| {
                this.finish_slack_slackbot_open(team_id, request, result, cx);
            },
        );
    }

    fn slack_slackbot_conversation_id(&self) -> Option<String> {
        self.slack_workspace()
            .into_iter()
            .flat_map(|workspace| workspace.sections.iter())
            .flat_map(|section| section.items.iter())
            .find(|item| {
                item.user_id.as_deref().is_some_and(slack_slackbot_user_id)
                    && !item.target_id.is_empty()
            })
            .map(|item| item.target_id.clone())
            .or_else(|| {
                self.slack_new_message_directory_snapshot
                    .as_ref()?
                    .candidates
                    .iter()
                    .find(|candidate| {
                        matches!(
                            candidate.target,
                            crate::ui::SlackDestinationTarget::Conversation { .. }
                        ) && candidate
                            .participant_user_ids
                            .iter()
                            .any(|user_id| slack_slackbot_user_id(user_id))
                    })
                    .map(|candidate| candidate.target.stable_id().to_string())
            })
            .or_else(|| {
                self.slack_dm_inbox_snapshot
                    .as_ref()?
                    .slackbot_conversation_id
                    .clone()
            })
    }

    fn finish_slack_slackbot_open(
        &mut self,
        team_id: String,
        request: SlackConversationOpenRequest,
        result: Result<SlackConversationOpenReceipt, String>,
        cx: &mut Context<Self>,
    ) {
        self.slack_slackbot_opening = false;
        if self
            .slack_workspace()
            .map(|workspace| workspace.team_id.as_str())
            != Some(team_id.as_str())
        {
            cx.notify();
            return;
        }
        match result {
            Ok(receipt) if receipt.team_id == team_id && receipt.user_ids == request.user_ids() => {
                self.slack_error = None;
                self.select_slack_conversation(&receipt.conversation_id, cx);
            }
            Ok(_) => {
                self.slack_error =
                    Some("Slack conversations.open returned another request.".to_string());
                cx.notify();
            }
            Err(error) => {
                self.slack_error = Some(error);
                cx.notify();
            }
        }
    }
}

fn slack_slackbot_user_id(user_id: &str) -> bool {
    matches!(user_id, SLACKBOT_USER_ID | LEGACY_SLACKBOT_USER_ID)
}
