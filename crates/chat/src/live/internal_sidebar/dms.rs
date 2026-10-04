mod items;
mod types;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Mutex;
use std::thread;

use crate::model::SlackDmInboxSnapshot;
use serde_json::Value;

use crate::live::cache::SlackUserPayloadCache;
use crate::live::payload::SlackAvatarPurpose;

use self::items::{cached_dm_users, dm_inbox_items, dm_inbox_user_ids, SlackDmInboxItemsInput};
use self::types::{
    decode_response, non_empty, SlackClientCountsResponse, SlackClientDmsResponse, SlackDmUser,
    SlackUsersListResponse,
};
use super::{boot::SlackDirectMessagePresence, SlackFallbackUser, SlackInternalSidebarClient};

const CLIENT_DMS: &str = "client.dms";
const CLIENT_COUNTS: &str = "client.counts";
const USERS_LIST: &str = "users.list";
const SLACKBOT_USER_ID: &str = "USLACK";
const DMS_PAGE_SIZE: usize = 24;
const USERS_PAGE_SIZE: usize = 200;
const FALLBACK_USER_WORKER_LIMIT: usize = 4;
const DM_INBOX_TARGETED_USER_INFO_LIMIT: usize = 64;

pub(crate) struct SlackDmInboxLoad {
    pub(crate) snapshot: SlackDmInboxSnapshot,
    pub(crate) users: HashMap<String, Value>,
}

pub(super) struct SlackSidebarDirectoryUser {
    pub(super) user: SlackFallbackUser,
    pub(super) presence: Option<SlackDirectMessagePresence>,
}

#[derive(Default)]
pub(super) struct SlackDmInboxCache {
    users: Mutex<Option<Vec<SlackDmUser>>>,
    users_fetch: Mutex<()>,
    counts: Mutex<Option<SlackClientCountsResponse>>,
    counts_fetch: Mutex<()>,
}

impl SlackInternalSidebarClient {
    pub(crate) fn invalidate_dm_counts(&self) -> Result<(), String> {
        *self
            .dm_inbox_cache
            .counts
            .lock()
            .map_err(|_| "Slack DMs counts cache mutex poisoned".to_string())? = None;
        Ok(())
    }

    pub(crate) fn load_dm_inbox(
        &self,
        team_id: &str,
        self_user_id: &str,
        cursor: Option<&str>,
        user_cache: &SlackUserPayloadCache,
    ) -> Result<SlackDmInboxLoad, String> {
        let (dms_result, counts_result) = thread::scope(|scope| {
            let dms = scope.spawn(|| self.load_dms_page(cursor));
            let counts = scope.spawn(|| self.load_dm_counts_cached());
            (
                dms.join()
                    .map_err(|_| "Slack client.dms request thread panicked".to_string()),
                counts
                    .join()
                    .map_err(|_| "Slack client.counts request thread panicked".to_string()),
            )
        });
        let dms = dms_result??;
        let counts = counts_result??;
        let users = cached_dm_users(user_cache, &dm_inbox_user_ids(&dms, self_user_id))?;
        let fallback_users = self.load_missing_dm_users(&dms, self_user_id, &users)?;
        let next_cursor = non_empty(dms.response_metadata.next_cursor);
        let inbox = dm_inbox_items(SlackDmInboxItemsInput {
            ims: dms.ims,
            mpims: dms.mpims,
            self_user_id,
            users: &users,
            fallback_users: &fallback_users,
            counts: &counts,
        })?;
        let cached_users = fallback_users
            .into_iter()
            .filter_map(|(user_id, user)| user.raw_payload.map(|payload| (user_id, payload)))
            .collect();
        Ok(SlackDmInboxLoad {
            snapshot: SlackDmInboxSnapshot {
                team_id: team_id.to_string(),
                self_user_id: self_user_id.to_string(),
                slackbot_conversation_id: inbox.slackbot_conversation_id,
                items: inbox.items,
                next_cursor,
            },
            users: cached_users,
        })
    }

