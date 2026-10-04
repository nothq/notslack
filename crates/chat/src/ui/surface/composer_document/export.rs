use std::ops::Range;

use crate::model::{
    SlackMessageDraft, SlackRichTextBlock, SlackRichTextBody, SlackRichTextInline,
    SlackRichTextListStyle, SlackRichTextSection, SlackRichTextStyle,
};

use super::lines::logical_line_ranges;
use super::{
    SlackComposerDocument, SlackComposerEntity, SlackComposerInlineStyle, SlackComposerLineBlock,
};

impl SlackComposerDocument {
    pub(crate) fn export_message_draft(&self) -> Result<SlackMessageDraft, String> {
        let text_after_leading_trim = self.text.trim_start();
        let sent_start = self.text.len() - text_after_leading_trim.len();
        let sent_text = text_after_leading_trim.trim_end();
        let sent_end = sent_start + sent_text.len();
        let sent_lines = logical_line_ranges(sent_text);
        let has_intersecting_style = self
            .runs
            .iter()
            .any(|run| run.range.start < sent_end && run.range.end > sent_start);
        let has_intersecting_link = self
            .links
            .iter()
            .any(|link| link.range.start < sent_end && link.range.end > sent_start);
        let has_intersecting_entity = self
            .entities
            .iter()
            .any(|entity| entity.range.start < sent_end && entity.range.end > sent_start);
        let has_structural_block = sent_lines.iter().any(|line| {
            self.exported_line_block(sent_start + line.start, line.is_empty())
                != SlackComposerLineBlock::Section
        });
        if !has_intersecting_style
            && !has_intersecting_link
            && !has_intersecting_entity
            && !has_structural_block
        {
            return SlackMessageDraft::plain_text(sent_text);
        }

        let mut blocks = Vec::with_capacity(sent_lines.len());
        let mut group_start = 0;
        while group_start < sent_lines.len() {
            let first_line = &sent_lines[group_start];
            let block =
                self.exported_line_block(sent_start + first_line.start, first_line.is_empty());
            let mut group_end = group_start + 1;
            while group_end < sent_lines.len() {
                let line = &sent_lines[group_end];
                if self.exported_line_block(sent_start + line.start, line.is_empty()) != block {
                    break;
                }
                group_end += 1;
            }
            let group_lines = &sent_lines[group_start..group_end];
            blocks.push(self.export_rich_text_block(block, sent_start, group_lines));
            group_start = group_end;
        }

        SlackMessageDraft::new(
            sent_text,
            Some(SlackRichTextBody {
                blocks,
                block_kit: Vec::new(),
            }),
        )
    }

    fn exported_line_block(&self, offset: usize, line_is_empty: bool) -> SlackComposerLineBlock {
        let block = self.line_block_at_offset(offset);
        if line_is_empty
            && matches!(
                block,
                SlackComposerLineBlock::OrderedList | SlackComposerLineBlock::BulletedList
            )
        {
            SlackComposerLineBlock::Section
        } else {
            block
        }
    }

    fn export_rich_text_block(
        &self,
        block: SlackComposerLineBlock,
        sent_start: usize,
        lines: &[Range<usize>],
    ) -> SlackRichTextBlock {
        assert!(!lines.is_empty(), "Slack rich-text block omitted its lines");
        match block {
            SlackComposerLineBlock::OrderedList | SlackComposerLineBlock::BulletedList => {
                let items = lines
                    .iter()
                    .map(|line| {
                        assert!(
                            !line.is_empty(),
                            "Slack composer list export cannot contain an empty item"
                        );
                        SlackRichTextSection {
                            elements: self
                                .rich_text_elements(sent_start + line.start..sent_start + line.end),
                        }
                    })
                    .collect();
                SlackRichTextBlock::List {
                    style: if block == SlackComposerLineBlock::OrderedList {
                        SlackRichTextListStyle::Ordered
                    } else {
                        SlackRichTextListStyle::Bullet
                    },
                    indent: 0,
                    offset: None,
                    items,
                }
            }
            SlackComposerLineBlock::Section
            | SlackComposerLineBlock::Quote
            | SlackComposerLineBlock::Preformatted => {
                let range = sent_start + lines[0].start..sent_start + lines[lines.len() - 1].end;
                let elements = if range.is_empty() {
                    vec![SlackRichTextInline::Text {
                        text: "\n".repeat(lines.len()),
                        style: SlackRichTextStyle::default(),
                    }]
                } else {
                    self.rich_text_elements(range)
                };
                match block {
                    SlackComposerLineBlock::Section => SlackRichTextBlock::Section { elements },
                    SlackComposerLineBlock::Quote => SlackRichTextBlock::Quote { elements },
                    SlackComposerLineBlock::Preformatted => {
                        SlackRichTextBlock::Preformatted { elements }
                    }
                    SlackComposerLineBlock::OrderedList | SlackComposerLineBlock::BulletedList => {
                        unreachable!("list blocks are exported through the list branch")
                    }
                }
            }
        }
    }

