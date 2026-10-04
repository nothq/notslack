use super::{Arc, Context, SlackProfilePanelState, SurfaceState, WorkspaceApi};

impl SurfaceState {
    pub(crate) fn open_slack_profile(&mut self, user_id: &str, cx: &mut Context<Self>) {
        if !self.is_slack_workspace() || !self.slack_workspace_api_capabilities.load_profile {
            return;
        }
        let requested_user_id = user_id.to_string();
        let label = self.slack_profile_label(user_id);
        self.slack_profile_panel = Some(SlackProfilePanelState::Loading {
            user_id: requested_user_id.clone(),
            label: label.clone(),
        });
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.show_missing_slack_profile_api(requested_user_id, label, cx);
            return;
        };
        self.spawn_slack_profile_load(requested_user_id, workspace_api, cx);
    }

    pub(in crate::ui::surface::state) fn slack_profile_label(&self, user_id: &str) -> String {
        self.slack_workspace()
            .filter(|workspace| {
                self.slack_directory_team_id.as_deref() == Some(workspace.team_id.as_str())
            })
            .and_then(|_| {
                self.slack_directory_rows
                    .iter()
                    .find(|row| row.presence_user_id.as_deref() == Some(user_id))
                    .map(|row| row.label.to_string())
            })
            .or_else(|| {
                self.slack_workspace().and_then(|workspace| {
                    workspace
                        .messages
                        .iter()
                        .find(|message| message.user_id.as_deref() == Some(user_id))
                        .map(|message| message.author.clone())
                        .or_else(|| {
                            workspace
                                .sections
                                .iter()
                                .flat_map(|section| section.items.iter())
                                .find(|item| item.user_id.as_deref() == Some(user_id))
                                .map(|item| item.label.clone())
                        })
                })
            })
            .unwrap_or_else(|| user_id.to_string())
    }

    pub(in crate::ui::surface::state) fn show_missing_slack_profile_api(
        &mut self,
        user_id: String,
        label: String,
        cx: &mut Context<Self>,
    ) {
        self.slack_profile_panel = Some(SlackProfilePanelState::Error {
            user_id,
            label,
            message: "missing Slack workspace api".to_string(),
        });
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn spawn_slack_profile_load(
        &mut self,
        requested_user_id: String,
        workspace_api: Arc<dyn WorkspaceApi>,
        cx: &mut Context<Self>,
    ) {
        self.spawn_background_task(
            requested_user_id.clone(),
            cx,
            move |requested_user_id| workspace_api.load_slack_profile(&requested_user_id),
            move |this, result, cx| {
                let Some(panel) = this.slack_profile_panel.as_ref() else {
                    return;
                };
                if panel.user_id() != requested_user_id {
                    return;
                }
                let label = panel.label().to_string();
                this.slack_profile_panel = Some(match result {
                    Ok(profile) => {
                        if let Some(url) = profile.avatar_image_url.clone() {
                            this.enqueue_slack_remote_image_url(url, cx);
                        }
                        SlackProfilePanelState::Loaded(Box::new(profile))
                    }
                    Err(message) => SlackProfilePanelState::Error {
                        user_id: requested_user_id.clone(),
                        label,
                        message,
                    },
                });
                cx.notify();
            },
        );
    }
}
