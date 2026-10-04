use crate::model::{ChatLaunchRoute, ChatTeamId, SlackWorkspaceConnectRequest};

use super::{
    AnyElement, App, AppContext, AppearanceMode, ChatStartup, Entity, SurfaceRoot, SurfaceState,
    SurfaceStateConfig,
};

/// The connection a newly created workspace surface starts with.
struct WorkspaceSurfaceConnection {
    connection_api: std::sync::Arc<dyn crate::model::SlackConnectionApi>,
    connection_generation: u64,
    launch_route: Option<ChatLaunchRoute>,
}

impl SurfaceRoot {
    pub fn render_preview(&mut self, cx: &mut App) -> AnyElement {
        self.prepare_workspace_rail(true, cx);
        self.ensure_surface_state(true, cx);
        self.sync_workspace_rail_presentation(true, cx);
        self.render_workspace_host_surface(cx)
    }

    pub fn render_standalone(&mut self, cx: &mut App) -> AnyElement {
        self.active = true;
        self.render_preview(cx)
    }

    pub(crate) fn ensure_surface(&mut self, cx: &mut App) -> Entity<SurfaceState> {
        if self.workspace_host.is_some() {
            return self.ensure_workspace_surface(None, cx);
        }
        self.ensure_single_surface(cx)
    }

    pub(super) fn current_surface(&self) -> Option<Entity<SurfaceState>> {
        self.workspace_host
            .as_ref()
            .and_then(|host| host.selected_surface())
            .or_else(|| self.surface.clone())
    }

    pub(super) fn ensure_workspace_surface(
        &mut self,
        route_override: Option<ChatLaunchRoute>,
        cx: &mut App,
    ) -> Entity<SurfaceState> {
        let selected_team_id = self
            .workspace_host
            .as_ref()
            .expect("workspace surface requires a Slack workspace host")
            .selected_team_id
            .clone();
        if let Some(surface) = self
            .workspace_host
            .as_ref()
            .and_then(|host| host.selected_surface())
        {
            self.sync_event_source_selection(cx);
            return surface;
        }
        let event_source = self.ensure_event_source(cx);
        let (slot_index, connection) =
            self.prepare_workspace_surface_creation(&selected_team_id, route_override);
        let surface = self.create_workspace_surface(selected_team_id.clone(), connection, cx);
        self.workspace_host
            .as_mut()
            .expect("workspace surface requires a Slack workspace host")
            .slots[slot_index]
            .surface = Some(surface.clone());
        self.attach_workspace_surface_to_rail(slot_index, &surface, cx);
        event_source.update(cx, |event_source, cx| {
            event_source.attach_surface(Some(selected_team_id), &surface, cx);
        });
        self.sync_event_source_selection(cx);
        surface
    }

    fn prepare_workspace_surface_creation(
        &mut self,
        selected_team_id: &ChatTeamId,
        route_override: Option<ChatLaunchRoute>,
    ) -> (usize, WorkspaceSurfaceConnection) {
        let host = self
            .workspace_host
            .as_mut()
            .expect("workspace surface requires a Slack workspace host");
        let slot_index = host.selected_slot_index();
        let connection_generation = {
            let slot = &mut host.slots[slot_index];
            slot.connection_generation = slot
                .connection_generation
                .checked_add(1)
                .expect("Slack workspace connection generation overflowed");
            slot.connection_generation
        };
        let launch_route = route_override.or_else(|| {
            host.initial_route
                .as_ref()
                .is_some_and(|route| route.team_id() == selected_team_id)
                .then(|| {
                    host.initial_route
                        .take()
                        .expect("checked initial Slack launch route")
                })
        });
        (
            slot_index,
            WorkspaceSurfaceConnection {
                connection_api: host.connection_api.clone(),
                connection_generation,
                launch_route,
            },
        )
    }

    fn create_workspace_surface(
        &mut self,
        team_id: ChatTeamId,
        connection: WorkspaceSurfaceConnection,
        cx: &mut App,
    ) -> Entity<SurfaceState> {
        let WorkspaceSurfaceConnection {
            connection_api,
            connection_generation,
            launch_route,
        } = connection;
        let request = SlackWorkspaceConnectRequest::try_new(team_id, launch_route)
            .expect("selected Slack launch route must target its workspace");
        self.create_surface(
            ChatStartup::ConnectionRequired {
                connection_api,
                request,
                generation: connection_generation,
            },
            cx,
        )
    }

    fn ensure_single_surface(&mut self, cx: &mut App) -> Entity<SurfaceState> {
        if let Some(surface) = self.surface.as_ref() {
            let surface = surface.clone();
            if self.input_dirty {
                let input = self.input.clone();
                let startup = self.startup.clone();
                self.input_dirty = false;
                surface.update(cx, |surface, cx| {
                    surface.set_input(input, startup, cx);
                });
            }
            self.sync_event_source_selection(cx);
            return surface;
        }
        let event_source = self.ensure_event_source(cx);
        let surface = self.create_surface(self.startup.clone(), cx);
        self.input_dirty = false;
        self.surface = Some(surface.clone());
        event_source.update(cx, |event_source, cx| {
            event_source.attach_surface(None, &surface, cx);
        });
        self.sync_event_source_selection(cx);
        surface
    }

    fn create_surface(&self, startup: ChatStartup, cx: &mut App) -> Entity<SurfaceState> {
        let input = self.input.clone();
        let config = SurfaceStateConfig {
            theme: self.theme,
            appearance_mode: AppearanceMode::current(cx),
            active: self.active,
            preview_width: self.preview_width,
            viewport_height: self.viewport_height,
        };
        let media_capture_api = self.media_capture_api.clone();
        let slack_sidebar_preferred_ratio = self.slack_sidebar_preferred_ratio;
        let legacy_slack_sidebar_width = self.legacy_slack_sidebar_width;
        let slack_workspace_switcher_expanded = self.slack_workspace_switcher_expanded;
        cx.new(move |cx| {
            let mut surface = SurfaceState::new(input, startup, config, cx);
            surface.media_capture_api = media_capture_api;
            surface.slack_sidebar_preferred_ratio = slack_sidebar_preferred_ratio;
            surface.legacy_slack_sidebar_width = legacy_slack_sidebar_width;
            surface.slack_workspace_switcher_expanded = slack_workspace_switcher_expanded;
            surface
        })
    }
}
