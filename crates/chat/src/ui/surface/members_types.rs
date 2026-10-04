use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Instant,
};

use gpui::SharedString;

use crate::ui::{
    initials, slack_avatar_fill, SlackConversationMember, SlackConversationMembersCursor,
    SlackConversationMembersSnapshot, SlackUserPresence,
};

use super::normalize_slack_dm_finder_text;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackMemberRow {
    pub(crate) user_id: SharedString,
    pub(crate) element_id: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) real_name: SharedString,
    pub(crate) primary_label: SharedString,
    pub(crate) display_name: Option<SharedString>,
    pub(crate) title: Option<SharedString>,
    pub(crate) search_key: SharedString,
    pub(crate) avatar_initials: SharedString,
    pub(crate) avatar_fill: u32,
    pub(crate) avatar_image_url: Option<SharedString>,
    pub(crate) presence: Option<SlackUserPresence>,
    pub(crate) is_self: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackExternalOrganizationBadge {
    pub(crate) team_id: SharedString,
    pub(crate) name: SharedString,
    pub(crate) image_url: Option<SharedString>,
    pub(crate) initials: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackExternalMembersSummary {
    pub(crate) team_id: SharedString,
    pub(crate) conversation_id: SharedString,
    pub(crate) external_member_count: usize,
    pub(crate) external_organization_count: usize,
    pub(crate) people_label: SharedString,
    pub(crate) organizations_label: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) organizations: Arc<[SlackExternalOrganizationBadge]>,
}

#[derive(Clone)]
pub(crate) struct SlackMembersLoad {
    pub(crate) generation: u64,
    pub(crate) conversation_id: String,
    pub(crate) cursor: Option<SlackConversationMembersCursor>,
    pub(crate) started_at: Instant,
    pub(crate) profile_enabled: bool,
}

pub(crate) struct PreparedSlackConversationMembers {
    pub(crate) snapshot: SlackConversationMembersSnapshot,
    pub(crate) rows: Arc<[SlackMemberRow]>,
    pub(crate) external_summary: Option<SlackExternalMembersSummary>,
    pub(crate) external_organization_badges_by_user_id:
        HashMap<String, SlackExternalOrganizationBadge>,
}

pub(crate) fn prepare_slack_conversation_members(
    snapshot: SlackConversationMembersSnapshot,
) -> Result<PreparedSlackConversationMembers, String> {
    if snapshot.team_id.trim().is_empty() {
        return Err("Slack conversation members omitted the team id".to_string());
    }
    if snapshot.conversation_id.trim().is_empty() {
        return Err("Slack conversation members omitted the conversation id".to_string());
    }
    let rows = snapshot
        .members
        .iter()
        .map(prepare_slack_member_row)
        .collect::<Result<Vec<_>, _>>()?
        .into();
    let external_summary = prepare_slack_external_members_summary(&snapshot)?;
    let external_organization_badges_by_user_id =
        prepare_slack_external_organization_badges_by_user_id(&snapshot, external_summary.as_ref());
    Ok(PreparedSlackConversationMembers {
        snapshot,
        rows,
        external_summary,
        external_organization_badges_by_user_id,
    })
}

fn prepare_slack_member_row(member: &SlackConversationMember) -> Result<SlackMemberRow, String> {
    if member.user_id.trim().is_empty() {
        return Err("Slack conversation member omitted the user id".to_string());
    }
    if member.real_name.trim().is_empty() {
        return Err(format!(
            "Slack conversation member {} omitted the real name",
            member.user_id
        ));
    }
    if member.team_id.trim().is_empty() {
        return Err(format!(
            "Slack conversation member {} omitted the team id",
            member.user_id
        ));
    }
    let display_name = member
        .display_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let title = member
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let self_label = if member.is_self { " (you)" } else { "" };
    let accessibility_label = [
        format!("{}{}", member.real_name, self_label),
        display_name.unwrap_or_default().to_string(),
        title.unwrap_or_default().to_string(),
    ]
    .into_iter()
    .filter(|value| !value.is_empty())
    .collect::<Vec<_>>()
    .join(", ");
    let search_key = normalize_slack_dm_finder_text(
        &[
            member.real_name.as_str(),
            display_name.unwrap_or_default(),
            title.unwrap_or_default(),
        ]
        .join(" "),
    );
    Ok(SlackMemberRow {
        user_id: member.user_id.clone().into(),
        element_id: format!("slack-channel-member-{}", member.user_id).into(),
        accessibility_label: accessibility_label.into(),
        real_name: member.real_name.clone().into(),
        primary_label: format!("{}{}", member.real_name, self_label).into(),
        display_name: display_name.map(Into::into),
        title: title.map(Into::into),
        search_key: search_key.into(),
        avatar_initials: initials(&member.real_name).into(),
        avatar_fill: slack_avatar_fill(&member.real_name),
        avatar_image_url: member.avatar_image_url.clone().map(Into::into),
        presence: member.presence,
        is_self: member.is_self,
    })
}

