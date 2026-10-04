use super::SurfaceState;
use crate::ui::surface::state::SlackSelfIdentity;

impl SurfaceState {
    pub(crate) fn slack_self_identity(&self) -> SlackSelfIdentity {
        self.slack_workspace()
            .map(|workspace| {
                (
                    workspace
                        .self_display_name
                        .clone()
                        .unwrap_or_else(|| "You".to_string()),
                    workspace.self_user_id.clone(),
                    workspace.self_avatar_label.clone(),
                    workspace.self_avatar_image_url.clone(),
                )
            })
            .unwrap_or_else(|| ("You".to_string(), None, None, None))
    }
}
