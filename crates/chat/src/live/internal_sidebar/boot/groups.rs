use std::collections::{BTreeSet, HashMap};

use super::types::{SlackBootConversation, SlackClientBootResponse, SlackClientCountsResponse};
use crate::live::internal_sidebar::dms::SlackSidebarDirectoryUser;
use crate::live::internal_sidebar::SlackFallbackUser;

pub(super) fn group_message_member_ids(
    boot: &SlackClientBootResponse,
    counts: &SlackClientCountsResponse,
    self_user_id: &str,
) -> Result<BTreeSet<String>, String> {
    Ok(visible_group_messages(boot, counts)?
        .into_iter()
        .flat_map(|conversation| conversation.members.iter())
        .filter(|user_id| user_id.as_str() != self_user_id)
        .cloned()
        .collect())
}

pub(super) fn group_message_labels(
    boot: &SlackClientBootResponse,
    counts: &SlackClientCountsResponse,
    self_user_id: &str,
    directory_users: &HashMap<String, SlackSidebarDirectoryUser>,
    fallback_users: &HashMap<String, SlackFallbackUser>,
) -> Result<HashMap<String, String>, String> {
    visible_group_messages(boot, counts)?
        .into_iter()
        .map(|conversation| {
            let mut participants = conversation
                .members
                .iter()
                .filter(|user_id| user_id.as_str() != self_user_id)
                .map(|user_id| {
                    let label = directory_users
                        .get(user_id.as_str())
                        .map(|user| user.user.label.as_str())
                        .or_else(|| {
                            fallback_users
                                .get(user_id.as_str())
                                .map(|user| user.label.as_str())
                        })
                        .ok_or_else(|| {
                            format!(
                                "Slack sidebar group message {} missing resolved member {user_id}",
                                conversation.id
                            )
                        })?;
                    Ok((user_id.as_str(), label))
                })
                .collect::<Result<Vec<_>, String>>()?;
            participants.sort_by(|left, right| {
                left.1
                    .to_ascii_lowercase()
                    .cmp(&right.1.to_ascii_lowercase())
                    .then_with(|| left.0.cmp(right.0))
            });
            let label = participants
                .into_iter()
                .map(|(_, label)| label)
                .collect::<Vec<_>>()
                .join(", ");
            if label.is_empty() {
                return Err(format!(
                    "Slack sidebar group message {} has no members other than self",
                    conversation.id
                ));
            }
            Ok((conversation.id.clone(), label))
        })
        .collect()
}

fn visible_group_messages<'a>(
    boot: &'a SlackClientBootResponse,
    counts: &SlackClientCountsResponse,
) -> Result<Vec<&'a SlackBootConversation>, String> {
    let group_messages = boot
        .channels
        .iter()
        .chain(boot.mpims.iter())
        .filter(|conversation| conversation.is_mpim)
        .map(|conversation| (conversation.id.as_str(), conversation))
        .collect::<HashMap<_, _>>();
    counts
        .mpims
        .iter()
        .map(|count| {
            group_messages
                .get(count.id.as_str())
                .copied()
                .ok_or_else(|| {
                    format!(
                        "Slack client.userBoot missing group-message metadata for visible MPIM {}",
                        count.id
                    )
                })
        })
        .collect()
}

pub(super) fn external_team_ids_by_conversation(
    boot: &SlackClientBootResponse,
    dm_user_ids: &[(String, String)],
) -> Result<HashMap<String, String>, String> {
    let ims = boot
        .ims
        .iter()
        .map(|conversation| (conversation.id.as_str(), conversation))
        .collect::<HashMap<_, _>>();
    dm_user_ids
        .iter()
        .filter_map(|(conversation_id, _)| {
            let conversation = ims.get(conversation_id.as_str())?;
            (conversation.is_shared || conversation.is_ext_shared || conversation.is_org_shared)
                .then_some((conversation_id, *conversation))
        })
        .map(|(conversation_id, conversation)| {
            let context_team_id = conversation.context_team_id.as_deref().ok_or_else(|| {
                format!(
                    "Slack client.userBoot shared IM {conversation_id} missing context_team_id"
                )
            })?;
            let external_team_ids = conversation
                .connected_team_ids
                .iter()
                .filter(|team_id| team_id.as_str() != context_team_id)
                .collect::<BTreeSet<_>>();
            if external_team_ids.len() != 1 {
                return Err(format!(
                    "Slack client.userBoot shared IM {conversation_id} has {} external connected teams",
                    external_team_ids.len()
                ));
            }
            Ok((
                conversation_id.clone(),
                (*external_team_ids
                    .first()
                    .expect("exactly one external connected team"))
                .clone(),
            ))
        })
        .collect()
}
