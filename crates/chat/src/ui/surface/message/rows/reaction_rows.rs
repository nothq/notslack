use std::collections::HashMap;

use super::body::slack_message_element_ids;
use super::reactions::{slack_reaction_group_shortcode, slack_reaction_presentation};
use crate::ui::surface::{SlackReactionOwnership, SlackReactionRow, SlackReactionVariantRow};
use crate::ui::SlackReaction;

struct SlackReactionGroupBuilder {
    shortcode: String,
    mutation_name: Option<String>,
    represented_base_name: Option<String>,
    count: u32,
    ownership: SlackReactionOwnership,
    variants: Vec<SlackReactionVariantRow>,
}

impl SlackReactionGroupBuilder {
    fn new(shortcode: String) -> Self {
        Self {
            shortcode,
            mutation_name: None,
            represented_base_name: None,
            count: 0,
            ownership: SlackReactionOwnership::OtherUsers,
            variants: Vec::new(),
        }
    }

    fn push(&mut self, reaction: &SlackReaction) {
        self.count = self
            .count
            .checked_add(reaction.count)
            .expect("Slack grouped reaction count overflowed");
        if reaction.active {
            self.ownership = SlackReactionOwnership::CurrentUser;
            if self.mutation_name.is_none() {
                self.mutation_name = Some(reaction.emoji.clone());
            }
        }
        if reaction.emoji == self.shortcode && self.represented_base_name.is_none() {
            self.represented_base_name = Some(reaction.emoji.clone());
        }
        let (display, image_cache_key) = slack_reaction_presentation(&reaction.emoji);
        self.variants.push(SlackReactionVariantRow {
            display: display.into(),
            image_cache_key: image_cache_key.map(Into::into),
        });
    }

    fn finish(self, message_id: &str, shared_message_id: &gpui::SharedString) -> SlackReactionRow {
        let mutation_name = self
            .mutation_name
            .or(self.represented_base_name)
            .unwrap_or_else(|| self.shortcode.clone());
        let reaction_noun = if self.count == 1 {
            "reaction"
        } else {
            "reactions"
        };
        let reaction_label = self.shortcode.replace('_', " ");
        SlackReactionRow {
            message_id: shared_message_id.clone(),
            mutation_name: mutation_name.into(),
            count: self.count,
            count_label: self.count.to_string().into(),
            ownership: self.ownership,
            variants: self.variants.into(),
            element_ids: slack_message_element_ids(
                &format!("slack-reaction-{message_id}-{}", self.shortcode),
                0,
            ),
            accessibility_label: format!(
                "{} {reaction_noun}, react with {reaction_label} emoji",
                self.count
            )
            .into(),
        }
    }
}

pub(crate) fn slack_reaction_rows(
    message_id: &str,
    reactions: &[SlackReaction],
) -> Vec<SlackReactionRow> {
    let shared_message_id: gpui::SharedString = message_id.to_string().into();
    let mut group_indices = HashMap::new();
    let mut groups = Vec::<SlackReactionGroupBuilder>::new();
    for reaction in reactions {
        let shortcode = slack_reaction_group_shortcode(&reaction.emoji);
        let group_index = match group_indices.get(shortcode).copied() {
            Some(group_index) => group_index,
            None => {
                let group_index = groups.len();
                group_indices.insert(shortcode.to_string(), group_index);
                groups.push(SlackReactionGroupBuilder::new(shortcode.to_string()));
                group_index
            }
        };
        groups[group_index].push(reaction);
    }
    groups
        .into_iter()
        .map(|group| group.finish(message_id, &shared_message_id))
        .collect()
}
