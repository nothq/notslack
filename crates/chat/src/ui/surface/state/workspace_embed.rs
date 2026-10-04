mod bootstrap;
mod refresh;

use super::{
    div, prepare_slack_conversation_snapshot, prepare_slack_dm_inbox_snapshot,
    prepare_slack_shell_snapshot, prepare_slack_sidebar_snapshot, px, AnyElement, Arc, ChatStartup,
    Context, IntoElement, ParentElement, PreparedSlackConversationSnapshot,
    PreparedSlackDmInboxSnapshot, PreparedSlackShellSnapshot, PreparedSlackSidebarSnapshot,
    SlackMainTab, SlackMessageNavigationRequest, SlackSurfaceActivationTiming, Styled,
    SurfaceState, Window, WorkspaceApi,
};
use crate::ui::{
    surface::{
        rgb, slack_palette, SLACK_HISTORY_HEIGHT, SLACK_MAIN_HEADER_HEIGHT,
        SLACK_SIDEBAR_HEADER_HEIGHT,
    },
    SlackMessageTimestamp, SlackWorkspaceConnection, SlackWorkspaceShell,
};
use gpui::{font, prelude::FluentBuilder};
use std::time::Instant;

struct SlackInitialStageLoadResult<T> {
    result: Result<T, String>,
    prepared_at: Option<Instant>,
}

struct SlackInitialWorkspaceRefresh {
    team_id: String,
    conversation_id: String,
    workspace_api: Arc<dyn WorkspaceApi>,
    initial_message_navigation: Option<SlackMessageNavigationRequest>,
}

struct PreparedChatWorkspaceConnection {
    connection: SlackWorkspaceConnection,
    shell: SlackWorkspaceShell,
}

struct SlackWorkspaceConnectionCompletion {
    request: crate::model::SlackWorkspaceConnectRequest,
    generation: u64,
    result: Result<PreparedChatWorkspaceConnection, String>,
}

impl SurfaceState {
    pub(crate) fn chat_surface_visible(&self) -> bool {
        self.active && (self.slack_workspace.is_some() || self.slack_shell.is_some())
    }

    pub(crate) fn ensure_chat_surface_state(
        &mut self,
        activation: Option<SlackSurfaceActivationTiming>,
        cx: &mut Context<Self>,
    ) {
        if !self.active || !self.embedded_shell {
            return;
        }
        if self.chat_surface_visible() {
            return;
        }
        if matches!(
            self.chat_startup,
            ChatStartup::ConnectionRequired { .. }
                | ChatStartup::Loading {
                    request_started: false,
                    ..
                }
        ) {
            if let Some(activation) = activation {
                self.start_slack_activation_load_profile(activation);
            }
            self.ensure_chat_connection(cx);
        }
    }

    pub(crate) fn render_chat_preview(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(workspace) = self.slack_workspace.clone() {
            self.schedule_slack_conversation_load_profile_finish(
                &workspace.conversation_id,
                window,
                cx,
            );
            self.schedule_slack_activation_load_profile_finish(window, cx);
            let surface = self.render_slack_surface(&workspace, cx);
            return (div()
                .size_full()
                .flex_grow(1.0)
                .min_w(px(0.0))
                .min_h(px(0.0))
                .child(surface))
            .into_any_element();
        }
        if self.slack_shell.is_some() {
            self.schedule_slack_activation_load_profile_finish(window, cx);
            return self.render_slack_bootstrap_shell();
        }
        if matches!(
            self.chat_startup,
            ChatStartup::ConnectionRequired { .. } | ChatStartup::Loading { .. }
        ) {
            self.schedule_slack_activation_pending_frame(window, cx);
        }
        let message = match &self.chat_startup {
            ChatStartup::ConnectionRequired { .. } | ChatStartup::Loading { .. } => {
                return self.render_slack_bootstrap_shell();
            }
            ChatStartup::Error { error, .. } => {
                format!("Slack failed to connect: {error}. Press R to retry.")
            }
            ChatStartup::Archive | ChatStartup::Ready { .. } => {
                "chat is unavailable in this embedded workspace".to_string()
            }
            #[cfg(any(test, feature = "test-support"))]
            ChatStartup::Fixture => "chat is unavailable in this fixture".to_string(),
        };
        self.render_embedded_work_in_progress_preview(message)
    }

