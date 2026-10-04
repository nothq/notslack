use std::sync::Arc;

use gpui::{px, FontStyle, FontWeight, Hsla, StrikethroughStyle, UnderlineStyle};
use gpui_components::text_input::TextInputHighlight;

use super::lines::logical_line_ranges;
use super::{SlackComposerDocument, SlackComposerLineBlock};

#[derive(Clone, Copy)]
struct HighlightPalette {
    text: Hsla,
    code: Hsla,
    code_block: Hsla,
    link: Hsla,
}

impl SlackComposerDocument {
    pub(crate) fn highlights(
        &mut self,
        text_color: Hsla,
        code_background: Hsla,
        code_block_background: Hsla,
        link_color: Hsla,
    ) -> Arc<[TextInputHighlight]> {
        let palette = HighlightPalette {
            text: text_color,
            code: code_background,
            code_block: code_block_background,
            link: link_color,
        };
        if self.highlight_cache_matches(palette) {
            return self.cached_highlights.clone();
        }
        let cut_points = self.highlight_cut_points();
        let highlights = self.build_highlights(&cut_points, palette);
        self.cache_highlights(highlights, palette)
    }

    fn highlight_cache_matches(&self, palette: HighlightPalette) -> bool {
        !self.highlights_dirty
            && self.cached_text_color == Some(palette.text)
            && self.cached_code_background == Some(palette.code)
            && self.cached_code_block_background == Some(palette.code_block)
            && self.cached_link_color == Some(palette.link)
    }

    fn highlight_cut_points(&self) -> Vec<usize> {
        let mut cut_points = Vec::with_capacity(
            self.runs
                .len()
                .saturating_mul(2)
                .saturating_add(self.links.len().saturating_mul(2))
                .saturating_add(self.entities.len().saturating_mul(2))
                .saturating_add(self.line_blocks.len().saturating_mul(2))
                .saturating_add(2),
        );
        cut_points.extend([0, self.text.len()]);
        for run in &self.runs {
            cut_points.extend([run.range.start, run.range.end]);
        }
        for link in &self.links {
            cut_points.extend([link.range.start, link.range.end]);
        }
        for entity in &self.entities {
            cut_points.extend([entity.range.start, entity.range.end]);
        }
        for (line_index, range) in logical_line_ranges(&self.text).into_iter().enumerate() {
            if self.line_blocks[line_index] == SlackComposerLineBlock::Preformatted {
                cut_points.push(range.start);
                cut_points.push(if range.end < self.text.len() {
                    range.end + 1
                } else {
                    range.end
                });
            }
        }
        cut_points.sort_unstable();
        cut_points.dedup();
        cut_points
    }

    fn build_highlights(
        &self,
        cut_points: &[usize],
        palette: HighlightPalette,
    ) -> Vec<TextInputHighlight> {
        let mut highlights = Vec::with_capacity(cut_points.len().saturating_sub(1));
        for boundary in cut_points.windows(2) {
            let range = boundary[0]..boundary[1];
            if range.is_empty() {
                continue;
            }
            if let Some(highlight) = self.highlight_for_range(range, palette) {
                push_text_input_highlight(&mut highlights, highlight);
            }
        }
        highlights
    }

    fn highlight_for_range(
        &self,
        range: std::ops::Range<usize>,
        palette: HighlightPalette,
    ) -> Option<TextInputHighlight> {
        let style = self.style_at(range.start);
        let mentioned = self.entity_at(range.start).is_some();
        let linked = !mentioned && self.link_at(range.start).is_some();
        let uses_link_color = mentioned || linked;
        let code_block =
            self.line_block_at_offset(range.start) == SlackComposerLineBlock::Preformatted;
        if style.is_empty() && !code_block && !linked && !mentioned {
            return None;
        }
        let color = if uses_link_color {
            palette.link
        } else {
            palette.text
        };
        Some(TextInputHighlight {
            range,
            color,
            background: if code_block {
                Some(palette.code_block)
            } else {
                style.code.then_some(palette.code)
            },
            font_family: None,
            background_corner_radius: px(0.0),
            background_padding_x: px(0.0),
            background_inset_y: px(0.0),
            font_weight: style.bold.then_some(FontWeight::BOLD),
            font_style: style.italic.then_some(FontStyle::Italic),
            underline: (style.underline || linked).then_some(UnderlineStyle {
                color: Some(color),
                thickness: px(1.0),
                wavy: false,
            }),
            strikethrough: style.strikethrough.then_some(StrikethroughStyle {
                color: Some(color),
                thickness: px(1.0),
            }),
            monospace: style.code || code_block,
        })
    }

    fn cache_highlights(
        &mut self,
        highlights: Vec<TextInputHighlight>,
        palette: HighlightPalette,
    ) -> Arc<[TextInputHighlight]> {
        self.cached_highlights = highlights.into();
        self.cached_text_color = Some(palette.text);
        self.cached_code_background = Some(palette.code);
        self.cached_code_block_background = Some(palette.code_block);
        self.cached_link_color = Some(palette.link);
        self.highlights_dirty = false;
        self.cached_highlights.clone()
    }
}

fn push_text_input_highlight(
    highlights: &mut Vec<TextInputHighlight>,
    highlight: TextInputHighlight,
) {
    if let Some(previous) = highlights.last_mut() {
        if previous.range.end == highlight.range.start
            && previous.color == highlight.color
            && previous.background == highlight.background
            && previous.font_weight == highlight.font_weight
            && previous.font_style == highlight.font_style
            && previous.underline == highlight.underline
            && previous.strikethrough == highlight.strikethrough
            && previous.monospace == highlight.monospace
        {
            previous.range.end = highlight.range.end;
            return;
        }
    }
    highlights.push(highlight);
}
