use super::{Context, SurfaceState};
use crate::ui::surface::SlackRailMenu;

impl SurfaceState {
    pub(crate) fn open_slack_workspace_panel(&mut self, cx: &mut Context<Self>) {
        self.toggle_slack_rail_menu(SlackRailMenu::Workspace, cx);
    }
}
