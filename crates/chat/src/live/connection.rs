use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use super::{runtime_registry::SlackWorkspaceRuntimeRegistry, SlackHostRuntime};

#[derive(Clone)]
struct ProductionSlackConnectionApi {
    directory: crate::model::SlackWorkspaceDirectory,
    host: Arc<SlackHostRuntime>,
}

static PROCESS_ACTIVE_SLACK_TEAM: OnceLock<Mutex<Option<crate::model::ChatTeamId>>> =
    OnceLock::new();
static SLACK_ACTIVE_TEAM_PERSISTENCE: OnceLock<Mutex<()>> = OnceLock::new();

pub fn production_slack_connection_api(
    route: Option<crate::model::ChatLaunchRoute>,
    host: Arc<SlackHostRuntime>,
) -> Arc<dyn crate::model::SlackConnectionApi> {
    let runtimes = host.registry();
    let directory = slack_workspace_directory(route.as_ref(), runtimes)
        .expect("authenticated Slack runtime inventory must form a workspace directory");
    Arc::new(ProductionSlackConnectionApi { directory, host })
}

impl crate::model::SlackConnectionApi for ProductionSlackConnectionApi {
    fn workspace_directory(&self) -> crate::model::SlackWorkspaceDirectory {
        self.directory.clone()
    }

    fn connect_workspace(
        &self,
        request: &crate::model::SlackWorkspaceConnectRequest,
    ) -> crate::model::SlackConnectionOutcome {
        self.host.registry().connection_for_team(request)
    }

    fn remember_active_workspace(&self, team_id: &crate::model::ChatTeamId) -> Result<(), String> {
        if !self.host.registry().contains_team(team_id.as_str()) {
            return Err(format!(
                "Slack workspace {} is not authenticated",
                team_id.as_str()
            ));
        }
        *process_active_slack_team()? = Some(team_id.clone());
        persist_active_slack_team(team_id)
    }
}

fn slack_workspace_directory(
    route: Option<&crate::model::ChatLaunchRoute>,
    runtimes: &SlackWorkspaceRuntimeRegistry,
) -> Result<crate::model::SlackWorkspaceDirectory, String> {
    let inventory_default = runtimes
        .authenticated_teams()
        .iter()
        .find(|workspace| workspace.is_default)
        .ok_or("Slack Desktop has no default authenticated workspace")?;
    let inventory_default = crate::model::ChatTeamId::try_from(inventory_default.team_id.clone())?;
    let initial_team_id = match route {
        Some(route) => {
            if !runtimes.contains_team(route.team_id().as_str()) {
                return Err(format!(
                    "Slack Desktop is not signed in to requested workspace {}",
                    route.team_id().as_str()
                ));
            }
            let mut active_team = process_active_slack_team()?;
            *active_team = Some(route.team_id().clone());
            route.team_id().clone()
        }
        None => {
            let mut active_team = process_active_slack_team()?;
            let selected = active_team
                .as_ref()
                .filter(|team_id| runtimes.contains_team(team_id.as_str()))
                .cloned()
                .unwrap_or(inventory_default);
            *active_team = Some(selected.clone());
            selected
        }
    };
    let ordered = runtimes
        .authenticated_teams()
        .iter()
        .map(|team| {
            crate::model::SlackWorkspaceDescriptor::try_new(
                crate::model::ChatTeamId::try_from(team.team_id.clone())?,
                team.workspace_name.clone(),
                team.workspace_logo_url.clone(),
                team.team_id == initial_team_id.as_str(),
            )
        })
        .collect::<Result<Vec<_>, String>>()?;
    crate::model::SlackWorkspaceDirectory::try_new(ordered, initial_team_id)
}

fn process_active_slack_team(
) -> Result<MutexGuard<'static, Option<crate::model::ChatTeamId>>, String> {
    PROCESS_ACTIVE_SLACK_TEAM
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "Slack active workspace mutex poisoned".to_string())
}

fn persist_active_slack_team(team_id: &crate::model::ChatTeamId) -> Result<(), String> {
    let _persistence = SLACK_ACTIVE_TEAM_PERSISTENCE
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "Slack active workspace persistence mutex poisoned".to_string())?;
    if process_active_slack_team()?.as_ref() != Some(team_id) {
        return Ok(());
    }
    super::remember_slack_active_team(team_id.as_str())
}
