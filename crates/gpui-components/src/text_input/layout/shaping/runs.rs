use std::ops::Range;

use gpui::{font, px, Font, Hsla, TextRun, UnderlineStyle};

use crate::text_input::TextInputHighlight;

pub(super) fn text_runs_for_text(
    text: &str,
    input_font: Font,
    color: Hsla,
    marked_range: Option<Range<usize>>,
    highlights: Option<&[TextInputHighlight]>,
) -> Vec<TextRun> {
    if text.is_empty() {
        return Vec::new();
    }
    let highlights = highlights.unwrap_or_default();
    let cut_points = text_run_cut_points(text, marked_range.as_ref(), highlights);
    let mut builder = TextRunBuilder {
        input_font,
        color,
        marked_range,
        highlights,
        highlight_index: 0,
    };
    cut_points
        .windows(2)
        .filter_map(|window| builder.run(window[0]..window[1]))
        .collect()
}

fn text_run_cut_points(
    text: &str,
    marked_range: Option<&Range<usize>>,
    highlights: &[TextInputHighlight],
) -> Vec<usize> {
    let mut cut_points = vec![0, text.len()];
    if let Some(range) = marked_range {
        cut_points.push(range.start.min(text.len()));
        cut_points.push(range.end.min(text.len()));
    }
    let mut previous_highlight_end = 0;
    for highlight in highlights {
        assert!(
            highlight.range.start <= highlight.range.end
                && highlight.range.end <= text.len()
                && text.is_char_boundary(highlight.range.start)
                && text.is_char_boundary(highlight.range.end),
            "text input highlight range must follow UTF-8 boundaries and fit the input"
        );
        assert!(
            previous_highlight_end <= highlight.range.start,
            "text input highlights must be sorted and non-overlapping"
        );
        previous_highlight_end = highlight.range.end;
        cut_points.push(highlight.range.start);
        cut_points.push(highlight.range.end);
    }
    cut_points.sort_unstable();
    cut_points.dedup();
    cut_points
}

struct TextRunBuilder<'a> {
    input_font: Font,
    color: Hsla,
    marked_range: Option<Range<usize>>,
    highlights: &'a [TextInputHighlight],
    highlight_index: usize,
}

impl TextRunBuilder<'_> {
    fn run(&mut self, range: Range<usize>) -> Option<TextRun> {
        if range.is_empty() {
            return None;
        }
        let marked = self.marked_range.as_ref().is_some_and(|marked_range| {
            range.start < marked_range.end
                && range.end > marked_range.start
                && !marked_range.is_empty()
        });
        while self
            .highlights
            .get(self.highlight_index)
            .is_some_and(|highlight| highlight.range.end <= range.start)
        {
            self.highlight_index += 1;
        }
        let highlight = self
            .highlights
            .get(self.highlight_index)
            .filter(|highlight| {
                highlight.range.start < range.end
                    && highlight.range.end > range.start
                    && !highlight.range.is_empty()
            });
        let mut run_font = highlight
            .and_then(|highlight| highlight.font_family.clone())
            .map(font)
            .unwrap_or_else(|| {
                if highlight.is_some_and(|highlight| highlight.monospace) {
                    font("Monaco")
                } else {
                    self.input_font.clone()
                }
            });
        if let Some(font_weight) = highlight.and_then(|highlight| highlight.font_weight) {
            run_font.weight = font_weight;
        }
        if let Some(font_style) = highlight.and_then(|highlight| highlight.font_style) {
            run_font.style = font_style;
        }
        Some(TextRun {
            len: range.len(),
            font: run_font,
            color: highlight.map_or(self.color, |highlight| highlight.color),
            background_color: highlight.and_then(|highlight| highlight.background),
            underline: if marked {
                Some(UnderlineStyle {
                    color: Some(self.color),
                    thickness: px(1.0),
                    wavy: false,
                })
            } else {
                highlight.and_then(|highlight| highlight.underline)
            },
            strikethrough: highlight.and_then(|highlight| highlight.strikethrough),
        })
    }
}
