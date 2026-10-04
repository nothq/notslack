use super::{
    next_slack_new_message_generation, normalize_slack_dm_finder_text,
    slack_new_message_draft_key_for_conversation, slack_new_message_draft_key_for_people, Arc,
    Context, ScrollStrategy, SlackDestinationTarget, SlackMainRoute, SlackNewMessageDestination,
    SlackNewMessagePerson, SurfaceState, SLACK_NEW_MESSAGE_INITIAL_VISIBLE_ROWS,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn set_slack_new_message_query(
        &mut self,
        query: String,
        cx: &mut Context<Self>,
    ) {
        if self.slack_main_route != SlackMainRoute::NewMessage
            || self.slack_new_message_query == query
        {
            return;
        }
        self.slack_new_message_query = query;
        self.slack_new_message_normalized_query =
            normalize_slack_dm_finder_text(&self.slack_new_message_query).into();
        self.slack_new_message_to_focused = true;
        self.rebuild_slack_new_message_results();
        self.queue_slack_new_message_visible_images(0, SLACK_NEW_MESSAGE_INITIAL_VISIBLE_ROWS, cx);
        cx.notify();
    }

    pub(crate) fn clear_slack_new_message_query(&mut self, cx: &mut Context<Self>) {
        if self.slack_new_message_query.is_empty() {
            return;
        }
        self.slack_new_message_query.clear();
        self.slack_new_message_normalized_query = Default::default();
        self.slack_new_message_to_focused = true;
        self.rebuild_slack_new_message_results();
        self.queue_slack_new_message_visible_images(0, SLACK_NEW_MESSAGE_INITIAL_VISIBLE_ROWS, cx);
        cx.notify();
    }

    pub(crate) fn rebuild_slack_new_message_results(&mut self) {
        if self.slack_main_route != SlackMainRoute::NewMessage || !self.slack_new_message_to_focused
        {
            self.slack_new_message_visible_row_indices = Arc::default();
            self.slack_new_message_selected_index = None;
            self.slack_new_message_prefetched_range = None;
            return;
        }
        let query = self.slack_new_message_normalized_query.as_ref();
        self.slack_new_message_visible_row_indices = self
            .slack_new_message_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                !self
                    .slack_new_message_selected_people
                    .iter()
                    .any(|person| person.user_id.as_ref() == row.target.stable_id())
                    && (query.is_empty() || row.search_key.contains(query))
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
            .into();
        self.slack_new_message_selected_index = None;
        if !self.slack_new_message_visible_row_indices.is_empty() {
            self.slack_new_message_scroll_handle
                .scroll_to_item(0, ScrollStrategy::Top);
        }
        self.slack_new_message_prefetched_range = None;
    }

    pub(crate) fn select_slack_new_message_candidate(
        &mut self,
        visible_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(row) = self
            .slack_new_message_visible_row_indices
            .get(visible_index)
            .and_then(|row_index| self.slack_new_message_rows.get(*row_index))
            .cloned()
        else {
            return;
        };
        self.cancel_slack_composer_capture_for_owner_change(cx);
        self.store_slack_new_message_draft(cx);
        self.slack_new_message_query.clear();
        self.slack_new_message_normalized_query = Default::default();
        self.slack_new_message_error = None;
        self.assert_slack_new_message_draft_moved();
        match row.target.clone() {
            SlackDestinationTarget::Conversation {
                conversation_id,
                kind,
            } => self.select_slack_new_message_conversation(row, conversation_id, kind, cx),
            SlackDestinationTarget::Person { user_id } => {
                if !self.select_slack_new_message_person(row, user_id, cx) {
                    return;
                }
            }
        }
        self.slack_new_message_to_focused = true;
        self.rebuild_slack_new_message_results();
        cx.notify();
    }

    fn assert_slack_new_message_draft_moved(&self) {
        assert!(
            self.slack_composer_files.is_empty(),
            "storing a Slack new-message draft must move every attached file"
        );
    }

    fn select_slack_new_message_conversation(
        &mut self,
        row: crate::ui::surface::SlackNewMessageCandidateRow,
        conversation_id: String,
        kind: crate::ui::SlackConversationKind,
        cx: &mut Context<Self>,
    ) {
        self.cancel_pending_slack_new_message_conversation_load(cx);
        self.slack_new_message_selected_people.clear();
        self.slack_new_message_destination = Some(SlackNewMessageDestination {
            conversation_id: conversation_id.clone().into(),
            label: row.label,
            kind,
        });
        self.slack_new_message_active_draft_key = Some(
            slack_new_message_draft_key_for_conversation(&conversation_id),
        );
        self.restore_slack_new_message_draft();
        self.slack_new_message_open_generation =
            next_slack_new_message_generation(self.slack_new_message_open_generation);
        self.slack_new_message_pending_open = None;
        self.select_slack_conversation_for_new_message(&conversation_id, cx);
    }

    fn select_slack_new_message_person(
        &mut self,
        row: crate::ui::surface::SlackNewMessageCandidateRow,
        user_id: String,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_new_message_selected_people.len()
            >= crate::model::SLACK_NEW_MESSAGE_MAX_PEOPLE
        {
            self.slack_new_message_error =
                Some("Slack group messages support up to 8 selected people.".to_string());
            cx.notify();
            return false;
        }
        self.cancel_pending_slack_new_message_conversation_load(cx);
        self.slack_new_message_selected_people
            .push(SlackNewMessagePerson {
                user_id: user_id.into(),
                label: row.label,
                avatar_image_url: row.avatar_image_url,
            });
        self.slack_new_message_destination = None;
        self.slack_new_message_active_draft_key = Some(slack_new_message_draft_key_for_people(
            &self.slack_new_message_selected_people,
        ));
        self.restore_slack_new_message_draft();
        self.queue_slack_new_message_people_open(cx);
        true
    }

    pub(in crate::ui::surface::state) fn activate_selected_slack_new_message_candidate(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if self.slack_new_message_visible_row_indices.is_empty() {
            return;
        }
        self.select_slack_new_message_candidate(
            self.slack_new_message_selected_index.unwrap_or(0),
            cx,
        );
    }

    pub(in crate::ui::surface::state) fn move_slack_new_message_selection(
        &mut self,
        direction: i32,
        cx: &mut Context<Self>,
    ) {
        let count = self.slack_new_message_visible_row_indices.len();
        if count == 0 {
            return;
        }
        let next = match self.slack_new_message_selected_index {
            None if direction < 0 => count - 1,
            None => 0,
            Some(current) if direction < 0 => current.saturating_sub(1),
            Some(current) => current.saturating_add(1).min(count - 1),
        };
        self.slack_new_message_selected_index = Some(next);
        self.slack_new_message_scroll_handle
            .scroll_to_item(next, ScrollStrategy::Nearest);
        self.queue_slack_new_message_visible_images(next, next.saturating_add(1), cx);
        cx.notify();
    }

    pub(crate) fn remove_slack_new_message_person(
        &mut self,
        user_id: &str,
        cx: &mut Context<Self>,
    ) {
        if self.slack_main_route != SlackMainRoute::NewMessage {
            return;
        }
        self.store_slack_new_message_draft(cx);
        let previous_count = self.slack_new_message_selected_people.len();
        self.slack_new_message_selected_people
            .retain(|person| person.user_id.as_ref() != user_id);
        if previous_count == self.slack_new_message_selected_people.len() {
            return;
        }
        self.cancel_pending_slack_new_message_conversation_load(cx);
        self.slack_new_message_destination = None;
        self.assert_slack_new_message_draft_moved();
        if self.slack_new_message_selected_people.is_empty() {
            self.slack_new_message_active_draft_key = None;
            self.replace_slack_send_draft_document(Default::default());
            self.slack_composer_focused = false;
            self.slack_new_message_open_generation =
                next_slack_new_message_generation(self.slack_new_message_open_generation);
            self.slack_new_message_pending_open = None;
        } else {
            self.slack_new_message_active_draft_key = Some(slack_new_message_draft_key_for_people(
                &self.slack_new_message_selected_people,
            ));
            self.restore_slack_new_message_draft();
            self.queue_slack_new_message_people_open(cx);
        }
        self.slack_new_message_to_focused = true;
        self.rebuild_slack_new_message_results();
        cx.notify();
    }

    pub(crate) fn clear_slack_new_message_destination(&mut self, cx: &mut Context<Self>) {
        if self.slack_main_route != SlackMainRoute::NewMessage {
            return;
        }
        self.store_slack_new_message_draft(cx);
        self.cancel_pending_slack_new_message_conversation_load(cx);
        self.slack_new_message_destination = None;
        self.slack_new_message_active_draft_key = None;
        self.replace_slack_send_draft_document(Default::default());
        self.assert_slack_new_message_draft_moved();
        self.slack_new_message_open_generation =
            next_slack_new_message_generation(self.slack_new_message_open_generation);
        self.slack_new_message_pending_open = None;
        self.slack_new_message_to_focused = true;
        self.rebuild_slack_new_message_results();
        cx.notify();
    }
}
