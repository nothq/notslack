use std::{fs::File, path::Path, sync::Arc};

use tempfile::TempPath;

use crate::{MediaCaptureError, VideoClipFailure};

pub(crate) struct PendingVideoFile {
    scratch_movie_path: TempPath,
    mp4_output_path: TempPath,
}

impl PendingVideoFile {
    pub(crate) fn create() -> Result<Self, MediaCaptureError> {
        Ok(Self {
            scratch_movie_path: create_absent_temp_path(".mov")?,
            mp4_output_path: create_absent_temp_path(".mp4")?,
        })
    }

    pub(crate) fn scratch_movie_path(&self) -> &Path {
        self.scratch_movie_path.as_ref()
    }

    pub(crate) fn mp4_output_path(&self) -> &Path {
        self.mp4_output_path.as_ref()
    }

    pub(crate) fn finalize(self) -> Result<CapturedVideoFile, MediaCaptureError> {
        let metadata = std::fs::metadata(self.mp4_output_path()).map_err(finalization_error)?;
        if !metadata.is_file() || metadata.len() == 0 {
            return Err(MediaCaptureError::VideoCaptureFailed(
                VideoClipFailure::Finalization(
                    "AVFoundation did not produce a non-empty regular MP4 file".to_string(),
                ),
            ));
        }
        let Self {
            scratch_movie_path: _,
            mp4_output_path,
        } = self;
        Ok(CapturedVideoFile {
            inner: Arc::new(CapturedVideoFileInner {
                path: mp4_output_path,
                size_bytes: metadata.len(),
            }),
        })
    }
}

fn create_absent_temp_path(suffix: &str) -> Result<TempPath, MediaCaptureError> {
    let file = tempfile::Builder::new()
        .prefix("notslack-video-clip-")
        .suffix(suffix)
        .tempfile()
        .map_err(output_error)?;
    let path = file.into_temp_path();
    let path_ref: &Path = path.as_ref();
    std::fs::remove_file(path_ref).map_err(output_error)?;
    Ok(path)
}

#[derive(Clone)]
pub struct CapturedVideoFile {
    inner: Arc<CapturedVideoFileInner>,
}

struct CapturedVideoFileInner {
    path: TempPath,
    size_bytes: u64,
}

impl CapturedVideoFile {
    pub fn path(&self) -> &Path {
        self.inner.path.as_ref()
    }

    pub fn size_bytes(&self) -> u64 {
        self.inner.size_bytes
    }

    pub fn open(&self) -> std::io::Result<File> {
        File::open(self.path())
    }
}

impl std::fmt::Debug for CapturedVideoFile {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CapturedVideoFile")
            .field("path", &self.path())
            .field("size_bytes", &self.size_bytes())
            .finish_non_exhaustive()
    }
}

fn output_error(error: std::io::Error) -> MediaCaptureError {
    MediaCaptureError::VideoCaptureFailed(VideoClipFailure::Configuration(error.to_string()))
}

fn finalization_error(error: std::io::Error) -> MediaCaptureError {
    MediaCaptureError::VideoCaptureFailed(VideoClipFailure::Finalization(error.to_string()))
}
