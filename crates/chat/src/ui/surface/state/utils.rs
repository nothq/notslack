use std::io::Read as _;

use super::SLACK_COMMON_EMOJI_SHORTCODES;

use crate::ui::surface::SlackPreparedUploadFile;
use crate::ui::{
    build_slack_composer_local_preview, slack_composer_local_preview_supported, SlackUploadFile,
    SLACK_COMPOSER_LOCAL_PREVIEW_MAX_BYTES,
};

pub(crate) fn slack_normalized_aux_panel_query(query: &str) -> String {
    query
        .trim()
        .trim_start_matches(['@', ':'])
        .trim_end_matches(':')
        .to_ascii_lowercase()
}

pub(crate) fn slack_aux_panel_matches_query(query: &str, value: &str) -> bool {
    query.is_empty() || value.to_ascii_lowercase().contains(query)
}

pub(crate) fn slack_emoji_group_label(group: emojis::Group) -> &'static str {
    match group {
        emojis::Group::SmileysAndEmotion => "Smileys & Emotion",
        emojis::Group::PeopleAndBody => "People & Body",
        emojis::Group::AnimalsAndNature => "Animals & Nature",
        emojis::Group::FoodAndDrink => "Food & Drink",
        emojis::Group::TravelAndPlaces => "Travel & Places",
        emojis::Group::Activities => "Activities",
        emojis::Group::Objects => "Objects",
        emojis::Group::Symbols => "Symbols",
        emojis::Group::Flags => "Flags",
    }
}

pub(crate) fn slack_emoji_match_score(emoji: &emojis::Emoji, query: &str) -> Option<u8> {
    let mut best_score =
        slack_merge_query_score(None, slack_text_query_score(emoji.as_str(), query));
    best_score = slack_merge_query_score(best_score, slack_text_query_score(emoji.name(), query));
    for shortcode in emoji.shortcodes() {
        best_score = slack_merge_query_score(best_score, slack_text_query_score(shortcode, query));
    }
    slack_merge_query_score(
        best_score,
        slack_text_query_score(slack_emoji_group_label(emoji.group()), query)
            .map(|score| score + 4),
    )
}

fn slack_text_query_score(value: &str, query: &str) -> Option<u8> {
    let normalized = value.to_ascii_lowercase();
    slack_normalized_text_query_score(&normalized, query)
}

pub(crate) fn slack_normalized_text_query_score(normalized: &str, query: &str) -> Option<u8> {
    if normalized == query {
        return Some(0);
    }
    if normalized.starts_with(query) {
        return Some(1);
    }
    if normalized
        .split([' ', '_', '-'])
        .any(|part| !part.is_empty() && part.starts_with(query))
    {
        return Some(2);
    }
    normalized.contains(query).then_some(3)
}

fn slack_merge_query_score(left: Option<u8>, right: Option<u8>) -> Option<u8> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    }
}

pub(crate) fn slack_common_emoji_rank(emoji: &emojis::Emoji) -> usize {
    emoji
        .shortcodes()
        .find_map(|shortcode| {
            SLACK_COMMON_EMOJI_SHORTCODES
                .iter()
                .position(|candidate| *candidate == shortcode)
        })
        .unwrap_or(usize::MAX)
}

pub(crate) fn load_slack_upload_files_from_paths(
    local_file_api: &dyn crate::model::SlackLocalFileApi,
    paths: Vec<std::path::PathBuf>,
) -> Result<Vec<SlackUploadFile>, String> {
    paths
        .into_iter()
        .map(|path| local_file_api.open_upload(path))
        .collect()
}

pub(crate) fn load_slack_prepared_upload_files_from_paths(
    local_file_api: &dyn crate::model::SlackLocalFileApi,
    paths: Vec<std::path::PathBuf>,
) -> Result<Vec<SlackPreparedUploadFile>, String> {
    paths
        .into_iter()
        .map(|path| load_slack_prepared_upload_file_from_path(local_file_api, path))
        .collect()
}

fn load_slack_prepared_upload_file_from_path(
    local_file_api: &dyn crate::model::SlackLocalFileApi,
    path: std::path::PathBuf,
) -> Result<SlackPreparedUploadFile, String> {
    let upload = local_file_api.open_upload(path)?;
    let preview = load_slack_composer_local_preview(&upload)?;
    Ok(SlackPreparedUploadFile::new(upload, preview))
}

fn load_slack_composer_local_preview(
    upload: &SlackUploadFile,
) -> Result<Option<std::sync::Arc<crate::ui::Image>>, String> {
    let max_bytes = u64::try_from(SLACK_COMPOSER_LOCAL_PREVIEW_MAX_BYTES)
        .map_err(|error| format!("Slack composer preview limit is invalid: {error}"))?;
    if upload.size_bytes() >= max_bytes
        || !slack_composer_local_preview_supported(upload.mimetype())
    {
        return Ok(None);
    }
    let capacity = usize::try_from(upload.size_bytes())
        .map_err(|error| format!("Slack composer preview size is invalid: {error}"))?;
    let mut bytes = Vec::with_capacity(capacity);
    upload
        .open_reader()?
        .take(max_bytes)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            format!(
                "failed to read attachment preview for {}: {error}",
                upload.name()
            )
        })?;
    Ok(build_slack_composer_local_preview(
        &bytes,
        upload.mimetype(),
    ))
}
