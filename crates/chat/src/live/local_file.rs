use std::{
    fs::File,
    io::{Error as IoError, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::model::{
    SlackFileUploadMetadata, SlackLocalFileApi, SlackUploadContent, SlackUploadFile,
};

pub fn production_slack_local_file_api() -> Arc<dyn SlackLocalFileApi> {
    Arc::new(ProductionSlackLocalFileApi)
}

struct ProductionSlackLocalFileApi;

impl SlackLocalFileApi for ProductionSlackLocalFileApi {
    fn open_upload(&self, path: PathBuf) -> Result<SlackUploadFile, String> {
        let name = upload_name(&path)?;
        let mimetype = upload_mimetype(&path);
        open_upload(path, name, mimetype)
    }

    fn open_named_upload(
        &self,
        path: PathBuf,
        name: String,
        mimetype: String,
    ) -> Result<SlackUploadFile, String> {
        open_upload(path, name, mimetype)
    }

    fn default_download_directory(&self) -> Result<PathBuf, String> {
        dirs::download_dir().ok_or_else(|| {
            "The operating system did not provide a Downloads directory.".to_string()
        })
    }

    fn save_video_file(&self, source: &Path, mut destination: PathBuf) -> Result<(), String> {
        destination.set_extension("mp4");
        std::fs::copy(source, &destination)
            .map(|_| ())
            .map_err(|error| {
                format!(
                    "failed to save video clip to {}: {error}",
                    destination.display()
                )
            })
    }
}

fn open_upload(path: PathBuf, name: String, mimetype: String) -> Result<SlackUploadFile, String> {
    let canonical_path = std::fs::canonicalize(&path).map_err(|error| {
        format!(
            "failed to resolve Slack upload source {}: {error}",
            path.display()
        )
    })?;
    let file = File::open(&canonical_path).map_err(|error| {
        format!(
            "failed to open Slack upload source {}: {error}",
            canonical_path.display()
        )
    })?;
    let file_metadata = file.metadata().map_err(|error| {
        format!(
            "failed to inspect Slack upload source {}: {error}",
            canonical_path.display()
        )
    })?;
    if !file_metadata.is_file() {
        return Err(format!(
            "Slack upload source {} must be a regular file",
            canonical_path.display()
        ));
    }
    let metadata = SlackFileUploadMetadata::parse(name, mimetype, file_metadata.len())?;
    let identity = upload_identity(&canonical_path, &file_metadata);
    let content = Arc::new(SlackUploadPath {
        #[cfg(any(unix, windows))]
        file,
        #[cfg(not(any(unix, windows)))]
        file: std::sync::Mutex::new(file),
    });
    Ok(SlackUploadFile::from_content(metadata, identity, content))
}

struct SlackUploadPath {
    #[cfg(any(unix, windows))]
    file: File,
    #[cfg(not(any(unix, windows)))]
    file: std::sync::Mutex<File>,
}

impl SlackUploadContent for SlackUploadPath {
    fn open_reader(&self) -> Result<Box<dyn Read + Send>, String> {
        Ok(Box::new(SlackUploadPathReader {
            source: Arc::new(self.try_clone()?),
            offset: 0,
        }))
    }
}

impl SlackUploadPath {
    fn try_clone(&self) -> Result<Self, String> {
        #[cfg(any(unix, windows))]
        let file = self
            .file
            .try_clone()
            .map_err(|error| format!("failed to clone Slack upload source handle: {error}"))?;
        #[cfg(not(any(unix, windows)))]
        let file = self
            .file
            .lock()
            .map_err(|_| "Slack upload source lock was poisoned".to_string())?
            .try_clone()
            .map_err(|error| format!("failed to clone Slack upload source handle: {error}"))?;
        Ok(Self {
            #[cfg(any(unix, windows))]
            file,
            #[cfg(not(any(unix, windows)))]
            file: std::sync::Mutex::new(file),
        })
    }
}

struct SlackUploadPathReader {
    source: Arc<SlackUploadPath>,
    offset: u64,
}

impl Read for SlackUploadPathReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = read_file_at(&self.source, buffer, self.offset)?;
        self.offset = self
            .offset
            .checked_add(
                u64::try_from(read).map_err(|error| std::io::Error::other(error.to_string()))?,
            )
            .ok_or_else(|| IoError::other("Slack upload file reader offset overflowed"))?;
        Ok(read)
    }
}

#[cfg(unix)]
fn read_file_at(
    source: &SlackUploadPath,
    buffer: &mut [u8],
    offset: u64,
) -> std::io::Result<usize> {
    use std::os::unix::fs::FileExt as _;

    source.file.read_at(buffer, offset)
}

#[cfg(windows)]
fn read_file_at(
    source: &SlackUploadPath,
    buffer: &mut [u8],
    offset: u64,
) -> std::io::Result<usize> {
    use std::os::windows::fs::FileExt as _;

    source.file.seek_read(buffer, offset)
}

#[cfg(not(any(unix, windows)))]
fn read_file_at(
    source: &SlackUploadPath,
    buffer: &mut [u8],
    offset: u64,
) -> std::io::Result<usize> {
    use std::io::{Seek as _, SeekFrom};

    let mut file = source
        .file
        .lock()
        .map_err(|_| IoError::other("Slack upload source lock was poisoned"))?;
    file.seek(SeekFrom::Start(offset))?;
    file.read(buffer)
}

fn upload_name(path: &Path) -> Result<String, String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "failed to determine attachment filename for {}",
                path.display()
            )
        })
}

fn upload_mimetype(path: &Path) -> String {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase());
    match extension.as_deref() {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        Some("pdf") => "application/pdf",
        Some("txt") | Some("md") | Some("log") => "text/plain",
        Some("json") => "application/json",
        Some("csv") => "text/csv",
        Some("zip") => "application/zip",
        Some("mp4") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("m4v") => "video/x-m4v",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("avi") => "video/x-msvideo",
        Some("m4a") => "audio/mp4",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn upload_identity(path: &Path, metadata: &std::fs::Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        format!("{}:{}:{}", path.display(), metadata.dev(), metadata.ino())
    }
    #[cfg(not(unix))]
    {
        path.display().to_string()
    }
}
