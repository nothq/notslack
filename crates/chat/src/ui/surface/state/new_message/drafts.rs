use super::{Arc, Context, SlackMainRoute, SurfaceState, Window};
use crate::ui::surface::{
    SlackComposerDraft, SlackMainComposerDraftOwner, SlackNewMessageDraftKey,
};

impl SurfaceState {
    pub(crate) fn store_slack_new_message_draft(&mut self, cx: &mut Context<Self>) {
        self.restore_slack_scheduled_edit_prior_draft(cx);
        let Some(key) = self.active_slack_new_message_draft_key() else {
            return;
        };
        let owner = SlackMainComposerDraftOwner::NewMessage(key.clone());
        if self
            .slack_active_main_composer_context
            .as_ref()
            .is_none_or(|context| context.owner != owner)
        {
            return;
        }
        let draft = self.take_slack_send_draft();
        self.clear_slack_main_composer_after_draft_taken(&owner);
        if let Some(existing) = self.slack_new_message_drafts.get(&key) {
            assert!(
                existing.files.is_empty()
                    || (existing.id == draft.id
                        && existing.files.same_identity_and_order(&draft.files)),
                "storing a Slack new-message draft cannot replace a different file-owning draft"
            );
        }
        if draft.is_empty() {
            self.slack_new_message_drafts.remove(&key);
        } else {
            self.slack_new_message_drafts.insert(key, draft);
        }
    }

    pub(crate) fn restore_slack_new_message_draft(&mut self) {
        if self.slack_routed_main_composer_context().is_none() {
            return;
        }
        self.install_slack_routed_main_composer_after_workspace_preparation();
        let draft = self
            .active_slack_new_message_draft_key()
            .as_ref()
            .and_then(|key| self.slack_new_message_drafts.remove(key));
        self.restore_slack_send_draft(draft);
        self.slack_composer_focused = false;
        self.slack_error = None;
    }

    pub(in crate::ui::surface::state) fn active_slack_new_message_draft(
        &self,
    ) -> Option<SlackComposerDraft> {
        (matches!(
            self.slack_active_main_composer_context
                .as_ref()
                .map(|context| &context.owner),
            Some(SlackMainComposerDraftOwner::NewMessage(_))
        ) && self.slack_active_scheduled_edit.is_none())
        .then(|| self.snapshot_slack_send_draft())
    }

    fn active_slack_new_message_draft_key(&self) -> Option<SlackNewMessageDraftKey> {
        let workspace = self.slack_workspace()?;
        Some(SlackNewMessageDraftKey {
            team_id: workspace.team_id.clone(),
            self_user_id: workspace.self_user_id.clone()?,
            draft_key: self.slack_new_message_active_draft_key.clone()?,
        })
    }

    pub(crate) fn queue_slack_new_message_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        if self.slack_main_route != SlackMainRoute::NewMessage {
            return;
        }
        let range = (
            visible_start.min(self.slack_new_message_visible_row_indices.len()),
            visible_end.min(self.slack_new_message_visible_row_indices.len()),
        );
        if self.slack_new_message_prefetched_range == Some(range) {
            return;
        }
        self.slack_new_message_prefetched_range = Some(range);
        let urls = self.slack_new_message_visible_row_indices[range.0..range.1]
            .iter()
            .filter_map(|row_index| self.slack_new_message_rows.get(*row_index))
            .filter_map(|row| row.avatar_image_url.as_ref())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }

    pub(crate) fn ensure_slack_new_message_focus_observers(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = self
            .slack_new_message_to_input
            .read(cx)
            .focus_handle_clone();
        if !self.slack_new_message_to_focus_observers_registered {
            cx.on_focus(&focus, window, |surface, _, cx| {
                if surface.slack_main_route == SlackMainRoute::NewMessage
                    && !surface.slack_new_message_to_focused
                {
                    surface.slack_new_message_to_focused = true;
                    surface.rebuild_slack_new_message_results();
                    cx.notify();
                }
            })
            .detach();
            cx.on_blur(&focus, window, |surface, _, cx| {
                if surface.slack_new_message_to_focused {
                    surface.slack_new_message_to_focused = false;
                    surface.slack_new_message_visible_row_indices = Arc::default();
                    surface.slack_new_message_selected_index = None;
                    surface.slack_new_message_prefetched_range = None;
                    cx.notify();
                }
            })
            .detach();
            self.slack_new_message_to_focus_observers_registered = true;
        }
        if self.slack_main_route != SlackMainRoute::NewMessage && focus.is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }
    }
}
