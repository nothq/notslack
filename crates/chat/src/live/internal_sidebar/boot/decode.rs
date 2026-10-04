use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::Value;

use crate::live::payload::SlackAvatarPurpose;

use super::super::SlackFallbackUser;
use super::types::{
    SlackClientBootResponse, SlackClientCountsResponse, SlackTeamInfoResponse,
    SlackUserInfoResponse,
};

type SlackDirectMessageUserLoad = (String, String);
type SlackDirectMessageUsersToLoad = Vec<SlackDirectMessageUserLoad>;

pub(super) fn decode_user_boot(body: &str) -> Result<SlackClientBootResponse, String> {
    let response = serde_json::from_str::<SlackClientBootResponse>(body).map_err(|error| {
        format!("failed to decode Slack internal client.userBoot response: {error}")
    })?;
    if !response.ok {
        return Err(format!(
            "Slack internal client.userBoot API failed: {}",
            response
                .error
                .unwrap_or_else(|| "unknown_error".to_string())
        ));
    }
    if response.channels.is_empty() {
        return Err("Slack internal client.userBoot API returned no channels".to_string());
    }
    Ok(response)
}

pub(in crate::live::internal_sidebar) fn decode_client_counts(
    body: &str,
) -> Result<SlackClientCountsResponse, String> {
    let response = serde_json::from_str::<SlackClientCountsResponse>(body).map_err(|error| {
        format!("failed to decode Slack internal client.counts response: {error}")
    })?;
    if !response.ok {
        return Err(format!(
            "Slack internal client.counts API failed: {}",
            response
                .error
                .unwrap_or_else(|| "unknown_error".to_string())
        ));
    }
    if response.conversations().next().is_some() && !response.has_unread_state_fields() {
        return Err(
            "Slack internal client.counts response did not include unread state fields".to_string(),
        );
    }
    if response.conversations().next().is_some() && response.activity_count().is_none() {
        return Err(
            "Slack internal client.counts response did not include activity rail count fields"
                .to_string(),
        );
    }
    Ok(response)
}

pub(super) fn direct_message_users_to_load(
    boot: &SlackClientBootResponse,
    counts: &SlackClientCountsResponse,
    section_conversation_ids: &HashSet<&str>,
) -> Result<SlackDirectMessageUsersToLoad, String> {
    let visible_ids = counts
        .ims
        .iter()
        .map(|conversation| conversation.id.as_str())
        .collect::<HashSet<_>>();
    let ims = boot
        .ims
        .iter()
        .map(|conversation| (conversation.id.as_str(), conversation))
        .collect::<HashMap<_, _>>();
    let mut conversation_ids = boot
        .channels_priority
        .keys()
        .filter(|id| visible_ids.contains(id.as_str()))
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    conversation_ids.extend(
        section_conversation_ids
            .iter()
            .copied()
            .filter(|id| ims.contains_key(id)),
    );
    conversation_ids.extend(
        counts
            .ims
            .iter()
            .filter(|conversation| conversation.mention_display_count().is_some())
            .map(|conversation| conversation.id.as_str()),
    );
    conversation_ids.extend(
        boot.ims
            .iter()
            .filter(|conversation| {
                visible_ids.contains(conversation.id.as_str())
                    && (conversation.is_shared
                        || conversation.is_ext_shared
                        || conversation.is_org_shared)
            })
            .map(|conversation| conversation.id.as_str()),
    );
    conversation_ids
        .into_iter()
        .map(|id| {
            let conversation = ims.get(id).ok_or_else(|| {
                format!("Slack client.userBoot missing IM metadata for visible sidebar DM {id}")
            })?;
            let user_id = conversation.user.as_deref().ok_or_else(|| {
                format!(
                    "Slack client.userBoot IM metadata missing user for visible sidebar DM {id}"
                )
            })?;
            Ok((id.to_string(), user_id.to_string()))
        })
        .collect()
}

pub(in crate::live::internal_sidebar) fn decode_user_info(
    user_id: &str,
    body: &str,
) -> Result<SlackFallbackUser, String> {
    let raw_response = serde_json::from_str::<Value>(body).map_err(|error| {
        format!("failed to decode Slack internal users.info response for {user_id}: {error}")
    })?;
    let response =
        serde_json::from_value::<SlackUserInfoResponse>(raw_response.clone()).map_err(|error| {
            format!("failed to decode Slack internal users.info response for {user_id}: {error}")
        })?;
    if !response.ok {
        return Err(format!(
            "Slack internal users.info API failed for {user_id}: {}",
            response
                .error
                .unwrap_or_else(|| "unknown_error".to_string())
        ));
    }
    let user = response
        .user
        .ok_or_else(|| format!("Slack internal users.info response missing user for {user_id}"))?;
    let label = user.display_name().ok_or_else(|| {
        format!("Slack internal users.info response missing display name for {user_id}")
    })?;
    let raw_payload = raw_response
        .get("user")
        .cloned()
        .ok_or_else(|| format!("Slack internal users.info response missing user for {user_id}"))?;
    let payload_user_id = raw_payload
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Slack internal users.info user missing id for {user_id}"))?;
    if payload_user_id != user_id {
        return Err(format!(
            "Slack internal users.info returned user {payload_user_id} for {user_id}"
        ));
    }
    Ok(SlackFallbackUser {
        label,
        compact_avatar_image_url: user.avatar_image_url(SlackAvatarPurpose::Sidebar),
        message_avatar_image_url: user.avatar_image_url(SlackAvatarPurpose::Message),
        raw_payload: Some(raw_payload),
    })
}

pub(in crate::live::internal_sidebar) fn decode_team_info(
    team_id: &str,
    body: &str,
) -> Result<String, String> {
    let response = serde_json::from_str::<SlackTeamInfoResponse>(body).map_err(|error| {
        format!("failed to decode Slack internal team.info response for {team_id}: {error}")
    })?;
    if !response.ok {
        return Err(format!(
            "Slack internal team.info API failed for {team_id}: {}",
            response
                .error
                .unwrap_or_else(|| "unknown_error".to_string())
        ));
    }
    let team = response
        .team
        .ok_or_else(|| format!("Slack internal team.info response missing team for {team_id}"))?;
    if team.id != team_id {
        return Err(format!(
            "Slack internal team.info response returned team {} for {team_id}",
            team.id
        ));
    }
    let label = team.name.trim();
    if label.is_empty() {
        return Err(format!(
            "Slack internal team.info response missing team name for {team_id}"
        ));
    }
    Ok(label.to_string())
}
