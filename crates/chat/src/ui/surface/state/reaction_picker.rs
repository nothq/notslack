use std::{
    collections::HashSet,
    sync::{Arc, OnceLock},
};

use super::{
    slack_common_emoji_rank, slack_emoji_match_score, slack_normalized_aux_panel_query,
    slack_normalized_text_query_score, SurfaceState,
};
use crate::ui::surface::{
    SlackReactionPickerContent, SlackReactionPickerEmoji, SlackReactionPickerEmojiPresentation,
    SlackReactionPickerListRow, SlackReactionPickerSelection, SlackReactionPickerSource,
    SlackReactionPickerState, SlackReactionSkinToneSupport, SlackStandardReactionPickerCatalog,
    SlackStandardReactionPickerCategory, SlackStandardReactionPickerEntry,
    SLACK_REACTION_PICKER_CATEGORY_COUNT, SLACK_STANDARD_REACTION_PICKER_CATEGORY_COUNT,
};
use gpui::{px, ListAlignment, ListState, SharedString};

mod catalog;
mod input;
mod skin_tone;

const SLACK_REACTION_PICKER_COLUMNS: usize = 9;
const SLACK_REACTION_PICKER_OVERDRAW: f32 = 66.0;
type SlackReactionPickerSections = Vec<(SharedString, Vec<SlackReactionPickerEmoji>)>;

static SLACK_STANDARD_REACTION_PICKER_CATALOG: OnceLock<SlackStandardReactionPickerCatalog> =
    OnceLock::new();

impl SurfaceState {
    pub(super) fn slack_reaction_picker_state(
        &self,
        source: SlackReactionPickerSource,
        selection: SlackReactionPickerSelection,
    ) -> SlackReactionPickerState {
        let SlackReactionPickerSource {
            identity,
            reactions,
        } = source;
        let SlackReactionPickerSelection {
            query,
            category_index,
        } = selection;
        let normalized_query = slack_normalized_aux_panel_query(&query);
        assert!(
            category_index < SLACK_REACTION_PICKER_CATEGORY_COUNT,
            "Slack reaction picker category index must remain in range"
        );
        let content =
            self.slack_reaction_picker_selected_content(&normalized_query, category_index);
        SlackReactionPickerState {
            identity,
            reactions,
            query,
            category_index,
            emojis: content.emojis,
            list_state: ListState::new(
                content.rows.len(),
                ListAlignment::Top,
                px(SLACK_REACTION_PICKER_OVERDRAW),
            ),
            rows: content.rows,
        }
    }

    fn slack_reaction_picker_selected_content(
        &self,
        normalized_query: &str,
        category_index: usize,
    ) -> SlackReactionPickerContent {
        let content = if normalized_query.is_empty() {
            match category_index {
                0 if self.slack_reaction_picker_catalog.frequent().is_empty() => {
                    slack_standard_reaction_picker_category(1).content().clone()
                }
                1..=SLACK_STANDARD_REACTION_PICKER_CATEGORY_COUNT => {
                    slack_standard_reaction_picker_category(category_index)
                        .content()
                        .clone()
                }
                0 | 9 => self
                    .slack_reaction_picker_catalog
                    .team_category_content(category_index, || {
                        Self::slack_reaction_picker_content(
                            self.slack_reaction_picker_team_category_sections(category_index),
                            "No emoji are available in this category.",
                        )
                    })
                    .clone(),
                _ => unreachable!("validated Slack reaction picker category must be handled"),
            }
        } else {
            Self::slack_reaction_picker_content(
                self.slack_reaction_picker_search_sections(normalized_query),
                "No emoji match this search.",
            )
        };
        content
    }

    fn slack_reaction_picker_content(
        sections: SlackReactionPickerSections,
        empty_label: &'static str,
    ) -> SlackReactionPickerContent {
        let mut emojis = Vec::new();
        let mut rows = Vec::new();
        for (title, section_emojis) in sections {
            if section_emojis.is_empty() {
                continue;
            }
            rows.push(SlackReactionPickerListRow::Heading(title));
            let start = emojis.len();
            emojis.extend(section_emojis);
            for grid_start in (start..emojis.len()).step_by(SLACK_REACTION_PICKER_COLUMNS) {
                rows.push(SlackReactionPickerListRow::EmojiGrid(
                    grid_start..(grid_start + SLACK_REACTION_PICKER_COLUMNS).min(emojis.len()),
                ));
            }
        }
        if rows.is_empty() {
            rows.push(SlackReactionPickerListRow::Empty(empty_label.into()));
        }
        SlackReactionPickerContent {
            emojis: Arc::from(emojis),
            rows: Arc::from(rows),
        }
    }

    fn slack_reaction_picker_team_category_sections(
        &self,
        category_index: usize,
    ) -> SlackReactionPickerSections {
        match category_index {
            0 => {
                let mut seen = HashSet::new();
                let frequent = self
                    .slack_reaction_picker_catalog
                    .frequent()
                    .iter()
                    .filter(|emoji| seen.insert(emoji.name.clone()))
                    .cloned()
                    .collect::<Vec<_>>();
                let smileys_and_people = slack_standard_reaction_picker_category(1)
                    .content()
                    .emojis
                    .iter()
                    .filter(|emoji| seen.insert(emoji.name.clone()))
                    .cloned()
                    .collect::<Vec<_>>();
                vec![
                    ("Frequently Used".into(), frequent),
                    ("Smileys & People".into(), smileys_and_people),
                ]
            }
            9 => vec![(
                "Custom".into(),
                self.slack_reaction_picker_catalog.custom().to_vec(),
            )],
            _ => {
                unreachable!("only team-dependent Slack reaction picker categories are rebuilt")
            }
        }
    }