    fn load_dms_page(&self, cursor: Option<&str>) -> Result<SlackClientDmsResponse, String> {
        let mut params = vec![
            ("count", DMS_PAGE_SIZE.to_string()),
            ("include_closed", "true".to_string()),
            ("include_channel", "true".to_string()),
            ("exclude_bots", "true".to_string()),
            ("priority_mode", "priority".to_string()),
            ("_x_reason", "dms-tab-populate".to_string()),
            ("_x_mode", "online".to_string()),
            ("_x_sonic", "true".to_string()),
            ("_x_app_name", "client".to_string()),
        ];
        if let Some(cursor) = cursor.map(str::trim).filter(|cursor| !cursor.is_empty()) {
            params.push(("cursor", cursor.to_string()));
        }
        let body = self.post_internal_method(CLIENT_DMS, params)?;
        let response = decode_response::<SlackClientDmsResponse>(CLIENT_DMS, &body)?;
        let item_count = response.ims.len().saturating_add(response.mpims.len());
        if item_count > DMS_PAGE_SIZE {
            return Err(format!(
                "Slack {CLIENT_DMS} returned {item_count} DMs for a {DMS_PAGE_SIZE}-item page"
            ));
        }
        Ok(response)
    }

    fn load_dm_counts(&self) -> Result<SlackClientCountsResponse, String> {
        let body = self.post_internal_method(
            CLIENT_COUNTS,
            vec![
                ("_x_app_name", "client".to_string()),
                ("_x_sonic", "true".to_string()),
            ],
        )?;
        decode_response(CLIENT_COUNTS, &body)
    }

    fn load_dm_counts_cached(&self) -> Result<SlackClientCountsResponse, String> {
        if let Some(counts) = self
            .dm_inbox_cache
            .counts
            .lock()
            .map_err(|_| "Slack DMs counts cache mutex poisoned".to_string())?
            .clone()
        {
            return Ok(counts);
        }
        let _fetch_guard = self
            .dm_inbox_cache
            .counts_fetch
            .lock()
            .map_err(|_| "Slack DMs counts fetch mutex poisoned".to_string())?;
        if let Some(counts) = self
            .dm_inbox_cache
            .counts
            .lock()
            .map_err(|_| "Slack DMs counts cache mutex poisoned".to_string())?
            .clone()
        {
            return Ok(counts);
        }
        let counts = self.load_dm_counts()?;
        *self
            .dm_inbox_cache
            .counts
            .lock()
            .map_err(|_| "Slack DMs counts cache mutex poisoned".to_string())? =
            Some(counts.clone());
        Ok(counts)
    }

    fn load_users_cached(&self) -> Result<Vec<SlackDmUser>, String> {
        if let Some(users) = self
            .dm_inbox_cache
            .users
            .lock()
            .map_err(|_| "Slack DMs users cache mutex poisoned".to_string())?
            .clone()
        {
            return Ok(users);
        }
        let _fetch_guard = self
            .dm_inbox_cache
            .users_fetch
            .lock()
            .map_err(|_| "Slack DMs users fetch mutex poisoned".to_string())?;
        if let Some(users) = self
            .dm_inbox_cache
            .users
            .lock()
            .map_err(|_| "Slack DMs users cache mutex poisoned".to_string())?
            .clone()
        {
            return Ok(users);
        }
        let users = self.load_users()?;
        *self
            .dm_inbox_cache
            .users
            .lock()
            .map_err(|_| "Slack DMs users cache mutex poisoned".to_string())? = Some(users.clone());
        Ok(users)
    }

    pub(crate) fn load_user_directory(&self) -> Result<HashMap<String, Value>, String> {
        self.load_users_cached()?
            .into_iter()
            .map(|user| {
                let user_id = user.id.clone();
                serde_json::to_value(user)
                    .map(|value| (user_id, value))
                    .map_err(|error| format!("failed to cache Slack users.list member: {error}"))
            })
            .collect()
    }

