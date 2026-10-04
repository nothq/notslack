use super::super::{Context, SlackComposerTarget, SurfaceState};
use crate::ui::surface::SlackReplyComposerTarget;

impl SurfaceState {
    pub(in crate::ui::surface) fn open_slack_reply_emoji_picker(
        &mut self,
        target: &SlackReplyComposerTarget,
        cx: &mut Context<Self>,
    ) {
        if !self.can_mutate_slack_reply_composer(target) {
            return;
        }
        if let SlackReplyComposerTarget::ThreadPanel { .. } = target {
            self.slack_thread_panel
                .as_mut()
                .expect("validated Slack thread disappeared while opening its emoji picker")
                .reply_composer_focused = true;
        }
        self.slack_composer_aux_target = Some(SlackComposerTarget::Reply(target.clone()));
        self.slack_mention_picker_state = None;
        self.set_slack_emoji_query(String::new(), cx);
    }

    pub(in crate::ui::surface) fn open_slack_reply_mention_picker(
        &mut self,
        target: &SlackReplyComposerTarget,
        cx: &mut Context<Self>,
    ) {
        if !self.can_mutate_slack_reply_composer(target) {
            return;
        }
        if let SlackReplyComposerTarget::ThreadPanel { .. } = target {
            self.slack_thread_panel
                .as_mut()
                .expect("validated Slack thread disappeared while opening its mention picker")
                .reply_composer_focused = true;
        }
        let target = SlackComposerTarget::Reply(target.clone());
        self.slack_composer_aux_target = Some(target.clone());
        self.mark_slack_toolbar_mention_picker(target);
        self.set_slack_mention_query(String::new(), cx);
    }
}
