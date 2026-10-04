use std::collections::{HashMap, HashSet};

use crate::model::{
    SlackDraftDestination, SlackDraftId, SlackDraftRevision, SlackDraftsSentCursor,
    SlackDraftsSentItem, SlackDraftsSentRequest, SlackDraftsSentSnapshot, SlackDraftsSentTab,
    SlackFileId,
};
use crate::model::{
    SlackDraftUpdateTarget, SlackDraftWriteTarget, SlackMessageClientId, SlackMessageTimestamp,
};
use serde::Deserialize;
use serde_json::Value;

use super::SlackLiveWorkspaceLoader;
use crate::live::payload::message::slack_rich_text_body_from_blocks;
use crate::live::payload::sidebar_dom::SlackSidebarSnapshot;

const DRAFTS_LIST: &str = "drafts.list";
const DRAFTS_PAGE_SIZE: usize = 25;

impl SlackLiveWorkspaceLoader {
    pub fn load_drafts_sent(
        &self,
        request: SlackDraftsSentRequest,
    ) -> Result<SlackDraftsSentSnapshot, String> {
        if request.team_id != self.team_id {
            return Err(format!(
                "Slack Drafts & sent request targeted team {} from runtime team {}",
                request.team_id, self.team_id
            ));
        }
        let mut parameters = vec![
            ("is_active", true.to_string()),
            ("limit", DRAFTS_PAGE_SIZE.to_string()),
        ];
        if let Some(cursor) = request.cursor.as_ref() {
            parameters.push(("next_ts", cursor.as_str().to_string()));
        }
        let payload = self.api.post(DRAFTS_LIST, &parameters)?;
        let response = serde_json::from_value::<SlackDraftsListResponse>(payload)
            .map_err(|error| format!("failed to decode Slack {DRAFTS_LIST} response: {error}"))?;
        self.decode_drafts_sent_response(request, response)
    }

