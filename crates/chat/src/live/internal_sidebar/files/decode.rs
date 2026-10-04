use crate::model::{
    SlackFileItem, SlackFileMode, SlackFilesPagination, SlackFilesRequest, SlackFilesSnapshot,
};
use serde::Deserialize;

use super::{FILES_PAGE_SIZE, SEARCH_MODULES_FILES};

type DecodedSlackFilesPage = (SlackFilesPaginationWire, Vec<SlackFileItem>);

pub(super) fn decode_files_response(
    request: &SlackFilesRequest,
    expected_query: &str,
    body: &str,
) -> Result<SlackFilesSnapshot, String> {
    let (pagination, items) = decode_files_page(request.page, expected_query, body)?;
    Ok(SlackFilesSnapshot {
        query: expected_query.to_string(),
        pagination: pagination.into(),
        items,
    })
}

pub(super) fn decode_files_page(
    requested_page: u32,
    expected_query: &str,
    body: &str,
) -> Result<DecodedSlackFilesPage, String> {
    let response = serde_json::from_str::<SlackFilesResponseWire>(body).map_err(|error| {
        format!("failed to decode Slack {SEARCH_MODULES_FILES} response: {error}")
    })?;
    if !response.ok {
        return Err(format!(
            "Slack {SEARCH_MODULES_FILES} failed: {}",
            response.error.as_deref().unwrap_or("unknown_error")
        ));
    }
    if response.module.as_deref() != Some("files") {
        return Err(format!(
            "Slack {SEARCH_MODULES_FILES} response returned module {}",
            response.module.as_deref().unwrap_or("<missing>")
        ));
    }
    let query = response
        .query
        .ok_or_else(|| format!("Slack {SEARCH_MODULES_FILES} response omitted query"))?;
    if query != expected_query {
        return Err(format!(
            "Slack {SEARCH_MODULES_FILES} echoed query {query:?} for requested {expected_query:?}"
        ));
    }
    let pagination = response
        .pagination
        .ok_or_else(|| format!("Slack {SEARCH_MODULES_FILES} response omitted pagination"))?;
    if pagination.page != requested_page {
        return Err(format!(
            "Slack {SEARCH_MODULES_FILES} returned page {} for requested page {}",
            pagination.page, requested_page
        ));
    }
    if pagination.per_page != FILES_PAGE_SIZE {
        return Err(format!(
            "Slack {SEARCH_MODULES_FILES} returned per_page {} for requested count {FILES_PAGE_SIZE}",
            pagination.per_page
        ));
    }
    let items = response
        .items
        .ok_or_else(|| format!("Slack {SEARCH_MODULES_FILES} response omitted items"))?;
    if items.len() > FILES_PAGE_SIZE as usize {
        return Err(format!(
            "Slack {SEARCH_MODULES_FILES} returned {} items for a {FILES_PAGE_SIZE}-item page",
            items.len()
        ));
    }
    let items = items
        .into_iter()
        .map(SlackFileWireItem::into_item)
        .collect::<Result<Vec<_>, _>>()?;
    Ok((pagination, items))
}

#[derive(Deserialize)]
struct SlackFilesResponseWire {
    ok: bool,
    error: Option<String>,
    pagination: Option<SlackFilesPaginationWire>,
    query: Option<String>,
    #[serde(rename = "filters")]
    _filters: serde::de::IgnoredAny,
    #[serde(rename = "ai_filters")]
    _ai_filters: serde::de::IgnoredAny,
    #[serde(rename = "manual_filters")]
    _manual_filters: serde::de::IgnoredAny,
    module: Option<String>,
    items: Option<Vec<SlackFileWireItem>>,
}

#[derive(Deserialize)]
pub(super) struct SlackFilesPaginationWire {
    pub(super) total_count: u32,
    per_page: u32,
    pub(super) page_count: u32,
    first: u32,
    last: u32,
    pub(super) page: u32,
}

