mod shortcut;

use super::{
    Context, SlackComposerLinkDialog, SlackComposerLinkEdit, SlackComposerLinkTarget,
    SlackReplyComposerTarget, SurfaceState,
};

impl SurfaceState {
    pub(crate) fn open_slack_composer_link_dialog(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.can_mutate_current_slack_send_draft() {
            return false;
        }
        let selection = {
            let input = self.slack_composer_input.read(cx);
            if input.text() == self.slack_composer_text {
                input.selection_range()
            } else {
                self.slack_composer_text.len()..self.slack_composer_text.len()
            }
        };
        let (edit, document_revision) = {
            let mut document = self.slack_composer_document.borrow_mut();
            document.reset_to_text_if_changed(&self.slack_composer_text);
            let Some(edit) = document.link_edit(selection) else {
                return false;
            };
            (edit, document.revision())
        };
        let Some(context) = self.slack_active_main_composer_context.as_ref() else {
            return false;
        };
        let target = SlackComposerLinkTarget::Main {
            team_id: context.target.team_id.clone(),
            conversation_id: context.target.conversation_id.clone(),
            source: context.source.clone(),
            draft_revision: self.slack_send_draft_revision,
            document_revision,
        };
        self.show_slack_composer_link_dialog(target, edit, cx);
        true
    }

    pub(crate) fn open_slack_thread_link_dialog(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(target) = self.slack_thread_panel_reply_composer_target() else {
            return false;
        };
        self.open_slack_reply_link_dialog(target, cx)
    }

    pub(in crate::ui::surface) fn open_slack_reply_link_dialog(
        &mut self,
        target: SlackReplyComposerTarget,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.can_mutate_slack_reply_composer(&target) {
            return false;
        }
        let selection = self.slack_reply_link_selection(&target, cx);
        let Some((edit, document_revision)) = self.slack_reply_link_edit(&target, selection) else {
            return false;
        };
        self.show_slack_composer_link_dialog(
            SlackComposerLinkTarget::Reply {
                target,
                document_revision,
            },
            edit,
            cx,
        );
        true
    }

    fn slack_reply_link_selection(
        &self,
        target: &SlackReplyComposerTarget,
        cx: &Context<Self>,
    ) -> std::ops::Range<usize> {
        let input = self
            .slack_reply_composer_input(target)
            .expect("validated Slack reply composer must retain its input");
        let draft_text = match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => self
                .slack_thread_panel
                .as_ref()
                .expect("validated Slack thread disappeared while opening a reply link")
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

    fn slack_reply_link_edit(
        &self,
        target: &SlackReplyComposerTarget,
        selection: std::ops::Range<usize>,
    ) -> Option<(SlackComposerLinkEdit, u64)> {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => {
                let panel = self
                    .slack_thread_panel
                    .as_ref()
                    .expect("validated Slack thread disappeared while reading a reply link");
                let draft = panel.reply_draft.borrow();
                Some((
                    draft.document.link_edit(selection)?,
                    draft.document.revision(),
                ))
            }
            SlackReplyComposerTarget::AllThreads { draft_key, .. } => {
                let draft = self.slack_composer_drafts.get(draft_key)?;
                Some((
                    draft.document.link_edit(selection)?,
                    draft.document.revision(),
                ))
            }
        }
    }

    fn show_slack_composer_link_dialog(
        &mut self,
        target: SlackComposerLinkTarget,
        edit: SlackComposerLinkEdit,
        cx: &mut Context<Self>,
    ) {
        let raw_url = edit
            .url
            .as_ref()
            .map_or_else(String::new, |url| url.as_str().to_string());
        let editing_existing_link = edit.url.is_some();
        self.slack_composer_link_dialog = Some(SlackComposerLinkDialog {
            target,
            range: edit.range,
            text: edit.text,
            raw_url,
            parsed_url: edit.url,
            editing_existing_link,
            error: None,
        });
        self.slack_composer_focused = false;
        if let Some(panel) = self.slack_thread_panel.as_mut() {
            panel.reply_composer_focused = false;
        }
        self.slack_composer_link_focus_pending = true;
        cx.notify();
    }

    pub(crate) fn set_slack_composer_link_text(&mut self, text: String, cx: &mut Context<Self>) {
        let Some(dialog) = self.slack_composer_link_dialog.as_mut() else {
            return;
        };
        if dialog.text == text {
            return;
        }
        dialog.text = text;
        dialog.error = None;
        cx.notify();
    }

    pub(crate) fn set_slack_composer_link_url(&mut self, raw_url: String, cx: &mut Context<Self>) {
        let Some(dialog) = self.slack_composer_link_dialog.as_mut() else {
            return;
        };
        if dialog.raw_url == raw_url {
            return;
        }
        dialog.set_url(raw_url);
        cx.notify();
    }

