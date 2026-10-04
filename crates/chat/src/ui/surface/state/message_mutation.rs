mod delete;

use crate::ui::surface::{
    prepare_slack_conversation_refresh, prepare_slack_conversation_snapshot,
    PreparedSlackConversationSnapshot, SlackComposerDocument, SlackMessageDeleteModal,
    SlackMessageDeleteRequest, SlackMessageEditRequest, SlackMessageEditTarget,
    SlackMessageEditView, SlackMessageForwardSource,
};
use crate::ui::{
    Context, SlackConversationKind, SlackMessage, SlackMessageDraft, SlackMessageTimestamp,
    SurfaceState,
};
use gpui::AppContext;

impl SurfaceState {
    pub(crate) fn slack_message_can_edit(&self, message_id: &str) -> bool {
        self.slack_workspace_api_capabilities.update_message
            && self.slack_pending_message_edit.is_none()
            && self
                .current_own_slack_message(message_id)
                .and_then(SlackComposerDocument::from_message)
                .is_some()
    }

    pub(crate) fn slack_message_can_delete(&self, message_id: &str) -> bool {
        self.slack_workspace_api_capabilities.delete_message
            && self.slack_pending_message_delete.is_none()
            && self.slack_pending_message_edit.is_none()
            && self.current_own_slack_message(message_id).is_some()
    }

    pub(crate) fn open_slack_message_edit(&mut self, message_id: &str, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.update_message
            || self.slack_pending_message_edit.is_some()
        {
            return;
        }
        let Some(message) = self.current_own_slack_message(message_id) else {
            return;
        };
        let Some(document) = SlackComposerDocument::from_message(message) else {
            return;
        };
        let Some(workspace) = self.slack_workspace() else {
            return;
        };
        let Ok(message_timestamp) = SlackMessageTimestamp::parse(message_id) else {
            return;
        };
        let target = SlackMessageEditTarget {
            team_id: workspace.team_id.clone(),
            conversation_id: workspace.conversation_id.clone(),
            message_timestamp,
        };
        self.slack_message_edit_generation = self
            .slack_message_edit_generation
            .checked_add(1)
            .expect("Slack message edit generation overflowed");
        let surface = cx.entity().downgrade();
        let editor_target = target.clone();
        let editor =
            cx.new(move |cx| SlackMessageEditView::new(surface, editor_target, document, cx));
        self.slack_reaction_picker = None;
        self.slack_message_menu = None;
        self.slack_message_menu_focus_pending = false;
        self.slack_message_forward_modal = None;
        self.slack_message_edit = Some(editor);
        self.slack_error = None;
        self.slack_message_list_state.remeasure();
        cx.notify();
    }

    pub(crate) fn cancel_slack_message_edit(
        &mut self,
        target: &SlackMessageEditTarget,
        cx: &mut Context<Self>,
    ) {
        let matches = self
            .slack_message_edit
            .as_ref()
            .is_some_and(|editor| editor.read(cx).target() == target);
        if !matches {
            return;
        }
        self.slack_message_edit = None;
        self.slack_pending_message_edit = None;
        self.slack_message_edit_generation = self
            .slack_message_edit_generation
            .checked_add(1)
            .expect("Slack message edit generation overflowed");
        self.slack_message_list_state.remeasure();
        cx.notify();
    }

    pub(crate) fn start_slack_message_edit(
        &mut self,
        target: SlackMessageEditTarget,
        draft: SlackMessageDraft,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if !self.slack_workspace_api_capabilities.update_message {
            return Err("Slack message editing is unavailable.".to_string());
        }
        if self.slack_pending_message_edit.is_some() {
            return Err("Another Slack message edit is pending.".to_string());
        }
        let editor_matches = self
            .slack_message_edit
            .as_ref()
            .is_some_and(|editor| editor.read(cx).target() == &target);
        if !editor_matches
            || !self
                .slack_message_action_target_is_current(&target.team_id, &target.conversation_id)
            || self
                .current_own_slack_message(target.message_timestamp.as_str())
                .is_none()
        {
            return Err("The Slack message changed while it was being edited.".to_string());
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return Err("Slack message editing requires a connected workspace.".to_string());
        };
        let existing = self
            .slack_conversation_snapshot
            .clone()
            .expect("current Slack edit target requires a conversation snapshot");
        let request = SlackMessageEditRequest {
            generation: self.slack_message_edit_generation,
            target,
            draft,
        };
        self.slack_pending_message_edit = Some(request.clone());
        let completion_request = request.clone();
        self.spawn_background_task(
            (request, existing),
            cx,
            move |(request, existing)| {
                let refreshed = workspace_api.update_slack_message(
                    &request.target.conversation_id,
                    &request.target.message_timestamp,
                    &request.draft,
                )?;
                let fallback = refreshed.clone();
                prepare_slack_conversation_refresh(existing, refreshed).map(|prepared| {
                    prepared.unwrap_or_else(|| prepare_slack_conversation_snapshot(fallback))
                })
            },
            move |this, result, cx| {
                this.finish_slack_message_edit(&completion_request, result, cx);
            },
        );
        Ok(())
    }

    fn finish_slack_message_edit(
        &mut self,
        request: &SlackMessageEditRequest,
        result: Result<PreparedSlackConversationSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_pending_message_edit.as_ref() != Some(request) {
            return;
        }
        self.slack_pending_message_edit = None;
        if self.slack_message_edit_generation != request.generation
            || !self.slack_message_action_target_is_current(
                &request.target.team_id,
                &request.target.conversation_id,
            )
        {
            return;
        }
        match result {
            Ok(prepared)
                if prepared.snapshot.team_id == request.target.team_id
                    && prepared.snapshot.conversation_id == request.target.conversation_id
                    && prepared
                        .snapshot
                        .messages
                        .iter()
                        .any(|message| message.id == request.target.message_timestamp.as_str()) =>
            {
                self.slack_message_edit = None;
                self.apply_prepared_slack_conversation_snapshot(prepared, cx);
            }
            Ok(_) => self.finish_slack_message_edit_error(
                &request.target,
                "Slack returned a mismatched edited message.".to_string(),
                cx,
            ),
            Err(error) => self.finish_slack_message_edit_error(&request.target, error, cx),
        }
    }

    fn finish_slack_message_edit_error(
        &mut self,
        target: &SlackMessageEditTarget,
        error: String,
        cx: &mut Context<Self>,
    ) {
        let Some(editor) = self
            .slack_message_edit
            .as_ref()
            .filter(|editor| editor.read(cx).target() == target)
        else {
            return;
        };
        editor.update(cx, |editor, cx| {
            editor.finish_pending(Some(error), cx);
        });
    }
}
