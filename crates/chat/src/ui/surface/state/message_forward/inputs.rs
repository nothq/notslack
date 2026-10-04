use super::{
    normalize_slack_dm_finder_text, slack_message_forward_destination_input_style,
    slack_message_forward_note_input_style, Arc, Context, Entity, Rc, ScrollStrategy,
    SlackNewMessageCandidateKind, SurfaceState, TextInput, TextInputAction, TextInputChange,
    TextInputProps, SLACK_MESSAGE_FORWARD_INITIAL_VISIBLE_ROWS,
};

impl SurfaceState {
    pub(crate) fn slack_message_forward_destination_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let modal = self
            .slack_message_forward_modal
            .as_ref()
            .expect("forward destination input requires modal state");
        let surface = cx.entity().downgrade();
        let on_change: TextInputChange = Rc::new(move |value, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.set_slack_message_forward_query(value, cx);
                })
                .ok();
        });
        let surface = cx.entity().downgrade();
        let on_submit: TextInputAction = Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.activate_selected_slack_message_forward_destination(cx);
                })
                .ok();
        });
        let surface = cx.entity().downgrade();
        let on_escape: TextInputAction = Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.close_slack_message_forward(cx);
                })
                .ok();
        });
        let on_up = Self::slack_message_forward_move_action(cx, -1);
        let on_down = Self::slack_message_forward_move_action(cx, 1);
        let props = TextInputProps::single_line(modal.query.clone())
            .placeholder("Search for channel or person")
            .style(slack_message_forward_destination_input_style())
            .bordered(false)
            .accessibility(
                self.slack_message_forward_destination_accessibility_id
                    .clone(),
                "Forward message destination",
            )
            .on_change(on_change)
            .on_submit(on_submit)
            .on_escape(on_escape)
            .on_up(on_up)
            .on_down(on_down);
        self.slack_message_forward_destination_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_message_forward_destination_input.clone()
    }

    fn slack_message_forward_move_action(
        cx: &mut Context<Self>,
        direction: i32,
    ) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.move_slack_message_forward_selection(direction, cx);
                })
                .ok();
        })
    }

    pub(crate) fn slack_message_forward_note_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let modal = self
            .slack_message_forward_modal
            .as_ref()
            .expect("forward note input requires modal state");
        let surface = cx.entity().downgrade();
        let on_change: TextInputChange = Rc::new(move |value, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    let Some(modal) = surface.slack_message_forward_modal.as_mut() else {
                        return;
                    };
                    if modal.note != value {
                        modal.note = value;
                        modal.error = None;
                        cx.notify();
                    }
                })
                .ok();
        });
        let surface = cx.entity().downgrade();
        let on_escape: TextInputAction = Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.close_slack_message_forward(cx);
                })
                .ok();
        });
        let props = TextInputProps::multiline(modal.note.clone())
            .placeholder("Add a message, if you’d like")
            .style(slack_message_forward_note_input_style())
            .accessibility(
                self.slack_message_forward_note_accessibility_id.clone(),
                "Forward message note",
            )
            .on_change(on_change)
            .on_escape(on_escape);
        self.slack_message_forward_note_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_message_forward_note_input.clone()
    }

    pub(in crate::ui::surface::state) fn set_slack_message_forward_query(
        &mut self,
        query: String,
        cx: &mut Context<Self>,
    ) {
        let Some(modal) = self.slack_message_forward_modal.as_mut() else {
            return;
        };
        if modal.destination.is_some() || modal.query == query {
            return;
        }
        modal.query = query;
        modal.normalized_query = normalize_slack_dm_finder_text(&modal.query).into();
        modal.error = None;
        self.rebuild_slack_message_forward_results();
        self.queue_slack_message_forward_visible_images(
            0,
            SLACK_MESSAGE_FORWARD_INITIAL_VISIBLE_ROWS,
            cx,
        );
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn rebuild_slack_message_forward_results(&mut self) {
        let open_people = self.slack_workspace_api_capabilities.open_conversation;
        let Some(modal) = self.slack_message_forward_modal.as_mut() else {
            return;
        };
        if modal.destination.is_some() {
            modal.visible_row_indices = Arc::default();
            modal.selected_index = None;
            return;
        }
        let query = modal.normalized_query.as_ref();
        modal.visible_row_indices = modal
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                (open_people || row.kind != SlackNewMessageCandidateKind::Person)
                    && (query.is_empty() || row.search_key.contains(query))
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>()
            .into();
        modal.selected_index = None;
        if !modal.visible_row_indices.is_empty() {
            modal.scroll_handle.scroll_to_item(0, ScrollStrategy::Top);
        }
    }

    pub(crate) fn move_slack_message_forward_selection(
        &mut self,
        direction: i32,
        cx: &mut Context<Self>,
    ) {
        let Some(modal) = self
            .slack_message_forward_modal
            .as_mut()
            .filter(|modal| modal.destination.is_none())
        else {
            return;
        };
        let count = modal.visible_row_indices.len();
        if count == 0 {
            return;
        }
        let next = match modal.selected_index {
            None if direction < 0 => count - 1,
            None => 0,
            Some(current) if direction < 0 => current.saturating_sub(1),
            Some(current) => current.saturating_add(1).min(count - 1),
        };
        modal.selected_index = Some(next);
        modal
            .scroll_handle
            .scroll_to_item(next, ScrollStrategy::Nearest);
        self.queue_slack_message_forward_visible_images(next, next.saturating_add(1), cx);
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn activate_selected_slack_message_forward_destination(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let Some(modal) = self.slack_message_forward_modal.as_ref() else {
            return;
        };
        if modal.visible_row_indices.is_empty() {
            return;
        }
        self.select_slack_message_forward_destination(modal.selected_index.unwrap_or(0), cx);
    }
}
