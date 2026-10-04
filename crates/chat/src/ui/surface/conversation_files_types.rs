use std::{collections::HashSet, sync::Arc};

use chrono::{DateTime, Utc};
use gpui::SharedString;

use crate::ui::{
    SlackConversationFilesFilter, SlackConversationFilesPage, SlackConversationFilesSnapshot,
    SlackConversationFilesSort, SlackConversationLinkItem, SlackFileItem, SlackFileMode,
};

use super::{slack_file_visual, SlackFileVisual};

pub(crate) const SLACK_CONVERSATION_FILES_CONTROL_HEIGHT: f32 = 41.0;
pub(crate) const SLACK_CONVERSATION_FILES_MEDIA_HEADER_HEIGHT: f32 = 43.0;
pub(crate) const SLACK_CONVERSATION_FILES_MEDIA_PREVIEW_HEIGHT: f32 = 141.0;
pub(crate) const SLACK_CONVERSATION_FILES_MEDIA_GRID_HEIGHT: f32 = 156.0;
pub(crate) const SLACK_CONVERSATION_FILES_ROW_HEIGHT: f32 = 62.0;
pub(crate) const SLACK_CONVERSATION_FILES_SPACER_HEIGHT: f32 = 16.0;
pub(crate) const SLACK_CONVERSATION_FILES_STATE_HEIGHT: f32 = 434.0;
pub(crate) const SLACK_CONVERSATION_MEDIA_COLUMNS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackConversationFilesMenu {
    Sort,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackConversationFilesRowVisual {
    File(SlackFileVisual),
    Link,
}

impl SlackConversationFilesRowVisual {
    pub(crate) fn tile_fill(self) -> u32 {
        match self {
            Self::File(visual) => visual.tile_fill(),
            Self::Link => 0xdddddd,
        }
    }

    pub(crate) fn glyph(self) -> &'static str {
        match self {
            Self::File(visual) => visual.glyph(),
            Self::Link => "↗",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackConversationFilesRow {
    pub(crate) id: SharedString,
    pub(crate) element_id: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) title: SharedString,
    pub(crate) metadata: SharedString,
    pub(crate) link_url: SharedString,
    pub(crate) thumbnail_url: Option<SharedString>,
    pub(crate) visual: SlackConversationFilesRowVisual,
    pub(crate) timestamp_seconds: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackConversationFilesExplorerRow {
    Controls,
    MediaHeader,
    MediaGrid {
        start: usize,
        end: usize,
        preview: bool,
    },
    Spacer,
    Result {
        source: SlackConversationFilesFilter,
        index: usize,
    },
    Loading,
    Empty,
    Error,
}

impl SlackConversationFilesExplorerRow {
    pub(crate) fn height(self) -> f32 {
        match self {
            Self::Controls => SLACK_CONVERSATION_FILES_CONTROL_HEIGHT,
            Self::MediaHeader => SLACK_CONVERSATION_FILES_MEDIA_HEADER_HEIGHT,
            Self::MediaGrid { preview: true, .. } => SLACK_CONVERSATION_FILES_MEDIA_PREVIEW_HEIGHT,
            Self::MediaGrid { preview: false, .. } => SLACK_CONVERSATION_FILES_MEDIA_GRID_HEIGHT,
            Self::Spacer => SLACK_CONVERSATION_FILES_SPACER_HEIGHT,
            Self::Result { .. } => SLACK_CONVERSATION_FILES_ROW_HEIGHT,
            Self::Loading | Self::Empty | Self::Error => SLACK_CONVERSATION_FILES_STATE_HEIGHT,
        }
    }
}

#[derive(Clone, Default)]
pub(crate) struct SlackConversationFilesCache {
    pub(crate) files: Arc<[SlackConversationFilesRow]>,
    pub(crate) media: Arc<[SlackConversationFilesRow]>,
    pub(crate) links: Arc<[SlackConversationFilesRow]>,
    pub(crate) all: Arc<[SlackConversationFilesRow]>,
    all_explorer_rows: Arc<[SlackConversationFilesExplorerRow]>,
    files_explorer_rows: Arc<[SlackConversationFilesExplorerRow]>,
    media_explorer_rows: Arc<[SlackConversationFilesExplorerRow]>,
    links_explorer_rows: Arc<[SlackConversationFilesExplorerRow]>,
    pub(crate) files_loaded: bool,
    pub(crate) media_loaded: bool,
    pub(crate) links_loaded: bool,
    pub(crate) files_next_page: Option<u32>,
    pub(crate) media_next_page: Option<u32>,
    pub(crate) links_next_page: Option<u32>,
}

impl SlackConversationFilesCache {
    pub(crate) fn rows(
        &self,
        filter: SlackConversationFilesFilter,
    ) -> &Arc<[SlackConversationFilesRow]> {
        match filter {
            SlackConversationFilesFilter::All => &self.all,
            SlackConversationFilesFilter::Files => &self.files,
            SlackConversationFilesFilter::Media => &self.media,
            SlackConversationFilesFilter::Links => &self.links,
        }
    }

    pub(crate) fn explorer_rows(
        &self,
        filter: SlackConversationFilesFilter,
    ) -> &Arc<[SlackConversationFilesExplorerRow]> {
        match filter {
            SlackConversationFilesFilter::All => &self.all_explorer_rows,
            SlackConversationFilesFilter::Files => &self.files_explorer_rows,
            SlackConversationFilesFilter::Media => &self.media_explorer_rows,
            SlackConversationFilesFilter::Links => &self.links_explorer_rows,
        }
    }

    pub(crate) fn loaded(&self, filter: SlackConversationFilesFilter) -> bool {
        match filter {
            SlackConversationFilesFilter::All => {
                self.files_loaded && self.media_loaded && self.links_loaded
            }
            SlackConversationFilesFilter::Files => self.files_loaded,
            SlackConversationFilesFilter::Media => self.media_loaded,
            SlackConversationFilesFilter::Links => self.links_loaded,
        }
    }

    fn rebuild_explorer_rows(&mut self, searching: bool) {
        self.all_explorer_rows =
            prepare_explorer_rows(self, SlackConversationFilesFilter::All, searching);
        self.files_explorer_rows =
            prepare_explorer_rows(self, SlackConversationFilesFilter::Files, searching);
        self.media_explorer_rows =
            prepare_explorer_rows(self, SlackConversationFilesFilter::Media, searching);
        self.links_explorer_rows =
            prepare_explorer_rows(self, SlackConversationFilesFilter::Links, searching);
    }
}

pub(crate) struct PreparedSlackConversationFilesSnapshot {
    pub(crate) snapshot: SlackConversationFilesSnapshot,
    pub(crate) cache: SlackConversationFilesCache,
}

pub(crate) fn prepare_slack_conversation_files_snapshot(
    snapshot: SlackConversationFilesSnapshot,
    mut cache: SlackConversationFilesCache,
    timezone: chrono_tz::Tz,
) -> Result<PreparedSlackConversationFilesSnapshot, String> {
    if let Some(page) = snapshot.files.as_ref() {
        let rows = prepare_file_page(page, timezone, "file")?;
        cache.files = merge_page_rows(&cache.files, rows, page.pagination.page);
        cache.files_loaded = true;
        cache.files_next_page = page.pagination.next_page();
    }
    if let Some(page) = snapshot.media.as_ref() {
        let rows = prepare_file_page(page, timezone, "media")?;
        cache.media = merge_page_rows(&cache.media, rows, page.pagination.page);
        cache.media_loaded = true;
        cache.media_next_page = page.pagination.next_page();
    }
    if let Some(page) = snapshot.links.as_ref() {
        let rows = prepare_link_page(page, timezone)?;
        cache.links = merge_page_rows(&cache.links, rows, page.pagination.page);
        cache.links_loaded = true;
        cache.links_next_page = page.pagination.next_page();
    }
    cache.all = prepare_all_rows(
        &cache,
        snapshot.sort,
        !snapshot.search_query.trim().is_empty(),
    );
    cache.rebuild_explorer_rows(!snapshot.search_query.trim().is_empty());
    Ok(PreparedSlackConversationFilesSnapshot { snapshot, cache })
}

fn prepare_explorer_rows(
    cache: &SlackConversationFilesCache,
    filter: SlackConversationFilesFilter,
    searching: bool,
) -> Arc<[SlackConversationFilesExplorerRow]> {
    let mut explorer_rows = vec![SlackConversationFilesExplorerRow::Controls];
    if filter == SlackConversationFilesFilter::All && !searching && !cache.media.is_empty() {
        explorer_rows.push(SlackConversationFilesExplorerRow::MediaHeader);
        explorer_rows.push(SlackConversationFilesExplorerRow::MediaGrid {
            start: 0,
            end: cache.media.len().min(SLACK_CONVERSATION_MEDIA_COLUMNS),
            preview: true,
        });
        explorer_rows.push(SlackConversationFilesExplorerRow::Spacer);
    }

    let rows = cache.rows(filter);
    if filter == SlackConversationFilesFilter::Media {
        explorer_rows.extend(
            (0..rows.len())
                .step_by(SLACK_CONVERSATION_MEDIA_COLUMNS)
                .map(|start| SlackConversationFilesExplorerRow::MediaGrid {
                    start,
                    end: (start + SLACK_CONVERSATION_MEDIA_COLUMNS).min(rows.len()),
                    preview: false,
                }),
        );
    } else {
        explorer_rows.extend(rows.iter().enumerate().map(|(index, _)| {
            SlackConversationFilesExplorerRow::Result {
                source: filter,
                index,
            }
        }));
    }

    let has_media_preview =
        filter == SlackConversationFilesFilter::All && !searching && !cache.media.is_empty();
    if rows.is_empty() && !has_media_preview {
        explorer_rows.push(SlackConversationFilesExplorerRow::Empty);
    }
    explorer_rows.into()
}

fn prepare_file_page(
    page: &SlackConversationFilesPage<SlackFileItem>,
    timezone: chrono_tz::Tz,
    source_label: &str,
) -> Result<Arc<[SlackConversationFilesRow]>, String> {
    let mut rows = page
        .items
        .iter()
        .map(|file| prepare_file_row(file, timezone, source_label))
        .collect::<Result<Vec<_>, _>>()?;
    retain_unique_rows(&mut rows);
    Ok(rows.into())
}

fn prepare_file_row(
    file: &SlackFileItem,
    timezone: chrono_tz::Tz,
    source_label: &str,
) -> Result<SlackConversationFilesRow, String> {
    let timestamp_seconds = i64::try_from(file.updated.unwrap_or(file.created))
        .map_err(|_| format!("Slack file {} returned a timestamp outside i64", file.id))?;
    let timestamp = DateTime::<Utc>::from_timestamp(timestamp_seconds, 0)
        .ok_or_else(|| format!("Slack file {} returned an invalid timestamp", file.id))?
        .with_timezone(&timezone);
    let metadata = format!("{} · {}", file.owner_label, timestamp.format("%b %-d, %Y"));
    let link_url = match file.mode {
        SlackFileMode::External => file
            .external_url
            .as_deref()
            .unwrap_or(file.permalink.as_str()),
        SlackFileMode::Hosted | SlackFileMode::List => file.permalink.as_str(),
    };
    let id: SharedString = format!("{source_label}:{}", file.id).into();
    Ok(SlackConversationFilesRow {
        element_id: format!("slack-conversation-{source_label}-{}", file.id).into(),
        accessibility_label: format!("{}, {metadata}", file.title).into(),
        id,
        title: file.title.clone().into(),
        metadata: metadata.into(),
        link_url: link_url.to_string().into(),
        thumbnail_url: file.thumbnail_url.clone().map(Into::into),
        visual: SlackConversationFilesRowVisual::File(slack_file_visual(file)),
        timestamp_seconds,
    })
}

fn prepare_link_page(
    page: &SlackConversationFilesPage<SlackConversationLinkItem>,
    timezone: chrono_tz::Tz,
) -> Result<Arc<[SlackConversationFilesRow]>, String> {
    let mut rows = page
        .items
        .iter()
        .map(|link| prepare_link_row(link, timezone))
        .collect::<Result<Vec<_>, _>>()?;
    retain_unique_rows(&mut rows);
    Ok(rows.into())
}

fn prepare_link_row(
    link: &SlackConversationLinkItem,
    timezone: chrono_tz::Tz,
) -> Result<SlackConversationFilesRow, String> {
    let timestamp_seconds = slack_link_timestamp_seconds(&link.timestamp)?;
    let timestamp = DateTime::<Utc>::from_timestamp(timestamp_seconds, 0)
        .ok_or_else(|| format!("Slack link {} returned an invalid timestamp", link.url))?
        .with_timezone(&timezone);
    let host = url::Url::parse(&link.url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "Link".to_string());
    let metadata = format!("{host} · {}", timestamp.format("%b %-d, %Y"));
    let id: SharedString = format!("link:{}:{}", link.timestamp, link.url).into();
    Ok(SlackConversationFilesRow {
        element_id: format!("slack-conversation-link-{}-{}", link.timestamp, link.url).into(),
        accessibility_label: format!("{}, {metadata}", link.title).into(),
        id,
        title: link.title.clone().into(),
        metadata: metadata.into(),
        link_url: link.url.clone().into(),
        thumbnail_url: link.icon_url.clone().map(Into::into),
        visual: SlackConversationFilesRowVisual::Link,
        timestamp_seconds,
    })
}

fn slack_link_timestamp_seconds(timestamp: &str) -> Result<i64, String> {
    let whole_seconds = timestamp
        .split_once('.')
        .map_or(timestamp, |(whole, _)| whole);
    let value = whole_seconds.parse::<i64>().map_err(|_| {
        format!("Slack link timestamp {timestamp:?} is not an integer epoch timestamp")
    })?;
    Ok(if value > 99_999_999_999 {
        value / 1_000
    } else {
        value
    })
}

fn merge_page_rows(
    existing: &Arc<[SlackConversationFilesRow]>,
    page: Arc<[SlackConversationFilesRow]>,
    page_number: u32,
) -> Arc<[SlackConversationFilesRow]> {
    if page_number == 1 {
        return page;
    }
    let mut seen = existing
        .iter()
        .map(|row| row.id.clone())
        .collect::<HashSet<_>>();
    let mut rows = Vec::with_capacity(existing.len() + page.len());
    rows.extend(existing.iter().cloned());
    rows.extend(
        page.iter()
            .filter(|row| seen.insert(row.id.clone()))
            .cloned(),
    );
    rows.into()
}

fn prepare_all_rows(
    cache: &SlackConversationFilesCache,
    sort: SlackConversationFilesSort,
    include_media: bool,
) -> Arc<[SlackConversationFilesRow]> {
    let mut rows = Vec::with_capacity(
        cache.files.len() + cache.links.len() + usize::from(include_media) * cache.media.len(),
    );
    rows.extend(cache.files.iter().cloned());
    if include_media {
        rows.extend(cache.media.iter().cloned());
    }
    rows.extend(cache.links.iter().cloned());
    match sort {
        SlackConversationFilesSort::Oldest => {
            rows.sort_by_key(|row| row.timestamp_seconds);
        }
        SlackConversationFilesSort::Newest | SlackConversationFilesSort::Relevant => {
            // Slack interleaves files, media, and links by timestamp for every
            // non-oldest All sort, including a Relevant search.
            rows.sort_by_key(|row| std::cmp::Reverse(row.timestamp_seconds));
        }
    }
    retain_unique_rows(&mut rows);
    rows.into()
}

fn retain_unique_rows(rows: &mut Vec<SlackConversationFilesRow>) {
    let mut seen = HashSet::with_capacity(rows.len());
    rows.retain(|row| seen.insert(row.id.clone()));
}
