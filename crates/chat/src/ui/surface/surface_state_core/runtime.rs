use std::{collections::HashMap, sync::Arc, time::Duration};

use super::{
    build_slack_remote_images, div, px, rgb, spawn_background_task_for_entity,
    spawn_timer_task_for_entity, AnyElement, App, Context, Image, IntoElement, KeyDownEvent,
    ListAlignment, ListOffset, ListState, ParentElement, SlackMessageRow, Styled, SurfaceState,
    Window, SLACK_MESSAGE_LIST_OVERDRAW, SLACK_SIDEBAR_LIST_OVERDRAW,
};

impl SurfaceState {
    pub(crate) fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.slack_text_entry_focused(window, cx) {
            return;
        }
        if self.handle_slack_key_down(event, cx) {
            cx.stop_propagation();
        }
    }

    pub(crate) fn slack_text_entry_focused(&self, window: &Window, cx: &App) -> bool {
        [
            &self.slack_composer_input,
            &self.slack_thread_composer_input,
            &self.slack_search_input,
            &self.slack_files_search_input,
            &self.slack_conversation_files_search_input,
            &self.slack_members_search_input,
            &self.slack_directory_search_input,
            &self.slack_home_finder_input,
            &self.slack_dm_finder_input,
            &self.slack_new_message_to_input,
            &self.slack_message_forward_destination_input,
            &self.slack_message_forward_note_input,
            &self.slack_reaction_picker_search_input,
            &self.slack_schedule_date_input,
            &self.slack_schedule_time_input,
            &self.slack_link_text_input,
            &self.slack_link_url_input,
            &self.slack_later_reminder_input,
        ]
        .into_iter()
        .chain(
            self.slack_all_threads_composers
                .values()
                .map(|composer| &composer.input),
        )
        .any(|input| input.read(cx).focus_handle_clone().is_focused(window))
    }

    pub(crate) fn is_embedded_workspace(&self) -> bool {
        self.embedded_shell
    }

    pub(crate) fn render_embedded_work_in_progress_preview(
        &self,
        message: impl Into<String>,
    ) -> AnyElement {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .items_center()
            .justify_center()
            .text_color(rgb(self.theme.text_secondary))
            .child(message.into())
            .into_any_element()
    }

    pub(crate) fn build_slack_remote_images(
        workspace: Option<&crate::ui::SlackWorkspace>,
    ) -> HashMap<String, Arc<Image>> {
        build_slack_remote_images(workspace)
    }

    pub(crate) fn build_slack_message_list_state(message_rows: &[SlackMessageRow]) -> ListState {
        let state = ListState::new(
            message_rows.len(),
            ListAlignment::Top,
            px(SLACK_MESSAGE_LIST_OVERDRAW),
        );
        Self::position_slack_message_list_for_conversation(&state, message_rows);
        state
    }

    pub(crate) fn position_slack_message_list_for_conversation(
        state: &ListState,
        message_rows: &[SlackMessageRow],
    ) {
        if let Some(item_ix) = message_rows
            .iter()
            .position(|row| row.unread_boundary_before)
        {
            state.scroll_to(ListOffset {
                item_ix,
                offset_in_item: px(0.0),
            });
        } else {
            state.scroll_to_end();
        }
    }

    pub(crate) fn build_slack_sidebar_list_state(row_count: usize) -> ListState {
        ListState::new(
            row_count,
            ListAlignment::Top,
            px(SLACK_SIDEBAR_LIST_OVERDRAW),
        )
    }

    pub(in crate::ui::surface::surface_state_core) fn default_slack_workspace(
    ) -> crate::ui::SlackWorkspace {
        crate::ui::SlackWorkspace {
            team_id: String::new(),
            conversation_id: String::new(),
            channel_kind: crate::ui::SlackConversationKind::Channel,
            workspace_name: "Slack".to_string(),
            workspace_logo_url: None,
            workspace_logo_image_base64: None,
            workspace_logo_image_mimetype: None,
            self_user_id: None,
            self_display_name: None,
            self_avatar_label: None,
            self_avatar_image_url: None,
            self_avatar_image_base64: None,
            self_avatar_image_mimetype: None,
            self_timezone_id: None,
            self_timezone_label: None,
            channel_name: "general".to_string(),
            channel_topic: String::new(),
            member_count: None,
            tabs: Vec::new(),
            sections: Vec::new(),
            direct_message_unread_states: Vec::new(),
            rail_badges: Default::default(),
            messages: Vec::new(),
            last_read: None,
            last_read_boundary_loaded: true,
            history_next_cursor: None,
            mention_suggestions: Vec::new(),
            emoji_picker_sections: Vec::new(),
            composer_notice: None,
            peer_notifications_paused: false,
            dm_peer_local_time_context: None,
            composer_draft_text: None,
            composer_placeholder: "Message #general".to_string(),
        }
    }

    pub(crate) fn spawn_background_task<Request, Result, Work, Apply>(
        &mut self,
        request: Request,
        cx: &mut Context<Self>,
        work: Work,
        apply: Apply,
    ) where
        Request: Send + 'static,
        Result: Send + 'static,
        Work: FnOnce(Request) -> Result + Send + 'static,
        Apply: FnOnce(&mut Self, Result, &mut Context<Self>) + 'static,
    {
        spawn_background_task_for_entity(request, cx, work, apply);
    }

    pub(crate) fn spawn_timer_task<Token, Apply>(
        &mut self,
        token: Token,
        delay: Duration,
        cx: &mut Context<Self>,
        apply: Apply,
    ) where
        Token: Send + 'static,
        Apply: FnOnce(&mut Self, Token, &mut Context<Self>) + 'static,
    {
        spawn_timer_task_for_entity(token, delay, cx, apply);
    }
}
