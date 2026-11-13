use super::super::{Context, SurfaceState};
use crate::ui::surface::{
    message::slack_reaction_picker_glyph, SlackComposerTarget,
    SlackReactionPickerEmojiPresentation, SlackReactionPickerIdentity,
    SlackReactionPickerSelection, SlackReactionPickerSource, SlackReactionSkinToneSupport,
    SLACK_REACTION_PICKER_CATEGORY_COUNT,
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
        if let Some(target) = identity.target() {
            if !self.slack_workspace_api_capabilities.mutate_reactions {
                self.fail_slack_reaction(
                    "Slack reaction changes are unavailable for this workspace.",
                    cx,
                );
                return;
            }
            if self.slack_reaction_pending_for_target(target.as_ref()) {
                return;
            }
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

    pub(in crate::ui::surface) fn activate_first_slack_reaction_picker_emoji(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some((identity, emoji)) = self.slack_reaction_picker.as_ref().and_then(|picker| {
            picker
                .emojis
                .first()
                .map(|emoji| (picker.identity(), emoji.clone()))
        }) else {
            return;
        };
        self.activate_slack_reaction_picker_emoji(
            &identity,
            &emoji.name,
            emoji.skin_tone_support,
            cx,
        );
    }

    pub(crate) fn activate_slack_reaction_picker_emoji(
        &mut self,
        identity: &SlackReactionPickerIdentity,
        reaction_name: &str,
        skin_tone_support: SlackReactionSkinToneSupport,
        cx: &mut Context<Self>,
    ) {
        let Some(picker) = self
            .slack_reaction_picker
            .as_ref()
            .filter(|picker| picker.identity == *identity)
        else {
            return;
        };
        let message_target = picker.identity.target().cloned();
        let composer_snippet = picker.identity.composer_target().cloned().map(|target| {
            let glyph = picker
                .emojis
                .iter()
                .find_map(|emoji| match &emoji.presentation {
                    SlackReactionPickerEmojiPresentation::Glyph(glyph)
                        if emoji.name.as_ref() == reaction_name =>
                    {
                        Some(glyph.clone())
                    }
                    _ => None,
                });
            (target, glyph)
        });
        let reactions = picker.reactions.clone();
        let reaction_name = self.slack_reaction_picker_name(reaction_name, skin_tone_support);
        self.slack_reaction_picker = None;
        self.slack_skin_tone_menu_open = false;
        self.slack_skin_tone_menu_focus_pending = false;
        if let Some(target) = message_target {
            self.toggle_slack_reaction_for_target_with_state(target, reactions, &reaction_name, cx);
        } else if let Some((target, glyph)) = composer_snippet {
            // Standard emoji go in as the character, with the chosen skin
            // tone; custom ones as their `:name:` code, as Slack writes them.
            let snippet = match glyph {
                Some(glyph) => format!(
                    "{} ",
                    slack_reaction_picker_glyph(
                        &glyph,
                        skin_tone_support,
                        self.slack_preferred_skin_tone_selection(),
                    )
                ),
                None => format!(":{reaction_name}: "),
            };
            self.insert_slack_picked_emoji(target, &snippet, cx);
        }
    }

    fn insert_slack_picked_emoji(
        &mut self,
        target: SlackComposerTarget,
        snippet: &str,
        cx: &mut Context<Self>,
    ) {
        match target {
            SlackComposerTarget::Main => self.insert_slack_composer_snippet(snippet, cx),
            SlackComposerTarget::Reply(target) => {
                self.insert_slack_reply_snippet(&target, snippet, cx);
            }
        }
    }

    pub(crate) fn reset_slack_reaction_context(&mut self) {
        self.slack_reaction_picker = None;
        self.slack_skin_tone_menu_open = false;
        self.slack_skin_tone_menu_focus_pending = false;
    }
}
