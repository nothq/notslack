use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Deserializer, Serialize};

pub(super) const SLACK_DESKTOP_SESSIONS_SCHEMA_VERSION: u8 = 3;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct SlackWebBuildTimestamp(String);

impl SlackWebBuildTimestamp {
    pub fn parse(raw: impl Into<String>) -> Result<Self, String> {
        let raw = raw.into();
        if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("Slack web build timestamp must contain only decimal digits".to_string());
        }
        Ok(Self(raw))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for SlackWebBuildTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Self::parse(raw).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct StoredSlackDesktopSession {
    pub(super) team_id: String,
    pub(super) team_domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) user_id: Option<String>,
    pub(super) xoxc_token: String,
    pub(super) cookie_header: String,
    pub(super) web_build_timestamp: SlackWebBuildTimestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) draft_count: Option<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct StoredSlackDesktopSessionRecord {
    pub(super) workspace_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) workspace_logo_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) notification_playback: Option<super::SlackNotificationPlayback>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) notification_sound: Option<crate::model::SlackNotificationSound>,
    #[serde(flatten)]
    pub(super) session: StoredSlackDesktopSession,
}

#[derive(Clone, Serialize, Deserialize)]
pub(super) struct StoredSlackDesktopSessions {
    schema_version: u8,
    ordered_team_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_team_id: Option<String>,
    sessions_by_team_id: BTreeMap<String, StoredSlackDesktopSessionRecord>,
}

impl StoredSlackDesktopSessions {
    pub(super) fn new(
        ordered_team_ids: Vec<String>,
        selected_team_id: Option<String>,
        sessions_by_team_id: BTreeMap<String, StoredSlackDesktopSessionRecord>,
    ) -> Result<Self, String> {
        Self {
            schema_version: SLACK_DESKTOP_SESSIONS_SCHEMA_VERSION,
            ordered_team_ids,
            selected_team_id,
            sessions_by_team_id,
        }
        .validate()
    }

    pub(super) fn validate(self) -> Result<Self, String> {
        self.validate_shape()?;
        let mut unique_team_ids = HashSet::with_capacity(self.ordered_team_ids.len());
        for team_id in &self.ordered_team_ids {
            validate_required_field(team_id, "workspace order team id")?;
            if !unique_team_ids.insert(team_id.clone()) {
                return Err(format!(
                    "stored Slack Desktop workspace order repeated team {team_id}"
                ));
            }
            let record = self.sessions_by_team_id.get(team_id).ok_or_else(|| {
                format!("stored Slack Desktop workspace order referenced unknown team {team_id}")
            })?;
            if record.session.team_id.as_str() != team_id {
                return Err(format!(
                    "stored Slack Desktop session map key {team_id} did not match team {}",
                    record.session.team_id
                ));
            }
        }

        let mut web_build_timestamp = None;
        for (team_id, record) in &self.sessions_by_team_id {
            if !unique_team_ids.contains(team_id) {
                return Err(format!(
                    "stored Slack Desktop session for team {team_id} was absent from workspace order"
                ));
            }
            validate_record(team_id, record)?;
            match web_build_timestamp {
                Some(expected) if expected != &record.session.web_build_timestamp => {
                    return Err(
                        "stored Slack Desktop sessions contained different web build timestamps"
                            .to_string(),
                    );
                }
                Some(_) => {}
                None => web_build_timestamp = Some(&record.session.web_build_timestamp),
            }
        }

        if let Some(selected_team_id) = self.selected_team_id.as_deref() {
            validate_required_field(selected_team_id, "selected team id")?;
            if !self.sessions_by_team_id.contains_key(selected_team_id) {
                return Err(format!(
                    "stored Slack Desktop selected team {selected_team_id} had no authenticated session"
                ));
            }
        }
        Ok(self)
    }

    fn validate_shape(&self) -> Result<(), String> {
        if self.schema_version != SLACK_DESKTOP_SESSIONS_SCHEMA_VERSION {
            return Err(format!(
                "stored Slack Desktop session schema version {} is unsupported",
                self.schema_version
            ));
        }
        if self.ordered_team_ids.is_empty() {
            return Err("stored Slack Desktop sessions did not include any workspaces".to_string());
        }
        if self.ordered_team_ids.len() != self.sessions_by_team_id.len() {
            return Err(
                "stored Slack Desktop workspace order did not match its session set".to_string(),
            );
        }
        Ok(())
    }

    pub(super) fn session(&self, team_id: &str) -> Option<&StoredSlackDesktopSession> {
        self.sessions_by_team_id
            .get(team_id)
            .map(|record| &record.session)
    }

    pub(super) fn selected_team_id(&self) -> Option<&str> {
        self.selected_team_id.as_deref()
    }

    pub(super) fn first_team_id(&self) -> &str {
        self.ordered_team_ids
            .first()
            .expect("validated Slack Desktop sessions have an ordered team")
    }

    pub(super) fn ordered_records(
        &self,
    ) -> impl ExactSizeIterator<Item = &StoredSlackDesktopSessionRecord> {
        self.ordered_team_ids.iter().map(|team_id| {
            self.sessions_by_team_id
                .get(team_id)
                .expect("validated Slack Desktop workspace order must reference a session")
        })
    }
}

fn validate_record(
    map_team_id: &str,
    record: &StoredSlackDesktopSessionRecord,
) -> Result<(), String> {
    validate_required_field(&record.workspace_name, "workspace name")?;
    if record
        .workspace_logo_url
        .as_deref()
        .is_some_and(|url| url.trim().is_empty())
    {
        return Err(format!(
            "stored Slack Desktop session for team {map_team_id} had an empty workspace logo URL"
        ));
    }
    let session = &record.session;
    validate_required_field(&session.team_id, "team id")?;
    if session.team_id != map_team_id {
        return Err(format!(
            "stored Slack Desktop session map key {map_team_id} did not match team {}",
            session.team_id
        ));
    }
    validate_required_field(&session.team_domain, "team domain")?;
    if session
        .user_id
        .as_deref()
        .is_some_and(|user_id| user_id.trim().is_empty())
    {
        return Err(format!(
            "stored Slack Desktop session for team {map_team_id} had an empty user id"
        ));
    }
    validate_required_field(&session.xoxc_token, "client token")?;
    if !session.xoxc_token.starts_with("xoxc-") {
        return Err(format!(
            "stored Slack Desktop session for team {map_team_id} did not contain a client token"
        ));
    }
    validate_required_field(&session.cookie_header, "cookie header")?;
    if session.draft_count.is_none() {
        return Err(format!(
            "stored Slack Desktop session for team {map_team_id} was missing required sidebar draft state"
        ));
    }
    Ok(())
}

fn validate_required_field(value: &str, label: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("stored Slack Desktop session is missing {label}"));
    }
    Ok(())
}
