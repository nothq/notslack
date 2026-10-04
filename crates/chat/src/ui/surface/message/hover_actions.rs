use crate::ui::surface::{SlackMessageRenderContext, SlackMessageRow, SurfaceState};

mod actions;

#[derive(Clone, Copy)]
pub(super) struct SlackMessageHoverActionsContext {
    render_context: SlackMessageRenderContext,
    reaction_supported: bool,
    reply_supported: bool,
    forward_supported: bool,
    save_supported: bool,
    more_supported: bool,
}

impl SlackMessageHoverActionsContext {
    pub(super) fn for_message(
        surface: &SurfaceState,
        row: &SlackMessageRow,
        render_context: SlackMessageRenderContext,
    ) -> Self {
        let target = row.action_target.as_deref();
        let current = target.is_some_and(|target| {
            surface
                .slack_message_action_target_is_current(target.team_id(), target.conversation_id())
        });
        let own_message = surface
            .slack_workspace()
            .and_then(|workspace| workspace.self_user_id.as_deref())
            .is_some_and(|self_user_id| row.user_id.as_deref() == Some(self_user_id));
        let edit_supported = own_message
            && current
            && row.edit_round_trip_supported
            && surface.slack_workspace_api_capabilities.update_message
            && surface.slack_pending_message_edit.is_none();
        let delete_supported = own_message
            && current
            && surface.slack_workspace_api_capabilities.delete_message
            && surface.slack_pending_message_delete.is_none()
            && surface.slack_pending_message_edit.is_none();
        Self {
            render_context,
            reaction_supported: surface.slack_workspace_api_capabilities.mutate_reactions
                && target.is_some(),
            reply_supported: target.is_some()
                && (surface.slack_workspace_api_capabilities.load_thread
                    || !row.replies.is_empty()),
            forward_supported: surface.slack_workspace_api_capabilities.forward_message
                && surface
                    .slack_workspace_api_capabilities
                    .load_destination_directory
                && target.is_some(),
            save_supported: surface
                .slack_workspace_api_capabilities
                .mutate_message_saved_state
                && target.is_some_and(|target| !target.is_thread_reply()),
            more_supported: target.is_some()
                && (!row.body.is_empty()
                    || surface
                        .slack_workspace_api_capabilities
                        .load_message_permalink
                    || surface
                        .slack_workspace_api_capabilities
                        .mark_conversation_read
                    || edit_supported
                    || delete_supported),
        }
    }

    pub(super) fn any_supported(self) -> bool {
        self.reaction_supported
            || self.reply_supported
            || self.forward_supported
            || self.save_supported
            || self.more_supported
    }
}
