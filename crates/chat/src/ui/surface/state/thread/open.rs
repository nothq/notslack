mod context;

use super::{
    build_slack_thread_list_rows, normalize_slack_thread_parent_row,
    normalize_slack_thread_reply_row, px, shape_slack_thread_reply_rows, slack_message_timezone,
    slack_thread_broadcast_label, slack_thread_reply_label, slack_thread_row_order, Context,
    ListAlignment, ListScrollEvent, ListState, SharedString, SlackComposerFormatAction,
    SlackLaterThreadTarget, SlackMessageRow, SlackMessageTimestamp, SlackThreadListLayout,
    SlackThreadPanelOrigin, SlackThreadPanelState, SurfaceState, Tz,
    SLACK_THREAD_INITIAL_IMAGE_ROW_LIMIT, SLACK_THREAD_LIST_OVERDRAW,
};
use crate::ui::surface::{
    SlackComposerDestination, SlackComposerDraft, SlackComposerDraftKey, SlackMessageActionTarget,
};

struct SlackThreadPanelSeed {
    origin: SlackThreadPanelOrigin,
    conversation_id: String,
    conversation_name: String,
    parent_message_id: String,
    timezone: Tz,
    parent_row: SlackMessageRow,
    parent_hydrated: bool,
    reply_rows: Vec<SlackMessageRow>,
    expected_reply_count: u32,
    broadcast_label: Option<SharedString>,
    generation: u64,
    can_load_thread: bool,
}

impl SurfaceState {
    pub(crate) fn open_slack_message_thread_target(
        &mut self,
        target: &SlackMessageActionTarget,
        cx: &mut Context<Self>,
    ) {
        if self
            .slack_workspace()
            .is_none_or(|workspace| workspace.team_id != target.team_id())
        {
            return;
        }
        if let Some(panel) = self.slack_thread_panel.as_mut().filter(|panel| {
            panel.conversation_id == target.conversation_id()
                && panel.parent_message_id == target.root_timestamp().as_str()
        }) {
            panel.reply_composer_focused = true;
            cx.notify();
            return;
        }
        if self.slack_conversation_id() == Some(target.conversation_id()) {
            self.open_slack_thread_panel(target.root_timestamp().as_str(), cx);
            if self.slack_thread_panel.as_ref().is_some_and(|panel| {
                panel.conversation_id == target.conversation_id()
                    && panel.parent_message_id == target.root_timestamp().as_str()
            }) {
                if let Some(panel) = self.slack_thread_panel.as_mut() {
                    panel.reply_composer_focused = true;
                }
                cx.notify();
                return;
            }
        }
        self.open_slack_all_threads_thread(
            target.conversation_id(),
            target.root_timestamp().as_str(),
            cx,
        );
    }

