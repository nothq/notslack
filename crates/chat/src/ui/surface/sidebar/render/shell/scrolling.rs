use super::{px, Context, ListOffset, SlackSidebarRow, SurfaceState};
use crate::ui::surface::SlackSidebarBoundaryTarget;
use gpui::Window;

impl SurfaceState {
    pub(crate) fn scroll_to_slack_sidebar_boundary(
        &mut self,
        target: SlackSidebarBoundaryTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_slack_sidebar_active_row_reveal();
        self.slack_sidebar_list_state.scroll_to(ListOffset {
            item_ix: target.row_index,
            offset_in_item: px(0.0),
        });
        self.mark_slack_remote_image_queue_dirty();
        cx.notify();
        cx.on_next_frame(window, |_, _, cx| {
            cx.notify();
        });
    }

    pub(super) fn slack_sidebar_visible_row_range(
        &self,
        row_count: usize,
    ) -> Option<(usize, usize)> {
        let viewport_bounds = self.slack_sidebar_list_state.viewport_bounds();
        let mut index = self
            .slack_sidebar_list_state
            .logical_scroll_top()
            .item_ix
            .min(row_count);
        let mut first = None;
        let mut last = None;
        while index < row_count {
            let Some(bounds) = self.slack_sidebar_list_state.bounds_for_item(index) else {
                break;
            };
            if bounds.top() >= viewport_bounds.bottom() {
                break;
            }
            if bounds.bottom() > viewport_bounds.top() {
                if first.is_none() {
                    first = Some(index);
                }
                last = Some(index);
            }
            index += 1;
        }
        first.zip(last)
    }
}

pub(super) fn slack_sidebar_boundary_targets(
    rows: &[SlackSidebarRow],
    visible_row_range: Option<(usize, usize)>,
) -> (
    Option<SlackSidebarBoundaryTarget>,
    Option<SlackSidebarBoundaryTarget>,
) {
    let Some((first_visible, last_visible)) = visible_row_range else {
        return (None, None);
    };
    let hidden_unread_above = first_visible
        .checked_sub(1)
        .and_then(|last_hidden| rows[last_hidden].boundary_state.last_target_at_or_before);
    let hidden_unread_below = last_visible
        .checked_add(1)
        .filter(|&first_hidden| first_hidden < rows.len())
        .and_then(|first_hidden| rows[first_hidden].boundary_state.next_target_at_or_after);
    (hidden_unread_above, hidden_unread_below)
}
