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

    /// The selected suggestion: the first one until Up or Down move.
    pub(super) fn first_slack_aux_panel_action(&self) -> Option<SlackAuxPanelRowAction> {
        let actions = self.slack_aux_panel_pickable_actions();
        actions
            .get(
                self.slack_aux_panel_selected
                    .min(actions.len().saturating_sub(1)),
            )
            .cloned()
    }

    /// The suggestions Up and Down move through, in the order shown.
    fn slack_aux_panel_pickable_actions(&self) -> Vec<SlackAuxPanelRowAction> {
        let Some(panel) = self.slack_aux_panel.as_ref() else {
            return Vec::new();
        };
        if panel.query_behavior == Some(SlackAuxPanelQueryBehavior::Mention) {
            return self
                .slack_visible_mention_picker_rows(panel)
                .filter_map(|row| row.action.clone())
                .collect();
        }
        panel
            .sections
            .iter()
            .flat_map(|section| section.rows.iter())
            .filter_map(|row| row.action.clone())
            .collect()
    }

    /// Which of the shown suggestions is selected, kept within the list.
    pub(crate) fn slack_aux_panel_selected_index(&self) -> usize {
        self.slack_aux_panel_selected.min(
            self.slack_aux_panel_pickable_actions()
                .len()
                .saturating_sub(1),
        )
    }

    /// Up (`-1`) or Down (`1`) in the mention suggestions, wrapping around.
    /// `false` when no mention list is open for `target`, so the key moves
    /// the caret as usual.
    pub(crate) fn move_slack_composer_aux_selection(
        &mut self,
        target: &SlackComposerTarget,
        step: isize,
        cx: &mut Context<Self>,
    ) -> bool {
        let open_here =
            self.slack_aux_panel.as_ref().is_some_and(|panel| {
                panel.query_behavior == Some(SlackAuxPanelQueryBehavior::Mention)
            }) && self.slack_composer_aux_target.as_ref() == Some(target);
        if !open_here {
            return false;
        }
        let count = self.slack_aux_panel_pickable_actions().len();
        if count == 0 {
            return true;
        }
        let current = self.slack_aux_panel_selected_index() as isize;
        self.slack_aux_panel_selected = (current + step).rem_euclid(count as isize) as usize;
        cx.notify();
        true
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
