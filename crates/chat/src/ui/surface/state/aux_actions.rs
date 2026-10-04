use super::{
    Context, SlackAuxPanelRowAction, SlackComposerTarget, SlackMentionInsertionMode, SurfaceState,
};

#[cfg(test)]
type SlackAttachmentMenuUpdate = fn(&mut SurfaceState, &str, &mut Context<SurfaceState>);

impl SurfaceState {
    pub(crate) fn activate_slack_aux_panel_action(
        &mut self,
        action: SlackAuxPanelRowAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            SlackAuxPanelRowAction::SelectConversation(conversation_id) => {
                self.select_slack_conversation(&conversation_id, cx);
            }
            SlackAuxPanelRowAction::OpenProfile(user_id) => {
                self.slack_aux_panel = None;
                self.open_slack_profile(&user_id, cx);
            }
            SlackAuxPanelRowAction::SelectMainTab(tab) => {
                self.select_slack_tab(tab, cx);
            }
            SlackAuxPanelRowAction::OpenMembersPanel => {
                self.open_slack_members_panel(cx);
            }
            SlackAuxPanelRowAction::InsertComposerSnippet(snippet) => {
                self.insert_slack_aux_composer_snippet(&snippet, cx);
            }
            SlackAuxPanelRowAction::InsertComposerUserMention { user_id, label } => {
                self.insert_slack_aux_user_mention(&user_id, &label, cx);
            }
            SlackAuxPanelRowAction::InsertComposerBroadcastMention(broadcast) => {
                self.insert_slack_aux_broadcast_mention(broadcast, cx);
            }
            #[cfg(test)]
            SlackAuxPanelRowAction::AttachComposerAttachment(attachment) => {
                self.attach_slack_draft_attachment(*attachment, cx);
            }
            SlackAuxPanelRowAction::OpenSearchQuery(query) => {
                self.open_slack_search_results(&query, cx);
            }
            SlackAuxPanelRowAction::SendDraftNow => {
                self.slack_aux_panel = None;
                self.submit_slack_composer(cx);
            }
            SlackAuxPanelRowAction::ToggleAttachmentExpanded(attachment_id) => {
                let Some(selection) = self.resolve_slack_attachment_selection_by_id(&attachment_id)
                else {
                    return;
                };
                self.toggle_slack_attachment_expanded(&selection, cx);
            }
            #[cfg(test)]
            SlackAuxPanelRowAction::ToggleAttachmentPlayback(_)
            | SlackAuxPanelRowAction::ToggleAttachmentTranscript(_)
            | SlackAuxPanelRowAction::CycleAttachmentPlaybackSpeed(_) => {
                self.activate_slack_attachment_action(action, cx);
            }
            SlackAuxPanelRowAction::OpenExternalConnection(name, company) => {
                self.open_slack_external_connection(&name, &company, cx);
            }
        }
    }

    fn insert_slack_aux_composer_snippet(&mut self, snippet: &str, cx: &mut Context<Self>) {
        let target = self.slack_composer_aux_target.take();
        self.slack_aux_panel = None;
        match target {
            Some(SlackComposerTarget::Main) => self.insert_slack_composer_snippet(snippet, cx),
            Some(SlackComposerTarget::Reply(target)) => {
                self.insert_slack_reply_snippet(&target, snippet, cx);
            }
            None => cx.notify(),
        }
    }

    fn take_slack_aux_mention_target(
        &mut self,
    ) -> Option<(SlackComposerTarget, SlackMentionInsertionMode)> {
        let target = self.slack_composer_aux_target.take()?;
        let insertion = self.take_slack_mention_insertion_mode(&target);
        self.slack_aux_panel = None;
        Some((target, insertion))
    }

    fn insert_slack_aux_user_mention(
        &mut self,
        user_id: &str,
        label: &str,
        cx: &mut Context<Self>,
    ) {
        match self.take_slack_aux_mention_target() {
            Some((SlackComposerTarget::Main, SlackMentionInsertionMode::Append)) => {
                self.insert_slack_composer_user_mention(user_id, label, cx);
            }
            Some((SlackComposerTarget::Reply(target), SlackMentionInsertionMode::Append)) => {
                self.insert_slack_reply_entity(
                    &target,
                    |document| document.append_user_mention(user_id, label),
                    cx,
                );
            }
            Some((SlackComposerTarget::Main, SlackMentionInsertionMode::Replace(range))) => {
                self.replace_slack_composer_range_with_user_mention(range, user_id, label, cx);
            }
            Some((
                SlackComposerTarget::Reply(target),
                SlackMentionInsertionMode::Replace(range),
            )) => {
                self.replace_slack_reply_range_with_entity(
                    &target,
                    |document, range| {
                        document.replace_range_with_user_mention(range, user_id, label)
                    },
                    range,
                    cx,
                );
            }
            Some((_, SlackMentionInsertionMode::Invalid)) | None => cx.notify(),
        }
    }

    fn insert_slack_aux_broadcast_mention(
        &mut self,
        broadcast: crate::model::SlackRichTextBroadcastRange,
        cx: &mut Context<Self>,
    ) {
        match self.take_slack_aux_mention_target() {
            Some((SlackComposerTarget::Main, SlackMentionInsertionMode::Append)) => {
                self.insert_slack_composer_broadcast_mention(broadcast, cx);
            }
            Some((SlackComposerTarget::Reply(target), SlackMentionInsertionMode::Append)) => {
                self.insert_slack_reply_entity(
                    &target,
                    |document| document.append_broadcast_mention(broadcast),
                    cx,
                );
            }
            Some((SlackComposerTarget::Main, SlackMentionInsertionMode::Replace(range))) => {
                self.replace_slack_composer_range_with_broadcast_mention(range, broadcast, cx);
            }
            Some((
                SlackComposerTarget::Reply(target),
                SlackMentionInsertionMode::Replace(range),
            )) => {
                self.replace_slack_reply_range_with_entity(
                    &target,
                    |document, range| {
                        document.replace_range_with_broadcast_mention(range, broadcast)
                    },
                    range,
                    cx,
                );
            }
            Some((_, SlackMentionInsertionMode::Invalid)) | None => cx.notify(),
        }
    }

    #[cfg(test)]
    fn activate_slack_attachment_action(
        &mut self,
        action: SlackAuxPanelRowAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            SlackAuxPanelRowAction::ToggleAttachmentPlayback(title) => {
                self.reopen_slack_attachment_menu(&title, cx, Self::toggle_slack_media_playback);
            }
            SlackAuxPanelRowAction::ToggleAttachmentTranscript(title) => {
                self.reopen_slack_attachment_menu(
                    &title,
                    cx,
                    Self::toggle_slack_attachment_transcript,
                );
            }
            SlackAuxPanelRowAction::CycleAttachmentPlaybackSpeed(title) => {
                self.reopen_slack_attachment_menu(&title, cx, Self::cycle_slack_media_speed);
            }
            _ => {}
        }
    }

    #[cfg(test)]
    fn reopen_slack_attachment_menu(
        &mut self,
        title: &str,
        cx: &mut Context<Self>,
        update: SlackAttachmentMenuUpdate,
    ) {
        update(self, title, cx);
        self.open_slack_attachment_menu(title, cx);
    }
}
