use std::{
    collections::{BTreeSet, HashMap, HashSet},
    thread,
};

use crate::model::{
    SlackConnectedOrganization, SlackConversationMember, SlackConversationMembersCursor,
    SlackConversationMembersSnapshot, SlackUserPresence,
};
use serde::Deserialize;
use serde_json::Value;

use super::SlackLiveWorkspaceLoader;
use crate::live::payload::{
    select_slack_avatar_image_url, users::load_slack_member_users_with_cache, SlackAvatarImageUrls,
    SlackAvatarPurpose,
};

const SLACK_CONVERSATION_MEMBERS_PAGE_SIZE: usize = 100;
const SLACK_CONNECTED_ORGANIZATION_BADGE_LIMIT: usize = 3;

pub(super) fn load_conversation_members(
    loader: &SlackLiveWorkspaceLoader,
    conversation_id: &str,
    cursor: Option<&SlackConversationMembersCursor>,
) -> Result<SlackConversationMembersSnapshot, String> {
    if conversation_id.trim().is_empty() {
        return Err("Slack conversation id must not be empty".to_string());
    }
    let mut params = vec![
        ("channel", conversation_id.to_string()),
        ("limit", SLACK_CONVERSATION_MEMBERS_PAGE_SIZE.to_string()),
    ];
    if let Some(cursor) = cursor {
        params.push(("cursor", cursor.as_str().to_string()));
    }
    let response = serde_json::from_value::<SlackConversationMembersResponse>(
        loader.api.post("conversations.members", &params)?,
    )
    .map_err(|error| format!("failed to decode Slack conversations.members response: {error}"))?;
    let connected_team_ids = connected_team_ids(
        &loader.load_conversation_detail(conversation_id)?,
        conversation_id,
        &loader.team_id,
    )?;
    let external_organization_count = connected_team_ids.len();
    let connected_organizations = load_connected_organizations(loader, &connected_team_ids)?;
    let member_ids = normalized_member_ids(response.members)?;
    let users = load_slack_member_users_with_cache(
        &loader.api,
        member_ids.clone(),
        &loader.user_cache,
        &loader.user_fetch_lock,
        || loader.ensure_user_directory_cache(),
    )?;
    let self_user_id = loader.load_self_user_id()?;
    let mut members = decoded_members(member_ids, &users, &self_user_id)?;
    members.sort_by_cached_key(|member| (member.real_name.to_lowercase(), member.user_id.clone()));
    let next_cursor = non_empty(response.response_metadata.next_cursor)
        .map(SlackConversationMembersCursor::parse)
        .transpose()?;
    Ok(SlackConversationMembersSnapshot {
        team_id: loader.team_id.clone(),
        conversation_id: conversation_id.to_string(),
        members,
        external_organization_count,
        connected_organizations,
        next_cursor,
    })
}

fn normalized_member_ids(member_ids: Vec<String>) -> Result<BTreeSet<String>, String> {
    member_ids
        .into_iter()
        .map(|user_id| {
            let user_id = user_id.trim().to_string();
            if user_id.is_empty() {
                Err("Slack conversations.members returned an empty user id".to_string())
            } else {
                Ok(user_id)
            }
        })
        .collect()
}

fn decoded_members(
    member_ids: BTreeSet<String>,
    users: &HashMap<String, Value>,
    self_user_id: &str,
) -> Result<Vec<SlackConversationMember>, String> {
    member_ids
        .into_iter()
        .map(|user_id| {
            let payload = users.get(&user_id).cloned().ok_or_else(|| {
                format!("Slack user cache is missing conversation member {user_id}")
            })?;
            let user = serde_json::from_value::<SlackMemberUser>(payload).map_err(|error| {
                format!("failed to decode Slack user payload for {user_id}: {error}")
            })?;
            (!user.is_hidden_from_members())
                .then(|| user.into_member(user_id, self_user_id))
                .transpose()
        })
        .collect::<Result<Vec<_>, String>>()
        .map(|members| members.into_iter().flatten().collect())
}

#[derive(Debug, Deserialize)]
struct SlackConversationMembersResponse {
    members: Vec<String>,
    #[serde(default)]
    response_metadata: SlackResponseMetadata,
}

#[derive(Debug, Default, Deserialize)]
struct SlackResponseMetadata {
    #[serde(default)]
    next_cursor: String,
}

#[derive(Debug, Deserialize)]
struct SlackConversationTeamContextResponse {
    channel: SlackConversationTeamContext,
}

#[derive(Debug, Deserialize)]
struct SlackConversationTeamContext {
    id: String,
    #[serde(default)]
    connected_team_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct SlackTeamInfoResponse {
    team: SlackTeamInfo,
}

#[derive(Debug, Deserialize)]
struct SlackTeamInfo {
    id: String,
    name: String,
    #[serde(default)]
    icon: SlackTeamIcon,
}

#[derive(Debug, Default, Deserialize)]
struct SlackTeamIcon {
    #[serde(default)]
    image_default: bool,
    #[serde(default)]
    image_34: Option<String>,
    #[serde(default)]
    image_44: Option<String>,
    #[serde(default)]
    image_68: Option<String>,
    #[serde(default)]
    image_88: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SlackMemberUser {
    team_id: String,
    #[serde(default)]
    real_name: Option<String>,
    #[serde(default)]
    presence: Option<String>,
    #[serde(default)]
    deleted: bool,
    #[serde(default)]
    is_bot: bool,
    #[serde(default)]
    is_app_user: bool,
    #[serde(default)]
    profile: SlackMemberProfile,
}

impl SlackMemberUser {
    fn is_hidden_from_members(&self) -> bool {
        self.deleted || self.is_bot || self.is_app_user
    }

