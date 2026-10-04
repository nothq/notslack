use std::{sync::Arc, time::Instant};

use crate::{
    model::{ChatLaunchRoute, ChatTeamId},
    ui::{MediaCaptureApi, SlackWorkspace, SLACK_SIDEBAR_DEFAULT_RATIO},
};

use super::{
    surface_event_source::ChatSelectedSurface, workspace_host::SlackWorkspaceHost, App, AppContext,
    AppearanceMode, ChatEventSource, ChatStartup, Context, Entity, KeyDownEvent,
    SlackConnectionApi, SlackSurfaceActivationTiming, SurfaceInput, SurfaceRoot, SurfaceState,
    SurfaceTheme, WorkspaceApi,
};

mod layout;

/// The selection generation and surface of a selected Slack workspace.
type SelectedSlackWorkspace = (u64, Entity<SurfaceState>);

impl SurfaceRoot {
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_compose(
        workspace: Option<SlackWorkspace>,
        workspace_api: Option<Arc<dyn WorkspaceApi>>,
    ) -> Self {
        Self::for_compose_with_media_capture(workspace, workspace_api, None)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn for_compose_with_media_capture(
        workspace: Option<SlackWorkspace>,
        workspace_api: Option<Arc<dyn WorkspaceApi>>,
        media_capture_api: Option<Arc<MediaCaptureApi>>,
    ) -> Self {
        let mut root = Self::from_input(SurfaceInput {
            workspace,
            workspace_api,
            local_file_api: None,
            embedded_shell: true,
            initial_thread_message_id: None,
        });
        root.media_capture_api = media_capture_api;
        root
    }

    pub fn archive(
        workspace: SlackWorkspace,
        workspace_api: Arc<dyn WorkspaceApi>,
        local_file_api: Arc<dyn crate::model::SlackLocalFileApi>,
    ) -> Self {
        Self::from_workspace_input(
            SurfaceInput {
                workspace: Some(workspace),
                workspace_api: Some(workspace_api),
                local_file_api: Some(local_file_api),
                embedded_shell: true,
                initial_thread_message_id: None,
            },
            ChatStartup::Archive,
        )
    }

    pub fn production(
        connection_api: Arc<dyn SlackConnectionApi>,
        initial_route: Option<ChatLaunchRoute>,
        media_capture_api: Arc<MediaCaptureApi>,
        local_file_api: Arc<dyn crate::model::SlackLocalFileApi>,
    ) -> Self {
        let workspace_host = SlackWorkspaceHost::new(connection_api, initial_route);
        Self {
            input: SurfaceInput {
                embedded_shell: true,
                local_file_api: Some(local_file_api),
                ..SurfaceInput::default()
            },
            startup: ChatStartup::Archive,
            media_capture_api: Some(media_capture_api),
            input_dirty: false,
            theme: SurfaceTheme::default(),
            active: false,
            preview_width: 900.0,
            viewport_height: 800.0,
            slack_sidebar_preferred_ratio: SLACK_SIDEBAR_DEFAULT_RATIO,
            legacy_slack_sidebar_width: None,
            slack_workspace_switcher_expanded: false,
            surface: None,
            workspace_host: Some(workspace_host),
            workspace_rail: None,
            event_source: None,
            activation_started_at: None,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn from_input(input: SurfaceInput) -> Self {
        Self::from_workspace_input(input, ChatStartup::Fixture)
    }

    fn from_workspace_input(input: SurfaceInput, startup: ChatStartup) -> Self {
        Self {
            input,
            startup,
            media_capture_api: None,
            input_dirty: true,
            theme: SurfaceTheme::default(),
            active: false,
            preview_width: 900.0,
            viewport_height: 800.0,
            slack_sidebar_preferred_ratio: SLACK_SIDEBAR_DEFAULT_RATIO,
            legacy_slack_sidebar_width: None,
            slack_workspace_switcher_expanded: false,
            surface: None,
            workspace_host: None,
            workspace_rail: None,
            event_source: None,
            activation_started_at: None,
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_input(&mut self, input: SurfaceInput) {
        self.input = input;
        self.startup = ChatStartup::Fixture;
        self.input_dirty = true;
    }

    pub fn set_theme(&mut self, theme: SurfaceTheme) {
        self.theme = theme;
    }

    pub fn set_preview_width(&mut self, preview_width: f32) {
        self.preview_width = preview_width;
    }

    pub fn set_viewport_height(&mut self, viewport_height: f32) {
        self.viewport_height = viewport_height;
    }

    pub fn has_workspace(&self) -> bool {
        self.workspace_host.is_some() || self.input.workspace.is_some()
    }

    pub fn event_source<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Entity<ChatEventSource> {
        let event_source = self.ensure_event_source(cx);
        self.ensure_surface(cx);
        self.sync_event_source_selection(cx);
        event_source
    }

    pub(super) fn ensure_event_source(&mut self, cx: &mut App) -> Entity<ChatEventSource> {
        if let Some(event_source) = self.event_source.as_ref() {
            return event_source.clone();
        }
        let event_source = cx.new(|_cx| ChatEventSource::new());
        self.event_source = Some(event_source.clone());
        let retained = match self.workspace_host.as_ref() {
            Some(host) => host
                .slots
                .iter()
                .filter_map(|slot| {
                    slot.surface
                        .clone()
                        .map(|surface| (Some(slot.descriptor.team_id.clone()), surface))
                })
                .collect::<Vec<_>>(),
            None => self
                .surface
                .clone()
                .map(|surface| vec![(None, surface)])
                .unwrap_or_default(),
        };
        event_source.update(cx, |event_source, cx| {
            for (team_id, surface) in retained {
                event_source.attach_surface(team_id, &surface, cx);
            }
        });
        self.sync_event_source_selection(cx);
        event_source
    }

    pub(super) fn sync_event_source_selection(&mut self, cx: &mut App) {
        let event_source = self.ensure_event_source(cx);
        let surface = self.current_surface();
        let (team_id, selection_generation) = match self.workspace_host.as_ref() {
            Some(host) => (
                Some(host.selected_team_id.clone()),
                host.selection_generation,
            ),
            None => {
                let team_id = surface
                    .as_ref()
                    .and_then(|surface| {
                        surface
                            .read(cx)
                            .slack_workspace()
                            .map(|workspace| workspace.team_id.clone())
                    })
                    .or_else(|| {
                        self.input
                            .workspace
                            .as_ref()
                            .map(|workspace| workspace.team_id.clone())
                    })
                    .map(|team_id| {
                        ChatTeamId::try_from(team_id)
                            .expect("mounted Slack workspace must have a valid team ID")
                    });
                (team_id, 0)
            }
        };
        let root_active = self.active;
        event_source.update(cx, |event_source, cx| {
            event_source.set_selected_surface(
                ChatSelectedSurface {
                    team_id,
                    selection_generation,
                    surface: surface.as_ref(),
                    root_active,
                },
                cx,
            );
        });
    }

    pub fn select_slack_workspace<AppState: 'static>(
        &mut self,
        team_id: &str,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let team_id = ChatTeamId::try_from(team_id.to_string())?;
        self.select_workspace(team_id, None, cx).map(|_| ())
    }

    pub(super) fn select_workspace<AppState: 'static>(
        &mut self,
        team_id: ChatTeamId,
        launch_route: Option<ChatLaunchRoute>,
        cx: &mut Context<AppState>,
    ) -> Result<SelectedSlackWorkspace, String> {
        let result = self.select_workspace_from_app(team_id, launch_route, cx);
        cx.notify();
        result
    }

    pub(super) fn select_workspace_from_app(
        &mut self,
        team_id: ChatTeamId,
        launch_route: Option<ChatLaunchRoute>,
        cx: &mut App,
    ) -> Result<SelectedSlackWorkspace, String> {
        let host = self
            .workspace_host
            .as_ref()
            .ok_or("Slack workspace selection is unavailable")?;
        if host.slot_index(&team_id).is_none() {
            return Err(format!(
                "Slack workspace {} is not authenticated",
                team_id.as_str()
            ));
        }
        let changed = host.selected_team_id != team_id;
        let previous_surface = changed.then(|| host.selected_surface()).flatten();
        if changed {
            let connection_api = host.connection_api.clone();
            let persisted_team_id = team_id.clone();
            self.workspace_host
                .as_mut()
                .expect("validated Slack workspace host")
                .select(team_id);
            cx.background_executor()
                .spawn(async move {
                    if let Err(error) = connection_api.remember_active_workspace(&persisted_team_id)
                    {
                        eprintln!("failed to remember active Slack workspace: {error}");
                    }
                })
                .detach();
            if let Some(previous_surface) = previous_surface {
                Self::deactivate_surface(&previous_surface, cx);
            }
        }
        let surface = self.ensure_workspace_surface(launch_route, cx);
        if self.active {
            self.ensure_surface_state(true, cx);
        } else {
            self.sync_event_source_selection(cx);
        }
        let selection_generation = self
            .workspace_host
            .as_ref()
            .expect("selected Slack workspace host")
            .selection_generation;
        self.sync_workspace_rail_presentation(self.active, cx);
        Ok((selection_generation, surface))
    }

    pub fn ensure_surface_state(&mut self, active: bool, cx: &mut App) {
        self.active = active;
        if !active {
            self.activation_started_at = None;
            for surface in self.retained_surfaces() {
                Self::deactivate_surface(&surface, cx);
            }
            self.sync_event_source_selection(cx);
            return;
        }
        let surface_construction_started_at = Instant::now();
        let surface_created = self.current_surface().is_none();
        let surface = self.ensure_surface(cx);
        let surface_ready_at = Instant::now();
        let activation =
            self.activation_started_at
                .take()
                .map(|activated_at| SlackSurfaceActivationTiming {
                    activated_at,
                    surface_construction_started_at,
                    surface_ready_at,
                    surface_created,
                });
        let theme = self.theme;
        let preview_width = self.preview_width;
        let viewport_height = self.viewport_height;
        surface.update(cx, |surface, cx| {
            surface.theme = theme;
            surface.appearance_mode = AppearanceMode::current(cx);
            surface.active = true;
            surface.preview_width = preview_width;
            surface.viewport_height = viewport_height;
            surface.ensure_chat_surface_state(activation, cx);
            surface.ensure_slack_remote_image_loads(cx);
            surface.ensure_slack_conversation_live_sync(cx);
            surface.ensure_slack_realtime_subscription(cx);
            surface.ensure_slack_realtime_pending_refreshes(cx);
        });
        self.sync_event_source_selection(cx);
    }

    pub fn prepare_frame(&mut self, active: bool, cx: &mut App) {
        self.prepare_workspace_rail(active, cx);
        self.ensure_surface_state(active, cx);
        self.sync_workspace_rail_presentation(active, cx);
    }

    pub fn handle_key_down(&mut self, event: &KeyDownEvent, cx: &mut App) -> bool {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.handle_slack_key_down(event, cx))
    }

    fn retained_surfaces(&self) -> Vec<Entity<SurfaceState>> {
        match self.workspace_host.as_ref() {
            Some(host) => host.retained_surfaces().collect(),
            None => self.surface.iter().cloned().collect(),
        }
    }

    fn deactivate_surface(surface: &Entity<SurfaceState>, cx: &mut App) {
        surface.update(cx, |surface, cx| {
            surface.active = false;
            surface.slack_expanded_attachment = None;
            surface.cancel_slack_composer_capture_for_owner_change(cx);
            surface.stop_slack_media_playback(cx);
            surface.ensure_slack_conversation_live_sync(cx);
        });
    }
}
