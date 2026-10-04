use std::rc::Rc;

use gpui::Entity;
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::{Context, SlackProfilePanelState, SurfaceState};
use crate::ui::surface::{
    slack_palette, SlackSelfStatusDialog, SlackStatusExpirationPreset, WorkspaceApi,
};
use crate::ui::{
    alpha, px, rgb, spawn_background_task_for_entity, SlackSelfStatus, SlackUserPresence,
};

type SlackSelfStatusMutation = (std::sync::Arc<dyn WorkspaceApi>, SlackSelfStatus, u64);
type SlackSelfPresenceMutation = (std::sync::Arc<dyn WorkspaceApi>, SlackUserPresence, u64);

impl SurfaceState {
    pub(crate) fn toggle_slack_self_menu(&mut self, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.subscribe_realtime {
            self.open_slack_self_panel(cx);
            return;
        }
        self.slack_self_menu_open = !self.slack_self_menu_open;
        self.slack_self_settings_error = None;
        cx.notify();
    }

    pub(crate) fn close_slack_self_menu(&mut self, cx: &mut Context<Self>) {
        if !self.slack_self_menu_open {
            return;
        }
        self.slack_self_menu_open = false;
        cx.notify();
    }

    pub(crate) fn open_slack_self_status_dialog(&mut self, cx: &mut Context<Self>) {
        let self_user_id = self.slack_self_identity().1;
        let loaded_text = self
            .slack_profile_panel
            .as_ref()
            .and_then(|panel| match panel {
                SlackProfilePanelState::Loaded(profile)
                    if Some(profile.user_id.as_str()) == self_user_id.as_deref() =>
                {
                    profile.status_text.clone()
                }
                _ => None,
            })
            .unwrap_or_default();
        let status = self.slack_self_status.as_ref();
        self.slack_self_status_dialog = Some(SlackSelfStatusDialog {
            text: status
                .map(|status| status.text().to_string())
                .unwrap_or(loaded_text),
            emoji: status
                .map(|status| status.emoji().to_string())
                .unwrap_or_default(),
            ..Default::default()
        });
        self.slack_self_menu_open = false;
        self.slack_self_settings_error = None;
        cx.notify();
    }

