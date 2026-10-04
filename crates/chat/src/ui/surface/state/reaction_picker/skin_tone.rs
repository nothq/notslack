use crate::ui::surface::{
    spawn_background_task_for_entity, Context, SlackReactionSkinToneSupport,
    SlackSkinToneLoadRequest, SlackSkinToneMutationRequest, SurfaceState, Window,
};
use crate::ui::{SlackPreferredSkinTone, SlackSkinTone};

impl SurfaceState {
    pub(crate) fn ensure_slack_skin_tone_menu_focus_observer(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.slack_skin_tone_menu_focus_observer_registered {
            return;
        }
        let focus = self.slack_skin_tone_menu_focus_handle.clone();
        cx.on_focus_out(&focus, window, |surface, _, window, cx| {
            if surface
                .slack_skin_tone_toggle_focus_handle
                .is_focused(window)
            {
                return;
            }
            if surface.slack_skin_tone_menu_open {
                surface.slack_skin_tone_menu_open = false;
                surface.slack_skin_tone_menu_focus_pending = false;
                cx.notify();
            }
        })
        .detach();
        self.slack_skin_tone_menu_focus_observer_registered = true;
    }

    pub(crate) fn sync_slack_preferred_skin_tone(&mut self, cx: &mut Context<Self>) {
        if !self
            .slack_workspace_api_capabilities
            .load_preferred_skin_tone
        {
            return;
        }
        let Some(team_id) = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone())
            .filter(|team_id| !team_id.is_empty())
        else {
            return;
        };
        if self.slack_skin_tone_team_id.as_deref() == Some(team_id.as_str()) {
            return;
        }
        self.reset_slack_preferred_skin_tone();
        self.slack_skin_tone_team_id = Some(team_id.clone());
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_skin_tone_error =
                Some("Skin-tone preferences require a connected Slack workspace.".into());
            cx.notify();
            return;
        };
        self.slack_skin_tone_generation = self
            .slack_skin_tone_generation
            .checked_add(1)
            .expect("Slack skin-tone request generation overflowed");
        let request = SlackSkinToneLoadRequest {
            generation: self.slack_skin_tone_generation,
            team_id,
        };
        self.slack_skin_tone_load_request = Some(request.clone());
        self.slack_skin_tone_error = None;
        spawn_background_task_for_entity(
            (workspace_api, request),
            cx,
            |(workspace_api, request)| {
                let result = workspace_api.load_slack_preferred_skin_tone();
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_preferred_skin_tone_load(request, result, cx);
            },
        );
    }

    pub(in crate::ui::surface) fn reset_slack_preferred_skin_tone(&mut self) {
        self.slack_skin_tone_generation = self
            .slack_skin_tone_generation
            .checked_add(1)
            .expect("Slack skin-tone request generation overflowed");
        self.slack_skin_tone_team_id = None;
        self.slack_preferred_skin_tone = None;
        self.slack_skin_tone_load_request = None;
        self.slack_skin_tone_mutation_request = None;
        self.slack_skin_tone_error = None;
        self.slack_skin_tone_menu_open = false;
        self.slack_skin_tone_menu_focus_pending = false;
    }

    fn finish_slack_preferred_skin_tone_load(
        &mut self,
        request: SlackSkinToneLoadRequest,
        result: Result<SlackPreferredSkinTone, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_skin_tone_generation != request.generation
            || self.slack_skin_tone_load_request.as_ref() != Some(&request)
            || self.slack_skin_tone_team_id.as_deref() != Some(request.team_id.as_str())
        {
            return;
        }
        self.slack_skin_tone_load_request = None;
        match result {
            Ok(preference) if preference.team_id == request.team_id => {
                self.slack_preferred_skin_tone = Some(preference);
                self.slack_skin_tone_error = None;
            }
            Ok(_) => {
                self.slack_preferred_skin_tone = None;
                self.slack_skin_tone_error =
                    Some("Slack returned a skin-tone preference for another workspace.".into());
            }
            Err(error) => {
                self.slack_preferred_skin_tone = None;
                self.slack_skin_tone_error = Some(error.into());
            }
        }
        cx.notify();
    }

    pub(crate) fn open_slack_skin_tone_menu(&mut self, cx: &mut Context<Self>) {
        let Some(preference) = self.slack_preferred_skin_tone.as_ref() else {
            return;
        };
        if !self
            .slack_workspace_api_capabilities
            .mutate_preferred_skin_tone
            || self.slack_skin_tone_mutation_request.is_some()
        {
            return;
        }
        self.slack_skin_tone_menu_selected = preference.selection;
        self.slack_skin_tone_menu_open = true;
        self.slack_skin_tone_menu_focus_pending = true;
        cx.notify();
    }

    pub(crate) fn toggle_slack_skin_tone_menu(&mut self, cx: &mut Context<Self>) {
        if self.slack_skin_tone_menu_open {
            self.close_slack_skin_tone_menu(cx);
        } else {
            self.open_slack_skin_tone_menu(cx);
        }
    }

    pub(crate) fn close_slack_skin_tone_menu(&mut self, cx: &mut Context<Self>) {
        if self.slack_skin_tone_menu_open {
            self.slack_skin_tone_menu_open = false;
            self.slack_skin_tone_menu_focus_pending = false;
            self.slack_reaction_picker_focus_pending = self.slack_reaction_picker.is_some();
            cx.notify();
        }
    }

    pub(crate) fn handle_slack_skin_tone_menu_key_down(
        &mut self,
        event: &crate::ui::KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.slack_skin_tone_menu_open {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.close_slack_skin_tone_menu(cx),
            "up" | "left" if !event.keystroke.modifiers.modified() => {
                self.move_slack_skin_tone_menu_selection(-1, cx);
            }
            "down" | "right" if !event.keystroke.modifiers.modified() => {
                self.move_slack_skin_tone_menu_selection(1, cx);
            }
            "enter" | "space" if !event.keystroke.modifiers.modified() => {
                if let Some(selection) = self.slack_skin_tone_menu_selected {
                    self.start_slack_skin_tone_mutation(selection, cx);
                }
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn select_slack_skin_tone(
        &mut self,
        selection: SlackSkinTone,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_skin_tone_menu_open {
            return;
        }
        self.slack_skin_tone_menu_selected = Some(selection);
        self.start_slack_skin_tone_mutation(selection, cx);
    }

    fn move_slack_skin_tone_menu_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let choices = self.slack_skin_tone_menu_choices();
        let next = match self.slack_skin_tone_menu_selected {
            Some(selected) => {
                let index = choices
                    .iter()
                    .position(|selection| *selection == selected)
                    .expect("Slack skin-tone menu selection must be a declared option");
                (index as isize + delta).rem_euclid(choices.len() as isize) as usize
            }
            None if delta < 0 => choices.len() - 1,
            None => 0,
        };
        self.slack_skin_tone_menu_selected = Some(choices[next]);
        cx.notify();
    }

    pub(crate) fn slack_skin_tone_menu_choices(&self) -> [SlackSkinTone; 6] {
        let mut choices = SlackSkinTone::ALL;
        let Some(active) = self.slack_preferred_skin_tone_selection() else {
            return choices;
        };
        let active_index = choices
            .iter()
            .position(|selection| *selection == active)
            .expect("Slack preferred skin tone must be a declared menu choice");
        choices[active_index..].rotate_left(1);
        choices
    }

    fn start_slack_skin_tone_mutation(&mut self, selected: SlackSkinTone, cx: &mut Context<Self>) {
        if !self
            .slack_workspace_api_capabilities
            .mutate_preferred_skin_tone
            || self.slack_skin_tone_mutation_request.is_some()
        {
            return;
        }
        let Some(preference) = self.slack_preferred_skin_tone.as_ref() else {
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_skin_tone_error =
                Some("Skin-tone preferences require a connected Slack workspace.".into());
            cx.notify();
            return;
        };
        if preference.selection == Some(selected) {
            self.close_slack_skin_tone_menu(cx);
            return;
        }
        let team_id = preference.team_id.clone();
        let previous = preference.selection;
        self.slack_skin_tone_generation = self
            .slack_skin_tone_generation
            .checked_add(1)
            .expect("Slack skin-tone request generation overflowed");
        let request = SlackSkinToneMutationRequest {
            generation: self.slack_skin_tone_generation,
            team_id,
            previous,
            selected,
        };
        self.slack_preferred_skin_tone = Some(SlackPreferredSkinTone {
            team_id: request.team_id.clone(),
            selection: Some(selected),
        });
        self.slack_skin_tone_mutation_request = Some(request.clone());
        self.slack_skin_tone_error = None;
        self.slack_skin_tone_menu_open = false;
        self.slack_skin_tone_menu_focus_pending = false;
        self.slack_reaction_picker_focus_pending = self.slack_reaction_picker.is_some();
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, request),
            cx,
            |(workspace_api, request)| {
                let result = workspace_api.mutate_slack_preferred_skin_tone(request.selected);
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_skin_tone_mutation(request, result, cx);
            },
        );
    }

    fn finish_slack_skin_tone_mutation(
        &mut self,
        request: SlackSkinToneMutationRequest,
        result: Result<SlackPreferredSkinTone, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_skin_tone_generation != request.generation
            || self.slack_skin_tone_mutation_request.as_ref() != Some(&request)
            || self.slack_skin_tone_team_id.as_deref() != Some(request.team_id.as_str())
        {
            return;
        }
        self.slack_skin_tone_mutation_request = None;
        match result {
            Ok(preference)
                if preference.team_id == request.team_id
                    && preference.selection == Some(request.selected) =>
            {
                self.slack_preferred_skin_tone = Some(preference);
                self.slack_skin_tone_error = None;
            }
            Ok(_) => {
                self.rollback_slack_skin_tone_mutation(&request);
                self.slack_skin_tone_error =
                    Some("Slack returned a mismatched preferred skin tone.".into());
            }
            Err(error) => {
                self.rollback_slack_skin_tone_mutation(&request);
                self.slack_skin_tone_error = Some(error.into());
            }
        }
        cx.notify();
    }

    fn rollback_slack_skin_tone_mutation(&mut self, request: &SlackSkinToneMutationRequest) {
        self.slack_preferred_skin_tone = Some(SlackPreferredSkinTone {
            team_id: request.team_id.clone(),
            selection: request.previous,
        });
    }

    pub(crate) fn slack_preferred_skin_tone_selection(&self) -> Option<SlackSkinTone> {
        self.slack_preferred_skin_tone
            .as_ref()
            .and_then(|preference| preference.selection)
    }

    pub(crate) fn slack_reaction_picker_name(
        &self,
        reaction_name: &str,
        support: SlackReactionSkinToneSupport,
    ) -> String {
        match (
            support,
            self.slack_preferred_skin_tone_selection()
                .and_then(SlackSkinTone::modifier),
        ) {
            (SlackReactionSkinToneSupport::Single, Some(modifier)) => {
                format!("{reaction_name}::skin-tone-{modifier}")
            }
            _ => reaction_name.to_string(),
        }
    }
}
