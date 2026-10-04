use super::{
    px, Context, ListOffset, SharedString, SlackMessageNavigationHighlight,
    SlackMessageNavigationRequest, SlackMessageNavigationTarget, SlackMessageRenderContext,
    SurfaceState, SLACK_MESSAGE_HIGHLIGHT_DURATION,
};

struct SlackMissingThreadMessage<'a> {
    target_timestamp: &'a str,
    parent_timestamp: &'a str,
    loading: bool,
    next_cursor: Option<String>,
    pagination_initialized: bool,
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn continue_slack_message_navigation(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(request) = self.slack_message_navigation.clone() else {
            return false;
        };
        let Some(workspace) = self.slack_workspace() else {
            return false;
        };
        if workspace.conversation_id != request.target.conversation_id() {
            return false;
        }
        if request.target.thread_timestamp().is_none() {
            return self.continue_slack_conversation_message_navigation(&request, cx);
        }
        self.continue_slack_thread_message_navigation(&request, cx)
    }

    fn continue_slack_conversation_message_navigation(
        &mut self,
        request: &SlackMessageNavigationRequest,
        cx: &mut Context<Self>,
    ) -> bool {
        let target = &request.target;
        let Some(index) = self
            .slack_message_rows
            .iter()
            .position(|row| row.id == target.message_timestamp().as_str())
        else {
            return self.continue_slack_message_navigation_history(target, cx);
        };
        self.slack_message_list_auto_position_active = false;
        self.slack_message_list_state.scroll_to(ListOffset {
            item_ix: index,
            offset_in_item: px(0.0),
        });
        self.finish_slack_message_navigation(
            request.generation,
            target.message_timestamp().as_str(),
            SlackMessageRenderContext::Conversation,
            cx,
        );
        true
    }

    fn continue_slack_thread_message_navigation(
        &mut self,
        request: &SlackMessageNavigationRequest,
        cx: &mut Context<Self>,
    ) -> bool {
        let target = &request.target;
        let thread_timestamp = target
            .thread_timestamp()
            .expect("thread navigation requires a thread timestamp");
        let parent_timestamp = thread_timestamp.as_str();
        if !self.ensure_slack_navigation_thread(target, parent_timestamp, cx) {
            return true;
        }
        self.continue_loaded_slack_thread_message_navigation(request, parent_timestamp, cx)
    }

    fn ensure_slack_navigation_thread(
        &mut self,
        target: &SlackMessageNavigationTarget,
        parent_timestamp: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_thread_panel.as_ref().is_some_and(|panel| {
            panel.origin.is_conversation()
                && panel.conversation_id == target.conversation_id()
                && panel.parent_message_id == parent_timestamp
        }) {
            return true;
        }
        if !self
            .slack_message_rows
            .iter()
            .any(|row| row.id == parent_timestamp)
        {
            self.continue_slack_message_navigation_history(target, cx);
            return false;
        }
        self.open_slack_thread_panel(parent_timestamp, cx);
        if self.slack_thread_panel.is_none() {
            self.fail_slack_message_navigation(
                format!(
                    "Slack thread {} is unavailable in this conversation",
                    parent_timestamp
                ),
                cx,
            );
            return false;
        }
        true
    }

    fn continue_loaded_slack_thread_message_navigation(
        &mut self,
        request: &SlackMessageNavigationRequest,
        parent_timestamp: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let target = &request.target;
        let panel = self
            .slack_thread_panel
            .as_ref()
            .expect("Slack navigation thread was ensured above");
        let target_timestamp = target.message_timestamp().as_str();
        if target_timestamp == parent_timestamp {
            self.finish_slack_message_navigation(
                request.generation,
                target_timestamp,
                SlackMessageRenderContext::Thread,
                cx,
            );
            return true;
        }
        if let Some(index) = panel
            .reply_rows
            .iter()
            .position(|row| row.id == target_timestamp)
        {
            panel.list_state.scroll_to(ListOffset {
                item_ix: index,
                offset_in_item: px(0.0),
            });
            self.finish_slack_message_navigation(
                request.generation,
                target_timestamp,
                SlackMessageRenderContext::Thread,
                cx,
            );
            return true;
        }
        let loading = panel.loading;
        let next_cursor = panel.next_cursor.clone();
        let pagination_initialized = panel.pagination_initialized;
        self.continue_missing_slack_thread_message(
            SlackMissingThreadMessage {
                target_timestamp,
                parent_timestamp,
                loading,
                next_cursor,
                pagination_initialized,
            },
            cx,
        )
    }

    fn continue_missing_slack_thread_message(
        &mut self,
        missing: SlackMissingThreadMessage<'_>,
        cx: &mut Context<Self>,
    ) -> bool {
        if missing.loading {
            return true;
        }
        if let Some(cursor) = missing.next_cursor {
            self.begin_slack_thread_load(Some(cursor), cx);
            return true;
        }
        if !missing.pagination_initialized {
            self.begin_slack_thread_load(None, cx);
            return true;
        }
        self.fail_slack_message_navigation(
            format!(
                "Slack message {} was not present in thread {}",
                missing.target_timestamp, missing.parent_timestamp
            ),
            cx,
        );
        true
    }

    pub(in crate::ui::surface::state) fn continue_slack_message_navigation_history(
        &mut self,
        target: &SlackMessageNavigationTarget,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_message_history_request.is_some() {
            return true;
        }
        if self
            .slack_conversation_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.history_next_cursor.as_ref())
            .is_some()
        {
            self.start_slack_message_history_page_load(cx);
            return true;
        }
        self.fail_slack_message_navigation(
            format!(
                "Slack message {} was not present in conversation {}",
                target.message_timestamp().as_str(),
                target.conversation_id()
            ),
            cx,
        );
        true
    }

    pub(in crate::ui::surface::state) fn finish_slack_message_navigation(
        &mut self,
        generation: u64,
        message_timestamp: &str,
        render_context: SlackMessageRenderContext,
        cx: &mut Context<Self>,
    ) {
        let Some(request) = self
            .slack_message_navigation
            .as_ref()
            .filter(|request| request.generation == generation)
        else {
            return;
        };
        let conversation_id: SharedString = request.target.conversation_id().to_string().into();
        self.slack_message_navigation = None;
        self.slack_message_navigation_highlight = Some(SlackMessageNavigationHighlight {
            generation,
            conversation_id,
            message_timestamp: message_timestamp.to_string().into(),
            render_context,
        });
        self.slack_message_navigation_focus_pending = true;
        self.slack_error = None;
        self.spawn_timer_task(
            generation,
            SLACK_MESSAGE_HIGHLIGHT_DURATION,
            cx,
            |this, generation, cx| {
                if this
                    .slack_message_navigation_highlight
                    .as_ref()
                    .is_some_and(|highlight| highlight.generation == generation)
                {
                    this.slack_message_navigation_highlight = None;
                    cx.notify();
                }
            },
        );
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn fail_slack_message_navigation(
        &mut self,
        error: String,
        cx: &mut Context<Self>,
    ) {
        self.slack_message_navigation = None;
        self.slack_message_navigation_highlight = None;
        self.slack_message_navigation_focus_pending = false;
        self.slack_error = Some(error);
        cx.notify();
    }

    pub(crate) fn slack_message_is_navigation_highlight(
        &self,
        message_timestamp: &str,
        render_context: SlackMessageRenderContext,
    ) -> bool {
        let Some(workspace) = self.slack_workspace() else {
            return false;
        };
        self.slack_message_navigation_highlight
            .as_ref()
            .is_some_and(|highlight| {
                highlight.matches(
                    &workspace.conversation_id,
                    message_timestamp,
                    render_context,
                )
            })
    }
}
