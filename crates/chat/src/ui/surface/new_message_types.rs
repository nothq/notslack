use std::sync::Arc;

use gpui::SharedString;

use crate::ui::{
    initials, slack_avatar_fill, SlackConversationKind, SlackConversationOpenRequest,
    SlackDestinationCandidate, SlackDestinationDirectorySnapshot, SlackDestinationTarget,
    SlackUserPresence,
};

use super::normalize_slack_dm_finder_text;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackNewMessageCandidateKind {
    Channel,
    PrivateChannel,
    DirectMessage,
    GroupMessage,
    Person,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackNewMessageCandidateRow {
    pub(crate) element_id: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) target: SlackDestinationTarget,
    pub(crate) presence_user_id: Option<SharedString>,
    pub(crate) kind: SlackNewMessageCandidateKind,
    pub(crate) label: SharedString,
    pub(crate) secondary_label: Option<SharedString>,
    pub(crate) search_key: SharedString,
    pub(crate) avatar_initials: SharedString,
    pub(crate) avatar_fill: u32,
    pub(crate) avatar_image_url: Option<SharedString>,
    pub(crate) presence: Option<SlackUserPresence>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackNewMessagePerson {
    pub(crate) user_id: SharedString,
    pub(crate) label: SharedString,
    pub(crate) avatar_image_url: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackNewMessageDestination {
    pub(crate) conversation_id: SharedString,
    pub(crate) label: SharedString,
    pub(crate) kind: SlackConversationKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackNewMessageOpenLoad {
    pub(crate) generation: u64,
    pub(crate) request: SlackConversationOpenRequest,
}

pub(crate) struct PreparedSlackDestinationDirectory {
    pub(crate) snapshot: SlackDestinationDirectorySnapshot,
    pub(crate) rows: Arc<[SlackNewMessageCandidateRow]>,
}

pub(crate) fn prepare_slack_destination_directory(
    snapshot: SlackDestinationDirectorySnapshot,
) -> Result<PreparedSlackDestinationDirectory, String> {
    if snapshot.team_id.trim().is_empty() {
        return Err("Slack destination directory omitted its team id".to_string());
    }
    if snapshot.self_user_id.trim().is_empty() {
        return Err("Slack destination directory omitted the signed-in user id".to_string());
    }
    let rows = snapshot
        .candidates
        .iter()
        .map(prepare_slack_destination_candidate)
        .collect::<Result<Vec<_>, String>>()?
        .into();
    Ok(PreparedSlackDestinationDirectory { snapshot, rows })
}

fn prepare_slack_destination_candidate(
    candidate: &SlackDestinationCandidate,
) -> Result<SlackNewMessageCandidateRow, String> {
    let stable_id = candidate.target.stable_id();
    if stable_id.trim().is_empty() {
        return Err("Slack destination directory candidate omitted its id".to_string());
    }
    if candidate.label.trim().is_empty() {
        return Err(format!(
            "Slack destination directory candidate {stable_id} omitted its label"
        ));
    }
    let kind = slack_new_message_candidate_kind(candidate, stable_id)?;
    let participant_labels = candidate.participant_labels.join(" ");
    let search_key = normalize_slack_dm_finder_text(
        &[
            candidate.label.as_str(),
            candidate.real_name.as_deref().unwrap_or_default(),
            candidate.email.as_deref().unwrap_or_default(),
            participant_labels.as_str(),
        ]
        .join(" "),
    );
    Ok(SlackNewMessageCandidateRow {
        element_id: format!("slack-new-message-destination-{stable_id}").into(),
        accessibility_label: slack_new_message_candidate_accessibility_label(candidate, kind)
            .into(),
        target: candidate.target.clone(),
        presence_user_id: slack_destination_candidate_presence_user_id(candidate).map(Into::into),
        kind,
        label: candidate.label.clone().into(),
        secondary_label: slack_new_message_candidate_secondary_label(candidate, kind)
            .map(Into::into),
        search_key: search_key.into(),
        avatar_initials: initials(&candidate.label).into(),
        avatar_fill: slack_avatar_fill(&candidate.label),
        avatar_image_url: candidate.avatar_image_url.clone().map(Into::into),
        presence: candidate.presence,
    })
}

fn slack_destination_candidate_presence_user_id(
    candidate: &SlackDestinationCandidate,
) -> Option<String> {
    match &candidate.target {
        SlackDestinationTarget::Person { user_id } => Some(user_id.clone()),
        SlackDestinationTarget::Conversation { kind, .. }
            if *kind == SlackConversationKind::DirectMessage =>
        {
            candidate.participant_user_ids.first().cloned()
        }
        _ => None,
    }
}

fn slack_new_message_candidate_kind(
    candidate: &SlackDestinationCandidate,
    stable_id: &str,
) -> Result<SlackNewMessageCandidateKind, String> {
    match &candidate.target {
        SlackDestinationTarget::Conversation { kind, .. } => match kind {
            SlackConversationKind::Channel => Ok(SlackNewMessageCandidateKind::Channel),
            SlackConversationKind::PrivateChannel => {
                Ok(SlackNewMessageCandidateKind::PrivateChannel)
            }
            SlackConversationKind::DirectMessage => Ok(SlackNewMessageCandidateKind::DirectMessage),
            SlackConversationKind::GroupMessage => Ok(SlackNewMessageCandidateKind::GroupMessage),
            SlackConversationKind::Unknown => Err(format!(
                "Slack destination directory candidate {stable_id} has unknown conversation kind"
            )),
        },
        SlackDestinationTarget::Person { .. } => Ok(SlackNewMessageCandidateKind::Person),
    }
}

fn slack_new_message_candidate_secondary_label(
    candidate: &SlackDestinationCandidate,
    kind: SlackNewMessageCandidateKind,
) -> Option<String> {
    candidate
        .real_name
        .as_deref()
        .filter(|real_name| *real_name != candidate.label)
        .or(candidate.email.as_deref())
        .map(str::to_string)
        .or_else(|| {
            (kind == SlackNewMessageCandidateKind::GroupMessage)
                .then(|| format!("{} people", candidate.participant_user_ids.len().max(2)))
        })
}

fn slack_new_message_candidate_accessibility_label(
    candidate: &SlackDestinationCandidate,
    kind: SlackNewMessageCandidateKind,
) -> String {
    let prefix = match kind {
        SlackNewMessageCandidateKind::Channel => "Channel",
        SlackNewMessageCandidateKind::PrivateChannel => "Private channel",
        SlackNewMessageCandidateKind::DirectMessage => "Direct message with",
        SlackNewMessageCandidateKind::GroupMessage => "Group direct message with",
        SlackNewMessageCandidateKind::Person => "Person",
    };
    format!("{prefix} {}", candidate.label)
}

pub(crate) fn slack_new_message_draft_key_for_conversation(conversation_id: &str) -> String {
    format!("conversation:{conversation_id}")
}

pub(crate) fn slack_new_message_draft_key_for_people(people: &[SlackNewMessagePerson]) -> String {
    let mut user_ids = people
        .iter()
        .map(|person| person.user_id.as_ref())
        .collect::<Vec<_>>();
    user_ids.sort_unstable();
    format!("people:{}", user_ids.join(","))
}
