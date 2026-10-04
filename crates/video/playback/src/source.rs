use std::{any::Any, fmt, path::Path, sync::Arc};

use gpui::SharedString;
use url::Url;

use crate::ffmpeg_player::{probe_video_metadata, VideoMetadata};

#[derive(Clone)]
pub struct VideoPlayerSource {
    pub id: SharedString,
    pub title: SharedString,
    pub subtitle: SharedString,
    pub url: String,
    pub start_seconds: Option<u32>,
    playback_start_seconds: Option<f64>,
    playback_end_seconds: Option<f64>,
    retained: Option<Arc<dyn Any + Send + Sync>>,
}

impl fmt::Debug for VideoPlayerSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VideoPlayerSource")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("subtitle", &self.subtitle)
            .field("url", &self.url)
            .field("start_seconds", &self.start_seconds)
            .field("playback_start_seconds", &self.playback_start_seconds)
            .field("playback_end_seconds", &self.playback_end_seconds)
            .field("retained", &self.retained.is_some())
            .finish()
    }
}

impl PartialEq for VideoPlayerSource {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.title == other.title
            && self.subtitle == other.subtitle
            && self.url == other.url
            && self.start_seconds == other.start_seconds
            && self.playback_start_seconds == other.playback_start_seconds
            && self.playback_end_seconds == other.playback_end_seconds
            && match (&self.retained, &other.retained) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
    }
}

impl VideoPlayerSource {
    pub fn new(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        subtitle: impl Into<SharedString>,
        url: impl Into<String>,
        start_seconds: Option<u32>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: subtitle.into(),
            url: url.into(),
            start_seconds,
            playback_start_seconds: None,
            playback_end_seconds: None,
            retained: None,
        }
    }

    pub fn retaining(mut self, value: impl Any + Send + Sync) -> Self {
        self.retained = Some(Arc::new(value));
        self
    }

    pub fn with_playback_interval(
        mut self,
        start_seconds: f64,
        end_seconds: f64,
    ) -> Result<Self, String> {
        if !start_seconds.is_finite()
            || !end_seconds.is_finite()
            || start_seconds < 0.0
            || end_seconds <= start_seconds
        {
            return Err("video playback interval must be finite, nonnegative, and nonempty".into());
        }
        self.playback_start_seconds = Some(start_seconds);
        self.playback_end_seconds = Some(end_seconds);
        Ok(self)
    }

    pub(crate) fn playback_start_seconds(&self) -> Option<f64> {
        self.playback_start_seconds
    }

    pub(crate) fn playback_end_seconds(&self) -> Option<f64> {
        self.playback_end_seconds
    }

    fn retained(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.retained.clone()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VideoFrameFit {
    #[default]
    Contain,
    Cover,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoPlayerConfig {
    pub show_playlist: bool,
    pub show_controls: bool,
    pub muted: bool,
    pub audio_enabled: bool,
    pub looping: bool,
    pub frame_fit: VideoFrameFit,
}

impl Default for VideoPlayerConfig {
    fn default() -> Self {
        Self {
            show_playlist: false,
            show_controls: true,
            muted: false,
            audio_enabled: true,
            looping: false,
            frame_fit: VideoFrameFit::default(),
        }
    }
}

pub(crate) struct PreparedVideoSource {
    uri: Url,
    metadata: Option<VideoMetadata>,
    retained: Option<Arc<dyn Any + Send + Sync>>,
}

impl PreparedVideoSource {
    fn direct(uri: Url) -> Self {
        Self {
            uri,
            metadata: None,
            retained: None,
        }
    }

    fn with_metadata(
        uri: Url,
        metadata: VideoMetadata,
        retained: Option<Arc<dyn Any + Send + Sync>>,
    ) -> Self {
        Self {
            uri,
            metadata: Some(metadata),
            retained,
        }
    }

    pub(crate) fn uri(&self) -> &Url {
        &self.uri
    }

    pub(crate) fn metadata(&self) -> Option<VideoMetadata> {
        self.metadata.clone()
    }

    pub(crate) fn retained(&self) -> Option<Arc<dyn Any + Send + Sync>> {
        self.retained.clone()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VideoSourceKind {
    Native,
    Unsupported(&'static str),
}

pub(crate) fn video_source_kind(url: &str) -> VideoSourceKind {
    match video_url_extension(url).as_deref() {
        Some("zst") => VideoSourceKind::Unsupported("Zstandard telemetry archive"),
        Some(_) | None => VideoSourceKind::Native,
    }
}

pub(crate) fn video_unsupported_label(url: &str) -> Option<&'static str> {
    match video_source_kind(url) {
        VideoSourceKind::Unsupported(label) => Some(label),
        VideoSourceKind::Native => None,
    }
}

fn video_url_extension(url: &str) -> Option<String> {
    let path = url
        .split_once('#')
        .map(|(path, _)| path)
        .unwrap_or(url)
        .split_once('?')
        .map(|(path, _)| path)
        .unwrap_or(url);
    Path::new(path)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
}

fn video_uri(source: &str) -> Result<Url, String> {
    Url::parse(source).or_else(|_| {
        Url::from_file_path(source).map_err(|_| format!("invalid video URL or file path: {source}"))
    })
}

pub(crate) fn prepare_video_source(source: &str) -> Result<PreparedVideoSource, String> {
    video_uri(source).map(PreparedVideoSource::direct)
}

pub(crate) fn prepare_player_source(
    source: &VideoPlayerSource,
) -> Result<PreparedVideoSource, String> {
    let mut prepared = prepare_video_source(source.url.as_str())?;
    prepared.retained = source.retained();
    Ok(prepared)
}

pub(crate) fn prepare_video_source_for_playback(
    source: &VideoPlayerSource,
) -> Result<PreparedVideoSource, String> {
    let uri = video_uri(source.url.as_str())?;
    let metadata = probe_video_metadata(&uri)
        .map_err(|error| format!("failed to inspect video source {uri}: {error}"))?;
    Ok(PreparedVideoSource::with_metadata(
        uri,
        metadata,
        source.retained(),
    ))
}

pub fn video_clock_label(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "0:00".to_string();
    }
    let total_seconds = seconds.round() as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
