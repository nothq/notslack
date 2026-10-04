mod loading;

use std::{collections::HashSet, rc::Rc, sync::Arc, time::Instant};

use gpui::{Entity, ScrollStrategy};
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::{Context, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    alpha, prepare_slack_conversation_members, px, rgb, slack_palette,
    spawn_background_task_for_entity, PreparedSlackConversationMembers, SlackMembersLoad,
};
use crate::ui::{
    SlackConversationKind, SlackConversationMember, SlackConversationMembersCursor,
    SlackConversationMembersSnapshot,
};

const SLACK_MEMBERS_INITIAL_VISIBLE_ROWS: usize = 12;
const SLACK_MEMBERS_PAGINATION_THRESHOLD: usize = 4;
type SlackMembersBackgroundRequest = (
    Arc<dyn WorkspaceApi>,
    SlackMembersLoad,
    Vec<SlackConversationMember>,
);

impl SurfaceState {
    pub(crate) fn slack_effective_member_count(
        &self,
        workspace: &crate::ui::SlackWorkspace,
    ) -> Option<u32> {
        self.slack_members_snapshot
            .as_ref()
            .filter(|snapshot| {
                snapshot.team_id == workspace.team_id
                    && snapshot.conversation_id == workspace.conversation_id
                    && snapshot.next_cursor.is_none()
            })
            .map(|snapshot| snapshot.members.len() as u32)
            .or(workspace.member_count)
    }

