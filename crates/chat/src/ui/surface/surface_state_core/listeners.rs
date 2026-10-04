use super::{Context, ListScrollEvent, SurfaceState};

impl SurfaceState {
    pub(in crate::ui::surface::surface_state_core) fn register_slack_list_scroll_handlers(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        self.register_slack_primary_list_scroll_handlers(cx);
        self.register_slack_rail_list_scroll_handlers(cx);
        self.register_slack_secondary_list_scroll_handlers(cx);
    }

    fn register_slack_primary_list_scroll_handlers(&mut self, cx: &mut Context<Self>) {
        self.slack_message_list_state
            .set_scroll_handler(cx.listener(|this, event: &ListScrollEvent, _, cx| {
                this.handle_slack_message_list_scroll(
                    event.visible_range.start,
                    event.visible_range.end,
                    event.is_following_tail,
                    cx,
                );
            }));
        self.slack_home_finder_list_state
            .set_scroll_handler(cx.listener(|this, event: &ListScrollEvent, _, cx| {
                this.queue_slack_home_finder_visible_images(
                    event.visible_range.start,
                    event.visible_range.end,
                    cx,
                );
            }));
        self.slack_dm_list_state.set_scroll_handler(cx.listener(
            |this, event: &ListScrollEvent, _, cx| {
                this.handle_slack_dm_list_scroll(
                    event.visible_range.start,
                    event.visible_range.end,
                    event.count,
                    cx,
                );
            },
        ));
    }

    fn register_slack_rail_list_scroll_handlers(&mut self, cx: &mut Context<Self>) {
        self.slack_all_threads_list_state
            .set_scroll_handler(cx.listener(|this, event: &ListScrollEvent, _, cx| {
                this.handle_slack_all_threads_list_scroll(
                    event.visible_range.start,
                    event.visible_range.end,
                    event.count,
                    cx,
                );
            }));
        self.slack_activity_list_state
            .set_scroll_handler(cx.listener(|this, event: &ListScrollEvent, _, cx| {
                this.handle_slack_activity_list_scroll(
                    event.visible_range.start,
                    event.visible_range.end,
                    event.count,
                    cx,
                );
            }));
        self.slack_activity_detail_list_state
            .set_scroll_handler(cx.listener(|this, event: &ListScrollEvent, _, cx| {
                this.handle_slack_activity_detail_list_scroll(
                    event.visible_range.start,
                    event.visible_range.end,
                    cx,
                );
            }));
        self.slack_later_list_state.set_scroll_handler(cx.listener(
            |this, event: &ListScrollEvent, _, cx| {
                this.handle_slack_later_list_scroll(
                    event.visible_range.start,
                    event.visible_range.end,
                    event.count,
                    cx,
                );
            },
        ));
    }

    fn register_slack_secondary_list_scroll_handlers(&mut self, cx: &mut Context<Self>) {
        self.slack_drafts_sent_list_state
            .set_scroll_handler(cx.listener(|this, event: &ListScrollEvent, _, cx| {
                this.handle_slack_drafts_sent_scroll(
                    event.visible_range.start,
                    event.visible_range.end,
                    event.count,
                    cx,
                );
            }));
        self.slack_conversation_files_list_state
            .set_scroll_handler(cx.listener(|this, event: &ListScrollEvent, _, cx| {
                this.handle_slack_conversation_files_scroll(
                    event.visible_range.start,
                    event.visible_range.end,
                    event.count,
                    cx,
                );
            }));
        self.slack_pins_list_state.set_scroll_handler(cx.listener(
            |this, event: &ListScrollEvent, _, cx| {
                this.handle_slack_pins_scroll(
                    event.visible_range.start,
                    event.visible_range.end,
                    event.count,
                    cx,
                );
            },
        ));
        self.slack_search_list_state.set_scroll_handler(cx.listener(
            |this, event: &ListScrollEvent, _, cx| {
                this.handle_slack_search_list_scroll(
                    event.visible_range.start,
                    event.visible_range.end,
                    event.count,
                    cx,
                );
            },
        ));
    }
}
