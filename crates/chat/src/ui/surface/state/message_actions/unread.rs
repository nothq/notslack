use super::{
    build_slack_dm_rows, mark_slack_sidebar_conversation_unread,
    prepare_slack_conversation_snapshot, ClipboardItem, Context, SlackConversationLiveTarget,
    SlackConversationReadReadiness, SlackMessageMarkUnreadRequest, SlackMessagePermalinkRequest,
    SlackMessageTimestamp, SurfaceState, SLACK_HISTORY_START_TIMESTAMP,
};

pub(super) struct SlackMessageMarkUnreadTarget {
    pub(super) team_id: String,
    pub(super) conversation_id: String,
    pub(super) target_timestamp: SlackMessageTimestamp,
    pub(super) cursor: SlackMessageTimestamp,
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn start_slack_message_permalink_copy(
        &mut self,
        team_id: String,
        conversation_id: String,
        message_timestamp: SlackMessageTimestamp,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.load_message_permalink
            || self.slack_pending_message_permalink.is_some()
            || self
                .slack_workspace()
                .is_none_or(|workspace| workspace.team_id != team_id)
        {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_error = Some("missing Slack workspace api".to_string());
            cx.notify();
            return;
        };
        self.slack_message_permalink_generation = self
            .slack_message_permalink_generation
            .checked_add(1)
            .expect("Slack message permalink request generation overflowed");
        let request = SlackMessagePermalinkRequest {
            generation: self.slack_message_permalink_generation,
            team_id,
            conversation_id,
            message_timestamp,
        };
        self.slack_pending_message_permalink = Some(request.clone());
        self.slack_error = None;
        cx.notify();

        let completion_request = request.clone();
        self.spawn_background_task(
            request,
            cx,
            move |request| {
                workspace_api.load_slack_message_permalink(
                    &request.conversation_id,
                    &request.message_timestamp,
                )
            },
            move |this, result, cx| {
                this.finish_slack_message_permalink_copy(&completion_request, result, cx);
            },
        );
    }

    fn finish_slack_message_permalink_copy(
        &mut self,
        request: &SlackMessagePermalinkRequest,
        result: Result<String, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_pending_message_permalink.as_ref() != Some(request) {
            return;
        }
        self.slack_pending_message_permalink = None;
        if self.slack_message_permalink_generation != request.generation
            || self
                .slack_workspace()
                .is_none_or(|workspace| workspace.team_id != request.team_id)
        {
            cx.notify();
            return;
        }
        match result {
            Ok(permalink) => cx.write_to_clipboard(ClipboardItem::new_string(permalink)),
            Err(message) => self.slack_error = Some(message),
        }
        cx.notify();
    }

    pub(super) fn start_slack_message_mark_unread(
        &mut self,
        target: SlackMessageMarkUnreadTarget,
        cx: &mut Context<Self>,
    ) {
        let SlackMessageMarkUnreadTarget {
            team_id,
            conversation_id,
            target_timestamp,
            cursor,
        } = target;
        if !self.slack_workspace_api_capabilities.mark_conversation_read
            || self.slack_pending_message_mark_unread.is_some()
            || self.slack_conversation_read_request.is_some()
            || !self.slack_message_action_target_is_current(&team_id, &conversation_id)
            || self
                .slack_message_mark_unread_cursor(target_timestamp.as_str())
                .as_ref()
                != Some(&cursor)
        {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_error = Some("missing Slack workspace api".to_string());
            cx.notify();
            return;
        };
        self.slack_message_mark_unread_generation = self
            .slack_message_mark_unread_generation
            .checked_add(1)
            .expect("Slack mark-unread request generation overflowed");
        let request = SlackMessageMarkUnreadRequest {
            generation: self.slack_message_mark_unread_generation,
            team_id,
            conversation_id,
            target_timestamp,
            cursor,
        };
        self.clear_slack_conversation_read_overlay_for_id(&request.conversation_id);
        self.slack_conversation_read_pending = None;
        self.slack_conversation_read_timer = None;
        self.slack_pending_message_mark_unread = Some(request.clone());
        self.slack_error = None;
        cx.notify();

        let completion_request = request.clone();
        self.spawn_background_task(
            request,
            cx,
            move |request| {
                workspace_api
                    .mark_slack_conversation_read(&request.conversation_id, &request.cursor)
            },
            move |this, result, cx| {
                this.finish_slack_message_mark_unread(&completion_request, result, cx);
            },
        );
    }

    pub(in crate::ui::surface::state) fn finish_slack_message_mark_unread(
        &mut self,
        request: &SlackMessageMarkUnreadRequest,
        result: Result<crate::ui::SlackConversationReadReceipt, String>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_pending_message_mark_unread.as_ref() != Some(request) {
            return;
        }
        self.slack_pending_message_mark_unread = None;
        self.schedule_queued_slack_conversation_reconciliation(cx);
        if self.slack_message_mark_unread_generation != request.generation
            || !self
                .slack_message_action_target_is_current(&request.team_id, &request.conversation_id)
        {
            cx.notify();
            return;
        }
        let receipt = match result {
            Ok(receipt)
                if receipt.team_id == request.team_id
                    && receipt.conversation_id == request.conversation_id
                    && receipt.last_read.as_str() == request.cursor.as_str() =>
            {
                receipt
            }
            Ok(_) => {
                self.slack_error =
                    Some("Slack mark-unread receipt targeted another conversation".to_string());
                cx.notify();
                return;
            }
            Err(message) => {
                self.slack_error = Some(message);
                cx.notify();
                return;
            }
        };

        let Some(mut snapshot) = self.slack_conversation_snapshot.clone() else {
            return;
        };
        snapshot.last_read = Some(receipt.last_read);
        snapshot.last_read_boundary_loaded = true;
        let prepared = prepare_slack_conversation_snapshot(snapshot);
        self.slack_conversation_read_state = None;
        self.apply_prepared_slack_conversation_snapshot(prepared, cx);
        self.hold_slack_conversation_unread_cursor(request);
        self.apply_slack_manual_unread_sidebar_state(request, cx);
    }