    pub(crate) fn ensure_chat_connection(&mut self, cx: &mut Context<Self>) {
        let (connection_api, request, generation, previous_connection) = match &self.chat_startup {
            ChatStartup::ConnectionRequired {
                connection_api,
                request,
                generation,
            } => (connection_api.clone(), request.clone(), *generation, None),
            ChatStartup::Loading {
                connection_api,
                request,
                generation,
                request_started: false,
                previous_connection,
            } => (
                connection_api.clone(),
                request.clone(),
                *generation,
                previous_connection.clone(),
            ),
            _ => return,
        };
        self.record_slack_activation_connection_started();
        self.chat_startup = ChatStartup::Loading {
            connection_api: connection_api.clone(),
            request: request.clone(),
            generation,
            request_started: true,
            previous_connection,
        };
        cx.notify();
        self.spawn_background_task(
            (connection_api, request, generation),
            cx,
            move |(connection_api, request, generation)| {
                let result = connection_api
                    .connect_workspace(&request)
                    .and_then(|connection| prepare_chat_workspace_connection(&request, connection));
                SlackWorkspaceConnectionCompletion {
                    request,
                    generation,
                    result,
                }
            },
            |this, completion, cx| match completion.result {
                Ok(connection) => this.apply_chat_workspace_connection(
                    &completion.request,
                    completion.generation,
                    connection,
                    cx,
                ),
                Err(error) => this.apply_chat_connection_error(
                    &completion.request,
                    completion.generation,
                    error,
                    cx,
                ),
            },
        );
    }

    pub(crate) fn reconnect_chat_workspace(&mut self, cx: &mut Context<Self>) {
        self.chat_startup = match &self.chat_startup {
            ChatStartup::Ready {
                connection_api,
                request,
                generation,
                connection,
            } => ChatStartup::Loading {
                connection_api: connection_api.clone(),
                request: request.clone(),
                generation: next_slack_connection_generation(*generation),
                request_started: false,
                previous_connection: Some(connection.clone()),
            },
            ChatStartup::Error {
                connection_api,
                request,
                generation,
                previous_connection,
                ..
            } => ChatStartup::Loading {
                connection_api: connection_api.clone(),
                request: request.clone(),
                generation: next_slack_connection_generation(*generation),
                request_started: false,
                previous_connection: previous_connection.clone(),
            },
            _ => return,
        };
        self.slack_error = None;
        self.ensure_chat_connection(cx);
    }

    fn apply_chat_workspace_connection(
        &mut self,
        request: &crate::model::SlackWorkspaceConnectRequest,
        generation: u64,
        prepared: PreparedChatWorkspaceConnection,
        cx: &mut Context<Self>,
    ) {
        let PreparedChatWorkspaceConnection { connection, shell } = prepared;
        let connection_api = match &self.chat_startup {
            ChatStartup::Loading {
                connection_api,
                request: pending_request,
                generation: pending_generation,
                ..
            } if pending_request == request && *pending_generation == generation => {
                connection_api.clone()
            }
            _ => return,
        };
        let workspace_api = connection.workspace_api.clone();
        let initial_message_anchor = connection.initial_message_anchor.clone();
        self.chat_startup = ChatStartup::Ready {
            connection_api,
            request: request.clone(),
            generation,
            connection,
        };
        self.apply_chat_workspace_capabilities(&workspace_api);
        let team_id = shell.team_id.clone();
        self.reset_slack_reaction_catalog();
        self.reset_slack_preferred_skin_tone();
        self.sync_slack_reaction_catalog_team(&team_id);
        let conversation_id = shell.conversation_id.clone();
        self.defer_slack_members_until_initial_conversation(&conversation_id);
        self.slack_shell = Some(shell);
        self.replace_slack_realtime_subscription(team_id.clone(), workspace_api.clone(), cx);
        self.record_slack_activation_shell_applied(&conversation_id);
        cx.notify();
        let initial_message_navigation = initial_message_anchor.map(|message_timestamp| {
            self.begin_initial_slack_message_navigation(
                team_id.clone(),
                conversation_id.clone(),
                message_timestamp,
            )
        });
        self.refresh_initial_slack_workspace(
            SlackInitialWorkspaceRefresh {
                team_id,
                conversation_id,
                workspace_api,
                initial_message_navigation,
            },
            cx,
        );
    }