    pub(crate) fn slack_members_search_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let palette = slack_palette(self.appearance_mode);
        let props = TextInputProps::single_line(self.slack_members_query.clone())
            .placeholder("Find members")
            .style(TextInputStyle {
                height: px(36.0),
                min_height: px(36.0),
                padding_x: px(0.0),
                padding_y: px(0.0),
                radius: px(0.0),
                background: alpha(0x000000, 0.0),
                border: alpha(0x000000, 0.0),
                focused_border: alpha(0x000000, 0.0),
                text: rgb(palette.main_text).into(),
                placeholder: rgb(palette.main_secondary_text).into(),
                selection: alpha(0x1264a3, 0.35),
                caret: rgb(palette.main_text).into(),
                font_size: px(15.0),
                line_height: px(22.0),
                font_family: Some("Lato".into()),
            })
            .bordered(false)
            .accessibility(
                self.slack_members_search_accessibility_id.clone(),
                "Find channel members",
            )
            .on_change(self.slack_members_search_on_change(cx))
            .on_escape(self.slack_members_search_on_escape(cx));
        self.slack_members_search_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_members_search_input.clone()
    }

    fn slack_members_search_on_change(&self, cx: &mut Context<Self>) -> TextInputChange {
        let surface = cx.entity().downgrade();
        Rc::new(move |query, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.set_slack_members_query(query, cx);
                })
                .ok();
        })
    }

    fn slack_members_search_on_escape(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |window, cx| {
            let focus = surface
                .update(cx, |surface, cx| {
                    surface.close_slack_members_panel(cx);
                    surface.focus_handle.clone()
                })
                .ok();
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        })
    }

    pub(crate) fn sync_slack_members_context(&mut self, cx: &mut Context<Self>) {
        let target = self.slack_workspace().and_then(|workspace| {
            (self
                .slack_workspace_api_capabilities
                .load_conversation_members
                && matches!(
                    workspace.channel_kind,
                    SlackConversationKind::Channel | SlackConversationKind::PrivateChannel
                )
                && !workspace.conversation_id.is_empty()
                && self.active_slack_workspace_api().is_some())
            .then(|| workspace.conversation_id.clone())
        });
        let Some(conversation_id) = target else {
            if self.slack_members_conversation_id.is_some()
                || self.slack_members_snapshot.is_some()
                || self.slack_members_panel_open
            {
                self.reset_slack_members_context();
                cx.notify();
            }
            return;
        };
        if self.slack_members_deferred_conversation_id.as_deref() == Some(conversation_id.as_str())
        {
            return;
        }
        if self.slack_members_deferred_conversation_id.is_some() {
            self.slack_members_deferred_conversation_id = None;
        }
        if self.slack_members_conversation_id.as_deref() == Some(conversation_id.as_str())
            && (self.slack_members_snapshot.is_some()
                || self.slack_members_request.is_some()
                || self.slack_members_error.is_some())
        {
            return;
        }
        self.reset_slack_members_context();
        self.slack_members_conversation_id = Some(conversation_id);
        self.begin_slack_members_page(None, cx);
    }

    pub(crate) fn defer_slack_members_until_initial_conversation(&mut self, conversation_id: &str) {
        assert!(
            !conversation_id.is_empty(),
            "deferred Slack members require a conversation id"
        );
        self.slack_members_deferred_conversation_id = Some(conversation_id.to_string());
        if Self::slack_load_profiling_enabled() {
            eprintln!("[notslack-slack-members-profile] stage=deferred conversation={conversation_id}");
        }
    }

    pub(crate) fn resume_slack_members_after_initial_conversation(
        &mut self,
        conversation_id: &str,
        cx: &mut Context<Self>,
    ) {
        if self.slack_members_deferred_conversation_id.as_deref() != Some(conversation_id) {
            return;
        }
        self.slack_members_deferred_conversation_id = None;
        if Self::slack_load_profiling_enabled() {
            eprintln!(
                "[notslack-slack-members-profile] stage=initial_conversation_completed conversation={conversation_id}"
            );
        }
        self.sync_slack_members_context(cx);
    }

    pub(crate) fn cancel_deferred_slack_members(&mut self, conversation_id: &str) {
        if self.slack_members_deferred_conversation_id.as_deref() == Some(conversation_id) {
            self.slack_members_deferred_conversation_id = None;
        }
    }

    pub(crate) fn reset_slack_members_context(&mut self) {
        self.slack_members_generation = self
            .slack_members_generation
            .checked_add(1)
            .expect("Slack members generation overflowed");
        self.slack_members_conversation_id = None;
        self.slack_members_snapshot = None;
        self.slack_external_members_summary = None;
        self.slack_external_organization_badges_by_user_id.clear();
        self.slack_members_rows = Arc::default();
        self.slack_members_visible_row_indices = Arc::default();
        self.slack_members_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_members_query.clear();
        self.slack_members_request = None;
        self.slack_members_loading = false;
        self.slack_members_error = None;
        self.slack_members_panel_open = false;
        self.slack_members_focus_pending = false;
        self.slack_members_prefetched_range = None;
        let (authority, data) = (&mut self.slack_presence_authority, &self.data);
        authority.reindex_members(data);
    }

    pub(crate) fn open_slack_members_panel(&mut self, cx: &mut Context<Self>) {
        self.sync_slack_members_context(cx);
        let valid_context = self.slack_workspace().is_some_and(|workspace| {
            self.slack_workspace_api_capabilities
                .load_conversation_members
                && workspace.channel_kind.is_channel()
                && self.slack_members_conversation_id.as_deref()
                    == Some(workspace.conversation_id.as_str())
        });
        if !valid_context {
            return;
        }
        self.slack_members_panel_open = true;
        self.slack_members_query.clear();
        self.rebuild_slack_members_visible_rows();
        self.slack_members_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_members_prefetched_range = None;
        self.slack_members_focus_pending = true;
        self.slack_aux_panel = None;
        self.slack_channel_menu_open = false;
        self.slack_channel_menu_selected_index = None;
        self.slack_channel_menu_submenu = None;
        self.slack_channel_submenu_selected_index = None;
        self.slack_channel_details_open = false;
        self.clear_slack_channel_details_view();
        self.slack_profile_panel = None;
        self.reset_slack_thread_context();
        self.slack_expanded_attachment = None;
        self.queue_slack_members_visible_images(0, SLACK_MEMBERS_INITIAL_VISIBLE_ROWS, cx);
        cx.notify();
    }

    pub(crate) fn close_slack_members_panel(&mut self, cx: &mut Context<Self>) {
        if !self.slack_members_panel_open {
            return;
        }
        self.slack_members_panel_open = false;
        self.slack_members_focus_pending = false;
        self.slack_members_query.clear();
        self.rebuild_slack_members_visible_rows();
        self.slack_members_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_members_prefetched_range = None;
        cx.notify();
    }

    pub(crate) fn retry_slack_members(&mut self, cx: &mut Context<Self>) {
        if self.slack_members_request.is_some() {
            return;
        }
        let cursor = self
            .slack_members_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.next_cursor.clone());
        self.begin_slack_members_page(cursor, cx);
    }

    pub(crate) fn open_slack_member_profile(&mut self, user_id: &str, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.load_profile {
            return;
        }
        self.slack_members_panel_open = false;
        self.slack_members_focus_pending = false;
        self.open_slack_profile(user_id, cx);
    }

    pub(crate) fn handle_slack_members_visible_range(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        self.queue_slack_members_visible_images(visible_start, visible_end, cx);
        if self.slack_members_request.is_some()
            || visible_end.saturating_add(SLACK_MEMBERS_PAGINATION_THRESHOLD)
                < self.slack_members_visible_row_indices.len()
        {
            return;
        }
        let cursor = self
            .slack_members_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.next_cursor.clone());
        if cursor.is_some() {
            self.begin_slack_members_page(cursor, cx);
        }
    }

    pub(crate) fn queue_slack_members_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_members_panel_open {
            return;
        }
        let range = (
            visible_start.min(self.slack_members_visible_row_indices.len()),
            visible_end.min(self.slack_members_visible_row_indices.len()),
        );
        if self.slack_members_prefetched_range == Some(range) {
            return;
        }
        self.slack_members_prefetched_range = Some(range);
        let urls = self.slack_members_visible_row_indices[range.0..range.1]
            .iter()
            .filter_map(|row_index| self.slack_members_rows.get(*row_index))
            .filter_map(|row| row.avatar_image_url.as_ref())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }

    fn set_slack_members_query(&mut self, query: String, cx: &mut Context<Self>) {
        if self.slack_members_query == query {
            return;
        }
        self.slack_members_query = query;
        self.rebuild_slack_members_visible_rows();
        if !self.slack_members_visible_row_indices.is_empty() {
            self.slack_members_scroll_handle
                .scroll_to_item(0, ScrollStrategy::Top);
        }
        self.slack_members_prefetched_range = None;
        self.queue_slack_members_visible_images(0, SLACK_MEMBERS_INITIAL_VISIBLE_ROWS, cx);
        if !self.slack_members_query.trim().is_empty() {
            let cursor = self
                .slack_members_snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.next_cursor.clone());
            if cursor.is_some() {
                self.begin_slack_members_page(cursor, cx);
            }
        }
        cx.notify();
    }

    fn rebuild_slack_members_visible_rows(&mut self) {
        let query = crate::ui::surface::normalize_slack_dm_finder_text(&self.slack_members_query);
        self.slack_members_visible_row_indices = self
            .slack_members_rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                (query.is_empty() || row.search_key.as_ref().contains(query.as_str()))
                    .then_some(index)
            })
            .collect::<Vec<_>>()
            .into();
    }
}

