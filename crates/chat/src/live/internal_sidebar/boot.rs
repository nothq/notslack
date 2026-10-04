mod counts;
mod decode;
mod groups;
mod prefs;
mod snapshot;
#[cfg(test)]
mod test_support;
mod types;

use std::collections::{BTreeSet, HashMap};
use std::thread;

use crate::live::cache::SlackUserPayloadCache;

pub(super) use self::decode::{decode_client_counts, decode_team_info, decode_user_info};
use self::decode::{decode_user_boot, direct_message_users_to_load};
use self::groups::{
    external_team_ids_by_conversation, group_message_labels, group_message_member_ids,
};
pub(super) use self::snapshot::snapshot_from_boot_state;
use self::types::SlackDirectMessageUser;
pub(super) use self::types::{
    SlackBootState, SlackClientBootResponse, SlackClientCountsResponse, SlackDirectMessagePresence,
};
use super::dms::SlackSidebarDirectoryUser;
use super::sections::SlackChannelSectionsResponse;
use super::{SlackFallbackUser, SlackInternalSidebarClient};

const CLIENT_USER_BOOT: &str = "client.userBoot";
const CLIENT_COUNTS: &str = "client.counts";
const SIDEBAR_TARGETED_USER_INFO_LIMIT: usize = 64;

struct SlackSidebarUsers {
    direct_messages: HashMap<String, SlackDirectMessageUser>,
    group_message_labels: HashMap<String, String>,
}

struct SlackDirectMessageUserLoad<'a> {
    boot: &'a SlackClientBootResponse,
    counts: &'a SlackClientCountsResponse,
    sections: &'a SlackChannelSectionsResponse,
    self_user_id: &'a str,
    user_cache: &'a SlackUserPayloadCache,
}

struct SlackResolvedSidebarUsers {
    directory_users: HashMap<String, SlackSidebarDirectoryUser>,
    fallback_users: HashMap<String, SlackFallbackUser>,
}

struct SlackDirectMessageResolution<'a> {
    directory_users: &'a HashMap<String, SlackSidebarDirectoryUser>,
    fallback_users: &'a HashMap<String, SlackFallbackUser>,
    external_team_ids: &'a HashMap<String, String>,
    external_team_labels: &'a HashMap<String, String>,
}

pub(super) struct SlackBootClient<'a> {
    inner: &'a SlackInternalSidebarClient,
}

impl<'a> SlackBootClient<'a> {
    pub(super) fn new(inner: &'a SlackInternalSidebarClient) -> Self {
        Self { inner }
    }

    pub(super) fn load(
        &self,
        sections: &SlackChannelSectionsResponse,
        user_cache: &SlackUserPayloadCache,
    ) -> Result<SlackBootState, String> {
        let (boot, counts) = self.load_boot_and_counts()?;
        let self_user = boot
            .self_user
            .as_ref()
            .ok_or_else(|| "Slack client.userBoot response missing self".to_string())?;
        let self_user_id = self_user.id.trim();
        if self_user_id.is_empty() {
            return Err("Slack client.userBoot response has empty self id".to_string());
        }
        let self_user_id = self_user_id.to_string();
        let dnd = boot
            .dnd
            .as_ref()
            .ok_or_else(|| "Slack client.userBoot response missing dnd".to_string())?;
        let self_notifications_paused = dnd.dnd_enabled || dnd.snooze_enabled;
        let sidebar_users = self.load_direct_message_users(SlackDirectMessageUserLoad {
            boot: &boot,
            counts: &counts,
            sections,
            self_user_id: &self_user_id,
            user_cache,
        })?;
        Ok(SlackBootState {
            boot,
            counts,
            dm_users: sidebar_users.direct_messages,
            group_message_labels: sidebar_users.group_message_labels,
            self_presence: None,
            self_notifications_paused,
        })
    }

    fn load_boot_and_counts(
        &self,
    ) -> Result<(SlackClientBootResponse, SlackClientCountsResponse), String> {
        thread::scope(|scope| {
            let boot = scope.spawn(|| self.load_user_boot());
            let counts = scope.spawn(|| self.load_counts());
            let boot = boot
                .join()
                .map_err(|_| "Slack client.userBoot request thread panicked".to_string())??;
            let counts = counts
                .join()
                .map_err(|_| "Slack client.counts request thread panicked".to_string())??;
            Ok((boot, counts))
        })
    }

    fn load_user_boot(&self) -> Result<SlackClientBootResponse, String> {
        let body = self.inner.post_internal_method(
            CLIENT_USER_BOOT,
            vec![
                ("_x_reason", "deferred-data".to_string()),
                ("version_all_channels", "false".to_string()),
                ("return_all_relevant_mpdms", "true".to_string()),
                (
                    "omit_extras",
                    "feature_usage_data,plan_info,salesforce_features".to_string(),
                ),
                ("_x_sonic", "true".to_string()),
                ("_x_app_name", "client".to_string()),
            ],
        )?;
        decode_user_boot(&body)
    }

