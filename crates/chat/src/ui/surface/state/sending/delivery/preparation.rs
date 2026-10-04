use crate::ui::surface::{
    slack_message_timezone, SlackComposerDraft, SlackMainComposerDraftHandle, SlackSendDraftSource,
    SurfaceState,
};
use crate::ui::{Context, SlackMessageDraft};

use super::super::PreparedSlackSend;

struct PreparedSlackSendTarget {
    team_id: String,
    self_user_id: String,
    conversation_id: String,
    draft_source: SlackSendDraftSource,
    author: String,
    avatar_label: Option<String>,
    avatar_image_url: Option<String>,
    timezone: chrono_tz::Tz,
}

impl SurfaceState {
    pub(crate) fn send_slack_message(&mut self, cx: &mut Context<Self>) {
        if self.slack_active_scheduled_edit.is_some() || self.slack_schedule_pending.is_some() {
            return;
        }
        if !self.slack_workspace_api_capabilities.send_message {
            return;
        }
        if !self.slack_composer_files.is_empty()
            && (!self.slack_workspace_api_capabilities.share_files
                || !self.slack_composer_files.slack_file_ids_ready())
        {
            return;
        }
        let Some(prepared) = self.prepare_slack_send_request(cx) else {
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            let _ = self.fail_slack_send::<()>("missing Slack workspace api", false, cx);
            return;
        };

        let (delivery, work) = match self.build_slack_send_work(prepared) {
            Ok(send) => send,
            Err(message) => {
                let _ = self.fail_slack_send::<()>(&message, true, cx);
                return;
            }
        };
        let request = delivery.request.clone();
        self.slack_pending_sends
            .insert(request.generation, request.clone());
        self.insert_slack_outbound_delivery(delivery);
        self.clear_matching_slack_send_draft(&request);
        self.clear_matching_slack_stored_send_draft(&request);
        self.slack_error = None;
        cx.notify();

        self.spawn_slack_send_attempt(workspace_api, work, cx);
    }

    fn prepare_slack_send_request(&mut self, cx: &mut Context<Self>) -> Option<PreparedSlackSend> {
        let PreparedSlackSendTarget {
            team_id,
            self_user_id,
            conversation_id,
            draft_source,
            author,
            avatar_label,
            avatar_image_url,
            timezone,
        } = self.prepare_slack_send_target(cx)?;
        let draft_text = self.slack_composer_text.clone();
        let text = draft_text.trim().to_string();
        if text.is_empty() && self.slack_composer_files.is_empty() {
            return self.fail_slack_send("Add a message or attachment before sending.", true, cx);
        }
        let message_draft = match self.export_slack_send_draft(&draft_text, text.is_empty()) {
            Ok(draft) => draft,
            Err(message) => return self.fail_slack_send(&message, true, cx),
        };
        if !self.slack_composer_files.is_empty()
            && !self.slack_workspace_api_capabilities.share_files
        {
            return self.fail_slack_send(
                "File sharing is unavailable for this Slack workspace.",
                true,
                cx,
            );
        }
        let file_ids = match self.slack_composer_files.projected_slack_file_ids() {
            Ok(file_ids) => file_ids,
            Err(message) => return self.fail_slack_send(&message, true, cx),
        };
        let (accepted_draft_handle, accepted_draft) = self.prepare_slack_accepted_draft(cx)?;
        Some(PreparedSlackSend {
            team_id,
            self_user_id,
            conversation_id,
            draft_source,
            author,
            avatar_label,
            avatar_image_url,
            timezone,
            draft_text,
            message_draft,
            accepted_draft_handle,
            accepted_draft,
            file_ids,
        })
    }

    fn prepare_slack_accepted_draft(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<(SlackMainComposerDraftHandle, SlackComposerDraft)> {
        let accepted_draft = self.snapshot_slack_send_draft();
        let accepted_draft_handle =
            self.current_slack_main_composer_draft_handle()
                .or_else(|| {
                    self.fail_slack_send(
                        "Slack message delivery requires an active conversation composer.",
                        false,
                        cx,
                    )
                })?;
        assert_eq!(
            accepted_draft.id, accepted_draft_handle.draft_id,
            "Slack outbound delivery must retain the exact accepted draft identity"
        );
        Some((accepted_draft_handle, accepted_draft))
    }

    fn prepare_slack_send_target(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<PreparedSlackSendTarget> {
        let context = self
            .slack_active_main_composer_context
            .clone()
            .filter(|context| {
                self.current_slack_send_target_id() == Some(context.target.conversation_id.as_str())
            })
            .or_else(|| {
                self.fail_slack_send(
                    "Wait for the selected Slack conversation to finish loading before sending.",
                    false,
                    cx,
                )
            })?;
        let (author, avatar_label, avatar_image_url, timezone) = {
            let workspace = self.slack_workspace()?;
            if workspace.team_id != context.target.team_id
                || workspace.self_user_id.as_deref() != Some(context.target.self_user_id.as_str())
            {
                return self.fail_slack_send(
                    "Slack message delivery requires the active authenticated workspace identity.",
                    false,
                    cx,
                );
            }
            (
                workspace
                    .self_display_name
                    .clone()
                    .unwrap_or_else(|| "You".to_string()),
                workspace.self_avatar_label.clone(),
                workspace.self_avatar_image_url.clone(),
                slack_message_timezone(workspace.self_timezone_id.as_deref()),
            )
        };
        Some(PreparedSlackSendTarget {
            team_id: context.target.team_id,
            self_user_id: context.target.self_user_id,
            conversation_id: context.target.conversation_id,
            draft_source: context.source,
            author,
            avatar_label,
            avatar_image_url,
            timezone,
        })
    }

    fn export_slack_send_draft(
        &self,
        draft_text: &str,
        text_is_empty: bool,
    ) -> Result<Option<SlackMessageDraft>, String> {
        if text_is_empty {
            return Ok(None);
        }
        let mut document = self.slack_composer_document.borrow_mut();
        document.reset_to_text_if_changed(draft_text);
        document.export_message_draft().map(Some)
    }
}
