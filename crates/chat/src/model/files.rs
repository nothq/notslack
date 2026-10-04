use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

const SLACK_FILE_ID_MAX_BYTES: usize = 255;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SlackFileStagingOperationId(String);

impl SlackFileStagingOperationId {
    pub fn parse(value: String) -> Result<Self, String> {
        let operation_id = uuid::Uuid::parse_str(&value)
            .map_err(|error| format!("Slack file-staging operation id must be a UUID: {error}"))?;
        if operation_id.hyphenated().to_string() != value {
            return Err(
                "Slack file-staging operation id must use canonical lowercase hyphenated UUID form"
                    .to_string(),
            );
        }
        Ok(Self(value))
    }

    pub fn generate() -> Self {
        Self(uuid::Uuid::new_v4().hyphenated().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SlackFileStagingOperationId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SlackFileStagingOperationId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SlackFileId(String);

impl SlackFileId {
    pub fn parse(value: String) -> Result<Self, String> {
        if value.is_empty()
            || value.len() > SLACK_FILE_ID_MAX_BYTES
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        {
            return Err(
                "Slack file id must be a bounded ASCII alphanumeric, hyphen, or underscore value"
                    .to_string(),
            );
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SlackFileId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SlackFileId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackFileUploadMetadata {
    name: String,
    mimetype: String,
    size_bytes: u64,
}

impl SlackFileUploadMetadata {
    pub fn parse(name: String, mimetype: String, size_bytes: u64) -> Result<Self, String> {
        if name.is_empty() || name.trim() != name || name.chars().any(char::is_control) {
            return Err(
                "Slack upload filename must be non-empty, unpadded, and contain no control characters"
                    .to_string(),
            );
        }
        let valid_mimetype = mimetype
            .split_once('/')
            .is_some_and(|(kind, subtype)| !kind.is_empty() && !subtype.is_empty());
        if !valid_mimetype
            || mimetype.trim() != mimetype
            || !mimetype.is_ascii()
            || mimetype
                .bytes()
                .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
        {
            return Err(
                "Slack upload mimetype must be an unpadded ASCII media type without whitespace or control characters"
                    .to_string(),
            );
        }
        Ok(Self {
            name,
            mimetype,
            size_bytes,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn mimetype(&self) -> &str {
        &self.mimetype
    }

    pub fn size_bytes(&self) -> u64 {
        self.size_bytes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackAllocatedUpload {
    file_id: SlackFileId,
    metadata: SlackFileUploadMetadata,
}

impl SlackAllocatedUpload {
    pub fn new(file_id: SlackFileId, metadata: SlackFileUploadMetadata) -> Self {
        Self { file_id, metadata }
    }

    pub fn file_id(&self) -> &SlackFileId {
        &self.file_id
    }

    pub fn name(&self) -> &str {
        self.metadata.name()
    }

    pub fn mimetype(&self) -> &str {
        self.metadata.mimetype()
    }

    pub fn size_bytes(&self) -> u64 {
        self.metadata.size_bytes()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackStagedFile {
    operation_id: SlackFileStagingOperationId,
    allocated: SlackAllocatedUpload,
}

impl SlackStagedFile {
    pub fn new(operation_id: SlackFileStagingOperationId, allocated: SlackAllocatedUpload) -> Self {
        Self {
            operation_id,
            allocated,
        }
    }

    pub fn operation_id(&self) -> &SlackFileStagingOperationId {
        &self.operation_id
    }

    pub fn allocated(&self) -> &SlackAllocatedUpload {
        &self.allocated
    }

    pub fn file_id(&self) -> &SlackFileId {
        self.allocated.file_id()
    }

    pub fn name(&self) -> &str {
        self.allocated.name()
    }

    pub fn mimetype(&self) -> &str {
        self.allocated.mimetype()
    }

    pub fn size_bytes(&self) -> u64 {
        self.allocated.size_bytes()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlackFilesBrowserSessionId(String);

impl SlackFilesBrowserSessionId {
    pub fn new(value: String) -> Result<Self, String> {
        if value.trim().is_empty() {
            return Err("Slack Files browser session id must not be empty".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackFilesScope {
    #[default]
    All,
    CreatedByYou,
    SharedWithYou,
}

impl SlackFilesScope {
    pub const ALL: [Self; 3] = [Self::All, Self::CreatedByYou, Self::SharedWithYou];

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::CreatedByYou => "Created by you",
            Self::SharedWithYou => "Shared with you",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackFilesTypeFilter {
    Lists,
    CanvasesAndDocuments,
    Spreadsheets,
    Presentations,
    Pdfs,
    Audio,
    Videos,
    Images,
    Snippets,
    Emails,
}

impl SlackFilesTypeFilter {
    pub const ALL: [Self; 10] = [
        Self::Lists,
        Self::CanvasesAndDocuments,
        Self::Spreadsheets,
        Self::Presentations,
        Self::Pdfs,
        Self::Audio,
        Self::Videos,
        Self::Images,
        Self::Snippets,
        Self::Emails,
    ];

    pub const DEFAULT: [Self; 5] = [
        Self::Lists,
        Self::CanvasesAndDocuments,
        Self::Spreadsheets,
        Self::Presentations,
        Self::Pdfs,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Lists => "Lists",
            Self::CanvasesAndDocuments => "Canvases and Documents",
            Self::Spreadsheets => "Spreadsheets",
            Self::Presentations => "Presentations",
            Self::Pdfs => "PDFs",
            Self::Audio => "Audio",
            Self::Videos => "Videos",
            Self::Images => "Images",
            Self::Snippets => "Snippets",
            Self::Emails => "Emails",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackFilesSort {
    #[default]
    RecentlyViewed,
    LastUpdated,
}

impl SlackFilesSort {
    pub const ALL: [Self; 2] = [Self::RecentlyViewed, Self::LastUpdated];

    pub fn label(self) -> &'static str {
        match self {
            Self::RecentlyViewed => "Recently viewed",
            Self::LastUpdated => "Last updated",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackFilesRequest {
    pub team_id: String,
    pub self_user_id: String,
    pub browser_session_id: SlackFilesBrowserSessionId,
    pub search_query: String,
    pub scope: SlackFilesScope,
    pub type_filters: Vec<SlackFilesTypeFilter>,
    pub sort: SlackFilesSort,
    pub page: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackFileMode {
    Hosted,
    External,
    List,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackFileItem {
    pub id: String,
    pub owner_user_id: String,
    pub owner_label: String,
    pub title: String,
    pub filetype: String,
    pub mimetype: String,
    pub mode: SlackFileMode,
    pub created: u64,
    pub updated: Option<u64>,
    pub last_read: u64,
    pub permalink: String,
    pub external_url: Option<String>,
    pub download_url: Option<String>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackFilesPagination {
    pub total_count: u32,
    pub per_page: u32,
    pub page_count: u32,
    pub first: u32,
    pub last: u32,
    pub page: u32,
}

impl SlackFilesPagination {
    pub fn next_page(&self) -> Option<u32> {
        (self.page < self.page_count).then(|| self.page + 1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackFilesSnapshot {
    pub query: String,
    pub pagination: SlackFilesPagination,
    pub items: Vec<SlackFileItem>,
}