fn prepare_slack_external_members_summary(
    snapshot: &SlackConversationMembersSnapshot,
) -> Result<Option<SlackExternalMembersSummary>, String> {
    if snapshot.next_cursor.is_some() {
        return Ok(None);
    }
    let external_member_count = snapshot
        .members
        .iter()
        .filter(|member| member.team_id != snapshot.team_id)
        .count();
    let external_organization_count = snapshot.external_organization_count;
    if external_member_count == 0 || external_organization_count == 0 {
        return Ok(None);
    }
    let organizations =
        prepare_external_organization_badges(snapshot, external_organization_count)?;
    let (people_label, organizations_label) =
        external_members_labels(external_member_count, external_organization_count);
    Ok(Some(SlackExternalMembersSummary {
        team_id: snapshot.team_id.clone().into(),
        conversation_id: snapshot.conversation_id.clone().into(),
        external_member_count,
        external_organization_count,
        accessibility_label: format!("{people_label} {organizations_label}").into(),
        people_label: people_label.into(),
        organizations_label: organizations_label.into(),
        organizations: organizations.into(),
    }))
}

fn prepare_external_organization_badges(
    snapshot: &SlackConversationMembersSnapshot,
    external_organization_count: usize,
) -> Result<Vec<SlackExternalOrganizationBadge>, String> {
    if snapshot.connected_organizations.len() > external_organization_count {
        return Err(format!(
            "Slack conversation {} returned {} organization badges for {} connected organizations",
            snapshot.conversation_id,
            snapshot.connected_organizations.len(),
            external_organization_count
        ));
    }
    let mut connected_team_ids = HashSet::new();
    snapshot
        .connected_organizations
        .iter()
        .map(|organization| {
            if organization.team_id == snapshot.team_id {
                return Err(format!(
                    "Slack conversation {} returned its own team as a connected organization",
                    snapshot.conversation_id
                ));
            }
            if !connected_team_ids.insert(organization.team_id.as_str()) {
                return Err(format!(
                    "Slack conversation {} returned duplicate connected organization {}",
                    snapshot.conversation_id, organization.team_id
                ));
            }
            Ok(SlackExternalOrganizationBadge {
                team_id: organization.team_id.clone().into(),
                name: organization.name.clone().into(),
                image_url: organization.icon_image_url.clone().map(Into::into),
                initials: slack_organization_initials(&organization.name).into(),
            })
        })
        .collect()
}

fn external_members_labels(
    external_member_count: usize,
    external_organization_count: usize,
) -> (String, String) {
    let count_label = if external_member_count >= 50 {
        "50+".to_string()
    } else {
        external_member_count.to_string()
    };
    let people_label = format!(
        "{count_label} external {}",
        if external_member_count == 1 {
            "person"
        } else {
            "people"
        }
    );
    let organizations_label = format!(
        "{} from {external_organization_count} {}",
        if external_member_count == 1 {
            "is"
        } else {
            "are"
        },
        if external_organization_count == 1 {
            "organization"
        } else {
            "organizations"
        }
    );
    (people_label, organizations_label)
}

fn prepare_slack_external_organization_badges_by_user_id(
    snapshot: &SlackConversationMembersSnapshot,
    summary: Option<&SlackExternalMembersSummary>,
) -> HashMap<String, SlackExternalOrganizationBadge> {
    let Some(summary) = summary else {
        return HashMap::new();
    };
    let badges_by_team_id = summary
        .organizations
        .iter()
        .map(|organization| (organization.team_id.as_ref(), organization))
        .collect::<HashMap<_, _>>();
    snapshot
        .members
        .iter()
        .filter(|member| member.team_id != snapshot.team_id)
        .filter_map(|member| {
            badges_by_team_id
                .get(member.team_id.as_str())
                .map(|badge| (member.user_id.clone(), (*badge).clone()))
        })
        .collect()
}

fn slack_organization_initials(name: &str) -> String {
    let words = name
        .split(|character: char| !character.is_alphanumeric())
        .filter_map(|word| {
            word.chars()
                .find(|character| character.is_alphabetic())
                .map(|initial| (initial, word))
        })
        .collect::<Vec<_>>();
    let initials: String = if words.len() >= 2 {
        words.iter().take(2).map(|(initial, _)| *initial).collect()
    } else {
        words
            .first()
            .map(|(_, word)| {
                word.chars()
                    .filter(|character| character.is_alphabetic())
                    .take(2)
                    .collect()
            })
            .unwrap_or_default()
    };
    initials.to_uppercase()
}
