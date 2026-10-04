mod workspace;

use std::{
    collections::HashMap,
    fs,
    ops::Deref,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use crate::model::{
    SlackConversationSnapshot, SlackDmInboxSnapshot, SlackLastReadTimestamp, SlackMessage,
    SlackShellSnapshot, SlackSidebarSnapshot,
};
use serde::{
    de::{MapAccess, Visitor},
    Deserialize, Deserializer, Serialize,
};
use serde_json::Value;

pub use workspace::SlackWorkspaceCacheStore;

pub const SLACK_CACHE_DIR_ENV: &str = "NOTSLACK_SLACK_CACHE_DIR";
const SLACK_WORKSPACE_CACHE_SCHEMA_VERSION: u32 = 10;
const SLACK_READ_RECEIPTS_CACHE_SCHEMA_VERSION: u32 = 1;
const SLACK_READ_RECEIPTS_CACHE_LIMIT: usize = 50_000;
const SLACK_CONFIRMED_SEND_CACHE_SCHEMA_VERSION: u32 = 2;
pub(crate) const SLACK_CONFIRMED_SEND_CACHE_LIMIT: usize = 100;
const SLACK_DM_INBOX_CACHE_SCHEMA_VERSION: u32 = 2;
const SLACK_DM_INBOX_CACHE_ITEM_LIMIT: usize = 250;
const SLACK_USER_PAYLOAD_CACHE_SCHEMA_VERSION: u32 = 1;
const SLACK_USER_PAYLOAD_CACHE_RECORD_LIMIT: usize = 50_000;
const SLACK_USER_PAYLOAD_CACHE_MAX_ENCRYPTED_BYTES: u64 = 64 * 1024 * 1024;
const SLACK_USER_ID_MAX_BYTES: usize = 128;

type SlackUserPayloads = HashMap<String, Value>;
type SlackUserPayloadStore = Mutex<SlackUserPayloads>;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SlackWorkspaceCache {
    schema_version: u32,
    fetched_at_unix_secs: u64,
    team_id: String,
    conversation_id: String,
    shell: Option<SlackShellSnapshot>,
    sidebar: Option<SlackSidebarSnapshot>,
    conversation: Option<SlackConversationSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SlackConfirmedSendCache {
    schema_version: u32,
    team_id: String,
    conversation_id: String,
    messages: Vec<SlackMessage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SlackDmInboxCache {
    schema_version: u32,
    fetched_at_unix_secs: u64,
    team_id: String,
    snapshot: SlackDmInboxSnapshot,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SlackReadReceiptsCache {
    schema_version: u32,
    team_id: String,
    receipts: HashMap<String, SlackLastReadTimestamp>,
}

#[derive(Debug, Deserialize)]
struct SlackUserPayloadCacheFile {
    schema_version: u32,
    #[serde(rename = "fetched_at_unix_secs")]
    _fetched_at_unix_secs: u64,
    team_id: String,
    users: SlackUserPayloadRecords,
}

#[derive(Serialize)]
struct SlackUserPayloadCacheWrite<'a> {
    schema_version: u32,
    fetched_at_unix_secs: u64,
    team_id: &'a str,
    users: &'a SlackUserPayloads,
}

#[derive(Debug)]
struct SlackUserPayloadRecords {
    records: SlackUserPayloads,
    exceeded_limit: bool,
}

impl<'de> Deserialize<'de> for SlackUserPayloadRecords {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SlackUserPayloadRecordsVisitor;

        impl<'de> Visitor<'de> for SlackUserPayloadRecordsVisitor {
            type Value = SlackUserPayloadRecords;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a map of Slack user IDs to raw user payloads")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut records = HashMap::with_capacity(
                    map.size_hint()
                        .unwrap_or_default()
                        .min(SLACK_USER_PAYLOAD_CACHE_RECORD_LIMIT),
                );
                let mut record_count = 0_usize;
                while let Some((user_id, user)) = map.next_entry::<String, Value>()? {
                    record_count = record_count.saturating_add(1);
                    if record_count <= SLACK_USER_PAYLOAD_CACHE_RECORD_LIMIT {
                        records.insert(user_id, user);
                    }
                }
                Ok(SlackUserPayloadRecords {
                    records,
                    exceeded_limit: record_count > SLACK_USER_PAYLOAD_CACHE_RECORD_LIMIT,
                })
            }
        }

        deserializer.deserialize_map(SlackUserPayloadRecordsVisitor)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SlackCachePaths {
    team_dir: PathBuf,
}

impl SlackCachePaths {
    fn new(cache_root: &Path, team_id: &str) -> Self {
        Self {
            team_dir: local_cache::account_cache_dir(cache_root, team_id),
        }
    }

    fn workspace_path(&self, conversation_id: &str) -> PathBuf {
        self.team_dir.join(format!(
            "workspace-{}.bin",
            local_cache::cache_key_hash(conversation_id)
        ))
    }

    fn confirmed_sends_path(&self, conversation_id: &str) -> PathBuf {
        self.team_dir.join(format!(
            "confirmed-sends-{}.bin",
            local_cache::cache_key_hash(conversation_id)
        ))
    }

    fn dm_inbox_path(&self) -> PathBuf {
        self.team_dir.join("dm-inbox.bin")
    }

    fn read_receipts_path(&self) -> PathBuf {
        self.team_dir.join("read-receipts.bin")
    }

    fn user_payloads_path(&self) -> PathBuf {
        self.team_dir.join("user-payloads.bin")
    }
}

pub(super) struct SlackUserPayloadCache {
    team_id: String,
    store: OnceLock<Result<SlackUserPayloadCacheStore, String>>,
}

struct SlackUserPayloadCacheStore {
    path: PathBuf,
    key: [u8; 32],
    users: SlackUserPayloadStore,
}

pub(super) struct SlackUserPayloadCacheReadGuard<'a> {
    users: MutexGuard<'a, SlackUserPayloads>,
}

impl Deref for SlackUserPayloadCacheReadGuard<'_> {
    type Target = SlackUserPayloads;

    fn deref(&self) -> &Self::Target {
        &self.users
    }
}