    pub(super) fn load_sidebar_users_from_cache(
        &self,
        user_ids: &BTreeSet<String>,
        user_cache: &SlackUserPayloadCache,
    ) -> Result<HashMap<String, SlackSidebarDirectoryUser>, String> {
        cached_dm_users(user_cache, user_ids)?
            .into_iter()
            .map(|user| {
                let label = user.display_name();
                let compact_avatar_image_url = user.avatar_image_url(SlackAvatarPurpose::Sidebar);
                let message_avatar_image_url = user.avatar_image_url(SlackAvatarPurpose::Message);
                let presence = user.sidebar_presence()?;
                Ok((
                    user.id,
                    SlackSidebarDirectoryUser {
                        user: SlackFallbackUser {
                            label,
                            compact_avatar_image_url,
                            message_avatar_image_url,
                            raw_payload: None,
                        },
                        presence,
                    },
                ))
            })
            .collect()
    }

    fn load_missing_dm_users(
        &self,
        dms: &SlackClientDmsResponse,
        self_user_id: &str,
        users: &[SlackDmUser],
    ) -> Result<HashMap<String, SlackFallbackUser>, String> {
        let known_user_ids = users
            .iter()
            .map(|user| user.id.as_str())
            .collect::<HashSet<_>>();
        let mut missing_user_ids = BTreeSet::new();
        for dm in dms.ims.iter().chain(dms.mpims.iter()) {
            if dm.channel.user.as_deref() == Some(SLACKBOT_USER_ID) {
                continue;
            }
            if let Some(user_id) = dm
                .message
                .user
                .as_ref()
                .filter(|user_id| !known_user_ids.contains(user_id.as_str()))
            {
                missing_user_ids.insert(user_id.clone());
            }
            if dm.channel.is_mpim {
                missing_user_ids.extend(
                    dm.channel
                        .members
                        .iter()
                        .filter(|user_id| user_id.as_str() != self_user_id)
                        .filter(|user_id| !known_user_ids.contains(user_id.as_str()))
                        .cloned(),
                );
            } else if let Some(user_id) = dm
                .channel
                .user
                .as_ref()
                .filter(|user_id| !known_user_ids.contains(user_id.as_str()))
            {
                missing_user_ids.insert(user_id.clone());
            }
        }
        if missing_user_ids.len() > DM_INBOX_TARGETED_USER_INFO_LIMIT {
            return Err(format!(
                "Slack DM inbox required {} uncached users; targeted hydration limit is {DM_INBOX_TARGETED_USER_INFO_LIMIT}",
                missing_user_ids.len()
            ));
        }
        self.load_fallback_users(missing_user_ids)
    }

    pub(super) fn load_fallback_users(
        &self,
        missing_user_ids: BTreeSet<String>,
    ) -> Result<HashMap<String, SlackFallbackUser>, String> {
        if missing_user_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let missing_user_ids = missing_user_ids.into_iter().collect::<Vec<_>>();
        let worker_count = missing_user_ids.len().min(FALLBACK_USER_WORKER_LIMIT);
        let chunk_size = missing_user_ids.len().div_ceil(worker_count);
        thread::scope(|scope| {
            missing_user_ids
                .chunks(chunk_size)
                .map(|user_ids| {
                    scope.spawn(move || {
                        user_ids
                            .iter()
                            .map(|user_id| {
                                self.load_fallback_user(user_id)
                                    .map(|user| (user_id.clone(), user))
                            })
                            .collect::<Result<Vec<_>, String>>()
                    })
                })
                .collect::<Vec<_>>()
                .into_iter()
                .map(|worker| {
                    worker
                        .join()
                        .map_err(|_| "Slack users.info fallback worker panicked".to_string())?
                })
                .collect::<Result<Vec<_>, String>>()
                .map(|users| users.into_iter().flatten().collect())
        })
    }

    fn load_users(&self) -> Result<Vec<SlackDmUser>, String> {
        let mut users = Vec::new();
        let mut cursor = None;
        loop {
            let mut params = vec![
                ("limit", USERS_PAGE_SIZE.to_string()),
                ("_x_app_name", "client".to_string()),
                ("_x_sonic", "true".to_string()),
            ];
            if let Some(next_cursor) = cursor.take() {
                params.push(("cursor", next_cursor));
            }
            let body = self.post_internal_method(USERS_LIST, params)?;
            let page = decode_response::<SlackUsersListResponse>(USERS_LIST, &body)?;
            users.extend(page.members);
            cursor = non_empty(page.response_metadata.next_cursor);
            if cursor.is_none() {
                break;
            }
        }
        Ok(users)
    }
}
