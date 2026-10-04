use super::{
    Context, Range, SlackComposerDocument, SlackComposerFormatAction, SlackComposerTarget,
    SurfaceState,
};
use crate::ui::surface::SlackReplyComposerTarget;
use gpui::{Entity, FocusHandle};
use gpui_components::text_input::TextInput;

mod insertion;
mod pickers;

impl SurfaceState {
    pub(in crate::ui::surface) fn slack_thread_panel_reply_composer_target(
        &self,
    ) -> Option<SlackReplyComposerTarget> {
        let panel = self.slack_thread_panel.as_ref()?;
        Some(SlackReplyComposerTarget::ThreadPanel {
            panel_generation: panel.generation,
            parent_message_id: panel.parent_message_id.clone().into(),
            draft_key: panel.reply_draft_key.clone(),
        })
    }

    pub(in crate::ui::surface) fn slack_reply_composer_input(
        &self,
        target: &SlackReplyComposerTarget,
    ) -> Option<Entity<TextInput>> {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => self
                .slack_reply_composer_target_is_current(target)
                .then(|| self.slack_thread_composer_input.clone()),
            SlackReplyComposerTarget::AllThreads { thread_key, .. } => self
                .slack_all_threads_composers
                .get(thread_key)
                .filter(|composer| composer.target == *target)
                .map(|composer| composer.input.clone()),
        }
    }

    pub(in crate::ui::surface) fn slack_reply_formatting_enabled(
        &self,
        target: &SlackReplyComposerTarget,
    ) -> bool {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => self
                .slack_thread_panel
                .as_ref()
                .filter(|_| self.slack_reply_composer_target_is_current(target))
                .is_some_and(|panel| panel.reply_formatting_enabled),
            SlackReplyComposerTarget::AllThreads { thread_key, .. } => self
                .slack_all_threads_composers
                .get(thread_key)
                .filter(|composer| composer.target == *target)
                .is_some_and(|composer| composer.formatting_enabled),
        }
    }

    pub(in crate::ui::surface) fn slack_reply_format_roving_target(
        &self,
        target: &SlackReplyComposerTarget,
    ) -> Option<SlackComposerFormatAction> {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => self
                .slack_thread_panel
                .as_ref()
                .filter(|_| self.slack_reply_composer_target_is_current(target))
                .map(|panel| panel.reply_format_roving_target),
            SlackReplyComposerTarget::AllThreads { thread_key, .. } => self
                .slack_all_threads_composers
                .get(thread_key)
                .filter(|composer| composer.target == *target)
                .map(|composer| composer.format_roving_target),
        }
    }

    pub(in crate::ui::surface) fn slack_reply_format_focus_handle(
        &self,
        target: &SlackReplyComposerTarget,
        action: SlackComposerFormatAction,
    ) -> Option<FocusHandle> {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => self
                .slack_reply_composer_target_is_current(target)
                .then(|| self.slack_thread_format_focus_handles[action.toolbar_index()].clone()),
            SlackReplyComposerTarget::AllThreads { thread_key, .. } => self
                .slack_all_threads_composers
                .get(thread_key)
                .filter(|composer| composer.target == *target)
                .map(|composer| composer.format_focus_handles[action.toolbar_index()].clone()),
        }
    }

    pub(in crate::ui::surface) fn slack_reply_composer_target_is_current(
        &self,
        target: &SlackReplyComposerTarget,
    ) -> bool {
        match target {
            SlackReplyComposerTarget::ThreadPanel {
                panel_generation,
                parent_message_id,
                draft_key,
            } => self.slack_thread_panel.as_ref().is_some_and(|panel| {
                panel.generation == *panel_generation
                    && panel.parent_message_id == parent_message_id.as_ref()
                    && panel.reply_draft_key == *draft_key
                    && self.slack_thread_panel_origin_is_current(panel)
            }),
            SlackReplyComposerTarget::AllThreads { thread_key, .. } => {
                self.slack_main_route == crate::ui::surface::SlackMainRoute::AllThreads
                    && self
                        .slack_all_threads_composers
                        .get(thread_key)
                        .is_some_and(|composer| composer.target == *target)
            }
        }
    }

    pub(in crate::ui::surface) fn can_mutate_slack_reply_composer(
        &self,
        target: &SlackReplyComposerTarget,
    ) -> bool {
        self.slack_workspace_api_capabilities.send_thread_reply
            && self.slack_reply_composer_target_is_current(target)
            && !self.slack_thread_reply_is_pending(target.draft_key())
    }

    pub(crate) fn toggle_slack_thread_reply_broadcast(&mut self, cx: &mut Context<Self>) {
        let Some(enabled) = self
            .slack_thread_panel
            .as_ref()
            .map(|panel| !panel.reply_draft.borrow().broadcast)
        else {
            return;
        };
        self.set_slack_thread_reply_broadcast(enabled, cx);
    }

    pub(crate) fn set_slack_thread_reply_broadcast(
        &mut self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.send_thread_reply {
            return;
        }
        let can_toggle = self.slack_thread_panel.as_ref().is_some_and(|panel| {
            panel.broadcast_label.is_some()
                && !self.slack_thread_reply_is_pending(&panel.reply_draft_key)
                && self.slack_thread_panel_origin_is_current(panel)
                && panel.reply_draft.borrow().broadcast != enabled
        });
        if !can_toggle {
            return;
        }
        let draft_token = self.next_slack_composer_draft_token();
        let panel = self
            .slack_thread_panel
            .as_mut()
            .expect("validated Slack thread disappeared while toggling reply broadcast");
        let draft = panel.reply_draft.get_mut();
        draft.broadcast = enabled;
        draft.token = draft_token;
        draft.client_message_id = None;
        let draft_key = panel.reply_draft_key.clone();
        if panel.reply_error.take().is_some() {
            panel.list_state.remeasure();
        }
        self.slack_composer_draft_changed(draft_key, draft_token, cx);
        cx.notify();
    }

    pub(crate) fn set_slack_thread_reply_text(
        &mut self,
        value: String,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.slack_workspace_api_capabilities.send_thread_reply
            || self.slack_thread_panel.as_ref().is_none_or(|panel| {
                self.slack_thread_reply_is_pending(&panel.reply_draft_key)
                    || !self.slack_thread_panel_origin_is_current(panel)
                    || panel.reply_draft.borrow().text() == value
            })
        {
            return false;
        }
        if let Some(target) = self.slack_thread_panel_reply_composer_target() {
            self.clear_slack_toolbar_mention_picker(&SlackComposerTarget::Reply(target));
        }
        let draft_token = self.next_slack_composer_draft_token();
        let panel = self
            .slack_thread_panel
            .as_mut()
            .expect("validated Slack thread disappeared while editing its draft");
        let changed = panel
            .reply_draft
            .get_mut()
            .replace_text(&value, draft_token);
        let draft_key = panel.reply_draft_key.clone();
        panel.reply_composer_focused = true;
        if panel.reply_error.take().is_some() {
            panel.list_state.remeasure();
        }
        if changed {
            self.slack_composer_draft_changed(draft_key, draft_token, cx);
        }
        cx.notify();
        changed
    }

    pub(in crate::ui::surface) fn toggle_slack_reply_formatting(
        &mut self,
        target: &SlackReplyComposerTarget,
        cx: &mut Context<Self>,
    ) {
        if !self.can_mutate_slack_reply_composer(target) {
            return;
        }
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => {
                let panel = self
                    .slack_thread_panel
                    .as_mut()
                    .expect("validated Slack thread disappeared while toggling reply formatting");
                panel.reply_formatting_enabled = !panel.reply_formatting_enabled;
                panel.list_state.remeasure();
            }
            SlackReplyComposerTarget::AllThreads { thread_key, .. } => {
                let composer = self
                    .slack_all_threads_composers
                    .get_mut(thread_key)
                    .expect("validated All Threads composer disappeared while toggling formatting");
                composer.formatting_enabled = !composer.formatting_enabled;
                self.slack_all_threads_list_state.remeasure();
            }
        }
        cx.notify();
    }

    pub(in crate::ui::surface) fn apply_slack_reply_format(
        &mut self,
        target: &SlackReplyComposerTarget,
        action: SlackComposerFormatAction,
        cx: &mut Context<Self>,
    ) {
        if !self.can_mutate_slack_reply_composer(target) {
            return;
        }
        self.set_slack_reply_format_roving_target(target, action);
        if action == SlackComposerFormatAction::Link {
            self.open_slack_reply_link_dialog(target.clone(), cx);
            return;
        }
        if !SlackComposerDocument::supports(action) {
            return;
        }
        let selection = self.slack_reply_format_selection(target, cx);
        let draft_token = self.next_slack_composer_draft_token();
        if !self.apply_slack_reply_format_to_draft(target, action, selection) {
            return;
        }
        self.finish_slack_reply_format(target, draft_token, cx);
        cx.notify();
    }

    pub(in crate::ui::surface) fn set_slack_reply_format_roving_target(
        &mut self,
        target: &SlackReplyComposerTarget,
        action: SlackComposerFormatAction,
    ) {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => {
                self.slack_thread_panel
                    .as_mut()
                    .expect("validated Slack thread disappeared while applying reply formatting")
                    .reply_format_roving_target = action;
            }
            SlackReplyComposerTarget::AllThreads { thread_key, .. } => {
                self.slack_all_threads_composers
                    .get_mut(thread_key)
                    .expect("validated All Threads composer disappeared while applying formatting")
                    .format_roving_target = action;
            }
        }
    }

    fn slack_reply_format_selection(
        &self,
        target: &SlackReplyComposerTarget,
        cx: &mut Context<Self>,
    ) -> Range<usize> {
        let input = self
            .slack_reply_composer_input(target)
            .expect("validated Slack reply composer must retain its input");
        let draft_text = match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => self
                .slack_thread_panel
                .as_ref()
                .expect("validated Slack thread disappeared while reading reply formatting")
                .reply_draft
                .borrow()
                .text()
                .to_string(),
            SlackReplyComposerTarget::AllThreads { draft_key, .. } => self
                .slack_composer_drafts
                .get(draft_key)
                .map(|draft| draft.text().to_string())
                .unwrap_or_default(),
        };
        let input = input.read(cx);
        if input.text() == draft_text {
            input.selection_range()
        } else {
            draft_text.len()..draft_text.len()
        }
    }

    fn apply_slack_reply_format_to_draft(
        &mut self,
        target: &SlackReplyComposerTarget,
        action: SlackComposerFormatAction,
        selection: Range<usize>,
    ) -> bool {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => self
                .slack_thread_panel
                .as_mut()
                .expect("validated Slack thread disappeared while applying reply formatting")
                .reply_draft
                .get_mut()
                .document
                .toggle_format(action, selection),
            SlackReplyComposerTarget::AllThreads {
                thread_key,
                draft_key,
                ..
            } => {
                if !self.slack_composer_drafts.contains_key(draft_key) {
                    let draft_id = self.next_slack_composer_draft_id();
                    self.slack_composer_drafts.insert(
                        draft_key.clone(),
                        crate::ui::surface::SlackComposerDraft::new(draft_id),
                    );
                }
                let applied = self
                    .slack_composer_drafts
                    .get_mut(draft_key)
                    .expect("All Threads draft was inserted before applying formatting")
                    .document
                    .toggle_format(action, selection);
                if applied {
                    self.slack_all_threads_reply_errors
                        .remove(thread_key.as_ref());
                }
                applied
            }
        }
    }

    fn finish_slack_reply_format(
        &mut self,
        target: &SlackReplyComposerTarget,
        draft_token: u64,
        cx: &mut Context<Self>,
    ) {
        match target {
            SlackReplyComposerTarget::ThreadPanel { draft_key, .. } => {
                let panel = self
                    .slack_thread_panel
                    .as_mut()
                    .expect("formatted Slack thread disappeared while updating its draft");
                let draft = panel.reply_draft.get_mut();
                draft.token = draft_token;
                draft.client_message_id = None;
                panel.reply_composer_focused = true;
                if panel.reply_error.take().is_some() {
                    panel.list_state.remeasure();
                }
                self.slack_composer_draft_changed(draft_key.clone(), draft_token, cx);
            }
            SlackReplyComposerTarget::AllThreads { draft_key, .. } => {
                let draft = self
                    .slack_composer_drafts
                    .get_mut(draft_key)
                    .expect("formatted All Threads draft disappeared while updating its token");
                draft.token = draft_token;
                draft.client_message_id = None;
                self.slack_composer_draft_changed(draft_key.clone(), draft_token, cx);
            }
        }
    }
}
