use std::{
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock},
};

use crate::model::SlackAttachmentMedia;
use crate::model::SlackPreparedMediaSource;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

use crate::live::api::SlackApiClient;

mod server;

use server::SlackMediaServer;

const SLACK_MEDIA_CAPABILITY_BYTES: usize = 32;

type SlackMediaCapabilityRegistry = HashMap<String, Arc<SlackMediaCapability>>;

#[derive(Clone)]
pub(super) struct SlackMediaProxy {
    inner: Arc<SlackMediaProxyInner>,
}

struct SlackMediaProxyInner {
    server: OnceLock<Result<SlackMediaServer, String>>,
    state: Arc<SlackMediaProxyState>,
}

pub(super) struct SlackMediaProxyState {
    pub(super) api: SlackApiClient,
    sources_by_token: RwLock<SlackMediaCapabilityRegistry>,
}

pub(super) struct SlackMediaCapability {
    source: SlackAttachmentMedia,
    content_profile: SlackMediaContentProfile,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum SlackMediaContentProfile {
    Audio,
    AudioMp4,
    Video,
}

impl SlackMediaProxy {
    pub(super) fn new(api: SlackApiClient) -> Self {
        Self {
            inner: Arc::new(SlackMediaProxyInner {
                server: OnceLock::new(),
                state: Arc::new(SlackMediaProxyState {
                    api,
                    sources_by_token: RwLock::new(HashMap::new()),
                }),
            }),
        }
    }

    pub(super) fn prepare(
        &self,
        media: &SlackAttachmentMedia,
        declared_mimetype: &str,
    ) -> Result<SlackPreparedMediaSource, String> {
        let capability = Arc::new(SlackMediaCapability::parse(media, declared_mimetype)?);
        let server = self
            .inner
            .server
            .get_or_init(|| SlackMediaServer::start(self.inner.state.clone()))
            .as_ref()
            .map_err(Clone::clone)?;
        let mut sources = self
            .inner
            .state
            .sources_by_token
            .write()
            .map_err(|_| "Slack media proxy source registry poisoned".to_string())?;
        let token = slack_media_capability_token(&sources)?;
        sources.insert(token.clone(), capability);
        SlackPreparedMediaSource::from_loopback_url(format!(
            "http://{}/media/{token}",
            server.local_addr()
        ))
    }

    pub(super) fn release(&self, source: &SlackPreparedMediaSource) -> Result<(), String> {
        let server = self
            .inner
            .server
            .get()
            .ok_or_else(|| "Slack media proxy has not started".to_string())?
            .as_ref()
            .map_err(Clone::clone)?;
        let prefix = format!("http://{}/media/", server.local_addr());
        let token = source
            .url()
            .strip_prefix(&prefix)
            .filter(|token| !token.is_empty() && !token.contains('/'))
            .ok_or_else(|| {
                "prepared Slack media source does not belong to this runtime".to_string()
            })?;
        let capability = self
            .inner
            .state
            .sources_by_token
            .write()
            .map_err(|_| "Slack media proxy source registry poisoned".to_string())?
            .remove(token);
        drop(capability);
        Ok(())
    }
}

impl SlackMediaProxyState {
    fn source(&self, token: &str) -> Result<Option<Arc<SlackMediaCapability>>, String> {
        self.sources_by_token
            .read()
            .map_err(|_| "Slack media proxy source registry poisoned".to_string())
            .map(|sources| sources.get(token).cloned())
    }
}

impl SlackMediaCapability {
    fn parse(media: &SlackAttachmentMedia, declared_mimetype: &str) -> Result<Self, String> {
        let content_profile = match (media.kind(), declared_mimetype) {
            (crate::model::SlackAttachmentMediaKind::Audio, "audio/mp4") => {
                SlackMediaContentProfile::AudioMp4
            }
            (crate::model::SlackAttachmentMediaKind::Audio, mimetype)
                if mimetype.starts_with("audio/") =>
            {
                SlackMediaContentProfile::Audio
            }
            (crate::model::SlackAttachmentMediaKind::Video, mimetype)
                if mimetype.starts_with("video/") =>
            {
                SlackMediaContentProfile::Video
            }
            (kind, mimetype) => {
                return Err(format!(
                    "Slack media kind {kind:?} does not match declared MIME type {mimetype}"
                ));
            }
        };
        Ok(Self {
            source: media.clone(),
            content_profile,
        })
    }

    pub(super) fn source(&self) -> &SlackAttachmentMedia {
        &self.source
    }

    pub(super) fn content_profile(&self) -> SlackMediaContentProfile {
        self.content_profile
    }
}

impl SlackMediaContentProfile {
    pub(super) fn accepts_response_content_type(self, content_type: &str) -> bool {
        match self {
            Self::Audio => content_type.starts_with("audio/"),
            Self::AudioMp4 => content_type.starts_with("audio/") || content_type == "video/mp4",
            Self::Video => content_type.starts_with("video/"),
        }
    }
}

fn slack_media_capability_token(sources: &SlackMediaCapabilityRegistry) -> Result<String, String> {
    loop {
        let mut bytes = [0_u8; SLACK_MEDIA_CAPABILITY_BYTES];
        getrandom::fill(&mut bytes)
            .map_err(|error| format!("failed to generate Slack media capability: {error}"))?;
        let candidate = URL_SAFE_NO_PAD.encode(bytes);
        if !sources.contains_key(&candidate) {
            return Ok(candidate);
        }
    }
}
