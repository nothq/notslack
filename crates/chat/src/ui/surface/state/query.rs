use super::{
    Context, SlackAuxPanelQueryBehavior, SlackAuxPanelRowAction, SlackComposerTarget, SurfaceState,
};

impl SurfaceState {
    pub(super) fn set_slack_emoji_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.set_slack_aux_panel(self.slack_emoji_panel(&query), cx);
    }

    pub(super) fn set_slack_mention_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.set_slack_aux_panel(self.slack_mention_panel(&query), cx);
    }

    pub(super) fn set_slack_aux_panel_query(
        &mut self,
        behavior: SlackAuxPanelQueryBehavior,
        query: String,
        cx: &mut Context<Self>,
    ) {
        match behavior {
            SlackAuxPanelQueryBehavior::SearchWorkspace => {
                self.set_slack_search_query(query, cx);
            }
            SlackAuxPanelQueryBehavior::Emoji => self.set_slack_emoji_query(query, cx),
            SlackAuxPanelQueryBehavior::Mention => self.set_slack_mention_query(query, cx),
        }
    }

    pub(super) fn is_slack_query_panel_open(&self) -> bool {
        self.slack_search_open
            || self
                .slack_aux_panel
                .as_ref()
                .and_then(|panel| panel.query.as_ref())
                .is_some()
    }

    pub(super) fn current_slack_aux_panel_query(
        &self,
    ) -> Option<(SlackAuxPanelQueryBehavior, String)> {
        self.slack_aux_panel
            .as_ref()
            .and_then(|panel| panel.query_behavior.zip(panel.query.clone()))
    }

    pub(super) fn first_slack_aux_panel_action(&self) -> Option<SlackAuxPanelRowAction> {
        self.slack_aux_panel.as_ref().and_then(|panel| {
            if panel.query_behavior == Some(SlackAuxPanelQueryBehavior::Mention) {
                return self
                    .slack_visible_mention_picker_rows(panel)
                    .next()
                    .and_then(|row| row.action.clone());
            }
            panel
                .sections
                .iter()
                .flat_map(|section| section.rows.iter())
                .find_map(|row| row.action.clone())
        })
    }

    pub(crate) fn activate_first_slack_composer_aux_action_if_open(
        &mut self,
        target: &SlackComposerTarget,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(panel) = self.slack_aux_panel.as_ref() else {
            return false;
        };
        if !matches!(
            panel.query_behavior,
            Some(SlackAuxPanelQueryBehavior::Emoji | SlackAuxPanelQueryBehavior::Mention)
        ) {
            return false;
        }
        if self.slack_composer_aux_target.as_ref() != Some(target) {
            return false;
        }
        if let Some(action) = self.first_slack_aux_panel_action() {
            self.activate_slack_aux_panel_action(action, cx);
        }
        true
    }
}
