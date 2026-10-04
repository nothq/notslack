use super::super::{Context, Range, SlackComposerDocument, SurfaceState};
use crate::ui::surface::SlackReplyComposerTarget;

impl SurfaceState {
    pub(in crate::ui::surface::state) fn insert_slack_thread_reply_snippet(
        &mut self,
        snippet: &str,
        cx: &mut Context<Self>,
    ) {
        if !self.can_send_slack_thread_reply() {
            return;
        }
        let draft_token = self.next_slack_composer_draft_token();
        let (draft_key, text) = {
            let panel = self
                .slack_thread_panel
                .as_mut()
                .expect("current Slack thread disappeared while inserting into its reply");
            let draft = panel.reply_draft.get_mut();
            let mut text = draft.text().to_string();
            if !text.is_empty() && !text.ends_with(' ') {
                text.push(' ');
            }
            text.push_str(snippet);
            draft.replace_text(&text, draft_token);
            panel.reply_composer_focused = true;
            if panel.reply_error.take().is_some() {
                panel.list_state.remeasure();
            }
            (panel.reply_draft_key.clone(), text)
        };
        self.slack_composer_draft_changed(draft_key, draft_token, cx);
        self.sync_slack_thread_composer_input_to_end(text, cx);
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn insert_slack_reply_snippet(
        &mut self,
        target: &SlackReplyComposerTarget,
        snippet: &str,
        cx: &mut Context<Self>,
    ) {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => {
                if self.slack_reply_composer_target_is_current(target) {
                    self.insert_slack_thread_reply_snippet(snippet, cx);
                }
            }
            SlackReplyComposerTarget::AllThreads {
                thread_key,
                draft_key,
                ..
            } => {
                if !self.can_mutate_slack_reply_composer(target) {
                    return;
                }
                let draft_token = self.next_slack_composer_draft_token();
                if !self.slack_composer_drafts.contains_key(draft_key) {
                    let draft_id = self.next_slack_composer_draft_id();
                    self.slack_composer_drafts.insert(
                        draft_key.clone(),
                        crate::ui::surface::SlackComposerDraft::new(draft_id),
                    );
                }
                let text = {
                    let draft = self
                        .slack_composer_drafts
                        .get_mut(draft_key)
                        .expect("All Threads draft was inserted before snippet insertion");
                    let mut text = draft.text().to_string();
                    if !text.is_empty() && !text.ends_with(' ') {
                        text.push(' ');
                    }
                    text.push_str(snippet);
                    draft.replace_text(&text, draft_token);
                    text
                };
                self.slack_all_threads_reply_errors
                    .remove(thread_key.as_ref());
                self.slack_composer_draft_changed(draft_key.clone(), draft_token, cx);
                self.sync_slack_reply_composer_input_to_end(target, text, cx);
                cx.notify();
            }
        }
    }

    pub(in crate::ui::surface::state) fn insert_slack_thread_reply_entity(
        &mut self,
        insert: impl FnOnce(&mut SlackComposerDocument),
        cx: &mut Context<Self>,
    ) {
        if !self.can_send_slack_thread_reply() {
            return;
        }
        let draft_token = self.next_slack_composer_draft_token();
        let (draft_key, text) = {
            let panel = self
                .slack_thread_panel
                .as_mut()
                .expect("current Slack thread disappeared while inserting into its reply");
            let draft = panel.reply_draft.get_mut();
            insert(&mut draft.document);
            draft.token = draft_token;
            draft.client_message_id = None;
            let text = draft.text().to_string();
            panel.reply_composer_focused = true;
            if panel.reply_error.take().is_some() {
                panel.list_state.remeasure();
            }
            (panel.reply_draft_key.clone(), text)
        };
        self.slack_composer_draft_changed(draft_key, draft_token, cx);
        self.sync_slack_thread_composer_input_to_end(text, cx);
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn insert_slack_reply_entity(
        &mut self,
        target: &SlackReplyComposerTarget,
        insert: impl FnOnce(&mut SlackComposerDocument),
        cx: &mut Context<Self>,
    ) {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => {
                if self.slack_reply_composer_target_is_current(target) {
                    self.insert_slack_thread_reply_entity(insert, cx);
                }
            }
            SlackReplyComposerTarget::AllThreads {
                thread_key,
                draft_key,
                ..
            } => {
                if !self.can_mutate_slack_reply_composer(target) {
                    return;
                }
                let draft_token = self.next_slack_composer_draft_token();
                if !self.slack_composer_drafts.contains_key(draft_key) {
                    let draft_id = self.next_slack_composer_draft_id();
                    self.slack_composer_drafts.insert(
                        draft_key.clone(),
                        crate::ui::surface::SlackComposerDraft::new(draft_id),
                    );
                }
                let text = {
                    let draft = self
                        .slack_composer_drafts
                        .get_mut(draft_key)
                        .expect("All Threads draft was inserted before entity insertion");
                    insert(&mut draft.document);
                    draft.token = draft_token;
                    draft.client_message_id = None;
                    draft.text().to_string()
                };
                self.slack_all_threads_reply_errors
                    .remove(thread_key.as_ref());
                self.slack_composer_draft_changed(draft_key.clone(), draft_token, cx);
                self.sync_slack_reply_composer_input_to_end(target, text, cx);
                cx.notify();
            }
        }
    }

