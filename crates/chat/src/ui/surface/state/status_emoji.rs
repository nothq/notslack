//! The emoji the status dialog's picker offers: your frequently used ones
//! first, or the matches for a search.

use std::collections::HashSet;

use super::{slack_emoji_match_score, SurfaceState, SLACK_COMMON_EMOJI_SHORTCODES};

/// How many emoji the picker shows at once.
const SLACK_STATUS_EMOJI_LIMIT: usize = 90;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackStatusEmojiChoice {
    pub(crate) shortcode: String,
    pub(crate) glyph: String,
}

impl SurfaceState {
    pub(crate) fn slack_status_emoji_choices(&self, query: &str) -> Vec<SlackStatusEmojiChoice> {
        let query = query.trim().trim_matches(':').to_lowercase();
        let mut seen = HashSet::new();
        let choices: Box<dyn Iterator<Item = &'static emojis::Emoji>> = if query.is_empty() {
            let frequent = self
                .slack_workspace()
                .and_then(|workspace| workspace.emoji_picker_sections.first())
                .map(|section| {
                    section
                        .rows
                        .iter()
                        .filter_map(|row| emojis::get_by_shortcode(&row.shortcode))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Box::new(
                frequent
                    .into_iter()
                    .chain(
                        SLACK_COMMON_EMOJI_SHORTCODES
                            .into_iter()
                            .filter_map(emojis::get_by_shortcode),
                    )
                    .chain(emojis::Group::SmileysAndEmotion.emojis())
                    .chain(emojis::Group::Activities.emojis())
                    .chain(emojis::Group::TravelAndPlaces.emojis()),
            )
        } else {
            let mut matches = emojis::iter()
                .filter_map(|emoji| Some((slack_emoji_match_score(emoji, &query)?, emoji)))
                .collect::<Vec<_>>();
            matches.sort_by_key(|(score, _)| *score);
            Box::new(matches.into_iter().map(|(_, emoji)| emoji))
        };
        choices
            .filter_map(|emoji| {
                Some(SlackStatusEmojiChoice {
                    shortcode: emoji.shortcode()?.to_string(),
                    glyph: emoji.as_str().to_string(),
                })
            })
            .filter(|choice| seen.insert(choice.glyph.clone()))
            .take(SLACK_STATUS_EMOJI_LIMIT)
            .collect()
    }
}
