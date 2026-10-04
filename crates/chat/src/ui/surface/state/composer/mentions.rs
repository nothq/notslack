use super::{Context, Range, SlackComposerDocument, SurfaceState};

impl SurfaceState {
    pub(crate) fn insert_slack_composer_snippet(&mut self, snippet: &str, cx: &mut Context<Self>) {
        if !self.can_mutate_current_slack_send_draft() {
            return;
        }
        let previous_length = self.slack_composer_text.len();
        if !self.slack_composer_text.is_empty() && !self.slack_composer_text.ends_with(' ') {
            self.slack_composer_text.push(' ');
        }
        self.slack_composer_text.push_str(snippet);
        let changed = self.slack_composer_text.len() != previous_length;
        if changed {
            self.advance_slack_send_draft_revision();
        }
        self.slack_composer_document
            .borrow_mut()
            .apply_text_edit(&self.slack_composer_text);
        if changed {
            self.slack_main_composer_draft_changed(cx);
        }
        self.sync_slack_composer_input_to_end(self.slack_composer_text.clone(), cx);
        self.slack_composer_focused = true;
        self.slack_error = None;
        cx.notify();
    }

    pub(crate) fn insert_slack_composer_user_mention(
        &mut self,
        user_id: &str,
        label: &str,
        cx: &mut Context<Self>,
    ) {
        self.insert_slack_composer_entity(
            |document| document.append_user_mention(user_id, label),
            cx,
        );
    }

    pub(crate) fn insert_slack_composer_broadcast_mention(
        &mut self,
        range: crate::model::SlackRichTextBroadcastRange,
        cx: &mut Context<Self>,
    ) {
        self.insert_slack_composer_entity(|document| document.append_broadcast_mention(range), cx);
    }

    pub(crate) fn replace_slack_composer_range_with_user_mention(
        &mut self,
        range: Range<usize>,
        user_id: &str,
        label: &str,
        cx: &mut Context<Self>,
    ) {
        self.replace_slack_composer_range_with_entity(
            |document, range| document.replace_range_with_user_mention(range, user_id, label),
            range,
            cx,
        );
    }

    pub(crate) fn replace_slack_composer_range_with_broadcast_mention(
        &mut self,
        range: Range<usize>,
        broadcast: crate::model::SlackRichTextBroadcastRange,
        cx: &mut Context<Self>,
    ) {
        self.replace_slack_composer_range_with_entity(
            |document, range| document.replace_range_with_broadcast_mention(range, broadcast),
            range,
            cx,
        );
    }

    pub(in crate::ui::surface::state) fn insert_slack_composer_entity(
        &mut self,
        insert: impl FnOnce(&mut SlackComposerDocument),
        cx: &mut Context<Self>,
    ) {
        if !self.can_mutate_current_slack_send_draft() {
            return;
        }
        let (text, document_changed) = {
            let mut document = self.slack_composer_document.borrow_mut();
            let previous_revision = document.revision();
            document.reset_to_text_if_changed(&self.slack_composer_text);
            insert(&mut document);
            (
                document.text().to_string(),
                document.revision() != previous_revision,
            )
        };
        let text_changed = self.replace_slack_send_draft_text(text.clone());
        if !text_changed && document_changed {
            self.advance_slack_send_draft_revision();
        }
        if text_changed || document_changed {
            self.slack_main_composer_draft_changed(cx);
        }
        self.sync_slack_composer_input_to_end(text, cx);
        self.slack_composer_focused = true;
        self.slack_error = None;
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn replace_slack_composer_range_with_entity(
        &mut self,
        replace: impl FnOnce(&mut SlackComposerDocument, Range<usize>) -> usize,
        range: Range<usize>,
        cx: &mut Context<Self>,
    ) {
        if !self.can_mutate_current_slack_send_draft() {
            return;
        }
        let (text, cursor, document_changed) = {
            let mut document = self.slack_composer_document.borrow_mut();
            let previous_revision = document.revision();
            document.reset_to_text_if_changed(&self.slack_composer_text);
            let cursor = replace(&mut document, range);
            (
                document.text().to_string(),
                cursor,
                document.revision() != previous_revision,
            )
        };
        let text_changed = self.replace_slack_send_draft_text(text.clone());
        if !text_changed && document_changed {
            self.advance_slack_send_draft_revision();
        }
        if text_changed || document_changed {
            self.slack_main_composer_draft_changed(cx);
        }
        self.sync_slack_composer_input_cursor(text, cursor, cx);
        self.slack_composer_focused = true;
        self.slack_error = None;
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn sync_slack_composer_input_to_end(
        &self,
        text: String,
        cx: &mut Context<Self>,
    ) {
        let cursor = text.len();
        self.sync_slack_composer_input_cursor(text, cursor, cx);
    }

    pub(in crate::ui::surface::state) fn sync_slack_composer_input_cursor(
        &self,
        text: String,
        cursor: usize,
        cx: &mut Context<Self>,
    ) {
        let input = self.slack_composer_input.clone();
        cx.defer(move |cx| {
            input.update(cx, |input, cx| {
                input.set_text_and_move_cursor(text, cursor, cx);
            });
        });
    }
}