    pub(crate) fn open_slack_thread_panel(
        &mut self,
        parent_message_id: &str,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace) = self.slack_workspace() else {
            return;
        };
        let Some(source_parent) = self.slack_thread_source_parent(parent_message_id) else {
            return;
        };
        let can_load_thread = self.slack_workspace_api_capabilities.load_thread;
        if !can_load_thread && source_parent.replies.is_empty() {
            return;
        }
        let timezone = slack_message_timezone(workspace.self_timezone_id.as_deref());
        let conversation_id = workspace.conversation_id.clone();
        let conversation_name = workspace.channel_name.clone();
        let conversation_kind = workspace.channel_kind;
        let expected_reply_count = source_parent.reply_count.unwrap_or(0);
        let mut reply_rows = source_parent.replies.clone();
        for row in &mut reply_rows {
            normalize_slack_thread_reply_row(row);
        }
        reply_rows.sort_by(slack_thread_row_order);
        reply_rows.dedup_by(|left, right| left.id == right.id);
        shape_slack_thread_reply_rows(&mut reply_rows, timezone);
        let mut parent_row = source_parent;
        normalize_slack_thread_parent_row(&mut parent_row);
        let generation = self.next_slack_thread_generation();
        self.check_in_current_slack_thread_draft();
        let panel = self.build_slack_thread_panel(
            SlackThreadPanelSeed {
                origin: SlackThreadPanelOrigin::Conversation,
                conversation_id,
                conversation_name: conversation_name.clone(),
                parent_message_id: parent_message_id.to_string(),
                timezone,
                parent_row,
                parent_hydrated: true,
                reply_rows,
                expected_reply_count,
                broadcast_label: slack_thread_broadcast_label(
                    conversation_kind,
                    &conversation_name,
                ),
                generation,
                can_load_thread,
            },
            cx,
        );
        self.install_slack_thread_panel(panel, true, can_load_thread, cx);
    }

    fn slack_thread_source_parent(&self, parent_message_id: &str) -> Option<SlackMessageRow> {
        self.slack_message_rows
            .iter()
            .find(|row| row.id == parent_message_id)
            .or_else(|| {
                self.slack_pins_rows
                    .iter()
                    .map(|row| &row.message)
                    .find(|row| row.id == parent_message_id)
            })
            .cloned()
    }

    pub(crate) fn open_slack_later_thread_detail(
        &mut self,
        target: SlackLaterThreadTarget,
        cx: &mut Context<Self>,
    ) {
        if self.slack_thread_panel.as_ref().is_some_and(|panel| {
            panel.origin.later_item_key() == Some(&target.item_key)
                && panel.conversation_id == target.conversation_id
                && panel.parent_message_id == target.thread_timestamp
                && panel.origin.later_selected_message_id()
                    == Some(target.selected_message_id.as_ref())
        }) {
            return;
        }
        let can_load_thread = self.slack_workspace_api_capabilities.load_thread;
        let timezone = slack_message_timezone(
            self.slack_workspace()
                .and_then(|workspace| workspace.self_timezone_id.as_deref()),
        );
        let selected_is_parent =
            target.selected_message_id.as_ref() == target.thread_timestamp.as_str();
        let mut reply_rows = (!selected_is_parent)
            .then(|| target.selected_message_row.clone())
            .into_iter()
            .collect::<Vec<_>>();
        shape_slack_thread_reply_rows(&mut reply_rows, timezone);
        let origin = SlackThreadPanelOrigin::Later {
            item_key: target.item_key,
            selected_message_id: target.selected_message_id,
        };
        let broadcast_label =
            slack_thread_broadcast_label(target.conversation_kind, &target.conversation_name);
        let generation = self.next_slack_thread_generation();
        self.check_in_current_slack_thread_draft();
        let panel = self.build_slack_thread_panel(
            SlackThreadPanelSeed {
                origin,
                conversation_id: target.conversation_id,
                conversation_name: target.conversation_name.to_string(),
                parent_message_id: target.thread_timestamp,
                timezone,
                parent_row: target.selected_message_row,
                parent_hydrated: selected_is_parent,
                reply_rows,
                expected_reply_count: target.expected_reply_count,
                broadcast_label,
                generation,
                can_load_thread,
            },
            cx,
        );
        self.install_slack_thread_panel(panel, false, can_load_thread, cx);
    }

    fn next_slack_thread_generation(&mut self) -> u64 {
        self.slack_thread_generation = self
            .slack_thread_generation
            .checked_add(1)
            .expect("Slack thread request generation overflowed");
        self.slack_thread_generation
    }

    fn build_slack_thread_panel(
        &mut self,
        seed: SlackThreadPanelSeed,
        cx: &mut Context<Self>,
    ) -> SlackThreadPanelState {
        let (reply_draft_key, reply_draft) =
            self.take_slack_thread_reply_draft(&seed.conversation_id, &seed.parent_message_id);
        let list_rows = build_slack_thread_list_rows(SlackThreadListLayout {
            is_conversation: seed.origin.is_conversation(),
            show_parent: seed.parent_hydrated,
            expected_reply_count: seed.expected_reply_count,
            reply_count: seed.reply_rows.len(),
            loading: false,
            has_error: false,
            show_composer: self.slack_workspace_api_capabilities.send_thread_reply,
        });
        let list_state = self.new_slack_thread_list_state(list_rows.len(), cx);
        SlackThreadPanelState {
            origin: seed.origin,
            conversation_id: seed.conversation_id,
            conversation_name: seed.conversation_name,
            parent_message_id: seed.parent_message_id,
            timezone: seed.timezone,
            parent_row: seed.parent_row,
            parent_hydrated: seed.parent_hydrated,
            reply_rows: seed.reply_rows.into(),
            list_rows,
            list_state,
            expected_reply_count: seed.expected_reply_count,
            reply_label: slack_thread_reply_label(seed.expected_reply_count),
            broadcast_label: seed.broadcast_label,
            generation: seed.generation,
            next_cursor: None,
            pagination_initialized: !seed.can_load_thread,
            loading: false,
            error: None,
            reply_draft_key,
            reply_draft: std::cell::RefCell::new(reply_draft),
            reply_formatting_enabled: true,
            reply_format_roving_target: SlackComposerFormatAction::Bold,
            reply_composer_focused: false,
            reply_error: None,
        }
    }

    fn take_slack_thread_reply_draft(
        &mut self,
        conversation_id: &str,
        parent_message_id: &str,
    ) -> (SlackComposerDraftKey, SlackComposerDraft) {
        let thread_timestamp = SlackMessageTimestamp::parse(parent_message_id)
            .expect("Slack thread panel parent must have a canonical message timestamp");
        let reply_draft_key = self
            .slack_composer_draft_key(SlackComposerDestination::Thread {
                conversation_id: conversation_id.to_string(),
                thread_timestamp,
            })
            .expect("Slack thread panel requires a connected workspace identity");
        let reply_draft = self.slack_composer_drafts.remove(&reply_draft_key);
        let reply_draft = match reply_draft {
            Some(draft) => draft,
            None => {
                let draft_id = self.next_slack_composer_draft_id();
                crate::ui::surface::SlackComposerDraft::new(draft_id)
            }
        };
        self.slack_send_draft_token_generation = self
            .slack_send_draft_token_generation
            .max(reply_draft.token);
        (reply_draft_key, reply_draft)
    }

    fn new_slack_thread_list_state(&self, row_count: usize, cx: &mut Context<Self>) -> ListState {
        let list_state = ListState::new(
            row_count,
            ListAlignment::Top,
            px(SLACK_THREAD_LIST_OVERDRAW),
        );
        list_state.set_scroll_handler(cx.listener(|this, event: &ListScrollEvent, _, cx| {
            this.handle_slack_thread_list_scroll(
                event.visible_range.start,
                event.visible_range.end,
                event.count,
                cx,
            );
        }));
        list_state
    }

    fn install_slack_thread_panel(
        &mut self,
        panel: SlackThreadPanelState,
        close_conversation_conflicts: bool,
        can_load_thread: bool,
        cx: &mut Context<Self>,
    ) {
        self.reset_slack_thread_read_context();
        self.slack_thread_panel = Some(panel);
        if close_conversation_conflicts {
            self.slack_aux_panel = None;
            self.slack_composer_aux_target = None;
            self.slack_mention_picker_state = None;
            self.slack_profile_panel = None;
            self.slack_composer_focused = false;
            self.slack_expanded_attachment = None;
            self.slack_dms_peek_visible = false;
        }
        let prefetch_end = self.slack_thread_panel.as_ref().map_or(0, |panel| {
            panel
                .list_rows
                .len()
                .min(SLACK_THREAD_INITIAL_IMAGE_ROW_LIMIT)
        });
        self.prefetch_slack_thread_images_for_range(0..prefetch_end, cx);
        cx.notify();
        if can_load_thread {
            self.begin_slack_thread_load(None, cx);
        }
    }
}
