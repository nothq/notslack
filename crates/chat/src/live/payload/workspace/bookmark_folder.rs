use std::collections::HashSet;

use crate::model::{
    SlackBookmarkFolderItem, SlackBookmarkFolderRequest, SlackBookmarkFolderSnapshot,
    SlackBookmarkSource,
};
use serde::Deserialize;

use super::SlackLiveWorkspaceLoader;

const BOOKMARKS_LIST: &str = "bookmarks.list";

impl SlackLiveWorkspaceLoader {
    pub fn load_bookmark_folder(
        &self,
        request: SlackBookmarkFolderRequest,
    ) -> Result<SlackBookmarkFolderSnapshot, String> {
        if request.team_id != self.team_id {
            return Err(format!(
                "Slack bookmark-folder request targeted team {} from runtime team {}",
                request.team_id, self.team_id
            ));
        }
        require_nonempty_request_id(&request.conversation_id, "conversation_id")?;
        require_nonempty_request_id(&request.folder_bookmark_id, "folder_bookmark_id")?;
        let payload = self.api.post(
            BOOKMARKS_LIST,
            &[("channel_id", request.conversation_id.clone())],
        )?;
        let response =
            serde_json::from_value::<SlackBookmarksListResponse>(payload).map_err(|error| {
                format!("failed to decode Slack {BOOKMARKS_LIST} response: {error}")
            })?;
        decode_bookmark_folder_response(request, response)
    }
}

fn decode_bookmark_folder_response(
    request: SlackBookmarkFolderRequest,
    response: SlackBookmarksListResponse,
) -> Result<SlackBookmarkFolderSnapshot, String> {
    let mut item_ids = HashSet::with_capacity(response.bookmarks.len());
    let items = response
        .bookmarks
        .into_iter()
        .map(decode_bookmark)
        .map(|item| {
            let item = item?;
            if !item_ids.insert(item.id.clone()) {
                return Err(format!(
                    "Slack {BOOKMARKS_LIST} returned duplicate bookmark {}",
                    item.id
                ));
            }
            Ok(item)
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .filter(|item| item.parent_id.as_deref() == Some(request.folder_bookmark_id.as_str()))
        .collect();
    Ok(SlackBookmarkFolderSnapshot {
        team_id: request.team_id,
        conversation_id: request.conversation_id,
        folder_bookmark_id: request.folder_bookmark_id,
        items,
    })
}

fn decode_bookmark(wire: SlackBookmarkWire) -> Result<SlackBookmarkFolderItem, String> {
    let id = require_nonempty_wire_string(wire.id, "id", "bookmark")?;
    let source_type = require_nonempty_wire_string(wire.source_type, "type", &id)?;
    let parent_id = optional_nonempty_wire_string(wire.parent_id, "parent_id", &id)?;
    let entity_id = optional_nonempty_wire_string(wire.entity_id, "entity_id", &id)?;
    let title = optional_content(wire.title);
    let link = optional_content(wire.link);
    let icon_url = optional_nonempty_wire_string(wire.icon_url, "icon_url", &id)?;
    let source = match source_type.as_str() {
        "link" => SlackBookmarkSource::Link {
            entity_id: entity_id.clone(),
        },
        "file" => SlackBookmarkSource::File {
            file_id: entity_id.clone().ok_or_else(|| {
                format!("Slack {BOOKMARKS_LIST} returned file bookmark {id} without entity_id")
            })?,
        },
        "pinned_message" => SlackBookmarkSource::PinnedMessage {
            entity_id: entity_id.clone(),
        },
        _ => SlackBookmarkSource::Unsupported {
            source_type,
            entity_id: entity_id.clone(),
        },
    };
    if matches!(
        &source,
        SlackBookmarkSource::Link { .. } | SlackBookmarkSource::File { .. }
    ) {
        if title.is_none() {
            return Err(format!(
                "Slack {BOOKMARKS_LIST} returned bookmark {id} without a title"
            ));
        }
        if link.is_none() {
            return Err(format!(
                "Slack {BOOKMARKS_LIST} returned bookmark {id} without a link"
            ));
        }
    }
    Ok(SlackBookmarkFolderItem {
        id,
        parent_id,
        title,
        link,
        icon_url,
        source,
    })
}

fn require_nonempty_request_id(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!(
            "Slack bookmark-folder request requires non-empty {field}"
        ))
    } else {
        Ok(())
    }
}

fn require_nonempty_wire_string(
    value: String,
    field: &str,
    item_label: &str,
) -> Result<String, String> {
    if value.trim().is_empty() {
        Err(format!(
            "Slack {BOOKMARKS_LIST} returned {item_label} without {field}"
        ))
    } else {
        Ok(value)
    }
}

fn optional_nonempty_wire_string(
    value: Option<String>,
    field: &str,
    item_id: &str,
) -> Result<Option<String>, String> {
    match value {
        Some(value) if value.trim().is_empty() => Err(format!(
            "Slack {BOOKMARKS_LIST} returned bookmark {item_id} with empty {field}"
        )),
        value => Ok(value),
    }
}

fn optional_content(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

#[derive(Deserialize)]
struct SlackBookmarksListResponse {
    bookmarks: Vec<SlackBookmarkWire>,
}

#[derive(Deserialize)]
struct SlackBookmarkWire {
    id: String,
    #[serde(rename = "type")]
    source_type: String,
    #[serde(default)]
    parent_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    link: Option<String>,
    #[serde(default)]
    icon_url: Option<String>,
    #[serde(default)]
    entity_id: Option<String>,
}
