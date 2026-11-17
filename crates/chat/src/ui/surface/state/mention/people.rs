//! The people a mention can name beyond those already on screen: everyone in
//! the workspace's destination directory, loaded once per workspace the first
//! time a mention picker opens.

use std::{collections::HashSet, sync::Arc};

use super::super::{
    slack_aux_panel_matches_query, Context, SlackAuxPanelQueryBehavior, SlackVisiblePersonRow,
    SurfaceState,
};
use crate::ui::{SlackDestinationDirectorySnapshot, SlackDestinationTarget};

pub(in crate::ui::surface::state) struct SlackMentionPeople {
    team_id: String,
    people: Arc<[SlackMentionPerson]>,
}

struct SlackMentionPerson {
    user_id: String,
    label: String,
    real_name: Option<String>,
    /// Both names in lowercase, made once so typing a query only compares.
    search_key: String,
}

impl SlackMentionPerson {
    fn new(user_id: String, label: String, real_name: Option<String>) -> Self {
        let search_key = match &real_name {
            Some(real_name) => format!("{}\n{}", label, real_name),
            None => label.clone(),
        }
        .to_ascii_lowercase();
        Self {
            user_id,
            label,
            real_name,
            search_key,
        }
    }
}

/// The picker shows a handful of people; matching stops well past that.
const SLACK_MENTION_PEOPLE_MATCH_LIMIT: usize = 50;

impl SlackMentionPeople {
    fn from_directory(directory: SlackDestinationDirectorySnapshot) -> Self {
        let people = directory
            .candidates
            .into_iter()
            .filter_map(|candidate| {
                let SlackDestinationTarget::Person { user_id } = candidate.target else {
                    return None;
                };
                let label = candidate.label.trim().to_string();
                let real_name = candidate
                    .real_name
                    .map(|real_name| real_name.trim().to_string())
                    .filter(|real_name| !real_name.is_empty() && *real_name != label);
                (!label.is_empty()).then(|| SlackMentionPerson::new(user_id, label, real_name))
            })
            .collect();
        Self {
            team_id: directory.team_id,
            people,
        }
    }

    fn empty(team_id: String) -> Self {
        Self {
            team_id,
            people: Arc::default(),
        }
    }
}

impl SurfaceState {
    /// Starts loading the workspace's people for the mention picker, unless
    /// they are loaded or loading already.
    pub(in crate::ui::surface::state) fn ensure_slack_mention_people(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self
            .slack_workspace_api_capabilities
            .load_destination_directory
        {
            return;
        }
        let Some(team_id) = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone())
            .filter(|team_id| !team_id.is_empty())
        else {
            return;
        };
        let known = self
            .slack_mention_people
            .as_ref()
            .is_some_and(|people| people.team_id == team_id);
        if known || self.slack_mention_people_loading_team_id.as_ref() == Some(&team_id) {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        self.slack_mention_people_loading_team_id = Some(team_id.clone());
        self.spawn_background_task(
            (workspace_api, team_id),
            cx,
            |(workspace_api, team_id)| {
                let people = workspace_api
                    .load_slack_destination_directory()
                    .map(SlackMentionPeople::from_directory);
                (team_id, people)
            },
            |this, (team_id, people), cx| {
                this.finish_slack_mention_people_load(team_id, people, cx);
            },
        );
    }

    /// A failed load is kept as an empty list, so typing does not retry it on
    /// every key; the picker then offers the people on screen, as before.
    fn finish_slack_mention_people_load(
        &mut self,
        team_id: String,
        people: Result<SlackMentionPeople, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_mention_people_loading_team_id.as_ref() != Some(&team_id) {
            return;
        }
        self.slack_mention_people_loading_team_id = None;
        self.slack_mention_people = Some(match people {
            Ok(people) if people.team_id == team_id => people,
            Ok(_) => SlackMentionPeople::empty(team_id),
            Err(error) => {
                eprintln!("Slack mention people load failed: {error}");
                SlackMentionPeople::empty(team_id)
            }
        });
        if let Some((SlackAuxPanelQueryBehavior::Mention, query)) =
            self.current_slack_aux_panel_query()
        {
            self.set_slack_mention_query(query, cx);
        }
    }

    /// The people matching a query: those on screen first, then the rest
    /// of the workspace, up to a limit. Each row's detail is the person's
    /// full name when it differs from the label, so a query matches either
    /// name.
    pub(in crate::ui::surface::state) fn slack_mention_people_rows(
        &self,
        normalized_query: &str,
    ) -> Vec<SlackVisiblePersonRow> {
        let visible = self.slack_visible_people_rows();
        let team_id = self.slack_workspace().map(|workspace| &workspace.team_id);
        let Some(directory) = self
            .slack_mention_people
            .as_ref()
            .filter(|people| Some(&people.team_id) == team_id)
        else {
            return visible
                .into_iter()
                .filter(|row| slack_mention_row_matches(row, normalized_query))
                .take(SLACK_MENTION_PEOPLE_MATCH_LIMIT)
                .collect();
        };
        merge_slack_mention_people(
            visible,
            &directory.people,
            normalized_query,
            SLACK_MENTION_PEOPLE_MATCH_LIMIT,
        )
    }
}