    fn into_member(
        self,
        user_id: String,
        self_user_id: &str,
    ) -> Result<SlackConversationMember, String> {
        let team_id = non_empty(self.team_id)
            .ok_or_else(|| format!("Slack conversation member {user_id} omitted the team id"))?;
        let real_name = [
            self.profile.real_name.as_deref(),
            self.real_name.as_deref(),
            self.profile.display_name.as_deref(),
        ]
        .into_iter()
        .flatten()
        .find_map(non_empty_str)
        .unwrap_or_else(|| user_id.clone());
        Ok(SlackConversationMember {
            is_self: user_id == self_user_id,
            user_id,
            team_id,
            real_name,
            display_name: self.profile.display_name.and_then(non_empty),
            title: self.profile.title.and_then(non_empty),
            avatar_image_url: select_slack_avatar_image_url(
                SlackAvatarPurpose::Message,
                SlackAvatarImageUrls {
                    image_24: self.profile.image_24.as_deref(),
                    image_32: self.profile.image_32.as_deref(),
                    image_48: self.profile.image_48.as_deref(),
                    image_72: self.profile.image_72.as_deref(),
                    image_192: self.profile.image_192.as_deref(),
                    image_512: self.profile.image_512.as_deref(),
                },
            ),
            presence: match self.presence.as_deref() {
                Some("active") => Some(SlackUserPresence::Active),
                Some("away") => Some(SlackUserPresence::Away),
                _ => None,
            },
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct SlackMemberProfile {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    real_name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    image_24: Option<String>,
    #[serde(default)]
    image_32: Option<String>,
    #[serde(default)]
    image_48: Option<String>,
    #[serde(default)]
    image_72: Option<String>,
    #[serde(default)]
    image_192: Option<String>,
    #[serde(default)]
    image_512: Option<String>,
}

fn connected_team_ids(
    payload: &serde_json::Value,
    conversation_id: &str,
    local_team_id: &str,
) -> Result<Vec<String>, String> {
    let context = serde_json::from_value::<SlackConversationTeamContextResponse>(payload.clone())
        .map_err(|error| format!("failed to decode Slack conversation team context: {error}"))?
        .channel;
    if context.id != conversation_id {
        return Err(format!(
            "Slack conversations.info returned conversation {} for {conversation_id}",
            context.id
        ));
    }
    let mut seen = HashSet::new();
    context
        .connected_team_ids
        .into_iter()
        .map(|team_id| {
            non_empty(team_id).ok_or_else(|| {
                format!("Slack conversation {conversation_id} returned an empty connected team id")
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|team_ids| {
            team_ids
                .into_iter()
                .filter(|team_id| team_id != local_team_id && seen.insert(team_id.clone()))
                .collect()
        })
}

fn load_connected_organizations(
    loader: &SlackLiveWorkspaceLoader,
    connected_team_ids: &[String],
) -> Result<Vec<SlackConnectedOrganization>, String> {
    thread::scope(|scope| {
        let team_ids = connected_team_ids
            .iter()
            .take(SLACK_CONNECTED_ORGANIZATION_BADGE_LIMIT)
            .cloned()
            .collect::<Vec<_>>();
        let requests = team_ids
            .iter()
            .map(|team_id| {
                let team_id = team_id.clone();
                scope.spawn(move || load_connected_organization(loader, &team_id))
            })
            .collect::<Vec<_>>();
        requests
            .into_iter()
            .zip(team_ids)
            .map(|(request, team_id)| {
                request
                    .join()
                    .map_err(|_| format!("Slack team.info request thread panicked for {team_id}"))?
            })
            .collect()
    })
}

fn load_connected_organization(
    loader: &SlackLiveWorkspaceLoader,
    team_id: &str,
) -> Result<SlackConnectedOrganization, String> {
    if let Some(organization) = loader
        .connected_organization_cache
        .lock()
        .map_err(|_| "Slack connected organization cache mutex poisoned".to_string())?
        .get(team_id)
        .cloned()
    {
        return Ok(organization);
    }
    let response = serde_json::from_value::<SlackTeamInfoResponse>(
        loader
            .api
            .post("team.info", &[("team", team_id.to_string())])?,
    )
    .map_err(|error| format!("failed to decode Slack team.info response for {team_id}: {error}"))?;
    let team = response.team;
    if team.id != team_id {
        return Err(format!(
            "Slack team.info returned team {} for {team_id}",
            team.id
        ));
    }
    let name = non_empty(team.name)
        .ok_or_else(|| format!("Slack team.info omitted the name for {team_id}"))?;
    let icon_image_url = (!team.icon.image_default)
        .then(|| {
            [
                team.icon.image_34,
                team.icon.image_44,
                team.icon.image_68,
                team.icon.image_88,
            ]
            .into_iter()
            .flatten()
            .find_map(non_empty)
        })
        .flatten();
    let organization = SlackConnectedOrganization {
        team_id: team_id.to_string(),
        name,
        icon_image_url,
    };
    loader
        .connected_organization_cache
        .lock()
        .map_err(|_| "Slack connected organization cache mutex poisoned".to_string())?
        .insert(team_id.to_string(), organization.clone());
    Ok(organization)
}

fn non_empty(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn non_empty_str(value: &str) -> Option<String> {
    non_empty(value.to_string())
}
