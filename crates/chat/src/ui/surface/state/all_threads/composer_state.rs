use super::{
    AppContext, Context, SlackAllThreadRow, SlackAllThreadsComposerState, SlackComposerDestination,
    SlackComposerDraftKey, SlackComposerFormatAction, SlackComposerTarget, SlackMainRoute,
    SlackMessageTimestamp, SlackReplyComposerTarget, SurfaceState, TextInput, TextInputProps,
    SLACK_ALL_THREADS_IMAGE_VISIBLE_OVERDRAW,
};

impl SurfaceState {
    pub(crate) fn toggle_slack_all_threads_reply_broadcast(
        &mut self,
        target: &SlackReplyComposerTarget,
        cx: &mut Context<Self>,
    ) {
        let SlackReplyComposerTarget::AllThreads {
            thread_key,
            draft_key,
            broadcast_supported,
        } = target
        else {
            return;
        };
        if !*broadcast_supported
            || !self.can_mutate_slack_reply_composer(target)
            || self.slack_thread_reply_mutation_is_blocked(draft_key)
        {
            return;
        }
        let key = draft_key.clone();
        let draft_token = self.next_slack_composer_draft_token();
        if !self.slack_composer_drafts.contains_key(&key) {
            let draft_id = self.next_slack_composer_draft_id();
            self.slack_composer_drafts.insert(
                key.clone(),
                crate::ui::surface::SlackComposerDraft::new(draft_id),
            );
        }
        let empty = {
            let draft = self
                .slack_composer_drafts
                .get_mut(&key)
                .expect("Slack All Threads draft was inserted before broadcast update");
            draft.broadcast = !draft.broadcast;
            draft.token = draft_token;
            draft.client_message_id = None;
            draft.is_empty()
        };
        if empty {
            self.slack_composer_drafts.remove(&key);
        }
        self.slack_composer_draft_changed(key, draft_token, cx);
        self.slack_all_threads_reply_errors
            .remove(thread_key.as_ref());
        cx.notify();
    }

    pub(super) fn sync_slack_all_threads_composer_inputs(&mut self, cx: &mut Context<Self>) {
        let start = self
            .slack_all_threads_visible_range
            .0
            .saturating_sub(SLACK_ALL_THREADS_IMAGE_VISIBLE_OVERDRAW)
            .min(self.slack_all_threads_rows.len());
        let end = self
            .slack_all_threads_visible_range
            .1
            .saturating_add(SLACK_ALL_THREADS_IMAGE_VISIBLE_OVERDRAW)
            .min(self.slack_all_threads_rows.len());
        let composers = self.slack_all_threads_rows[start..end]
            .iter()
            .map(|row| {
                let target = self.slack_all_threads_composer_target(row);
                let draft = self
                    .slack_composer_drafts
                    .get(target.draft_key())
                    .map(|draft| draft.text().to_string())
                    .unwrap_or_default();
                (row.key.clone(), target, draft)
            })
            .collect::<Vec<_>>();
        let current_keys = composers
            .iter()
            .map(|(key, _, _)| key.clone())
            .collect::<std::collections::HashSet<_>>();
        self.slack_all_threads_composers
            .retain(|key, _| current_keys.contains(key));
        self.dismiss_hidden_slack_all_threads_aux_target();
        for (key, target, draft) in composers {
            if let Some(composer) = self.slack_all_threads_composers.get(&key) {
                assert_eq!(
                    composer.target, target,
                    "Slack All Threads composer target changed without resetting its surface"
                );
                continue;
            }
            let input = cx.new(|cx| TextInput::new(TextInputProps::multiline(draft), cx));
            self.slack_all_threads_composers.insert(
                key,
                SlackAllThreadsComposerState {
                    target,
                    input,
                    formatting_enabled: false,
                    format_roving_target: SlackComposerFormatAction::Bold,
                    format_focus_handles: std::array::from_fn(|_| cx.focus_handle()),
                },
            );
        }
    }

    fn dismiss_hidden_slack_all_threads_aux_target(&mut self) {
        let target_is_visible =
            self.slack_composer_aux_target
                .as_ref()
                .is_none_or(|target| {
                    !matches!(
                        target,
                        SlackComposerTarget::Reply(SlackReplyComposerTarget::AllThreads { .. })
                    ) || self.slack_all_threads_composers.values().any(|composer| {
                        target == &SlackComposerTarget::Reply(composer.target.clone())
                    })
                });
        if !target_is_visible {
            self.slack_aux_panel = None;
            self.slack_composer_aux_target = None;
            self.slack_mention_picker_state = None;
        }
    }