    pub(in crate::ui::surface::state) fn hold_slack_conversation_unread_cursor(
        &mut self,
        request: &SlackMessageMarkUnreadRequest,
    ) {
        let Some(state) = self.slack_conversation_read_state.as_mut() else {
            return;
        };
        if state.target
            != (SlackConversationLiveTarget {
                team_id: request.team_id.clone(),
                conversation_id: request.conversation_id.clone(),
            })
        {
            return;
        }
        state.floor = Some(request.cursor.clone());
        state.readiness = SlackConversationReadReadiness::HeldUnread;
    }

    pub(in crate::ui::surface::state) fn apply_slack_manual_unread_sidebar_state(
        &mut self,
        request: &SlackMessageMarkUnreadRequest,
        cx: &mut Context<Self>,
    ) {
        let workspace_changed = self
            .slack_workspace_mut()
            .filter(|workspace| workspace.team_id == request.team_id)
            .is_some_and(|workspace| {
                mark_slack_sidebar_conversation_unread(
                    &mut workspace.sections,
                    &request.conversation_id,
                )
            });
        let sidebar_changed = self
            .slack_sidebar_snapshot
            .as_mut()
            .filter(|snapshot| snapshot.team_id == request.team_id)
            .is_some_and(|snapshot| {
                mark_slack_sidebar_conversation_unread(
                    &mut snapshot.sections,
                    &request.conversation_id,
                )
            });
        let dm_changed = self
            .slack_dm_inbox_snapshot
            .as_mut()
            .filter(|snapshot| snapshot.team_id == request.team_id)
            .and_then(|snapshot| {
                snapshot
                    .items
                    .iter_mut()
                    .find(|item| item.conversation_id == request.conversation_id)
            })
            .is_some_and(|item| {
                let changed = !item.unread;
                item.unread = true;
                changed
            });
        if workspace_changed || sidebar_changed {
            self.refresh_slack_sidebar_rows();
        }
        self.apply_slack_dm_manual_unread_change(dm_changed);
        if workspace_changed || sidebar_changed || dm_changed {
            cx.notify();
        }
    }

    fn apply_slack_dm_manual_unread_change(&mut self, changed: bool) {
        if !changed {
            return;
        }
        self.slack_dm_rows = build_slack_dm_rows(
            self.slack_dm_inbox_snapshot
                .as_ref()
                .expect("changed Slack DM unread state requires an inbox snapshot"),
        );
        {
            let (authority, data) = (&mut self.slack_presence_authority, &mut self.data);
            authority.overlay_dm_rows(&mut data.slack_dm_rows);
        }
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_dm(data);
        }
        self.refresh_slack_dm_visible_rows();
    }

    pub(crate) fn slack_message_action_target_is_current(
        &self,
        team_id: &str,
        conversation_id: &str,
    ) -> bool {
        self.slack_workspace().is_some_and(|workspace| {
            workspace.team_id == team_id && workspace.conversation_id == conversation_id
        }) && self
            .slack_conversation_snapshot
            .as_ref()
            .is_some_and(|snapshot| {
                snapshot.team_id == team_id && snapshot.conversation_id == conversation_id
            })
            && self
                .slack_pending_conversation_id
                .as_deref()
                .is_none_or(|pending| pending == conversation_id)
    }

    pub(crate) fn slack_message_mark_unread_cursor(
        &self,
        target_message_id: &str,
    ) -> Option<SlackMessageTimestamp> {
        let workspace = self.slack_workspace()?;
        let self_user_id = workspace.self_user_id.as_deref()?;
        let snapshot = self.slack_conversation_snapshot.as_ref()?;
        let target_index = snapshot
            .messages
            .iter()
            .position(|message| message.id == target_message_id)?;
        let target_is_self =
            snapshot.messages[target_index].user_id.as_deref() == Some(self_user_id);
        let boundary_index = if target_is_self {
            snapshot.messages[..target_index]
                .iter()
                .rposition(|message| message.user_id.as_deref() != Some(self_user_id))
                .unwrap_or(target_index)
        } else {
            target_index
        };
        let predecessor_index = boundary_index.checked_sub(1);
        match predecessor_index {
            Some(index) => SlackMessageTimestamp::parse(&snapshot.messages[index].id).ok(),
            None if snapshot.history_next_cursor.is_none() => {
                SlackMessageTimestamp::parse(SLACK_HISTORY_START_TIMESTAMP).ok()
            }
            None => None,
        }
    }
}