    fn slack_reaction_picker_search_sections(
        &self,
        normalized_query: &str,
    ) -> SlackReactionPickerSections {
        let mut matches = (1..=SLACK_STANDARD_REACTION_PICKER_CATEGORY_COUNT)
            .flat_map(|category_index| {
                slack_standard_reaction_picker_category(category_index)
                    .entries()
                    .iter()
            })
            .filter_map(|entry| {
                let source = entry.source();
                slack_emoji_match_score(source, normalized_query).map(|score| {
                    (
                        score,
                        slack_common_emoji_rank(source),
                        source.name(),
                        entry.emoji().clone(),
                    )
                })
            })
            .collect::<Vec<_>>();
        matches.extend(
            self.slack_reaction_picker_catalog
                .custom()
                .iter()
                .filter_map(|emoji| {
                    slack_normalized_text_query_score(&emoji.search_key, normalized_query)
                        .map(|score| (score, usize::MAX, emoji.name.as_ref(), emoji.clone()))
                }),
        );
        matches.sort_by(
            |(left_score, left_rank, left_name, _), (right_score, right_rank, right_name, _)| {
                left_score
                    .cmp(right_score)
                    .then_with(|| left_rank.cmp(right_rank))
                    .then_with(|| left_name.cmp(right_name))
            },
        );
        vec![(
            "Search Results".into(),
            matches.into_iter().map(|(_, _, _, emoji)| emoji).collect(),
        )]
    }

    fn slack_reaction_picker_emoji(
        emoji: &'static emojis::Emoji,
    ) -> Option<SlackReactionPickerEmoji> {
        let shortcode = emoji.shortcode()?;
        Self::slack_reaction_picker_emoji_with_name(emoji, shortcode)
    }

    fn slack_reaction_picker_emoji_named(name: &str) -> Option<SlackReactionPickerEmoji> {
        let emoji = emojis::get_by_shortcode(name)?;
        Self::slack_reaction_picker_emoji_with_name(emoji, name)
    }

    fn slack_reaction_picker_emoji_with_name(
        emoji: &'static emojis::Emoji,
        reaction_name: &str,
    ) -> Option<SlackReactionPickerEmoji> {
        let primary_shortcode = emoji.shortcode()?;
        Some(SlackReactionPickerEmoji {
            name: reaction_name.to_string().into(),
            search_key: format!(
                "{} {}",
                emoji.name(),
                emoji.shortcodes().collect::<Vec<_>>().join(" ")
            )
            .to_ascii_lowercase()
            .into(),
            presentation: SlackReactionPickerEmojiPresentation::Glyph(emoji.as_str().into()),
            accessibility_label: SharedString::from(format!(
                "React with {}, :{}:",
                emoji.name(),
                primary_shortcode
            )),
            skin_tone_support: slack_reaction_skin_tone_support(emoji),
        })
    }
}

fn slack_reaction_skin_tone_support(emoji: &'static emojis::Emoji) -> SlackReactionSkinToneSupport {
    if emoji.skin_tone() == Some(emojis::SkinTone::Default)
        && emoji
            .skin_tones()
            .is_some_and(|skin_tones| skin_tones.count() == 6)
    {
        SlackReactionSkinToneSupport::Single
    } else {
        SlackReactionSkinToneSupport::None
    }
}

fn slack_standard_reaction_picker_catalog() -> &'static SlackStandardReactionPickerCatalog {
    SLACK_STANDARD_REACTION_PICKER_CATALOG.get_or_init(Default::default)
}

fn slack_standard_reaction_picker_category(
    category_index: usize,
) -> &'static SlackStandardReactionPickerCategory {
    slack_standard_reaction_picker_catalog().category(category_index, || {
        let (title, groups) = slack_standard_reaction_picker_category_spec(category_index);
        let entries = emojis::iter()
            .filter(|source| groups.contains(&source.group()))
            .filter_map(|source| {
                SurfaceState::slack_reaction_picker_emoji(source)
                    .map(|emoji| SlackStandardReactionPickerEntry::new(source, emoji))
            })
            .collect::<Vec<_>>();
        let content = SurfaceState::slack_reaction_picker_content(
            vec![(
                title.into(),
                entries.iter().map(|entry| entry.emoji().clone()).collect(),
            )],
            "No emoji are available in this category.",
        );
        SlackStandardReactionPickerCategory::new(Arc::from(entries), content)
    })
}

fn slack_standard_reaction_picker_category_spec(
    category_index: usize,
) -> (&'static str, &'static [emojis::Group]) {
    match category_index {
        1 => (
            "Smileys & People",
            &[
                emojis::Group::SmileysAndEmotion,
                emojis::Group::PeopleAndBody,
            ],
        ),
        2 => ("Animals & Nature", &[emojis::Group::AnimalsAndNature]),
        3 => ("Food & Drink", &[emojis::Group::FoodAndDrink]),
        4 => ("Travel & Places", &[emojis::Group::TravelAndPlaces]),
        5 => ("Activities", &[emojis::Group::Activities]),
        6 => ("Objects", &[emojis::Group::Objects]),
        7 => ("Symbols", &[emojis::Group::Symbols]),
        8 => ("Flags", &[emojis::Group::Flags]),
        _ => panic!("standard Slack reaction picker category index must be between 1 and 8"),
    }
}
