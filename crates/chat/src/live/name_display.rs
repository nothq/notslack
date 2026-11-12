//! Whether a workspace shows people by their full name or their display name.
//!
//! Slack keeps this as a workspace preference that each user may override
//! (Preferences > Messages & media > Names). The answer is recorded per team
//! when a workspace first loads, and consulted wherever a person is named.

use std::{
    collections::BTreeMap,
    sync::{Mutex, RwLock},
};

use serde_json::Value;

use super::SlackApiClient;

const USER_OVERRIDE: &str = "display_real_names_override";
const TEAM_PREFERENCE: &str = "display_real_names";

static FULL_NAMES_BY_TEAM: RwLock<BTreeMap<String, bool>> = RwLock::new(BTreeMap::new());
static FETCH: Mutex<()> = Mutex::new(());

/// Asks Slack once per team which name to show. A failure leaves the team
/// unrecorded, so names stay full names and the next load asks again.
pub(in crate::live) fn ensure_team_name_display(team_id: &str, api: &SlackApiClient) {
    let Ok(_fetch) = FETCH.lock() else {
        return;
    };
    if recorded(team_id).is_some() {
        return;
    }
    match fetch_shows_full_names(api) {
        Ok(full_names) => {
            if let Ok(mut teams) = FULL_NAMES_BY_TEAM.write() {
                teams.insert(team_id.to_string(), full_names);
            }
        }
        Err(error) => eprintln!("Slack name display preference failed: {error}"),
    }
}

fn fetch_shows_full_names(api: &SlackApiClient) -> Result<bool, String> {
    let user = api.post("users.prefs.get", &[("prefs", USER_OVERRIDE.to_string())])?;
    let user_override = user.pointer(&format!("/prefs/{USER_OVERRIDE}"));
    if let Some(full_names) = user_override.and_then(override_shows_full_names) {
        return Ok(full_names);
    }
    let team = api.post("team.prefs.get", &[("prefs", TEAM_PREFERENCE.to_string())])?;
    team.pointer(&format!("/prefs/{TEAM_PREFERENCE}"))
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("Slack team.prefs.get response omitted {TEAM_PREFERENCE}"))
}

/// The user's override: 1 shows full names, -1 display names, and anything
/// else follows the workspace.
fn override_shows_full_names(value: &Value) -> Option<bool> {
    match value.as_i64() {
        Some(1) => Some(true),
        Some(-1) => Some(false),
        _ => None,
    }
}

fn recorded(team_id: &str) -> Option<bool> {
    FULL_NAMES_BY_TEAM.read().ok()?.get(team_id).copied()
}

/// A person from another team (a shared channel, an Enterprise Grid org) is
/// shown the way the signed-in workspaces agree on. Full names are Slack's
/// default, and the answer when nothing is known.
fn shows_full_names(team_id: Option<&str>) -> bool {
    let Ok(teams) = FULL_NAMES_BY_TEAM.read() else {
        return true;
    };
    resolve_shows_full_names(&teams, team_id)
}

fn resolve_shows_full_names(teams: &BTreeMap<String, bool>, team_id: Option<&str>) -> bool {
    if let Some(full_names) = team_id.and_then(|team_id| teams.get(team_id)) {
        return *full_names;
    }
    teams.is_empty() || teams.values().any(|full_names| *full_names)
}

/// The name to show for a person of `team_id`: the full name or the display
/// name first, as that team prefers, falling back to the other.
pub(in crate::live) fn preferred_person_name<'a>(
    team_id: Option<&str>,
    display_name: Option<&'a str>,
    full_names: impl IntoIterator<Item = Option<&'a str>>,
) -> Option<&'a str> {
    order_person_names(shows_full_names(team_id), display_name, full_names)
}

fn order_person_names<'a>(
    full_names_first: bool,
    display_name: Option<&'a str>,
    full_names: impl IntoIterator<Item = Option<&'a str>>,
) -> Option<&'a str> {
    let non_empty = |name: Option<&'a str>| name.map(str::trim).filter(|name| !name.is_empty());
    let display_name = non_empty(display_name);
    let full_name = full_names.into_iter().find_map(non_empty);
    if full_names_first {
        full_name.or(display_name)
    } else {
        display_name.or(full_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn teams(entries: &[(&str, bool)]) -> BTreeMap<String, bool> {
        entries
            .iter()
            .map(|(team_id, full_names)| (team_id.to_string(), *full_names))
            .collect()
    }

    #[test]
    fn display_names_come_first_when_the_team_prefers_them() {
        let name = order_person_names(false, Some("Will"), [None, Some("Chenyuan Wang")]);
        assert_eq!(name, Some("Will"));
    }

    #[test]
    fn full_names_come_first_when_the_team_prefers_them() {
        let name = order_person_names(true, Some("Will"), [Some("Chenyuan Wang"), None]);
        assert_eq!(name, Some("Chenyuan Wang"));
    }

    #[test]
    fn a_blank_display_name_falls_back_to_the_full_name() {
        let name = order_person_names(false, Some("  "), [Some("Chenyuan Wang")]);
        assert_eq!(name, Some("Chenyuan Wang"));
    }

    #[test]
    fn the_user_override_decides_before_the_workspace() {
        assert_eq!(override_shows_full_names(&Value::from(1)), Some(true));
        assert_eq!(override_shows_full_names(&Value::from(-1)), Some(false));
        assert_eq!(override_shows_full_names(&Value::from(0)), None);
    }

    #[test]
    fn a_known_team_uses_its_own_preference() {
        let teams = teams(&[("T1", false), ("T2", true)]);
        assert!(!resolve_shows_full_names(&teams, Some("T1")));
        assert!(resolve_shows_full_names(&teams, Some("T2")));
    }

    #[test]
    fn an_unknown_team_follows_the_workspaces_when_they_agree() {
        assert!(!resolve_shows_full_names(
            &teams(&[("T1", false)]),
            Some("T9")
        ));
        assert!(resolve_shows_full_names(
            &teams(&[("T1", false), ("T2", true)]),
            None
        ));
        assert!(resolve_shows_full_names(&teams(&[]), Some("T9")));
    }
}