    fn rich_text_elements(&self, range: Range<usize>) -> Vec<SlackRichTextInline> {
        assert!(
            !range.is_empty()
                && range.end <= self.text.len()
                && self.text.is_char_boundary(range.start)
                && self.text.is_char_boundary(range.end),
            "Slack composer rich-text range must follow UTF-8 boundaries"
        );
        let cut_points = self.rich_text_cut_points(&range);
        let mut elements = Vec::with_capacity(cut_points.len().saturating_sub(1));
        for boundary in cut_points.windows(2) {
            elements.push(self.rich_text_element(boundary[0]..boundary[1]));
        }
        elements
    }

    fn rich_text_cut_points(&self, range: &Range<usize>) -> Vec<usize> {
        let mut cut_points = Vec::with_capacity(
            self.runs
                .len()
                .saturating_add(self.links.len())
                .saturating_add(self.entities.len())
                .saturating_mul(2)
                .saturating_add(2),
        );
        cut_points.extend([range.start, range.end]);
        for entity in self
            .entities
            .iter()
            .filter(|entity| entity.range.start < range.end && entity.range.end > range.start)
        {
            assert!(
                range.start <= entity.range.start && entity.range.end <= range.end,
                "Slack composer rich-text export must not split an entity"
            );
            cut_points.extend([entity.range.start, entity.range.end]);
        }
        for run in &self.runs {
            if run.range.start < range.end && run.range.end > range.start {
                for boundary in [
                    run.range.start.max(range.start),
                    run.range.end.min(range.end),
                ] {
                    if !self.offset_is_inside_entity(boundary) {
                        cut_points.push(boundary);
                    }
                }
            }
        }
        for link in &self.links {
            if link.range.start < range.end && link.range.end > range.start {
                for boundary in [
                    link.range.start.max(range.start),
                    link.range.end.min(range.end),
                ] {
                    if !self.offset_is_inside_entity(boundary) {
                        cut_points.push(boundary);
                    }
                }
            }
        }
        cut_points.sort_unstable();
        cut_points.dedup();
        cut_points
    }

    fn rich_text_element(&self, range: Range<usize>) -> SlackRichTextInline {
        let style = slack_rich_text_style(self.style_at(range.start));
        if let Some(entity) = self.entity_at(range.start) {
            assert_eq!(
                entity.range, range,
                "Slack composer rich-text export must emit each entity exactly once"
            );
            return rich_entity_inline(&entity.entity, style);
        }
        if let Some(link) = self.link_at(range.start) {
            return SlackRichTextInline::Link {
                url: link.url.as_str().to_string(),
                label: Some(self.text[range].to_string()),
                unsafe_url: false,
                style,
            };
        }
        rich_text_element(&self.text, range, style)
    }
}

fn rich_entity_inline(
    entity: &SlackComposerEntity,
    style: SlackRichTextStyle,
) -> SlackRichTextInline {
    match entity {
        SlackComposerEntity::User { user_id, label } => SlackRichTextInline::User {
            user_id: user_id.clone(),
            label: label.clone(),
            style,
        },
        SlackComposerEntity::Channel { channel_id, label } => SlackRichTextInline::Channel {
            channel_id: channel_id.clone(),
            label: label.clone(),
            style,
        },
        SlackComposerEntity::Broadcast(range) => SlackRichTextInline::Broadcast {
            range: range.clone(),
            style,
        },
        SlackComposerEntity::Emoji {
            name,
            unicode,
            skin_tone,
        } => SlackRichTextInline::Emoji {
            name: name.clone(),
            unicode: unicode.clone(),
            skin_tone: *skin_tone,
            style,
        },
    }
}

fn rich_text_element(
    text: &str,
    range: Range<usize>,
    style: SlackRichTextStyle,
) -> SlackRichTextInline {
    assert!(
        range.start < range.end
            && range.end <= text.len()
            && text.is_char_boundary(range.start)
            && text.is_char_boundary(range.end),
        "Slack composer send range must follow UTF-8 boundaries"
    );
    SlackRichTextInline::Text {
        text: text[range].to_string(),
        style,
    }
}

fn slack_rich_text_style(style: SlackComposerInlineStyle) -> SlackRichTextStyle {
    SlackRichTextStyle {
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
        strike: style.strikethrough,
        code: style.code,
    }
}