    fn load_counts(&self) -> Result<SlackClientCountsResponse, String> {
        let body = self.inner.post_internal_method(
            CLIENT_COUNTS,
            vec![
                ("_x_app_name", "client".to_string()),
                ("_x_sonic", "true".to_string()),
            ],
        )?;
        decode_client_counts(&body)
    }

    fn load_direct_message_users(
        &self,
        input: SlackDirectMessageUserLoad<'_>,
    ) -> Result<SlackSidebarUsers, String> {
        let SlackDirectMessageUserLoad {
            boot,
            counts,
            sections,
            self_user_id,
            user_cache,
        } = input;
        let section_conversation_ids = sections
            .channel_sections
            .iter()
            .flat_map(|section| section.channel_ids_page.channel_ids.iter())
            .chain(boot.starred.iter())
            .map(String::as_str)
            .collect();
        let dm_user_ids = direct_message_users_to_load(boot, counts, &section_conversation_ids)?;
        let external_team_ids = external_team_ids_by_conversation(boot, &dm_user_ids)?;
        let group_message_member_ids = group_message_member_ids(boot, counts, self_user_id)?;
        let required_user_ids = dm_user_ids
            .iter()
            .map(|(_, user_id)| user_id.clone())
            .chain(group_message_member_ids.iter().cloned())
            .collect::<BTreeSet<_>>();
        let resolved_users =
            self.resolve_sidebar_users(required_user_ids, group_message_member_ids, user_cache)?;
        let group_message_labels = group_message_labels(
            boot,
            counts,
            self_user_id,
            &resolved_users.directory_users,
            &resolved_users.fallback_users,
        )?;
        let external_team_labels = self.load_external_team_labels(&external_team_ids)?;
        let direct_messages = resolved_direct_message_users(
            dm_user_ids,
            SlackDirectMessageResolution {
                directory_users: &resolved_users.directory_users,
                fallback_users: &resolved_users.fallback_users,
                external_team_ids: &external_team_ids,
                external_team_labels: &external_team_labels,
            },
        )?;
        Ok(SlackSidebarUsers {
            direct_messages,
            group_message_labels,
        })
    }

    fn resolve_sidebar_users(
        &self,
        required_user_ids: BTreeSet<String>,
        group_message_member_ids: BTreeSet<String>,
        user_cache: &SlackUserPayloadCache,
    ) -> Result<SlackResolvedSidebarUsers, String> {
        let directory_users = self
            .inner
            .load_sidebar_users_from_cache(&required_user_ids, user_cache)?;
        let missing_user_ids = group_message_member_ids
            .into_iter()
            .chain(required_user_ids)
            .filter(|user_id| !directory_users.contains_key(user_id.as_str()))
            .collect::<BTreeSet<_>>();
        if missing_user_ids.len() > SIDEBAR_TARGETED_USER_INFO_LIMIT {
            return Err(format!(
                "Slack sidebar required {} uncached users; targeted hydration limit is {SIDEBAR_TARGETED_USER_INFO_LIMIT}",
                missing_user_ids.len()
            ));
        }
        let fallback_users = self.inner.load_fallback_users(missing_user_ids)?;
        user_cache.extend(fallback_users.iter().filter_map(|(user_id, user)| {
            user.raw_payload
                .clone()
                .map(|payload| (user_id.clone(), payload))
        }))?;
        Ok(SlackResolvedSidebarUsers {
            directory_users,
            fallback_users,
        })
    }

    fn load_external_team_labels(
        &self,
        external_team_ids: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, String> {
        external_team_ids
            .values()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|team_id| {
                self.inner
                    .load_external_team_label(team_id)
                    .map(|label| (team_id.clone(), label))
            })
            .collect()
    }
}

fn resolved_direct_message_users(
    dm_user_ids: Vec<(String, String)>,
    resolution: SlackDirectMessageResolution<'_>,
) -> Result<HashMap<String, SlackDirectMessageUser>, String> {
    let SlackDirectMessageResolution {
        directory_users,
        fallback_users,
        external_team_ids,
        external_team_labels,
    } = resolution;
    dm_user_ids
        .into_iter()
        .map(|(conversation_id, user_id)| {
            let directory_user = directory_users.get(user_id.as_str());
            let user = directory_user
                .map(|user| &user.user)
                .or_else(|| fallback_users.get(user_id.as_str()))
                .ok_or_else(|| format!("Slack sidebar missing resolved user {user_id}"))?;
            let presence = directory_user.and_then(|user| user.presence);
            let secondary_context = external_team_ids
                .get(conversation_id.as_str())
                .and_then(|team_id| external_team_labels.get(team_id.as_str()))
                .cloned();
            let user = SlackDirectMessageUser {
                user_id,
                label: user.label.clone(),
                avatar_image_url: user.compact_avatar_image_url.clone(),
                secondary_context,
                presence,
            };
            Ok((conversation_id, user))
        })
        .collect()
}
