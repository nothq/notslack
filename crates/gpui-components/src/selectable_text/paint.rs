use gpui::{fill, point, Bounds, TextAlign, TextLayout, Window};
use std::ops::Range;

pub(super) fn paint_text_selection(
    text_layout: &TextLayout,
    selected_range: &Range<usize>,
    selection_color: gpui::Hsla,
    text_align: TextAlign,
    window: &mut Window,
) {
    if selected_range.is_empty() {
        return;
    }
    let bounds = text_layout.bounds();
    let line_height = text_layout.line_height();
    let mut hard_line_start = 0;
    let mut visual_line_index = 0;
    for line in text_layout.line_layouts() {
        let mut visual_line_start = 0;
        let visual_line_ends = line
            .wrap_boundaries()
            .iter()
            .map(|boundary| line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index)
            .chain(std::iter::once(line.len()));
        for visual_line_end in visual_line_ends {
            let selection_start = selected_range
                .start
                .saturating_sub(hard_line_start)
                .max(visual_line_start);
            let selection_end = selected_range
                .end
                .saturating_sub(hard_line_start)
                .min(visual_line_end);
            if selection_start < selection_end {
                let unwrapped = &line.unwrapped_layout;
                let line_start_x = unwrapped.x_for_index(visual_line_start);
                let line_end_x = unwrapped.x_for_index(visual_line_end);
                let line_width = line_end_x - line_start_x;
                let origin_x = match text_align {
                    TextAlign::Left => bounds.left(),
                    TextAlign::Center => {
                        (bounds.left() * 2.0 + bounds.size.width - line_width) / 2.0
                    }
                    TextAlign::Right => bounds.right() - line_width,
                };
                let start_x = origin_x + unwrapped.x_for_index(selection_start) - line_start_x;
                let end_x = origin_x + unwrapped.x_for_index(selection_end) - line_start_x;
                let top = bounds.top() + line_height * visual_line_index as f32;
                window.paint_quad(fill(
                    Bounds::from_corners(point(start_x, top), point(end_x, top + line_height)),
                    selection_color,
                ));
            }
            visual_line_start = visual_line_end;
            visual_line_index += 1;
        }
        hard_line_start += line.len() + 1;
    }
}
