use super::{Context, SurfaceRoot};

impl SurfaceRoot {
    pub fn set_slack_sidebar_ratio_basis_points<AppState: 'static>(
        &mut self,
        ratio_basis_points: u16,
        cx: &mut Context<AppState>,
    ) {
        let ratio = f32::from(ratio_basis_points) / 10_000.0;
        if self.legacy_slack_sidebar_width.is_none()
            && (self.slack_sidebar_preferred_ratio - ratio).abs() < f32::EPSILON
        {
            return;
        }
        self.slack_sidebar_preferred_ratio = ratio;
        self.legacy_slack_sidebar_width = None;
        for surface in self.retained_surfaces() {
            surface.update(cx, |surface, cx| {
                surface.set_slack_sidebar_preferred_ratio(ratio, cx);
            });
        }
    }

    pub fn set_legacy_slack_sidebar_width<AppState: 'static>(
        &mut self,
        logical_pixels: u16,
        cx: &mut Context<AppState>,
    ) {
        let width = f32::from(logical_pixels);
        self.legacy_slack_sidebar_width = Some(width);
        for surface in self.retained_surfaces() {
            surface.update(cx, |surface, cx| {
                surface.set_legacy_slack_sidebar_width(width, cx);
            });
        }
    }

    pub fn set_slack_workspace_switcher_expanded<AppState: 'static>(
        &mut self,
        expanded: bool,
        cx: &mut Context<AppState>,
    ) -> bool {
        if self.slack_workspace_switcher_expanded == expanded {
            return false;
        }
        self.slack_workspace_switcher_expanded = expanded;
        for surface in self.retained_surfaces() {
            surface.update(cx, |surface, cx| {
                surface.set_slack_workspace_switcher_expanded(expanded, cx);
            });
        }
        true
    }
}
