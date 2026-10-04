use gpui::Window;

use super::{
    Context, SlackMessageTimestamp, SlackThreadListRow, SlackThreadReadIdentity,
    SlackThreadReadMetadata, SlackThreadReadReadiness, SlackThreadReadState, SlackThreadRequest,
    SurfaceState,
};

impl SurfaceState {
    pub(in crate::ui::surface::state::thread) fn install_slack_thread_read_load(
        &mut self,
        request: &SlackThreadRequest,
        read_metadata: Option<SlackThreadReadMetadata>,
    ) {
        if request.cursor.is_some() {
            if let Some(state) = self.slack_thread_read_state.as_mut().filter(|state| {
                state.generation == request.generation
                    && state.target.team_id == request.team_id
                    && state.target.conversation_id == request.conversation_id
                    && state.target.thread_timestamp == request.thread_timestamp
            }) {
                state.readiness = SlackThreadReadReadiness::AwaitingVisibility {
                    check_scheduled: false,
                };
            }
            return;
        }

        self.slack_thread_read_pending = None;
        self.slack_thread_read_timer = None;
        self.slack_thread_read_failures = 0;
        let Some(read_metadata) = read_metadata.filter(|metadata| {
            self.slack_workspace_api_capabilities.mark_thread_read
                && metadata.subscribed
                && metadata.unread_count > 0
        }) else {
            self.slack_thread_read_state = None;
            return;
        };
        self.slack_thread_read_state = Some(SlackThreadReadState {
            generation: request.generation,
            target: SlackThreadReadIdentity {
                team_id: request.team_id.clone(),
                conversation_id: request.conversation_id.clone(),
                thread_timestamp: request.thread_timestamp.clone(),
            },
            floor: read_metadata.last_read,
            unread_count: read_metadata.unread_count,
            readiness: SlackThreadReadReadiness::AwaitingVisibility {
                check_scheduled: false,
            },
        });
    }

    pub(crate) fn schedule_slack_thread_read_visibility_check(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.slack_thread_read_state.as_mut() else {
            return;
        };
        let SlackThreadReadReadiness::AwaitingVisibility { check_scheduled } = &mut state.readiness
        else {
            return;
        };
        if *check_scheduled {
            return;
        }
        *check_scheduled = true;
        let generation = state.generation;
        let target = state.target.clone();
        cx.on_next_frame(window, move |this, _window, cx| {
            this.finish_slack_thread_read_visibility_check(generation, &target, cx);
        });
    }

    fn finish_slack_thread_read_visibility_check(
        &mut self,
        generation: u64,
        target: &SlackThreadReadIdentity,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.slack_thread_read_state.as_mut() else {
            return;
        };
        if state.generation != generation || state.target != *target {
            return;
        }
        state.readiness = SlackThreadReadReadiness::AwaitingVisibility {
            check_scheduled: false,
        };
        if !self.slack_thread_read_identity_is_current(generation, target) {
            return;
        }
        let Some(message_timestamp) = self.newest_visible_slack_thread_reply_timestamp() else {
            return;
        };
        let Some(state) = self.slack_thread_read_state.as_mut() else {
            return;
        };
        if state.generation != generation || state.target != *target {
            return;
        }
        state.readiness = SlackThreadReadReadiness::Ready;
        self.queue_slack_thread_read_at(message_timestamp, cx);
    }

    pub(in crate::ui::surface::state) fn queue_slack_thread_read_for_visible_range(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        if visible_start >= visible_end {
            return;
        }
        let message_timestamp = self.slack_thread_panel.as_ref().and_then(|panel| {
            panel
                .list_rows
                .get(visible_start..visible_end.min(panel.list_rows.len()))?
                .iter()
                .filter_map(|row| match row {
                    SlackThreadListRow::Reply(reply_index) => panel
                        .reply_rows
                        .get(*reply_index)
                        .and_then(|reply| SlackMessageTimestamp::parse(reply.id.as_ref()).ok()),
                    SlackThreadListRow::Parent
                    | SlackThreadListRow::ReplyDivider
                    | SlackThreadListRow::Loading
                    | SlackThreadListRow::Error
                    | SlackThreadListRow::Composer => None,
                })
                .max_by_key(SlackMessageTimestamp::sort_key)
        });
        if let Some(message_timestamp) = message_timestamp {
            self.queue_slack_thread_read_at(message_timestamp, cx);
        }
    }

    pub(super) fn newest_visible_slack_thread_reply_timestamp(
        &self,
    ) -> Option<SlackMessageTimestamp> {
        let panel = self.slack_thread_panel.as_ref()?;
        let viewport = panel.list_state.viewport_bounds();
        if viewport.size.height <= gpui::px(0.0) {
            return None;
        }
        let mut index = panel
            .list_state
            .logical_scroll_top()
            .item_ix
            .min(panel.list_rows.len().checked_sub(1)?);
        let mut newest_visible = None;
        while let Some(bounds) = panel.list_state.bounds_for_item(index) {
            if bounds.top() >= viewport.bottom() {
                break;
            }
            if bounds.bottom() > viewport.top() {
                if let Some(timestamp) = panel.list_rows.get(index).and_then(|row| match row {
                    SlackThreadListRow::Reply(reply_index) => panel
                        .reply_rows
                        .get(*reply_index)
                        .and_then(|reply| SlackMessageTimestamp::parse(reply.id.as_ref()).ok()),
                    SlackThreadListRow::Parent
                    | SlackThreadListRow::ReplyDivider
                    | SlackThreadListRow::Loading
                    | SlackThreadListRow::Error
                    | SlackThreadListRow::Composer => None,
                }) {
                    if newest_visible
                        .as_ref()
                        .is_none_or(|current: &SlackMessageTimestamp| {
                            current.sort_key() < timestamp.sort_key()
                        })
                    {
                        newest_visible = Some(timestamp);
                    }
                }
            }
            index += 1;
            if index >= panel.list_rows.len() {
                break;
            }
        }
        newest_visible
    }
}