impl From<SlackFilesPaginationWire> for SlackFilesPagination {
    fn from(pagination: SlackFilesPaginationWire) -> Self {
        Self {
            total_count: pagination.total_count,
            per_page: pagination.per_page,
            page_count: pagination.page_count,
            first: pagination.first,
            last: pagination.last,
            page: pagination.page,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
enum SlackFileWireItem {
    Hosted {
        #[serde(flatten)]
        common: SlackFileCommonWire,
        url_private_download: String,
    },
    External {
        #[serde(flatten)]
        common: SlackFileCommonWire,
        external_url: String,
    },
    List {
        #[serde(flatten)]
        common: SlackFileCommonWire,
        updated: u64,
        url_private_download: String,
    },
}

struct SlackFileWireParts {
    common: SlackFileCommonWire,
    mode: SlackFileMode,
    updated: Option<u64>,
    external_url: Option<String>,
    download_url: Option<String>,
}

impl SlackFileWireItem {
    fn into_item(self) -> Result<SlackFileItem, String> {
        let parts = self.into_parts();
        parts.common.into_item(
            parts.mode,
            parts.updated,
            parts.external_url,
            parts.download_url,
        )
    }

    fn into_parts(self) -> SlackFileWireParts {
        match self {
            Self::Hosted {
                common,
                url_private_download,
            } => SlackFileWireParts {
                common,
                mode: SlackFileMode::Hosted,
                updated: None,
                external_url: None,
                download_url: Some(url_private_download),
            },
            Self::External {
                common,
                external_url,
            } => SlackFileWireParts {
                common,
                mode: SlackFileMode::External,
                updated: None,
                external_url: Some(external_url),
                download_url: None,
            },
            Self::List {
                common,
                updated,
                url_private_download,
            } => SlackFileWireParts {
                common,
                mode: SlackFileMode::List,
                updated: Some(updated),
                external_url: None,
                download_url: Some(url_private_download),
            },
        }
    }
}

#[derive(Deserialize)]
struct SlackFileCommonWire {
    id: String,
    iid: String,
    file_access: String,
    filetype: String,
    mimetype: String,
    name: String,
    title: String,
    user: String,
    username: String,
    created: u64,
    last_read: u64,
    permalink: String,
    #[serde(default)]
    thumb_360: Option<String>,
    #[serde(default)]
    thumb_480: Option<String>,
    #[serde(default)]
    thumb_720: Option<String>,
    #[serde(default)]
    thumb_960: Option<String>,
}

impl SlackFileCommonWire {
    fn into_item(
        self,
        mode: SlackFileMode,
        updated: Option<u64>,
        external_url: Option<String>,
        download_url: Option<String>,
    ) -> Result<SlackFileItem, String> {
        self.validate(external_url.as_deref(), download_url.as_deref())?;
        let title = slack_file_title(&self.id, self.title, self.name)?;
        let thumbnail_url = [
            self.thumb_960,
            self.thumb_720,
            self.thumb_480,
            self.thumb_360,
        ]
        .into_iter()
        .flatten()
        .find(|url| !url.trim().is_empty());
        Ok(SlackFileItem {
            id: self.id,
            owner_user_id: self.user,
            owner_label: self.username,
            title,
            filetype: self.filetype,
            mimetype: self.mimetype,
            mode,
            created: self.created,
            updated,
            last_read: self.last_read,
            permalink: self.permalink,
            external_url,
            download_url,
            thumbnail_url,
        })
    }

    fn validate(
        &self,
        external_url: Option<&str>,
        download_url: Option<&str>,
    ) -> Result<(), String> {
        require_nonempty("id", &self.id)?;
        require_nonempty("iid", &self.iid)?;
        require_nonempty("filetype", &self.filetype)?;
        require_nonempty("mimetype", &self.mimetype)?;
        require_nonempty("user", &self.user)?;
        require_nonempty("username", &self.username)?;
        require_nonempty("permalink", &self.permalink)?;
        if self.created == 0 || self.last_read == 0 {
            return Err(format!(
                "Slack {SEARCH_MODULES_FILES} returned zero created/last_read for file {}",
                self.id
            ));
        }
        if self.file_access != "visible" {
            return Err(format!(
                "Slack {SEARCH_MODULES_FILES} returned file {} with unsupported file_access {}",
                self.id, self.file_access
            ));
        }
        if let Some(url) = external_url {
            require_nonempty("external_url", url)?;
        }
        if let Some(url) = download_url {
            require_nonempty("url_private_download", url)?;
        }
        Ok(())
    }
}

fn slack_file_title(id: &str, title: String, name: String) -> Result<String, String> {
    nonempty(title).or_else(|| nonempty(name)).ok_or_else(|| {
        format!("Slack {SEARCH_MODULES_FILES} returned file {id} without a title or name")
    })
}

fn nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn require_nonempty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!(
            "Slack {SEARCH_MODULES_FILES} response returned empty {field}"
        ))
    } else {
        Ok(())
    }
}
