use std::{
    io::{Error as IoError, ErrorKind, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::model::SlackFileUploadMetadata;

pub trait SlackUploadContent: Send + Sync + 'static {
    fn open_reader(&self) -> Result<Box<dyn Read + Send>, String>;
}

pub trait SlackLocalFileApi: Send + Sync + 'static {
    fn open_upload(&self, path: PathBuf) -> Result<SlackUploadFile, String>;

    fn open_named_upload(
        &self,
        path: PathBuf,
        name: String,
        mimetype: String,
    ) -> Result<SlackUploadFile, String>;

    fn default_download_directory(&self) -> Result<PathBuf, String>;

    fn save_video_file(&self, source: &Path, destination: PathBuf) -> Result<(), String>;
}

#[derive(Clone)]
pub struct SlackUploadFile {
    metadata: SlackFileUploadMetadata,
    source: SlackUploadSource,
}

impl SlackUploadFile {
    pub fn from_bytes(
        name: impl Into<String>,
        bytes: Arc<[u8]>,
        mimetype: impl Into<String>,
    ) -> Result<Self, String> {
        let size_bytes = u64::try_from(bytes.len())
            .map_err(|error| format!("Slack upload byte length is invalid: {error}"))?;
        let metadata = SlackFileUploadMetadata::parse(name.into(), mimetype.into(), size_bytes)?;
        Ok(Self {
            metadata,
            source: SlackUploadSource::Bytes(bytes),
        })
    }

    pub fn from_content(
        metadata: SlackFileUploadMetadata,
        source_identity: impl Into<String>,
        content: Arc<dyn SlackUploadContent>,
    ) -> Self {
        Self {
            metadata,
            source: SlackUploadSource::Content {
                identity: source_identity.into(),
                content,
            },
        }
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

    pub fn metadata(&self) -> &SlackFileUploadMetadata {
        &self.metadata
    }

    pub fn open_reader(&self) -> Result<SlackUploadReader, String> {
        let source: Box<dyn Read + Send> = match &self.source {
            SlackUploadSource::Bytes(bytes) => Box::new(std::io::Cursor::new(Arc::clone(bytes))),
            SlackUploadSource::Content { content, .. } => content.open_reader()?,
        };
        Ok(SlackUploadReader {
            source,
            remaining: self.size_bytes(),
        })
    }
}

impl std::fmt::Debug for SlackUploadFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SlackUploadFile")
            .field("metadata", &self.metadata)
            .field("source", &self.source)
            .finish()
    }
}

impl PartialEq for SlackUploadFile {
    fn eq(&self, other: &Self) -> bool {
        self.metadata == other.metadata && self.source == other.source
    }
}

impl Eq for SlackUploadFile {}

#[derive(Clone)]
enum SlackUploadSource {
    Bytes(Arc<[u8]>),
    Content {
        identity: String,
        content: Arc<dyn SlackUploadContent>,
    },
}

impl std::fmt::Debug for SlackUploadSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bytes(bytes) => formatter.debug_tuple("Bytes").field(&bytes.len()).finish(),
            Self::Content { identity, .. } => formatter
                .debug_struct("Content")
                .field("identity", identity)
                .finish_non_exhaustive(),
        }
    }
}

impl PartialEq for SlackUploadSource {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Bytes(left), Self::Bytes(right)) => left == right,
            (
                Self::Content { identity: left, .. },
                Self::Content {
                    identity: right, ..
                },
            ) => left == right,
            _ => false,
        }
    }
}

impl Eq for SlackUploadSource {}

pub struct SlackUploadReader {
    source: Box<dyn Read + Send>,
    remaining: u64,
}

impl Read for SlackUploadReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.remaining == 0 || buffer.is_empty() {
            return Ok(0);
        }
        let permitted = usize::try_from(self.remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        let read = self.source.read(&mut buffer[..permitted])?;
        if read == 0 {
            return Err(IoError::new(
                ErrorKind::UnexpectedEof,
                format!(
                    "Slack upload source ended with {} declared bytes remaining",
                    self.remaining
                ),
            ));
        }
        self.remaining -=
            u64::try_from(read).map_err(|error| IoError::new(ErrorKind::InvalidData, error))?;
        Ok(read)
    }
}
