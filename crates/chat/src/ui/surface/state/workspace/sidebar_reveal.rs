use super::{
    px, slack_sidebar_reveal_anchor, Context, ListOffset, SlackRailView, SlackSidebarRevealState,
    SlackSidebarRowKind, SurfaceState, Window,
};

impl SurfaceState {
    pub(crate) fn queue_slack_sidebar_active_row_reveal(&mut self, conversation_id: &str) {
        self.queue_slack_sidebar_active_row_reveal_for_stage(conversation_id, false);
    }

    pub(super) fn queue_slack_sidebar_active_row_reveal_for_stage(
        &mut self,
        conversation_id: &str,
        awaiting_live_sidebar: bool,
    ) {
        if conversation_id.is_empty() {
            return;
        }
        self.slack_sidebar_reveal = Some(SlackSidebarRevealState::pending(
            conversation_id.to_string(),
            awaiting_live_sidebar,
        ));
    }

    pub(crate) fn cancel_slack_sidebar_active_row_reveal(&mut self) {
        self.slack_sidebar_reveal = None;
    }

    pub(crate) fn schedule_slack_sidebar_active_row_reveal(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.slack_active_rail_view != SlackRailView::Home {
            return;
        }
        let Some(reveal) = self.slack_sidebar_reveal.as_mut() else {
            return;
        };
        if reveal.scheduled {
            return;
        }
        reveal.scheduled = true;
        cx.on_next_frame(window, |this, window, cx| {
            this.reveal_slack_active_sidebar_row(window, cx);
        });
    }

    fn reveal_slack_active_sidebar_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(mut reveal) = self.slack_sidebar_reveal.take() else {
            return;
        };
        if self.slack_active_rail_view != SlackRailView::Home {
            reveal.scheduled = false;
            self.slack_sidebar_reveal = Some(reveal);
            return;
        }
        let Some(active_index) = self.slack_sidebar_rows.iter().position(|row| {
            matches!(
                &row.kind,
                SlackSidebarRowKind::Item { item, .. }
                    if item.target_id == reveal.conversation_id
            )
        }) else {
            self.slack_sidebar_reveal = Some(reveal);
            return;
        };
        let viewport_bounds = self.slack_sidebar_list_state.viewport_bounds();
        if viewport_bounds.size.height <= px(0.0) {
            reveal.scheduled = false;
            self.slack_sidebar_reveal = Some(reveal);
            return;
        }
        if self
            .slack_sidebar_list_state
            .bounds_for_item(active_index)
            .is_some_and(|bounds| {
                bounds.top() >= viewport_bounds.top() && bounds.bottom() <= viewport_bounds.bottom()
            })
        {
            if reveal.awaiting_live_sidebar {
                self.slack_sidebar_reveal = Some(reveal);
            }
            return;
        }
        let anchor_index = slack_sidebar_reveal_anchor(
            &self.slack_sidebar_rows,
            active_index,
            viewport_bounds.size.height.as_f32(),
        );
        self.slack_sidebar_list_state.scroll_to(ListOffset {
            item_ix: anchor_index,
            offset_in_item: px(0.0),
        });
        if reveal.awaiting_live_sidebar {
            self.slack_sidebar_reveal = Some(reveal);
        }
        cx.notify();
        cx.on_next_frame(window, |_, _, cx| {
            cx.notify();
        });
    }
}