    pub(crate) fn ensure_slack_all_threads_composer(
        &mut self,
        target: SlackReplyComposerTarget,
        cx: &mut Context<Self>,
    ) {
        let thread_key = target
            .all_threads_key()
            .expect("All Threads composer target must carry its thread key")
            .clone();
        if let Some(composer) = self.slack_all_threads_composers.get(&thread_key) {
            assert_eq!(
                composer.target, target,
                "Slack All Threads composer target changed without resetting its surface"
            );
            return;
        }
        let draft = self
            .slack_composer_drafts
            .get(target.draft_key())
            .map(|draft| draft.text().to_string())
            .unwrap_or_default();
        let input = cx.new(|cx| TextInput::new(TextInputProps::multiline(draft), cx));
        self.slack_all_threads_composers.insert(
            thread_key,
            SlackAllThreadsComposerState {
                target,
                input,
                formatting_enabled: false,
                format_roving_target: SlackComposerFormatAction::Bold,
                format_focus_handles: std::array::from_fn(|_| cx.focus_handle()),
            },
        );
    }

    pub(in crate::ui::surface) fn slack_all_threads_composer_target(
        &self,
        row: &SlackAllThreadRow,
    ) -> SlackReplyComposerTarget {
        let thread_timestamp = SlackMessageTimestamp::parse(&row.parent.id)
            .expect("Slack All Threads parent must have a canonical message timestamp");
        let draft_key = self
            .slack_composer_draft_key(SlackComposerDestination::Thread {
                conversation_id: row.conversation_id.to_string(),
                thread_timestamp,
            })
            .expect("Slack All Threads route must retain its workspace draft identity");
        SlackReplyComposerTarget::AllThreads {
            thread_key: row.key.clone(),
            draft_key,
            broadcast_supported: row.broadcast_label.is_some(),
        }
    }

    pub(in crate::ui::surface::state) fn slack_all_threads_surface_thread_key(
        &self,
        draft_key: &SlackComposerDraftKey,
    ) -> Option<String> {
        if self.slack_main_route != SlackMainRoute::AllThreads
            || self
                .slack_composer_draft_key(draft_key.destination.clone())
                .as_ref()
                != Some(draft_key)
        {
            return None;
        }
        let SlackComposerDestination::Thread {
            conversation_id,
            thread_timestamp,
        } = &draft_key.destination
        else {
            return None;
        };
        let row = self.slack_all_threads_rows.iter().find(|row| {
            row.conversation_id.as_ref() == conversation_id
                && row.parent.id == thread_timestamp.as_str()
        })?;
        self.slack_all_threads_snapshot
            .as_ref()?
            .threads
            .iter()
            .any(|thread| thread.id.as_str() == row.key.as_ref())
            .then(|| row.key.to_string())
    }

    pub(in crate::ui::surface) fn replace_slack_all_threads_reply_draft_text_for_target(
        &mut self,
        target: &SlackReplyComposerTarget,
        text: String,
        cx: &mut Context<Self>,
    ) -> bool {
        let SlackReplyComposerTarget::AllThreads {
            thread_key,
            draft_key,
            ..
        } = target
        else {
            return false;
        };
        if !self.can_mutate_slack_reply_composer(target)
            || self
                .slack_composer_drafts
                .get(draft_key)
                .map_or(text.is_empty(), |draft| draft.text() == text)
        {
            return false;
        }
        let draft_token = self.next_slack_composer_draft_token();
        if !self.slack_composer_drafts.contains_key(draft_key) {
            let draft_id = self.next_slack_composer_draft_id();
            self.slack_composer_drafts.insert(
                draft_key.clone(),
                crate::ui::surface::SlackComposerDraft::new(draft_id),
            );
        }
        let empty = {
            let draft = self
                .slack_composer_drafts
                .get_mut(draft_key)
                .expect("All Threads draft was inserted before its typed text update");
            draft.replace_text(&text, draft_token);
            draft.is_empty()
        };
        if empty {
            self.slack_composer_drafts.remove(draft_key);
        }
        self.slack_all_threads_reply_errors
            .remove(thread_key.as_ref());
        self.slack_composer_draft_changed(draft_key.clone(), draft_token, cx);
        true
    }
}
