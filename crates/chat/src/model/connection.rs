use std::{collections::HashSet, sync::Arc};

use crate::model::{
    ChatLaunchRoute, ChatTeamId, SlackMessageTimestamp, SlackWorkspaceApi, SlackWorkspaceShell,
};

pub type SlackConnectionOutcome = Result<SlackWorkspaceConnection, String>;

pub trait SlackConnectionApi: Send + Sync + 'static {
    fn workspace_directory(&self) -> SlackWorkspaceDirectory;

    fn connect_workspace(&self, request: &SlackWorkspaceConnectRequest) -> SlackConnectionOutcome;

    fn remember_active_workspace(&self, team_id: &ChatTeamId) -> Result<(), String>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackWorkspaceDescriptor {
    pub team_id: ChatTeamId,
    pub workspace_name: String,
    pub workspace_logo_url: Option<String>,
    pub is_default: bool,
}

impl SlackWorkspaceDescriptor {
    pub fn try_new(
        team_id: ChatTeamId,
        workspace_name: String,
        workspace_logo_url: Option<String>,
        is_default: bool,
    ) -> Result<Self, String> {
        let workspace_name = workspace_name.trim();
        if workspace_name.is_empty() {
            return Err(format!(
                "Slack workspace {} has no display name",
                team_id.as_str()
            ));
        }
        let workspace_logo_url = workspace_logo_url
            .map(|url| {
                let url = url.trim();
                if url.is_empty() {
                    return Err(format!(
                        "Slack workspace {} has an empty logo URL",
                        team_id.as_str()
                    ));
                }
                Ok(url.to_string())
            })
            .transpose()?;
        Ok(Self {
            team_id,
            workspace_name: workspace_name.to_string(),
            workspace_logo_url,
            is_default,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackWorkspaceDirectory {
    ordered: Arc<[SlackWorkspaceDescriptor]>,
    initial_team_id: ChatTeamId,
}

impl SlackWorkspaceDirectory {
    pub fn try_new(
        ordered: Vec<SlackWorkspaceDescriptor>,
        initial_team_id: ChatTeamId,
    ) -> Result<Self, String> {
        if ordered.is_empty() {
            return Err("Slack workspace directory is empty".to_string());
        }
        let mut team_ids = HashSet::with_capacity(ordered.len());
        let mut default_count = 0;
        for workspace in &ordered {
            if !team_ids.insert(workspace.team_id.clone()) {
                return Err(format!(
                    "Slack workspace directory repeated team {}",
                    workspace.team_id.as_str()
                ));
            }
            default_count += usize::from(workspace.is_default);
        }
        if default_count != 1 {
            return Err(format!(
                "Slack workspace directory has {default_count} default workspaces"
            ));
        }
        if !team_ids.contains(&initial_team_id) {
            return Err(format!(
                "Slack initial workspace {} is absent from the directory",
                initial_team_id.as_str()
            ));
        }
        Ok(Self {
            ordered: ordered.into(),
            initial_team_id,
        })
    }

    pub fn ordered(&self) -> &[SlackWorkspaceDescriptor] {
        &self.ordered
    }

    pub const fn initial_team_id(&self) -> &ChatTeamId {
        &self.initial_team_id
    }

    pub fn contains(&self, team_id: &ChatTeamId) -> bool {
        self.ordered
            .iter()
            .any(|workspace| workspace.team_id == *team_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackWorkspaceConnectRequest {
    team_id: ChatTeamId,
    launch_route: Option<ChatLaunchRoute>,
}

impl SlackWorkspaceConnectRequest {
    pub fn try_new(
        team_id: ChatTeamId,
        launch_route: Option<ChatLaunchRoute>,
    ) -> Result<Self, String> {
        if launch_route
            .as_ref()
            .is_some_and(|route| route.team_id() != &team_id)
        {
            return Err(format!(
                "Slack connect route does not target team {}",
                team_id.as_str()
            ));
        }
        Ok(Self {
            team_id,
            launch_route,
        })
    }

    pub const fn team_id(&self) -> &ChatTeamId {
        &self.team_id
    }

    pub const fn launch_route(&self) -> Option<&ChatLaunchRoute> {
        self.launch_route.as_ref()
    }
}

pub struct SlackWorkspaceConnection {
    pub team_id: ChatTeamId,
    pub shell: Box<SlackWorkspaceShell>,
    pub workspace_api: Arc<dyn SlackWorkspaceApi>,
    pub initial_message_anchor: Option<SlackMessageTimestamp>,
}

impl Clone for SlackWorkspaceConnection {
    fn clone(&self) -> Self {
        Self {
            team_id: self.team_id.clone(),
            shell: self.shell.clone(),
            workspace_api: self.workspace_api.clone(),
            initial_message_anchor: self.initial_message_anchor.clone(),
        }
    }
}