    pub(crate) fn close_slack_composer_link_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.slack_composer_link_dialog.take() else {
            return;
        };
        self.slack_composer_link_focus_pending = false;
        match dialog.target {
            SlackComposerLinkTarget::Main { .. } => self.slack_composer_focused = true,
            SlackComposerLinkTarget::Reply { target, .. } => {
                if let SlackReplyComposerTarget::ThreadPanel { .. } = target {
                    if self.slack_reply_composer_target_is_current(&target) {
                        self.slack_thread_panel
                            .as_mut()
                            .expect("validated Slack thread disappeared while closing its link")
                            .reply_composer_focused = true;
                    }
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn save_slack_composer_link_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.slack_composer_link_dialog.clone() else {
            return;
        };
        let Some(url) = dialog.parsed_url.clone().filter(|_| dialog.can_save()) else {
            return;
        };
        let result = match &dialog.target {
            SlackComposerLinkTarget::Main { .. } => {
                if self.slack_schedule_blocks_current_composer_mutation() {
                    if let Some(current) = self.slack_composer_link_dialog.as_mut() {
                        current.error = Some(
                            "Wait for the pending Slack scheduled-draft mutation to finish."
                                .to_string(),
                        );
                    }
                    cx.notify();
                    return;
                }
                self.save_main_slack_composer_link(&dialog, url)
            }
            SlackComposerLinkTarget::Reply { .. } => {
                self.save_reply_slack_composer_link(&dialog, url)
            }
        };
        if let Err(message) = result {
            if let Some(current) = self.slack_composer_link_dialog.as_mut() {
                current.error = Some(message);
            }
            cx.notify();
            return;
        }
        match &dialog.target {
            SlackComposerLinkTarget::Main { .. } => self.slack_main_composer_draft_changed(cx),
            SlackComposerLinkTarget::Reply { target, .. } => {
                let changed = match target {
                    SlackReplyComposerTarget::ThreadPanel { .. } => self
                        .slack_thread_panel
                        .as_ref()
                        .filter(|_| self.slack_reply_composer_target_is_current(target))
                        .map(|panel| panel.reply_draft.borrow().token),
                    SlackReplyComposerTarget::AllThreads { draft_key, .. } => self
                        .slack_composer_drafts
                        .get(draft_key)
                        .map(|draft| draft.token),
                };
                if let Some(token) = changed {
                    self.slack_composer_draft_changed(target.draft_key().clone(), token, cx);
                }
            }
        }
        self.close_slack_composer_link_dialog(cx);
    }

    fn save_main_slack_composer_link(
        &mut self,
        dialog: &SlackComposerLinkDialog,
        url: crate::ui::surface::SlackComposerLinkUrl,
    ) -> Result<(), String> {
        let SlackComposerLinkTarget::Main {
            team_id,
            conversation_id,
            source,
            draft_revision,
            document_revision,
        } = &dialog.target
        else {
            unreachable!("main Slack link save requires a main composer target");
        };
        if self
            .slack_active_main_composer_context
            .as_ref()
            .is_none_or(|context| {
                context.target.team_id != *team_id
                    || context.target.conversation_id != *conversation_id
                    || context.source != *source
            })
            || self.slack_send_draft_revision != *draft_revision
        {
            return Err("The message changed while the link dialog was open.".to_string());
        }
        let mut document = self.snapshot_slack_send_draft_document();
        if document.revision() != *document_revision {
            return Err(
                "The message formatting changed while the link dialog was open.".to_string(),
            );
        }
        document.replace_range_with_link(dialog.range.clone(), &dialog.text, url);
        self.replace_slack_send_draft_document(document);
        Ok(())
    }

    fn save_reply_slack_composer_link(
        &mut self,
        dialog: &SlackComposerLinkDialog,
        url: crate::ui::surface::SlackComposerLinkUrl,
    ) -> Result<(), String> {
        let SlackComposerLinkTarget::Reply {
            target,
            document_revision,
        } = &dialog.target
        else {
            unreachable!("reply Slack link save requires a reply composer target");
        };
        if !self.can_mutate_slack_reply_composer(target) {
            return Err("The Slack thread changed while the link dialog was open.".to_string());
        }
        let current_revision = self.slack_reply_link_document_revision(target)?;
        if current_revision != *document_revision {
            return Err("The reply formatting changed while the link dialog was open.".to_string());
        }
        let draft_token = self.next_slack_composer_draft_token();
        self.replace_slack_reply_link(target, dialog, url, draft_token);
        Ok(())
    }

    fn slack_reply_link_document_revision(
        &self,
        target: &SlackReplyComposerTarget,
    ) -> Result<u64, String> {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => Ok(self
                .slack_thread_panel
                .as_ref()
                .expect("validated Slack thread disappeared while saving its link")
                .reply_draft
                .borrow()
                .document
                .revision()),
            SlackReplyComposerTarget::AllThreads { draft_key, .. } => self
                .slack_composer_drafts
                .get(draft_key)
                .ok_or_else(|| {
                    "The Slack thread draft closed while the link dialog was open.".to_string()
                })
                .map(|draft| draft.document.revision()),
        }
    }

    fn replace_slack_reply_link(
        &mut self,
        target: &SlackReplyComposerTarget,
        dialog: &SlackComposerLinkDialog,
        url: crate::ui::surface::SlackComposerLinkUrl,
        draft_token: u64,
    ) {
        match target {
            SlackReplyComposerTarget::ThreadPanel { .. } => {
                let panel = self
                    .slack_thread_panel
                    .as_mut()
                    .expect("validated Slack thread disappeared while saving its link");
                let draft = panel.reply_draft.get_mut();
                draft
                    .document
                    .replace_range_with_link(dialog.range.clone(), &dialog.text, url);
                draft.token = draft_token;
                draft.client_message_id = None;
                panel.list_state.remeasure();
            }
            SlackReplyComposerTarget::AllThreads {
                thread_key,
                draft_key,
                ..
            } => {
                let draft = self
                    .slack_composer_drafts
                    .get_mut(draft_key)
                    .expect("validated All Threads draft disappeared while saving its link");
                draft
                    .document
                    .replace_range_with_link(dialog.range.clone(), &dialog.text, url);
                draft.token = draft_token;
                draft.client_message_id = None;
                self.slack_all_threads_reply_errors
                    .remove(thread_key.as_ref());
            }
        }
    }
}

/// A pasted URL as Slack's composer shows it: the link, displayed without
/// its `https://`. `None` unless the paste is one web address.
pub(crate) fn slack_pasted_link(
    text: &str,
) -> Option<(String, crate::ui::surface::SlackComposerLinkUrl)> {
    let text = text.trim();
    if text.chars().any(char::is_whitespace)
        || !(text.starts_with("https://") || text.starts_with("http://"))
    {
        return None;
    }
    let url = crate::ui::surface::SlackComposerLinkUrl::parse(text)?;
    let display = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
        .unwrap_or(text)
        .trim_end_matches('/');
    (!display.is_empty()).then(|| (display.to_string(), url))
}

impl SurfaceState {
    /// Puts a pasted URL into the main draft as a link over `range`.
    /// Returns the draft's new text and where the caret goes, or `None` when
    /// the draft can't change now (the paste then goes in as plain text).
    pub(crate) fn paste_slack_composer_link(
        &mut self,
        range: std::ops::Range<usize>,
        display: &str,
        url: crate::ui::surface::SlackComposerLinkUrl,
        cx: &mut Context<Self>,
    ) -> Option<(String, usize)> {
        if !self.can_mutate_current_slack_send_draft()
            || self.slack_schedule_blocks_current_composer_mutation()
        {
            return None;
        }
        let mut document = self.snapshot_slack_send_draft_document();
        let text = document.text();
        if range.start > range.end
            || range.end > text.len()
            || !text.is_char_boundary(range.start)
            || !text.is_char_boundary(range.end)
        {
            return None;
        }
        document.replace_range_with_link(range.clone(), display, url);
        let text = document.text().to_string();
        self.replace_slack_send_draft_document(document);
        self.slack_main_composer_draft_changed(cx);
        cx.notify();
        Some((text, range.start + display.len()))
    }
}

impl SurfaceState {
    /// A right click in the main composer: over a link, opens its menu.
    pub(crate) fn right_click_slack_composer(
        &mut self,
        offset: usize,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let link = self
            .snapshot_slack_send_draft_document()
            .link_range_at(offset);
        self.slack_composer_link_menu =
            link.map(|(range, url)| crate::ui::surface::SlackComposerLinkMenu {
                range,
                url,
                position,
            });
        cx.notify();
    }