    pub(crate) fn close_slack_self_status_dialog(&mut self, cx: &mut Context<Self>) {
        if self
            .slack_self_status_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.saving)
        {
            return;
        }
        if self.slack_self_status_dialog.take().is_some() {
            self.slack_self_settings_generation =
                self.slack_self_settings_generation.wrapping_add(1);
            cx.notify();
        }
    }

    pub(crate) fn cycle_slack_self_status_expiration(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.slack_self_status_dialog.as_mut() else {
            return;
        };
        let index = SlackStatusExpirationPreset::ALL
            .iter()
            .position(|preset| *preset == dialog.expiration)
            .expect("Slack status expiration preset must be known");
        dialog.expiration =
            SlackStatusExpirationPreset::ALL[(index + 1) % SlackStatusExpirationPreset::ALL.len()];
        dialog.error = None;
        cx.notify();
    }

    pub(crate) fn submit_slack_self_status(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self
            .slack_self_status_dialog
            .as_ref()
            .filter(|dialog| !dialog.saving)
        else {
            return;
        };
        let status = match dialog.expiration.expiration().and_then(|expiration| {
            SlackSelfStatus::new(dialog.text.clone(), dialog.emoji.clone(), expiration)
        }) {
            Ok(status) => status,
            Err(error) => {
                self.slack_self_status_dialog
                    .as_mut()
                    .expect("validated Slack status dialog must remain open")
                    .error = Some(error);
                cx.notify();
                return;
            }
        };
        self.begin_slack_self_status_mutation(status, cx);
    }

    pub(crate) fn clear_slack_self_status(&mut self, cx: &mut Context<Self>) {
        if self
            .slack_self_status_dialog
            .as_ref()
            .is_some_and(|dialog| dialog.saving)
        {
            return;
        }
        self.begin_slack_self_status_mutation(SlackSelfStatus::cleared(), cx);
    }

    fn begin_slack_self_status_mutation(
        &mut self,
        status: SlackSelfStatus,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_self_status_dialog
                .as_mut()
                .expect("Slack status mutation requires an open dialog")
                .error = Some("Slack status requires a connected workspace".to_string());
            cx.notify();
            return;
        };
        self.slack_self_settings_generation = self.slack_self_settings_generation.wrapping_add(1);
        let generation = self.slack_self_settings_generation;
        let dialog = self
            .slack_self_status_dialog
            .as_mut()
            .expect("Slack status mutation requires an open dialog");
        dialog.saving = true;
        dialog.error = None;
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, status, generation),
            cx,
            |(workspace_api, status, generation): SlackSelfStatusMutation| {
                let result = workspace_api.mutate_slack_self_status(&status);
                (generation, result)
            },
            |this, (generation, result), cx| {
                if generation != this.slack_self_settings_generation
                    || this.slack_self_status_dialog.is_none()
                {
                    return;
                }
                match result {
                    Ok(status) => {
                        this.apply_slack_self_status(status);
                        this.slack_self_status_dialog = None;
                    }
                    Err(error) => {
                        let dialog = this
                            .slack_self_status_dialog
                            .as_mut()
                            .expect("validated Slack status dialog must remain open");
                        dialog.saving = false;
                        dialog.error = Some(error);
                    }
                }
                cx.notify();
            },
        );
    }

    fn apply_slack_self_status(&mut self, status: SlackSelfStatus) {
        let self_user_id = self.slack_self_identity().1;
        if let Some(SlackProfilePanelState::Loaded(profile)) = self.slack_profile_panel.as_mut() {
            if Some(profile.user_id.as_str()) == self_user_id.as_deref() {
                profile.status_text =
                    (!status.text().is_empty()).then(|| status.text().to_string());
            }
        }
        self.slack_self_status = (!status.is_clear()).then_some(status);
        self.slack_self_settings_error = None;
    }

    pub(crate) fn toggle_slack_self_presence(&mut self, cx: &mut Context<Self>) {
        if self.slack_self_presence_mutating {
            return;
        }
        let Some(workspace) = self.slack_workspace() else {
            return;
        };
        let next_presence = match self
            .slack_presence_authority
            .resolve_self_presence(workspace)
        {
            Some(SlackUserPresence::Away) => SlackUserPresence::Active,
            _ => SlackUserPresence::Away,
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        self.slack_self_settings_generation = self.slack_self_settings_generation.wrapping_add(1);
        let generation = self.slack_self_settings_generation;
        self.slack_self_presence_mutating = true;
        self.slack_self_menu_open = false;
        self.slack_self_settings_error = None;
        cx.notify();
        spawn_background_task_for_entity(
            (workspace_api, next_presence, generation),
            cx,
            |(workspace_api, presence, generation): SlackSelfPresenceMutation| {
                let result = workspace_api.mutate_slack_self_presence(presence);
                (generation, result)
            },
            |this, (generation, result), cx| {
                if generation != this.slack_self_settings_generation {
                    return;
                }
                this.slack_self_presence_mutating = false;
                match result {
                    Ok(presence) => {
                        let (authority, data) =
                            (&mut this.slack_presence_authority, &mut this.data);
                        authority.apply_self_presence(data, presence);
                        this.slack_self_settings_error = None;
                    }
                    Err(error) => this.slack_self_settings_error = Some(error),
                }
                cx.notify();
            },
        );
    }

    pub(crate) fn slack_self_status_text_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let dialog = self
            .slack_self_status_dialog
            .as_ref()
            .expect("Slack status text input requires an open dialog");
        let surface = cx.entity().downgrade();
        let on_change: TextInputChange = Rc::new(move |value, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    if let Some(dialog) = surface.slack_self_status_dialog.as_mut() {
                        dialog.text = value;
                        dialog.error = None;
                        cx.notify();
                    }
                })
                .ok();
        });
        let props = TextInputProps::single_line(dialog.text.clone())
            .placeholder("What’s your status?")
            .style(self.slack_self_status_input_style())
            .accessibility(
                self.slack_self_status_text_accessibility_id.clone(),
                "Status text",
            )
            .on_change(on_change)
            .on_submit(self.slack_self_status_submit_action(cx))
            .on_escape(self.slack_self_status_close_action(cx));
        self.slack_self_status_text_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_self_status_text_input.clone()
    }

    pub(crate) fn slack_self_status_emoji_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let dialog = self
            .slack_self_status_dialog
            .as_ref()
            .expect("Slack status emoji input requires an open dialog");
        let surface = cx.entity().downgrade();
        let on_change: TextInputChange = Rc::new(move |value, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    if let Some(dialog) = surface.slack_self_status_dialog.as_mut() {
                        dialog.emoji = value;
                        dialog.error = None;
                        cx.notify();
                    }
                })
                .ok();
        });
        let props = TextInputProps::single_line(dialog.emoji.clone())
            .placeholder(":speech_balloon:")
            .style(self.slack_self_status_input_style())
            .accessibility(
                self.slack_self_status_emoji_accessibility_id.clone(),
                "Status emoji",
            )
            .on_change(on_change)
            .on_submit(self.slack_self_status_submit_action(cx))
            .on_escape(self.slack_self_status_close_action(cx));
        self.slack_self_status_emoji_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_self_status_emoji_input.clone()
    }

    fn slack_self_status_submit_action(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| surface.submit_slack_self_status(cx))
                .ok();
        })
    }

    fn slack_self_status_close_action(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| surface.close_slack_self_status_dialog(cx))
                .ok();
        })
    }

    fn slack_self_status_input_style(&self) -> TextInputStyle {
        let palette = slack_palette(self.appearance_mode);
        TextInputStyle {
            height: px(38.0),
            min_height: px(38.0),
            padding_x: px(10.0),
            padding_y: px(8.0),
            radius: px(6.0),
            background: rgb(palette.composer_bg).into(),
            border: rgb(palette.composer_border).into(),
            focused_border: rgb(palette.composer_focused_border).into(),
            text: rgb(palette.main_text).into(),
            placeholder: rgb(palette.composer_placeholder).into(),
            selection: alpha(palette.link, 0.28),
            caret: rgb(palette.main_text).into(),
            font_size: px(15.0),
            line_height: px(22.0),
            font_family: Some("Lato".into()),
        }
    }
}