fn slack_mention_row_matches(row: &SlackVisiblePersonRow, normalized_query: &str) -> bool {
    slack_aux_panel_matches_query(normalized_query, &row.label)
        || row
            .detail
            .as_deref()
            .is_some_and(|detail| slack_aux_panel_matches_query(normalized_query, detail))
}

/// Typing in the picker runs this on every key, over everyone in the
/// workspace, so it only compares precomputed lowercase names and stops at
/// `limit` matches.
fn merge_slack_mention_people(
    mut rows: Vec<SlackVisiblePersonRow>,
    people: &[SlackMentionPerson],
    normalized_query: &str,
    limit: usize,
) -> Vec<SlackVisiblePersonRow> {
    for row in &mut rows {
        if let Some(person) = people.iter().find(|person| person.user_id == row.user_id) {
            row.detail = person.real_name.clone().filter(|name| *name != row.label);
        }
    }
    let on_screen = rows
        .iter()
        .map(|row| row.user_id.clone())
        .collect::<HashSet<_>>();
    rows.retain(|row| slack_mention_row_matches(row, normalized_query));
    rows.truncate(limit);
    for person in people {
        if rows.len() >= limit {
            break;
        }
        if (normalized_query.is_empty() || person.search_key.contains(normalized_query))
            && !on_screen.contains(&person.user_id)
        {
            rows.push(SlackVisiblePersonRow {
                user_id: person.user_id.clone(),
                label: person.label.clone(),
                accessory: None,
                detail: person.real_name.clone(),
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn person(user_id: &str, label: &str, real_name: Option<&str>) -> SlackMentionPerson {
        SlackMentionPerson::new(
            user_id.to_string(),
            label.to_string(),
            real_name.map(str::to_string),
        )
    }

    fn row(user_id: &str, label: &str, detail: &str) -> SlackVisiblePersonRow {
        SlackVisiblePersonRow {
            user_id: user_id.to_string(),
            label: label.to_string(),
            accessory: None,
            detail: Some(detail.to_string()),
        }
    }

    fn summary(rows: &[SlackVisiblePersonRow]) -> Vec<(&str, &str, Option<&str>)> {
        rows.iter()
            .map(|row| {
                (
                    row.user_id.as_str(),
                    row.label.as_str(),
                    row.detail.as_deref(),
                )
            })
            .collect()
    }

    #[test]
    fn people_off_screen_follow_the_people_on_screen() {
        let rows = merge_slack_mention_people(
            vec![row("U1", "Ada", "Visible in the active conversation.")],
            &[
                person("U2", "Will", Some("Chenyuan Wang")),
                person("U1", "Ada", Some("Ada Lovelace")),
            ],
            "",
            50,
        );
        assert_eq!(
            summary(&rows),
            [
                ("U1", "Ada", Some("Ada Lovelace")),
                ("U2", "Will", Some("Chenyuan Wang")),
            ]
        );
    }

    #[test]
    fn a_query_matches_either_name_and_stops_at_the_limit() {
        let people = [
            person("U2", "Will", Some("Chenyuan Wang")),
            person("U3", "Willa", None),
            person("U4", "Grace", None),
        ];
        let rows = merge_slack_mention_people(Vec::new(), &people, "wang", 50);
        assert_eq!(summary(&rows), [("U2", "Will", Some("Chenyuan Wang"))]);
        let rows = merge_slack_mention_people(Vec::new(), &people, "will", 1);
        assert_eq!(summary(&rows), [("U2", "Will", Some("Chenyuan Wang"))]);
    }

    #[test]
    fn a_person_missing_from_the_directory_keeps_their_row() {
        let rows = merge_slack_mention_people(
            vec![row("U9", "Guest", "Visible in the active conversation.")],
            &[],
            "",
            50,
        );
        assert_eq!(
            summary(&rows),
            [("U9", "Guest", Some("Visible in the active conversation."))]
        );
    }

    #[test]
    fn the_directory_keeps_only_people_and_drops_a_repeated_name() {
        let candidate =
            |target, label: &str, real_name: Option<&str>| crate::ui::SlackDestinationCandidate {
                target,
                label: label.to_string(),
                real_name: real_name.map(str::to_string),
                email: None,
                avatar_image_url: None,
                presence: None,
                participant_user_ids: Vec::new(),
                participant_labels: Vec::new(),
            };
        let people = SlackMentionPeople::from_directory(SlackDestinationDirectorySnapshot {
            team_id: "T1".to_string(),
            self_user_id: "U0".to_string(),
            candidates: vec![
                candidate(
                    SlackDestinationTarget::Person {
                        user_id: "U2".to_string(),
                    },
                    "Will",
                    Some("Chenyuan Wang"),
                ),
                candidate(
                    SlackDestinationTarget::Person {
                        user_id: "U3".to_string(),
                    },
                    "Grace Hopper",
                    Some("Grace Hopper"),
                ),
                candidate(
                    SlackDestinationTarget::Conversation {
                        conversation_id: "C1".to_string(),
                        kind: crate::ui::SlackConversationKind::Channel,
                    },
                    "general",
                    None,
                ),
            ],
        });
        let listed = people
            .people
            .iter()
            .map(|person| (person.label.as_str(), person.real_name.as_deref()))
            .collect::<Vec<_>>();
        assert_eq!(
            listed,
            [("Will", Some("Chenyuan Wang")), ("Grace Hopper", None)]
        );
    }
}