    pub(crate) fn close_slack_composer_link_menu(&mut self, cx: &mut Context<Self>) {
        if self.slack_composer_link_menu.take().is_some() {
            cx.notify();
        }
    }

    /// Edit link: selects the link, then opens the link dialog on it.
    pub(crate) fn edit_slack_composer_menu_link(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.slack_composer_link_menu.take() else {
            return;
        };
        self.slack_composer_input.update(cx, |input, cx| {
            input.set_selection(menu.range.clone(), false, cx);
        });
        self.open_slack_composer_link_dialog(cx);
        cx.notify();
    }

    /// Display as URL: the link shows its full address instead of its text.
    pub(crate) fn show_slack_composer_menu_link_as_url(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.slack_composer_link_menu.take() else {
            return;
        };
        let address = menu.url.as_str().to_string();
        if let Some((text, caret)) =
            self.paste_slack_composer_link(menu.range, &address, menu.url, cx)
        {
            self.slack_composer_input.update(cx, |input, cx| {
                input.set_text_and_move_cursor(text, caret, cx);
            });
        }
    }
}

#[cfg(test)]
mod pasted_link_tests {
    use super::slack_pasted_link;

    #[test]
    fn a_pasted_web_address_becomes_a_link_shown_without_https() {
        let (display, url) =
            slack_pasted_link("https://github.com/GeniusEducation/sisui/pull/10470").unwrap();
        assert_eq!(display, "github.com/GeniusEducation/sisui/pull/10470");
        assert_eq!(
            url.as_str(),
            "https://github.com/GeniusEducation/sisui/pull/10470"
        );
        assert!(slack_pasted_link("see https://example.com").is_none());
        assert!(slack_pasted_link("plain words").is_none());
        assert!(slack_pasted_link("ftp://example.com").is_none());
    }
}
