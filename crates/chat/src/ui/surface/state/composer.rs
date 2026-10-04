mod attachments;
mod links;
mod mentions;

use std::ops::Range;

use super::{
    load_slack_prepared_upload_files_from_paths, Context, SlackComposerFormatAction,
    SlackComposerLinkDialog, SlackComposerLinkEdit, SlackComposerLinkTarget,
    SlackReplyComposerTarget, SurfaceState, Window,
};
use crate::ui::surface::SlackComposerDocument;
use crate::ui::SlackUploadFile;

impl SurfaceState {
    pub(crate) fn toggle_slack_formatting(&mut self, cx: &mut Context<Self>) {
        if !self.can_mutate_current_slack_send_draft() {
            return;
        }
        self.slack_formatting_enabled = !self.slack_formatting_enabled;
        cx.notify();
    }

    pub(crate) fn apply_slack_composer_format(
        &mut self,
        action: SlackComposerFormatAction,
        cx: &mut Context<Self>,
    ) {
        if !self.can_mutate_current_slack_send_draft() {
            return;
        }
        self.slack_composer_format_roving_target = action;
        if action == SlackComposerFormatAction::Link {
            self.open_slack_composer_link_dialog(cx);
            return;
        }
        if !SlackComposerDocument::supports(action) {
            return;
        }
        let selection = {
            let input = self.slack_composer_input.read(cx);
            if input.text() == self.slack_composer_text {
                input.selection_range()
            } else {
                self.slack_composer_text.len()..self.slack_composer_text.len()
            }
        };
        let applied = {
            let mut document = self.slack_composer_document.borrow_mut();
            document.reset_to_text_if_changed(&self.slack_composer_text);
            document.toggle_format(action, selection)
        };
        if !applied {
            return;
        }
        self.advance_slack_send_draft_revision();
        self.slack_main_composer_draft_changed(cx);
        self.slack_formatting_enabled = true;
        self.slack_composer_focused = true;
        self.slack_error = None;
        cx.notify();
    }
}
