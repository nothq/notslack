use super::super::{Context, SurfaceState};
use crate::ui::surface::{
    SlackReactionPickerIdentity, SlackReactionPickerSelection, SlackReactionPickerSource,
    SlackReactionSkinToneSupport, SLACK_REACTION_PICKER_CATEGORY_COUNT,
};

impl SurfaceState {
    pub(crate) fn slack_reaction_pending_for_target(
        &self,
        target: &crate::ui::surface::SlackMessageActionTarget,
    ) -> bool {
        self.slack_pending_reactions.contains_key(target)
    }

    pub(crate) fn toggle_slack_reaction_picker_for_target(
        &mut self,
        source: SlackReactionPickerSource,
        cx: &mut Context<Self>,
    ) {
        let identity = &source.identity;
        if self
            .slack_reaction_picker
            .as_ref()
            .is_some_and(|picker| picker.identity.matches_surface(identity))
        {
            self.slack_reaction_picker = None;
            self.slack_reaction_picker_focus_pending = false;
            self.slack_skin_tone_menu_open = false;
            self.slack_skin_tone_menu_focus_pending = false;
            cx.notify();
            return;
        }
        if !self.slack_workspace_api_capabilities.mutate_reactions {
            self.fail_slack_reaction(
                "Slack reaction changes are unavailable for this workspace.",
                cx,
            );
            return;
        }
        if self.slack_reaction_pending_for_target(identity.target().as_ref()) {
            return;
        }
        self.slack_aux_panel = None;
        self.slack_composer_focused = false;
        self.slack_error = None;
        self.slack_reaction_picker = Some(self.slack_reaction_picker_state(
            source,
            SlackReactionPickerSelection {
                query: String::new(),
                category_index: 0,
            },
        ));
        self.slack_skin_tone_menu_open = false;
        self.slack_skin_tone_menu_focus_pending = false;
        self.slack_reaction_picker_focus_pending = true;
        self.begin_slack_reaction_catalog_load(cx);
        cx.notify();
    }

    pub(crate) fn close_slack_reaction_picker(&mut self, cx: &mut Context<Self>) {
        if self.slack_reaction_picker.take().is_some() {
            self.slack_reaction_picker_focus_pending = false;
            self.slack_skin_tone_menu_open = false;
            self.slack_skin_tone_menu_focus_pending = false;
            cx.notify();
        }
    }

    pub(crate) fn set_slack_reaction_picker_query(
        &mut self,
        query: String,
        cx: &mut Context<Self>,
    ) {
        let Some(picker) = self.slack_reaction_picker.as_ref() else {
            return;
        };
        let source = picker.source();
        let category_index = picker.category_index;
        self.slack_reaction_picker = Some(self.slack_reaction_picker_state(
            source,
            SlackReactionPickerSelection {
                query,
                category_index,
            },
        ));
        cx.notify();
    }

    pub(crate) fn select_slack_reaction_picker_category(
        &mut self,
        category_index: usize,
        cx: &mut Context<Self>,
    ) {
        if category_index >= SLACK_REACTION_PICKER_CATEGORY_COUNT {
            return;
        }
        let Some(picker) = self.slack_reaction_picker.as_ref() else {
            return;
        };
        if picker.category_index == category_index && picker.query.is_empty() {
            return;
        }
        let source = picker.source();
        self.slack_reaction_picker = Some(self.slack_reaction_picker_state(
            source,
            SlackReactionPickerSelection {
                query: String::new(),
                category_index,
            },
        ));
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn activate_first_slack_reaction_picker_emoji(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some(picker) = self.slack_reaction_picker.as_ref() else {
            return;
        };
        let Some(emoji) = picker.emojis.first() else {
            return;
        };
        let target = picker.identity.target().clone();
        let reactions = picker.reactions.clone();
        let reaction_name = self.slack_reaction_picker_name(&emoji.name, emoji.skin_tone_support);
        self.slack_reaction_picker = None;
        self.slack_skin_tone_menu_open = false;
        self.slack_skin_tone_menu_focus_pending = false;
        self.toggle_slack_reaction_for_target_with_state(target, reactions, &reaction_name, cx);
    }

    pub(crate) fn activate_slack_reaction_picker_emoji(
        &mut self,
        identity: &SlackReactionPickerIdentity,
        reaction_name: &str,
        skin_tone_support: SlackReactionSkinToneSupport,
        cx: &mut Context<Self>,
    ) {
        let Some((target, reactions)) = self
            .slack_reaction_picker
            .as_ref()
            .filter(|picker| picker.identity == *identity)
            .map(|picker| (picker.identity.target().clone(), picker.reactions.clone()))
        else {
            return;
        };
        let reaction_name = self.slack_reaction_picker_name(reaction_name, skin_tone_support);
        self.slack_reaction_picker = None;
        self.slack_skin_tone_menu_open = false;
        self.slack_skin_tone_menu_focus_pending = false;
        self.toggle_slack_reaction_for_target_with_state(target, reactions, &reaction_name, cx);
    }

    pub(crate) fn reset_slack_reaction_context(&mut self) {
        self.slack_reaction_picker = None;
        self.slack_skin_tone_menu_open = false;
        self.slack_skin_tone_menu_focus_pending = false;
    }
}
