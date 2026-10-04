use std::collections::HashSet;

use super::{
    slack_common_emoji_rank, slack_emoji_group_label, slack_emoji_match_score,
    slack_normalized_aux_panel_query, Context, SlackAuxPanelQueryBehavior, SlackAuxPanelRow,
    SlackAuxPanelRowAction, SlackAuxPanelSection, SlackAuxPanelState, SlackComposerTarget,
    SurfaceState, SLACK_COMMON_EMOJI_SHORTCODES,
};

impl SurfaceState {
    pub(crate) fn open_slack_emoji_picker(&mut self, cx: &mut Context<Self>) {
        if !self.can_mutate_current_slack_send_draft() {
            return;
        }
        self.slack_composer_aux_target = Some(SlackComposerTarget::Main);
        self.slack_mention_picker_state = None;
        self.set_slack_emoji_query(String::new(), cx);
    }

    pub(crate) fn open_slack_mention_picker(&mut self, cx: &mut Context<Self>) {
        if !self.can_mutate_current_slack_send_draft() {
            return;
        }
        self.slack_composer_aux_target = Some(SlackComposerTarget::Main);
        self.mark_slack_toolbar_mention_picker(SlackComposerTarget::Main);
        self.set_slack_mention_query(String::new(), cx);
    }

    pub(super) fn slack_emoji_panel(&self, query: &str) -> SlackAuxPanelState {
        SlackAuxPanelState {
            title: "Emoji".to_string(),
            subtitle: Some("Search and insert emoji into the current draft".to_string()),
            query: Some(query.to_string()),
            query_behavior: Some(SlackAuxPanelQueryBehavior::Emoji),
            sections: self.slack_emoji_sections(query),
        }
    }

    fn slack_emoji_sections(&self, query: &str) -> Vec<SlackAuxPanelSection> {
        let normalized_query = slack_normalized_aux_panel_query(query);
        if normalized_query.is_empty() {
            if let Some(sections) = self.slack_workspace_emoji_sections() {
                return sections;
            }
            return self.slack_default_emoji_sections();
        }
        self.slack_search_emoji_sections(&normalized_query)
    }

    fn slack_workspace_emoji_sections(&self) -> Option<Vec<SlackAuxPanelSection>> {
        let workspace = self.slack_workspace()?;
        if workspace.emoji_picker_sections.is_empty() {
            return None;
        }
        let sections = workspace
            .emoji_picker_sections
            .iter()
            .map(|section| SlackAuxPanelSection {
                title: Some(section.title.clone()),
                rows: section
                    .rows
                    .iter()
                    .map(|row| SlackAuxPanelRow {
                        label: format!(":{}:", row.shortcode),
                        emoji_glyph: Some(
                            emojis::get_by_shortcode(&row.shortcode)
                                .map(|emoji| emoji.as_str().into())
                                .unwrap_or_else(|| format!(":{}:", row.shortcode).into()),
                        ),
                        detail: None,
                        accessory: None,
                        image_url: row.image_url.clone(),
                        image_base64: row.image_base64.clone(),
                        image_mimetype: row.image_mimetype.clone(),
                        muted: false,
                        action: Some(SlackAuxPanelRowAction::InsertComposerSnippet(
                            format!(":{}: ", row.shortcode).into(),
                        )),
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        (!sections.is_empty()).then_some(sections)
    }

    fn slack_default_emoji_sections(&self) -> Vec<SlackAuxPanelSection> {
        let mut seen = HashSet::new();
        let suggested_rows = SLACK_COMMON_EMOJI_SHORTCODES
            .into_iter()
            .filter_map(emojis::get_by_shortcode)
            .filter(|emoji| seen.insert(emoji.as_str().to_string()))
            .map(Self::slack_emoji_picker_row)
            .collect::<Vec<_>>();
        let mut sections = Vec::new();
        if !suggested_rows.is_empty() {
            sections.push(SlackAuxPanelSection {
                title: Some("Suggested".to_string()),
                rows: suggested_rows,
            });
        }
        let rows = [
            emojis::Group::SmileysAndEmotion,
            emojis::Group::PeopleAndBody,
        ]
        .into_iter()
        .flat_map(|group| group.emojis())
        .filter(|emoji| seen.insert(emoji.as_str().to_string()))
        .take(54)
        .map(Self::slack_emoji_picker_row)
        .collect::<Vec<_>>();
        if !rows.is_empty() {
            sections.push(SlackAuxPanelSection {
                title: Some("Smileys & People".to_string()),
                rows,
            });
        }
        if sections.is_empty() {
            sections.push(SlackAuxPanelSection {
                title: None,
                rows: vec![Self::slack_aux_label_row("No emoji available.")],
            });
        }
        sections
    }

    fn slack_search_emoji_sections(&self, normalized_query: &str) -> Vec<SlackAuxPanelSection> {
        let mut matches = emojis::iter()
            .filter_map(|emoji| {
                slack_emoji_match_score(emoji, normalized_query).map(|score| {
                    (
                        score,
                        slack_common_emoji_rank(emoji),
                        emoji.name(),
                        Self::slack_emoji_picker_row(emoji),
                    )
                })
            })
            .collect::<Vec<_>>();
        matches.sort_by(
            |(left_score, left_rank, left_name, _): &(u8, usize, &str, SlackAuxPanelRow),
             (right_score, right_rank, right_name, _): &(u8, usize, &str, SlackAuxPanelRow)| {
                left_score
                    .cmp(right_score)
                    .then_with(|| left_rank.cmp(right_rank))
                    .then_with(|| left_name.cmp(right_name))
            },
        );
        let rows = matches
            .into_iter()
            .map(|(_, _, _, row)| row)
            .take(24)
            .collect::<Vec<_>>();
        if rows.is_empty() {
            vec![SlackAuxPanelSection {
                title: None,
                rows: vec![Self::slack_aux_label_row("No emoji match this query.")],
            }]
        } else {
            vec![SlackAuxPanelSection {
                title: Some("Results".to_string()),
                rows,
            }]
        }
    }

    fn slack_emoji_picker_row(emoji: &'static emojis::Emoji) -> SlackAuxPanelRow {
        let primary_shortcode = emoji.shortcode().map(|shortcode| format!(":{shortcode}:"));
        let label = primary_shortcode.as_ref().map_or_else(
            || format!("{} {}", emoji.as_str(), emoji.name()),
            |shortcode| format!("{} {}", emoji.as_str(), shortcode),
        );
        let mut row = Self::slack_aux_row(
            label,
            Some(format!(
                "{} · {}",
                emoji.name(),
                slack_emoji_group_label(emoji.group())
            )),
            None,
            false,
            Some(SlackAuxPanelRowAction::InsertComposerSnippet(
                format!("{} ", emoji.as_str()).into(),
            )),
        );
        row.emoji_glyph = Some(emoji.as_str().into());
        row
    }
}
