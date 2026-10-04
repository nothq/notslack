use std::{collections::HashSet, sync::Arc};

use chrono::{DateTime, Datelike, Utc};
use gpui::SharedString;

use crate::ui::{SlackFileItem, SlackFileMode, SlackFilesSnapshot, SlackFilesTypeFilter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackFilesMenu {
    Types,
    Sort,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlackFilesSidebarSelection {
    #[default]
    All,
    Type(SlackFilesTypeFilter),
    RecentlyViewed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackFileVisual {
    Pdf,
    List,
    Document,
    Spreadsheet,
    Presentation,
    Audio,
    Video,
    Image,
    Snippet,
    Email,
    File,
}

impl SlackFileVisual {
    pub(crate) fn tile_fill(self) -> u32 {
        match self {
            Self::Pdf => 0xe01e5a,
            Self::List => 0xd89b00,
            Self::Document => 0x4285f4,
            Self::Spreadsheet => 0x188038,
            Self::Presentation => 0xf9ab00,
            Self::Audio => 0x7b5cc7,
            Self::Video => 0x1d9bd1,
            Self::Image => 0x2eb67d,
            Self::Snippet => 0x611f69,
            Self::Email => 0x1264a3,
            Self::File => 0x5c5f66,
        }
    }

    pub(crate) fn glyph(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::List => "☷",
            Self::Document => "D",
            Self::Spreadsheet => "S",
            Self::Presentation => "P",
            Self::Audio => "♪",
            Self::Video => "▶",
            Self::Image => "▧",
            Self::Snippet => "</>",
            Self::Email => "@",
            Self::File => "F",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackFileRow {
    pub(crate) id: SharedString,
    pub(crate) element_id: SharedString,
    pub(crate) accessibility_label: SharedString,
    pub(crate) title: SharedString,
    pub(crate) metadata: SharedString,
    pub(crate) link_url: SharedString,
    pub(crate) visual: SlackFileVisual,
}

pub(crate) struct PreparedSlackFilesSnapshot {
    pub(crate) snapshot: SlackFilesSnapshot,
    pub(crate) rows: Arc<[SlackFileRow]>,
}

pub(crate) fn prepare_slack_files_snapshot(
    snapshot: SlackFilesSnapshot,
    self_user_id: &str,
    timezone: chrono_tz::Tz,
) -> Result<PreparedSlackFilesSnapshot, String> {
    let mut rows = snapshot
        .items
        .iter()
        .map(|file| prepare_slack_file_row(file, self_user_id, timezone))
        .collect::<Result<Vec<_>, _>>()?;
    let mut seen_ids = HashSet::with_capacity(rows.len());
    rows.retain(|row| seen_ids.insert(row.id.clone()));
    Ok(PreparedSlackFilesSnapshot {
        snapshot,
        rows: rows.into(),
    })
}

fn prepare_slack_file_row(
    file: &SlackFileItem,
    self_user_id: &str,
    timezone: chrono_tz::Tz,
) -> Result<SlackFileRow, String> {
    let last_read = i64::try_from(file.last_read)
        .ok()
        .and_then(|seconds| DateTime::<Utc>::from_timestamp(seconds, 0))
        .ok_or_else(|| format!("Slack file {} returned an invalid last_read", file.id))?
        .with_timezone(&timezone);
    let owner = if file.owner_user_id == self_user_id {
        format!("{} (you)", file.owner_label)
    } else {
        file.owner_label.clone()
    };
    let metadata = format!(
        "{owner} · Last viewed on {} {}",
        last_read.format("%B"),
        ordinal_day(last_read.day())
    );
    let link_url = match file.mode {
        SlackFileMode::External => file
            .external_url
            .as_deref()
            .unwrap_or(file.permalink.as_str()),
        SlackFileMode::Hosted | SlackFileMode::List => file.permalink.as_str(),
    };
    let title = file.title.clone();
    Ok(SlackFileRow {
        id: file.id.clone().into(),
        element_id: format!("slack-file-{}", file.id).into(),
        accessibility_label: format!("{title}, {metadata}").into(),
        title: title.into(),
        metadata: metadata.into(),
        link_url: link_url.to_string().into(),
        visual: slack_file_visual(file),
    })
}

pub(crate) fn slack_file_visual(file: &SlackFileItem) -> SlackFileVisual {
    let filetype = file.filetype.to_ascii_lowercase();
    let mimetype = file.mimetype.to_ascii_lowercase();
    if filetype == "pdf" || mimetype == "application/pdf" {
        SlackFileVisual::Pdf
    } else if file.mode == SlackFileMode::List || filetype == "list" {
        SlackFileVisual::List
    } else if matches!(
        filetype.as_str(),
        "quip" | "canvas" | "gdoc" | "doc" | "docx"
    ) || mimetype.contains("document")
        || mimetype.contains("slack-doc")
    {
        SlackFileVisual::Document
    } else if matches!(filetype.as_str(), "gsheet" | "xls" | "xlsx")
        || mimetype.contains("spreadsheet")
    {
        SlackFileVisual::Spreadsheet
    } else if matches!(filetype.as_str(), "gpres" | "ppt" | "pptx")
        || mimetype.contains("presentation")
    {
        SlackFileVisual::Presentation
    } else if mimetype.starts_with("audio/") {
        SlackFileVisual::Audio
    } else if mimetype.starts_with("video/") {
        SlackFileVisual::Video
    } else if mimetype.starts_with("image/") {
        SlackFileVisual::Image
    } else if filetype == "email" || mimetype.contains("message") {
        SlackFileVisual::Email
    } else if filetype == "snippet" || mimetype.starts_with("text/") {
        SlackFileVisual::Snippet
    } else {
        SlackFileVisual::File
    }
}

fn ordinal_day(day: u32) -> String {
    let suffix = if (11..=13).contains(&(day % 100)) {
        "th"
    } else {
        match day % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    format!("{day}{suffix}")
}