    fn apply_chat_workspace_capabilities(&mut self, workspace_api: &Arc<dyn WorkspaceApi>) {
        self.slack_workspace_api_capabilities = workspace_api.capabilities();
        if self.slack_active_tab == SlackMainTab::Canvas
            && !self.slack_workspace_api_capabilities.load_canvas
        {
            self.slack_active_tab = SlackMainTab::Messages;
        }
        if self.slack_active_tab == SlackMainTab::BookmarkFolder
            && !self.slack_workspace_api_capabilities.load_bookmark_folder
        {
            self.reset_slack_bookmark_folder_context();
        }
    }
}

fn prepare_chat_workspace_connection(
    request: &crate::model::SlackWorkspaceConnectRequest,
    connection: SlackWorkspaceConnection,
) -> Result<PreparedChatWorkspaceConnection, String> {
    if connection.team_id != *request.team_id() {
        return Err(format!(
            "Slack connection for team {} returned team {}",
            request.team_id().as_str(),
            connection.team_id.as_str()
        ));
    }
    let shell = (*connection.shell).clone();
    if shell.team_id != request.team_id().as_str() {
        return Err(format!(
            "Slack connection for team {} returned a shell for team {}",
            request.team_id().as_str(),
            shell.team_id
        ));
    }
    Ok(PreparedChatWorkspaceConnection { connection, shell })
}

fn next_slack_connection_generation(generation: u64) -> u64 {
    generation
        .checked_add(1)
        .expect("Slack connection generation overflowed")
}

fn load_initial_slack_conversation(
    workspace_api: &dyn WorkspaceApi,
    team_id: &str,
    conversation_id: &str,
    anchor_timestamp: Option<&SlackMessageTimestamp>,
) -> Result<PreparedSlackConversationSnapshot, String> {
    let conversation = match anchor_timestamp {
        Some(anchor_timestamp) => {
            workspace_api.load_slack_conversation_at(conversation_id, anchor_timestamp)?
        }
        None => workspace_api.load_slack_conversation(conversation_id)?,
    };
    if conversation.team_id != team_id || conversation.conversation_id != conversation_id {
        return Err("Slack conversation response targeted a different workspace".to_string());
    }
    Ok(prepare_slack_conversation_snapshot(conversation))
}

fn prepare_initial_stage<T>(
    profile_enabled: bool,
    load: impl FnOnce() -> Result<T, String>,
) -> SlackInitialStageLoadResult<T> {
    let result = load();
    let prepared_at = (profile_enabled && result.is_ok()).then(Instant::now);
    SlackInitialStageLoadResult {
        result,
        prepared_at,
    }
}

fn slack_bootstrap_sidebar_rows(color: gpui::Rgba) -> AnyElement {
    div()
        .flex_grow(1.0)
        .min_h(px(0.0))
        .px(px(16.0))
        .pt(px(12.0))
        .flex()
        .flex_col()
        .gap(px(14.0))
        .children(
            [96.0, 132.0, 78.0, 116.0, 88.0]
                .map(|width| div().w(px(width)).h(px(12.0)).rounded(px(4.0)).bg(color)),
        )
        .into_any_element()
}

fn slack_bootstrap_message_rows(color: gpui::Rgba) -> AnyElement {
    div()
        .flex_grow(1.0)
        .min_h(px(0.0))
        .px(px(24.0))
        .pt(px(28.0))
        .flex()
        .flex_col()
        .gap(px(24.0))
        .children([0.82_f32, 0.62, 0.74, 0.48].map(|width| {
            div()
                .w_full()
                .flex()
                .gap(px(12.0))
                .child(div().size(px(36.0)).rounded_full().bg(color))
                .child(
                    div()
                        .flex_grow(width)
                        .max_w(px(560.0 * width))
                        .flex()
                        .flex_col()
                        .gap(px(8.0))
                        .child(div().w(px(112.0)).h(px(12.0)).rounded(px(4.0)).bg(color))
                        .child(div().w_full().h(px(10.0)).rounded(px(4.0)).bg(color)),
                )
        }))
        .into_any_element()
}

fn report_chat_connection_error(error: impl AsRef<str>) {
    eprintln!("[notslack chat] connection: {}", error.as_ref());
}