impl SlackUserPayloadCache {
    pub(super) fn for_team(team_id: &str) -> Self {
        Self {
            team_id: team_id.to_string(),
            store: OnceLock::new(),
        }
    }

    pub(super) fn lock(&self) -> Result<SlackUserPayloadCacheReadGuard<'_>, String> {
        self.loaded_store()?
            .users
            .lock()
            .map(|users| SlackUserPayloadCacheReadGuard { users })
            .map_err(|_| "slack user cache mutex poisoned".to_string())
    }

    pub(super) fn insert(&self, user_id: String, user: Value) -> Result<(), String> {
        self.extend([(user_id, user)])
    }

    pub(super) fn extend(
        &self,
        users: impl IntoIterator<Item = (String, Value)>,
    ) -> Result<(), String> {
        let users = users.into_iter().collect::<Vec<_>>();
        for (user_id, user) in &users {
            validate_slack_user_payload(user_id, user)?;
        }
        let store = self.loaded_store()?;
        let mut cached_users = store
            .users
            .lock()
            .map_err(|_| "slack user cache mutex poisoned".to_string())?;
        let added_record_count = users
            .iter()
            .filter(|(user_id, _)| !cached_users.contains_key(user_id))
            .count();
        if cached_users.len().saturating_add(added_record_count)
            > SLACK_USER_PAYLOAD_CACHE_RECORD_LIMIT
        {
            return Err(format!(
                "Slack user payload cache exceeded {SLACK_USER_PAYLOAD_CACHE_RECORD_LIMIT} records"
            ));
        }
        let mut changed = false;
        for (user_id, user) in users {
            if cached_users.get(&user_id) != Some(&user) {
                cached_users.insert(user_id, user);
                changed = true;
            }
        }
        if changed {
            let cache = SlackUserPayloadCacheWrite {
                schema_version: SLACK_USER_PAYLOAD_CACHE_SCHEMA_VERSION,
                fetched_at_unix_secs: fetched_at_unix_secs(),
                team_id: &self.team_id,
                users: &cached_users,
            };
            if let Err(error) = local_cache::write_encrypted_json(&store.path, &store.key, &cache) {
                eprintln!("Slack user payload cache write failed: {error}");
            }
        }
        Ok(())
    }

    fn loaded_store(&self) -> Result<&SlackUserPayloadCacheStore, String> {
        self.store
            .get_or_init(|| {
                let started_at = Instant::now();
                let store = (|| {
                    let cache_key = format!("slack-v1|{}", self.team_id);
                    let key = local_cache::load_or_create_cache_key(&cache_key, "Slack")?;
                    let cache_root = default_slack_cache_root_dir()?;
                    let path =
                        SlackCachePaths::new(&cache_root, &self.team_id).user_payloads_path();
                    let users = load_slack_user_payloads(&path, &key, &self.team_id)?;
                    if std::env::var_os("NOTSLACK_SLACK_LOAD_PROFILE").is_some() {
                        eprintln!(
                            "[notslack-slack-user-cache-profile] outcome=loaded elapsed={:.2}ms records={}",
                            started_at.elapsed().as_secs_f64() * 1_000.0,
                            users.len(),
                        );
                    }
                    Ok(SlackUserPayloadCacheStore {
                        path,
                        key,
                        users: Mutex::new(users),
                    })
                })();
                if store.is_err()
                    && std::env::var_os("NOTSLACK_SLACK_LOAD_PROFILE").is_some()
                {
                    eprintln!(
                            "[notslack-slack-user-cache-profile] outcome=failed elapsed={:.2}ms",
                            started_at.elapsed().as_secs_f64() * 1_000.0,
                        );
                }
                store
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}

fn load_slack_user_payloads(
    path: &Path,
    key: &[u8; 32],
    team_id: &str,
) -> Result<SlackUserPayloads, String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() > SLACK_USER_PAYLOAD_CACHE_MAX_ENCRYPTED_BYTES => {
            return Err(format!(
                "Slack user payload cache {} was {} bytes; limit is {} bytes",
                path.display(),
                metadata.len(),
                SLACK_USER_PAYLOAD_CACHE_MAX_ENCRYPTED_BYTES
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "failed to inspect Slack user payload cache {}: {error}",
                path.display()
            ));
        }
    }
    let Some(cache) = local_cache::read_encrypted_json::<SlackUserPayloadCacheFile>(path, key)?
    else {
        return Ok(HashMap::new());
    };
    if cache.schema_version != SLACK_USER_PAYLOAD_CACHE_SCHEMA_VERSION || cache.team_id != team_id {
        return Ok(HashMap::new());
    }
    if cache.users.exceeded_limit {
        return Err(format!(
            "Slack user payload cache contained more than {SLACK_USER_PAYLOAD_CACHE_RECORD_LIMIT} records"
        ));
    }
    for (user_id, user) in &cache.users.records {
        validate_slack_user_payload(user_id, user)?;
    }
    Ok(cache.users.records)
}

fn validate_slack_user_payload(user_id: &str, user: &Value) -> Result<(), String> {
    if user_id.is_empty()
        || user_id.len() > SLACK_USER_ID_MAX_BYTES
        || !user_id.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        return Err(format!(
            "Slack user payload cache contained invalid user ID {user_id:?}"
        ));
    }
    let payload_user_id = user
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("Slack user payload {user_id} omitted its id"))?;
    if payload_user_id != user_id {
        return Err(format!(
            "Slack user payload cache key {user_id} did not match payload ID {payload_user_id}"
        ));
    }
    Ok(())
}

fn fetched_at_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after Unix epoch for Slack cache")
        .as_secs()
}

pub fn default_slack_cache_root_dir() -> Result<PathBuf, String> {
    local_cache::default_cache_root_dir(SLACK_CACHE_DIR_ENV, "slack", "Slack")
}
