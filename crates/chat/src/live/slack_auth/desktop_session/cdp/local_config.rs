use serde::Deserialize;

use super::parse::required_local_config_field;

#[derive(Clone)]
pub(super) struct SidebarRequestCredentials {
    pub(super) team_id: String,
    pub(super) team_domain: String,
    pub(super) xoxc_token: String,
    pub(super) user_id: Option<String>,
}

#[derive(Deserialize)]
struct SlackLocalConfig {
    teams: std::collections::BTreeMap<String, SlackLocalConfigTeam>,
}

#[derive(Deserialize)]
struct SlackLocalConfigTeam {
    domain: String,
    token: String,
    #[serde(default)]
    user_id: Option<String>,
}

pub(super) fn local_config_credentials(
    raw: &str,
) -> Result<Vec<SidebarRequestCredentials>, String> {
    let config = serde_json::from_str::<SlackLocalConfig>(raw)
        .map_err(|error| format!("failed to decode Slack Desktop localConfig_v2: {error}"))?;
    config
        .teams
        .into_iter()
        .map(|(team_id, team)| {
            let team_id = required_local_config_field(&team_id, "team id")?;
            let team_domain = required_local_config_field(&team.domain, "domain")?;
            let xoxc_token = required_local_config_field(&team.token, "token")?;
            if !xoxc_token.starts_with("xoxc-") {
                return Err("Slack Desktop localConfig_v2 token was not a client token".to_string());
            }
            let user_id = team
                .user_id
                .as_deref()
                .map(str::trim)
                .filter(|user_id| !user_id.is_empty())
                .map(ToOwned::to_owned);
            Ok(SidebarRequestCredentials {
                team_id,
                team_domain,
                xoxc_token,
                user_id,
            })
        })
        .collect()
}