    fn decode_drafts_sent_response(
        &self,
        request: SlackDraftsSentRequest,
        response: SlackDraftsListResponse,
    ) -> Result<SlackDraftsSentSnapshot, String> {
        if response.drafts.len() > DRAFTS_PAGE_SIZE {
            return Err(format!(
                "Slack {DRAFTS_LIST} returned {} drafts for a {DRAFTS_PAGE_SIZE}-item page",
                response.drafts.len()
            ));
        }
        let next_cursor = response
            .has_more
            .then(|| {
                response
                    .drafts
                    .last()
                    .ok_or_else(|| {
                        format!("Slack {DRAFTS_LIST} returned has_more without a final draft")
                    })
                    .and_then(|draft| SlackDraftsSentCursor::new(draft.last_updated_ts.clone()))
            })
            .transpose()?;
        if request.cursor.as_ref() == next_cursor.as_ref() && next_cursor.is_some() {
            return Err(format!(
                "Slack {DRAFTS_LIST} returned the same pagination cursor"
            ));
        }
        let sidebar = self.cached_sidebar_snapshot()?;
        let users = self
            .user_cache
            .lock()
            .map_err(|_| "slack user cache mutex poisoned".to_string())?;
        let mut ids = HashSet::with_capacity(response.drafts.len());
        let items = response
            .drafts
            .into_iter()
            .filter(|draft| draft.belongs_to(request.tab))
            .map(|draft| {
                if !ids.insert(draft.id.clone()) {
                    return Err(format!(
                        "Slack {DRAFTS_LIST} returned duplicate draft {}",
                        draft.id
                    ));
                }
                draft.into_item(&self.team_id, &users, sidebar.as_ref())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(SlackDraftsSentSnapshot {
            team_id: self.team_id.clone(),
            tab: request.tab,
            items,
            next_cursor,
        })
    }

    pub(super) fn reconcile_scheduled_draft_create(
        &self,
        client_message_id: &SlackMessageClientId,
        write_target: &SlackDraftWriteTarget,
        post_at_unix_seconds: i64,
        file_ids: &[SlackFileId],
    ) -> Result<Option<crate::model::SlackScheduledDraftReceipt>, String> {
        let expected_targets = std::slice::from_ref(write_target);
        self.reconcile_scheduled_draft_page(
            |item| item.client_message_id == client_message_id.as_str(),
            expected_targets,
            post_at_unix_seconds,
            file_ids,
        )
    }

    pub(super) fn reconcile_scheduled_draft_update(
        &self,
        update_target: SlackDraftUpdateTarget<'_>,
        write_targets: &[SlackDraftWriteTarget],
        post_at_unix_seconds: i64,
        file_ids: &[SlackFileId],
    ) -> Result<Option<crate::model::SlackScheduledDraftReceipt>, String> {
        self.reconcile_scheduled_draft_page(
            |item| {
                &item.id == update_target.target().draft_id()
                    && &item.revision != update_target.target().revision()
            },
            write_targets,
            post_at_unix_seconds,
            file_ids,
        )
    }

    fn reconcile_scheduled_draft_page(
        &self,
        identity_matches: impl Fn(&SlackDraftsSentItem) -> bool,
        write_targets: &[SlackDraftWriteTarget],
        post_at_unix_seconds: i64,
        file_ids: &[SlackFileId],
    ) -> Result<Option<crate::model::SlackScheduledDraftReceipt>, String> {
        let expected_post_at = scheduled_draft_timestamp(post_at_unix_seconds)?;
        let mut cursor: Option<SlackDraftsSentCursor> = None;
        let mut requested_cursors = HashSet::new();
        let mut draft_ids = HashSet::new();
        let mut matching_item = None;
        loop {
            if let Some(cursor) = cursor.as_ref() {
                if !requested_cursors.insert(cursor.clone()) {
                    return Err(format!(
                        "Slack {DRAFTS_LIST} repeated pagination cursor {}",
                        cursor.as_str()
                    ));
                }
            }
            let snapshot = self.load_drafts_sent(SlackDraftsSentRequest {
                team_id: self.team_id.clone(),
                tab: SlackDraftsSentTab::Scheduled,
                cursor,
            })?;
            for item in snapshot.items {
                if !draft_ids.insert(item.id.clone()) {
                    return Err(format!(
                        "Slack {DRAFTS_LIST} returned duplicate scheduled draft {} across pages",
                        item.id
                    ));
                }
                if !identity_matches(&item)
                    || item.scheduled_unix_seconds != expected_post_at
                    || item.file_ids.as_slice() != file_ids
                    || !draft_destinations_match(&item.destinations, write_targets)?
                {
                    continue;
                }
                if matching_item.replace(item).is_some() {
                    return Err(
                        "Slack drafts.list returned multiple matching scheduled drafts".to_string(),
                    );
                }
            }
            let Some(next_cursor) = snapshot.next_cursor else {
                break;
            };
            cursor = Some(next_cursor);
        }
        Ok(
            matching_item.map(|item| crate::model::SlackScheduledDraftReceipt {
                target: crate::model::SlackDraftTarget::new(item.id, item.revision),
                post_at_unix_seconds,
            }),
        )
    }
}

fn scheduled_draft_timestamp(post_at_unix_seconds: i64) -> Result<u64, String> {
    u64::try_from(post_at_unix_seconds).map_err(|_| {
        format!(
            "Slack scheduled-draft reconciliation received invalid timestamp {post_at_unix_seconds}"
        )
    })
}

fn draft_destinations_match(
    destinations: &[SlackDraftDestination],
    write_targets: &[SlackDraftWriteTarget],
) -> Result<bool, String> {
    if destinations.len() != write_targets.len() {
        return Ok(false);
    }
    destinations
        .iter()
        .zip(write_targets)
        .map(|(destination, write_target)| {
            SlackDraftWriteTarget::from_loaded(destination).map(|loaded| &loaded == write_target)
        })
        .try_fold(true, |matches, next| {
            next.map(|next_matches| matches && next_matches)
        })
}

#[derive(Deserialize)]
struct SlackDraftsListResponse {
    #[serde(default)]
    drafts: Vec<SlackDraftWire>,
    #[serde(default)]
    has_more: bool,
}

#[derive(Deserialize)]
struct SlackDraftWire {
    id: String,
    date_created: u64,
    user_id: String,
    team_id: String,
    last_updated_ts: String,
    #[serde(default)]
    blocks: Vec<Value>,
    #[serde(default)]
    is_deleted: bool,
    #[serde(default)]
    is_sent: bool,
    #[serde(default)]
    client_msg_id: String,
    #[serde(default)]
    date_scheduled: u64,
    #[serde(default)]
    destinations: Vec<SlackDraftDestinationWire>,
    #[serde(default)]
    file_ids: Vec<SlackFileId>,
}

impl SlackDraftWire {
    fn belongs_to(&self, tab: SlackDraftsSentTab) -> bool {
        if self.is_deleted {
            return false;
        }
        match tab {
            SlackDraftsSentTab::Drafts => !self.is_sent && self.date_scheduled == 0,
            SlackDraftsSentTab::Scheduled => !self.is_sent && self.date_scheduled > 0,
            SlackDraftsSentTab::Sent => self.is_sent,
        }
    }

    fn into_item(
        self,
        runtime_team_id: &str,
        users: &HashMap<String, Value>,
        sidebar: Option<&SlackSidebarSnapshot>,
    ) -> Result<SlackDraftsSentItem, String> {
        require_non_empty(DRAFTS_LIST, &self.user_id, "user_id")?;
        require_non_empty(DRAFTS_LIST, &self.team_id, "team_id")?;
        if self.team_id != runtime_team_id {
            return Err(format!(
                "Slack {DRAFTS_LIST} returned draft {} for team {} from runtime team {runtime_team_id}",
                self.id, self.team_id
            ));
        }
        let id = SlackDraftId::parse(self.id)?;
        let revision = SlackDraftRevision::parse(self.last_updated_ts)?;
        let rich_body = slack_rich_text_body_from_blocks(&self.blocks, users, sidebar);
        let body = rich_body
            .as_ref()
            .map(crate::model::SlackRichTextBody::plain_text)
            .unwrap_or_default();
        if self.destinations.is_empty() {
            return Err(format!(
                "Slack {DRAFTS_LIST} returned draft {id} without a destination"
            ));
        }
        let destinations = self
            .destinations
            .into_iter()
            .map(|destination| destination.into_destination(sidebar))
            .collect::<Result<Vec<_>, _>>()?;
        validate_file_ids(&self.file_ids)?;
        Ok(SlackDraftsSentItem {
            id,
            team_id: self.team_id,
            user_id: self.user_id,
            created_unix_seconds: self.date_created,
            revision,
            scheduled_unix_seconds: self.date_scheduled,
            client_message_id: self.client_msg_id,
            body,
            rich_body,
            destinations,
            file_ids: self.file_ids,
        })
    }
}

#[derive(Deserialize)]
struct SlackDraftDestinationWire {
    channel_id: String,
    #[serde(default)]
    user_ids: Vec<String>,
    #[serde(default)]
    thread_ts: Option<String>,
    #[serde(default)]
    message_ts: Option<String>,
    #[serde(default)]
    broadcast: bool,
}

impl SlackDraftDestinationWire {
    fn into_destination(
        self,
        sidebar: Option<&SlackSidebarSnapshot>,
    ) -> Result<SlackDraftDestination, String> {
        require_non_empty(DRAFTS_LIST, &self.channel_id, "destination.channel_id")?;
        if let Some(timestamp) = self.thread_ts.as_deref() {
            SlackMessageTimestamp::parse(timestamp)?;
        }
        if let Some(timestamp) = self.message_ts.as_deref() {
            SlackMessageTimestamp::parse(timestamp)?;
        }
        let sidebar_item = sidebar.and_then(|sidebar| sidebar.item(&self.channel_id));
        Ok(SlackDraftDestination {
            label: sidebar_item
                .map(|item| item.label.clone())
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| self.channel_id.clone()),
            avatar_image_url: sidebar_item.and_then(|item| item.avatar_image_url.clone()),
            conversation_id: self.channel_id,
            user_ids: self.user_ids,
            thread_timestamp: self.thread_ts,
            message_timestamp: self.message_ts,
            broadcast: self.broadcast,
        })
    }
}

fn require_non_empty(method: &str, value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("Slack {method} returned an empty {field}"))
    } else {
        Ok(())
    }
}

fn validate_file_ids(file_ids: &[SlackFileId]) -> Result<(), String> {
    let mut unique_file_ids = HashSet::with_capacity(file_ids.len());
    for file_id in file_ids {
        if !unique_file_ids.insert(file_id) {
            return Err(format!(
                "Slack {DRAFTS_LIST} returned duplicate file id {file_id}"
            ));
        }
    }
    Ok(())
}
