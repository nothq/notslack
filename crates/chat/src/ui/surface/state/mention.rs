mod panels;

use std::ops::Range;

use super::{
    slack_aux_panel_matches_query, slack_normalized_aux_panel_query, Context,
    SlackAuxPanelQueryBehavior, SlackAuxPanelRow, SlackAuxPanelRowAction, SlackAuxPanelSection,
    SlackAuxPanelState, SlackComposerTarget, SlackMentionInsertionMode, SlackMentionPickerState,
    SlackReplyComposerTarget, SurfaceState, SLACK_SPECIAL_MENTIONS,
};

impl SurfaceState {
    pub(crate) fn initialize_slack_inline_mention_observers(&mut self, cx: &mut Context<Self>) {
        let main_input = self.slack_composer_input.clone();
        cx.observe(&main_input, |this, input, cx| {
            let selection = input.read(cx).selection_range();
            this.sync_slack_main_inline_mention(selection, cx);
        })
        .detach();

        let thread_input = self.slack_thread_composer_input.clone();
        cx.observe(&thread_input, |this, input, cx| {
            let selection = input.read(cx).selection_range();
            this.sync_slack_thread_inline_mention(selection, cx);
        })
        .detach();
    }

    pub(super) fn slack_mention_panel(&self, query: &str) -> SlackAuxPanelState {
        SlackAuxPanelState {
            title: "Mention people".to_string(),
            subtitle: Some("Search people and special mentions".to_string()),
            query: Some(query.to_string()),
            query_behavior: Some(SlackAuxPanelQueryBehavior::Mention),
            sections: self.slack_mention_sections(query),
        }
    }

