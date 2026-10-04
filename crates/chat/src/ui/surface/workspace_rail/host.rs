use gpui::{
    AppContext, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Styled,
};

use super::super::{div, px, AnyElement, App, SurfaceRoot, SLACK_HISTORY_HEIGHT};
use super::SlackWorkspaceRail;
use crate::ui::{SurfaceState, SLACK_WORKSPACE_RAIL_WIDTH};

impl SurfaceRoot {
    pub(in crate::ui::surface) fn prepare_workspace_rail(&mut self, active: bool, cx: &mut App) {
        let Some(rail) = self.ensure_workspace_rail(cx) else {
            return;
        };
        let pending_selection = active
            .then(|| rail.update(cx, |rail, _cx| rail.take_pending_selection()))
            .flatten();
        if let Some(team_id) = pending_selection {
            if let Err(error) = self.select_workspace_from_app(team_id, None, cx) {
                eprintln!("failed to select Slack workspace: {error}");
            }
        }
        self.sync_workspace_rail_presentation(active, cx);
    }

    pub(in crate::ui::surface) fn sync_workspace_rail_presentation(
        &mut self,
        active: bool,
        cx: &mut App,
    ) {
        let Some(rail) = self.workspace_rail.clone() else {
            return;
        };
        let host = self
            .workspace_host
            .as_ref()
            .expect("Slack workspace rail requires a retained workspace host");
        let selected_team_id = host.selected_team_id.clone();
        let expanded = self.slack_workspace_switcher_expanded;
        rail.update(cx, |rail, cx| {
            rail.sync_presentation(selected_team_id, active, expanded, cx);
        });
    }

    pub(in crate::ui::surface) fn attach_workspace_surface_to_rail(
        &mut self,
        index: usize,
        surface: &gpui::Entity<SurfaceState>,
        cx: &mut App,
    ) {
        let Some(rail) = self.workspace_rail.clone() else {
            return;
        };
        rail.update(cx, |rail, cx| rail.attach_surface(index, surface, cx));
    }

    pub(in crate::ui::surface) fn render_workspace_host_surface(
        &mut self,
        cx: &mut App,
    ) -> AnyElement {
        let surface = self.ensure_surface(cx);
        let Some(rail) = self.workspace_rail.clone() else {
            return surface.into_any_element();
        };
        let add_button_rail = rail.clone();
        let workspace_switcher_expanded = self.slack_workspace_switcher_expanded;
        let workspace_rail_width = if workspace_switcher_expanded {
            SLACK_WORKSPACE_RAIL_WIDTH
        } else {
            0.0
        };
        div()
            .size_full()
            .relative()
            .capture_any_mouse_down(move |event: &MouseDownEvent, window, cx| {
                if !workspace_switcher_expanded
                    || event.button != MouseButton::Left
                    || !add_button_rail.read(cx).add_button_contains(event.position)
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                add_button_rail.update(cx, |rail, cx| {
                    rail.toggle_add_menu(false, window, cx);
                });
            })
            .child(div().size_full().child(surface))
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(SLACK_HISTORY_HEIGHT))
                    .bottom(px(0.0))
                    .w(px(workspace_rail_width))
                    .child(rail),
            )
            .into_any_element()
    }

    fn ensure_workspace_rail(&mut self, cx: &mut App) -> Option<gpui::Entity<SlackWorkspaceRail>> {
        let host = self.workspace_host.as_ref()?;
        if let Some(rail) = self.workspace_rail.as_ref() {
            return Some(rail.clone());
        }
        let descriptors = host
            .slots
            .iter()
            .map(|slot| slot.descriptor.clone())
            .collect();
        let selected_team_id = host.selected_team_id.clone();
        let rail = cx.new(move |cx| SlackWorkspaceRail::new(descriptors, selected_team_id, cx));
        self.workspace_rail = Some(rail.clone());
        let retained = self
            .workspace_host
            .as_ref()
            .expect("Slack workspace rail requires a retained workspace host")
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| slot.surface.clone().map(|surface| (index, surface)))
            .collect::<Vec<_>>();
        for (index, surface) in retained {
            self.attach_workspace_surface_to_rail(index, &surface, cx);
        }
        Some(rail)
    }
}
