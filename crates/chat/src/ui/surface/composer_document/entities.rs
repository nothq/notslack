use std::ops::Range;

use crate::model::SlackRichTextBroadcastRange;

use super::normalize::normalize_entity_runs;
use super::{SlackComposerDocument, SlackComposerEntity, SlackComposerEntityRun};

impl SlackComposerDocument {
    pub(crate) fn append_user_mention(&mut self, user_id: &str, label: &str) {
        self.append_entity(SlackComposerEntity::User {
            user_id: user_id.to_string(),
            label: label.to_string(),
        });
    }

    pub(crate) fn append_broadcast_mention(&mut self, range: SlackRichTextBroadcastRange) {
        self.append_entity(SlackComposerEntity::Broadcast(range));
    }

    pub(crate) fn replace_range_with_user_mention(
        &mut self,
        range: Range<usize>,
        user_id: &str,
        label: &str,
    ) -> usize {
        self.replace_range_with_entity(
            range,
            SlackComposerEntity::User {
                user_id: user_id.to_string(),
                label: label.to_string(),
            },
        )
    }

    pub(crate) fn replace_range_with_broadcast_mention(
        &mut self,
        range: Range<usize>,
        broadcast: SlackRichTextBroadcastRange,
    ) -> usize {
        self.replace_range_with_entity(range, SlackComposerEntity::Broadcast(broadcast))
    }

    pub(crate) fn range_overlaps_entity(&self, range: &Range<usize>) -> bool {
        self.entities
            .iter()
            .any(|entity| entity.range.start < range.end && range.start < entity.range.end)
    }

    fn append_entity(&mut self, entity: SlackComposerEntity) {
        entity.assert_canonical();
        let needs_separator = !self.text.is_empty() && !self.text.ends_with(' ');
        let mut updated_text = String::with_capacity(
            self.text
                .len()
                .saturating_add(usize::from(needs_separator))
                .saturating_add(entity.display_len())
                .saturating_add(1),
        );
        updated_text.push_str(&self.text);
        if needs_separator {
            updated_text.push(' ');
        }
        let entity_start = updated_text.len();
        entity.push_display_text(&mut updated_text);
        let entity_end = updated_text.len();
        updated_text.push(' ');

        self.apply_text_edit(&updated_text);
        let mut entities = std::mem::take(&mut self.entities);
        entities.push(SlackComposerEntityRun {
            range: entity_start..entity_end,
            entity,
        });
        self.entities = normalize_entity_runs(entities, &self.text);
        self.highlights_dirty = true;
    }

    fn replace_range_with_entity(
        &mut self,
        range: Range<usize>,
        entity: SlackComposerEntity,
    ) -> usize {
        self.assert_valid_range(&range);
        assert!(
            !range.is_empty(),
            "Slack composer entity completion requires an active query range"
        );
        entity.assert_canonical();
        let needs_trailing_space = self.text[range.end..]
            .chars()
            .next()
            .is_none_or(|character| !character.is_whitespace());
        let mut updated_text = String::with_capacity(
            self.text
                .len()
                .saturating_sub(range.len())
                .saturating_add(entity.display_len())
                .saturating_add(usize::from(needs_trailing_space)),
        );
        updated_text.push_str(&self.text[..range.start]);
        let entity_start = updated_text.len();
        entity.push_display_text(&mut updated_text);
        let entity_end = updated_text.len();
        if needs_trailing_space {
            updated_text.push(' ');
        }
        let cursor = updated_text.len();
        updated_text.push_str(&self.text[range.end..]);

        let previous_revision = self.revision;
        self.apply_text_edit(&updated_text);
        let mut entities = std::mem::take(&mut self.entities);
        entities.push(SlackComposerEntityRun {
            range: entity_start..entity_end,
            entity,
        });
        self.entities = normalize_entity_runs(entities, &self.text);
        self.highlights_dirty = true;
        if self.revision == previous_revision {
            self.advance_revision();
        }
        cursor
    }
}
