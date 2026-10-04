use gpui::{px, Context, Pixels, Window};

use super::super::{
    layout, shape_text_layout, text_input_caret_width, TextInput, TextInputMode, TextInputWrapMode,
};

impl TextInput {
    pub fn measured_visible_line_count(&self) -> usize {
        self.visible_line_count(self.measured_visual_line_count)
    }

    pub fn measured_line_count(&self) -> usize {
        self.measured_visual_line_count.max(1)
    }

    fn visible_line_count(&self, line_count: usize) -> usize {
        match self.mode {
            TextInputMode::SingleLine => 1,
            TextInputMode::Multiline { max_visible_lines } => max_visible_lines
                .map(|max_lines| line_count.min(max_lines).max(1))
                .unwrap_or(line_count.max(1)),
        }
    }

    pub(in crate::text_input) fn invalidate_layout(&mut self) {
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.layout_cache = None;
    }

    pub(in crate::text_input) fn refresh_layout_for_last_width(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self.last_bounds else {
            return;
        };
        self.refresh_layout(bounds.size.width, window, cx);
    }

    pub(in crate::text_input) fn refresh_layout(
        &mut self,
        width: Pixels,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let render_placeholder = self.content.is_empty();
        if self
            .layout_cache
            .as_ref()
            .is_some_and(|cache| cache.matches(self.layout_revision, width, render_placeholder))
        {
            return;
        }

        let previous_visible_line_count = self.measured_visible_line_count();
        let cache = shape_text_layout(self, width, render_placeholder, window);
        self.measured_visual_line_count = if render_placeholder {
            1
        } else {
            cache.visual_line_count
        };
        self.layout_cache = Some(cache);
        self.ensure_cursor_visible();

        if self.measured_visible_line_count() != previous_visible_line_count {
            if let Some(on_layout_change) = self.callbacks.on_layout_change.clone() {
                cx.defer_in(window, move |_input, window, cx| {
                    on_layout_change(window, cx);
                });
            }
        }
    }

    pub(in crate::text_input) fn layout_lines(&self) -> &[super::super::TextLayoutLine] {
        self.layout_cache
            .as_ref()
            .map_or(&[], |cache| cache.lines.as_ref())
    }

    fn total_content_height(&self) -> Pixels {
        self.style.line_height * self.measured_visual_line_count.max(1)
    }

    fn viewport_content_height(&self) -> Pixels {
        self.style.line_height * self.measured_visible_line_count()
    }

    pub(in crate::text_input) fn max_scroll_y(&self) -> Pixels {
        (self.total_content_height() - self.viewport_content_height()).max(px(0.0))
    }

    pub(in crate::text_input) fn max_scroll_x(&self) -> Pixels {
        if self.content.is_empty() || self.soft_wraps() {
            return px(0.0);
        }
        self.layout_cache.as_ref().map_or(px(0.0), |cache| {
            (cache.content_width + text_input_caret_width() - cache.width).max(px(0.0))
        })
    }

    pub(in crate::text_input) const fn soft_wraps(&self) -> bool {
        matches!(self.mode, TextInputMode::Multiline { .. })
            && matches!(self.wrap_mode, TextInputWrapMode::SoftWrap)
    }

    pub(in crate::text_input) fn ensure_cursor_visible(&mut self) {
        let Some(position) = layout::content_position_for_offset(
            self.cursor_offset(),
            self.layout_lines(),
            self.style.line_height,
        ) else {
            self.scroll_x = px(0.0);
            self.scroll_y = px(0.0);
            return;
        };
        let viewport_width = self
            .layout_cache
            .as_ref()
            .map_or(px(0.0), |cache| cache.width);
        if position.x < self.scroll_x {
            self.scroll_x = position.x;
        } else if position.x + text_input_caret_width() > self.scroll_x + viewport_width {
            self.scroll_x = position.x + text_input_caret_width() - viewport_width;
        }
        self.scroll_x = self.scroll_x.clamp(px(0.0), self.max_scroll_x());
        let viewport_height = self.viewport_content_height();
        if position.y < self.scroll_y {
            self.scroll_y = position.y;
        } else if position.y + self.style.line_height > self.scroll_y + viewport_height {
            self.scroll_y = position.y + self.style.line_height - viewport_height;
        }
        self.scroll_y = self.scroll_y.clamp(px(0.0), self.max_scroll_y());
    }
}