    pub(crate) fn slack_visible_mention_picker_rows<'a>(
        &'a self,
        panel: &'a SlackAuxPanelState,
    ) -> impl Iterator<Item = &'a SlackAuxPanelRow> + 'a {
        let query = panel.query.as_deref().unwrap_or("");
        let self_name = self
            .slack_workspace()
            .and_then(|workspace| workspace.self_display_name.as_deref())
            .unwrap_or("You");
        let filter_empty_query = query.is_empty()
            && panel
                .sections
                .iter()
                .flat_map(|section| section.rows.iter())
                .any(|row| {
                    !row.label.starts_with('@') && !row.label.eq_ignore_ascii_case(self_name)
                });
        panel
            .sections
            .iter()
            .flat_map(|section| section.rows.iter())
            .filter(move |row| {
                if filter_empty_query {
                    !row.label.starts_with('@') && !row.label.eq_ignore_ascii_case(self_name)
                } else {
                    true
                }
            })
            .take(6)
    }

    pub(crate) fn mark_slack_toolbar_mention_picker(&mut self, target: SlackComposerTarget) {
        self.slack_mention_picker_state = Some(SlackMentionPickerState::Toolbar { target });
    }

    pub(crate) fn clear_slack_toolbar_mention_picker(
        &mut self,
        target: &SlackComposerTarget,
    ) -> bool {
        if matches!(
            self.slack_mention_picker_state.as_ref(),
            Some(SlackMentionPickerState::Toolbar {
                target: current_target
            }) if current_target == target
        ) {
            self.slack_mention_picker_state = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn dismiss_slack_inline_mention_picker(&mut self) {
        if let Some(SlackMentionPickerState::Inline { dismissed, .. }) =
            self.slack_mention_picker_state.as_mut()
        {
            *dismissed = true;
        }
    }

    pub(crate) fn take_slack_mention_insertion_mode(
        &mut self,
        target: &SlackComposerTarget,
    ) -> SlackMentionInsertionMode {
        let Some(state) = self.slack_mention_picker_state.take() else {
            return SlackMentionInsertionMode::Invalid;
        };
        match state {
            SlackMentionPickerState::Toolbar {
                target: current_target,
            } => {
                if &current_target == target {
                    SlackMentionInsertionMode::Append
                } else {
                    SlackMentionInsertionMode::Invalid
                }
            }
            SlackMentionPickerState::Inline {
                target: current_target,
                range,
                document_revision,
                dismissed,
            } => {
                if dismissed
                    || &current_target != target
                    || self.slack_composer_document_revision(target) != Some(document_revision)
                    || slack_active_inline_mention_range(
                        self.slack_composer_text_for_mention_target(target)
                            .as_deref()
                            .unwrap_or_default(),
                        &(range.end..range.end),
                    )
                    .as_ref()
                        != Some(&range)
                    || !self.slack_mention_panel_targets(target)
                    || self
                        .slack_aux_panel
                        .as_ref()
                        .and_then(|panel| panel.query.as_deref())
                        != self.slack_inline_mention_query(target, &range).as_deref()
                {
                    return SlackMentionInsertionMode::Invalid;
                }
                SlackMentionInsertionMode::Replace(range)
            }
        }
    }

    fn sync_slack_main_inline_mention(&mut self, selection: Range<usize>, cx: &mut Context<Self>) {
        let target = SlackComposerTarget::Main;
        if !self.slack_composer_focused || !self.has_current_slack_send_target() {
            self.close_slack_inline_mention(&target, cx);
            return;
        }
        let (candidate, revision) = {
            let document = self.slack_composer_document.borrow();
            let candidate =
                slack_active_inline_mention_range(&self.slack_composer_text, &selection)
                    .filter(|range| !document.range_overlaps_entity(range));
            (candidate, document.revision())
        };
        self.sync_slack_inline_mention_candidate(target, candidate, revision, cx);
    }

    fn sync_slack_thread_inline_mention(
        &mut self,
        selection: Range<usize>,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.slack_thread_panel_reply_composer_target() else {
            if matches!(
                self.slack_composer_aux_target.as_ref(),
                Some(SlackComposerTarget::Reply(
                    SlackReplyComposerTarget::ThreadPanel { .. }
                ))
            ) {
                self.slack_aux_panel = None;
                self.slack_composer_aux_target = None;
                self.slack_mention_picker_state = None;
                cx.notify();
            }
            return;
        };
        let composer_target = SlackComposerTarget::Reply(target);
        let Some(panel) = self
            .slack_thread_panel
            .as_ref()
            .filter(|panel| panel.reply_composer_focused)
        else {
            self.close_slack_inline_mention(&composer_target, cx);
            return;
        };
        let (candidate, revision) = {
            let draft = panel.reply_draft.borrow();
            let candidate = slack_active_inline_mention_range(draft.text(), &selection)
                .filter(|range| !draft.document.range_overlaps_entity(range));
            (candidate, draft.document.revision())
        };
        self.sync_slack_inline_mention_candidate(composer_target, candidate, revision, cx);
    }

    fn sync_slack_inline_mention_candidate(
        &mut self,
        target: SlackComposerTarget,
        candidate: Option<Range<usize>>,
        document_revision: u64,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            self.slack_mention_picker_state.as_ref(),
            Some(SlackMentionPickerState::Toolbar {
                target: current_target
            }) if current_target == &target
        ) {
            return;
        }
        if self.slack_inline_mention_candidate_unchanged(
            &target,
            candidate.as_ref(),
            document_revision,
        ) {
            return;
        }

        let Some(range) = candidate else {
            self.close_slack_inline_mention(&target, cx);
            return;
        };
        let Some(query) = self.slack_inline_mention_query(&target, &range) else {
            self.close_slack_inline_mention(&target, cx);
            return;
        };
        self.slack_mention_picker_state = Some(SlackMentionPickerState::Inline {
            target: target.clone(),
            range,
            document_revision,
            dismissed: false,
        });
        self.slack_composer_aux_target = Some(target.clone());
        self.set_slack_mention_query(query, cx);
        if target == SlackComposerTarget::Main {
            self.slack_composer_focused = true;
        }
    }

    fn slack_inline_mention_candidate_unchanged(
        &self,
        target: &SlackComposerTarget,
        candidate: Option<&Range<usize>>,
        document_revision: u64,
    ) -> bool {
        let Some(SlackMentionPickerState::Inline {
            target: current_target,
            range,
            document_revision: current_revision,
            dismissed,
        }) = self.slack_mention_picker_state.as_ref()
        else {
            return false;
        };
        if current_target != target
            || candidate != Some(range)
            || *current_revision != document_revision
        {
            return false;
        }
        *dismissed
            || (self.slack_mention_panel_targets(target)
                && self
                    .slack_aux_panel
                    .as_ref()
                    .and_then(|panel| panel.query.as_deref())
                    == self.slack_inline_mention_query(target, range).as_deref())
    }

    fn close_slack_inline_mention(&mut self, target: &SlackComposerTarget, cx: &mut Context<Self>) {
        if !matches!(
            self.slack_mention_picker_state.as_ref(),
            Some(SlackMentionPickerState::Inline {
                target: current_target,
                ..
            }) if current_target == target
        ) {
            return;
        }
        self.slack_mention_picker_state = None;
        if self.slack_mention_panel_targets(target) {
            self.slack_aux_panel = None;
            self.slack_composer_aux_target = None;
        }
        cx.notify();
    }

    fn slack_mention_panel_targets(&self, target: &SlackComposerTarget) -> bool {
        self.slack_aux_panel.as_ref().is_some_and(|panel| {
            panel.query_behavior == Some(SlackAuxPanelQueryBehavior::Mention)
                && self.slack_composer_aux_target.as_ref() == Some(target)
        })
    }

    fn slack_composer_text_for_mention_target(
        &self,
        target: &SlackComposerTarget,
    ) -> Option<String> {
        match target {
            SlackComposerTarget::Main => Some(self.slack_composer_text.clone()),
            SlackComposerTarget::Reply(target @ SlackReplyComposerTarget::ThreadPanel { .. }) => {
                self.slack_thread_panel
                    .as_ref()
                    .filter(|_| self.slack_reply_composer_target_is_current(target))
                    .map(|panel| panel.reply_draft.borrow().text().to_string())
            }
            SlackComposerTarget::Reply(SlackReplyComposerTarget::AllThreads {
                draft_key, ..
            }) => self
                .slack_composer_drafts
                .get(draft_key)
                .map(|draft| draft.text().to_string()),
        }
    }

    fn slack_composer_document_revision(&self, target: &SlackComposerTarget) -> Option<u64> {
        match target {
            SlackComposerTarget::Main => Some(self.slack_composer_document.borrow().revision()),
            SlackComposerTarget::Reply(target @ SlackReplyComposerTarget::ThreadPanel { .. }) => {
                self.slack_thread_panel
                    .as_ref()
                    .filter(|_| self.slack_reply_composer_target_is_current(target))
                    .map(|panel| panel.reply_draft.borrow().document.revision())
            }
            SlackComposerTarget::Reply(SlackReplyComposerTarget::AllThreads {
                draft_key, ..
            }) => self
                .slack_composer_drafts
                .get(draft_key)
                .map(|draft| draft.document.revision()),
        }
    }

    fn slack_inline_mention_query(
        &self,
        target: &SlackComposerTarget,
        range: &Range<usize>,
    ) -> Option<String> {
        let text = self.slack_composer_text_for_mention_target(target)?;
        text.get(range.start.checked_add(1)?..range.end)
            .map(str::to_string)
    }
}

fn slack_active_inline_mention_range(text: &str, selection: &Range<usize>) -> Option<Range<usize>> {
    if !selection.is_empty() || selection.end > text.len() || !text.is_char_boundary(selection.end)
    {
        return None;
    }
    let caret = selection.end;
    for (offset, character) in text[..caret].char_indices().rev() {
        if character == '@' {
            let starts_at_boundary = text[..offset].chars().next_back().is_none_or(|previous| {
                previous != '@' && !slack_inline_mention_query_character(previous)
            });
            return starts_at_boundary.then_some(offset..caret);
        }
        if !slack_inline_mention_query_character(character) {
            return None;
        }
    }
    None
}

fn slack_inline_mention_query_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '-' | '.' | '\'')
}