    pub(in crate::ui::surface::state) fn replace_slack_thread_reply_range_with_entity(
        &mut self,
        replace: impl FnOnce(&mut SlackComposerDocument, Range<usize>) -> usize,
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) {
        if !self.can_send_slack_thread_reply() {
            return;
        }
        let draft_token = self.next_slack_composer_draft_token();
        let (draft_key, text, cursor) = {
            let panel = self
                .slack_thread_panel
                .as_mut()
                .expect("current Slack thread disappeared while completing its mention");
            let draft = panel.reply_draft.get_mut();
            let cursor = replace(&mut draft.document, range);
            draft.token = draft_token;
            draft.client_message_id = None;
            let text = draft.text().to_string();
            panel.reply_composer_focused = true;
            if panel.reply_error.take().is_some() {
                panel.list_state.remeasure();
            }
            (panel.reply_draft_key.clone(), text, cursor)
        };
        self.slack_composer_draft_changed(draft_key, draft_token, cx);
        self.sync_slack_thread_composer_input_cursor(text, cursor, cx);
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn replace_slack_reply_range_with_entity(
        &mut self,
        target: &SlackReplyComposerTarget,
        replace: impl FnOnce(&mut SlackComposerDocument, Range<usize>) -> usize,
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => {
                if self.slack_reply_composer_target_is_current(target) {
                    self.replace_slack_thread_reply_range_with_entity(replace, range, cx);
                }
            }
            SlackReplyComposerTarget::AllThreads {
                thread_key,
                draft_key,
                ..
            } => {
                if !self.can_mutate_slack_reply_composer(target) {
                    return;
                }
                let draft_token = self.next_slack_composer_draft_token();
                let Some(draft) = self.slack_composer_drafts.get_mut(draft_key) else {
                    return;
                };
                let cursor = replace(&mut draft.document, range);
                draft.token = draft_token;
                draft.client_message_id = None;
                let text = draft.text().to_string();
                self.slack_all_threads_reply_errors
                    .remove(thread_key.as_ref());
                self.slack_composer_draft_changed(draft_key.clone(), draft_token, cx);
                self.sync_slack_reply_composer_input_cursor(target, text, cursor, cx);
                cx.notify();
            }
        }
    }

    pub(in crate::ui::surface::state) fn sync_slack_thread_composer_input_to_end(
        &self,
        text: String,
        cx: &mut Context<Self>,
    ) {
        let cursor = text.len();
        self.sync_slack_thread_composer_input_cursor(text, cursor, cx);
    }

    fn sync_slack_reply_composer_input_to_end(
        &self,
        target: &SlackReplyComposerTarget,
        text: String,
        cx: &mut Context<Self>,
    ) {
        let cursor = text.len();
        self.sync_slack_reply_composer_input_cursor(target, text, cursor, cx);
    }

    fn sync_slack_reply_composer_input_cursor(
        &self,
        target: &SlackReplyComposerTarget,
        text: String,
        cursor: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(input) = self.slack_reply_composer_input(target) else {
            return;
        };
        cx.defer(move |cx| {
            input.update(cx, |input, cx| {
                input.set_text_and_move_cursor(text, cursor, cx);
            });
        });
    }

    pub(in crate::ui::surface::state) fn sync_slack_thread_composer_input_cursor(
        &self,
        text: String,
        cursor: usize,
        cx: &mut Context<Self>,
    ) {
        let input = self.slack_thread_composer_input.clone();
        cx.defer(move |cx| {
            input.update(cx, |input, cx| {
                input.set_text_and_move_cursor(text, cursor, cx);
            });
        });
    }
}