fn prepare_slack_members_page(
    mut existing_members: Vec<SlackConversationMember>,
    snapshot: SlackConversationMembersSnapshot,
    request: &SlackMembersLoad,
) -> Result<PreparedSlackConversationMembers, String> {
    if snapshot.conversation_id != request.conversation_id {
        return Err("Slack members response did not match the requested conversation.".to_string());
    }
    if request.cursor.is_some() && snapshot.next_cursor.as_ref() == request.cursor.as_ref() {
        return Err("Slack members pagination repeated the current cursor.".to_string());
    }
    let mut seen = existing_members
        .iter()
        .map(|member| member.user_id.clone())
        .collect::<HashSet<_>>();
    existing_members.extend(
        snapshot
            .members
            .into_iter()
            .filter(|member| seen.insert(member.user_id.clone())),
    );
    existing_members
        .sort_by_cached_key(|member| (member.real_name.to_lowercase(), member.user_id.clone()));
    prepare_slack_conversation_members(SlackConversationMembersSnapshot {
        team_id: snapshot.team_id,
        conversation_id: snapshot.conversation_id,
        members: existing_members,
        external_organization_count: snapshot.external_organization_count,
        connected_organizations: snapshot.connected_organizations,
        next_cursor: snapshot.next_cursor,
    })
}
