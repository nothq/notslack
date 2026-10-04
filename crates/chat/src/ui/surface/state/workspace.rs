mod apply;
mod conversation_snapshot;
mod drafts;
mod image_helpers;
mod images;
mod reset;
mod sidebar_reveal;
mod snapshots;
mod staging;

pub(in crate::ui::surface::state) use image_helpers::{
    slack_attachment_remote_image_urls, slack_message_reaction_remote_image_urls,
    slack_sidebar_layout_changed, slack_sidebar_reveal_anchor,
};
use std::collections::HashSet;
use std::sync::Arc;

#[cfg(test)]
use super::SlackMediaState;
use super::{
    build_slack_message_chunks, build_slack_sidebar_rows, prepare_slack_workspace, px, ChatStartup,
    Context, Image, PreparedSlackConversationSnapshot, PreparedSlackShellSnapshot,
    PreparedSlackSidebarSnapshot, PreparedSlackWorkspace, SlackMainTab, SlackMessageRow,
    SlackRailView, SlackRemoteImagePrefetchRequest, SlackSidebarRow, SlackSidebarRowKind,
    SurfaceState, Window, WorkspaceApi,
};
use crate::ui::surface::{
    SlackComposerDestination, SlackComposerDocument, SlackComposerDraft, SlackComposerDraftKey,
    SlackConversationLiveTarget, SlackMainComposerDraftOwner, SlackMainRoute,
    SlackSidebarRevealState, SLACK_SIDEBAR_ROW_HEIGHT,
};
use crate::ui::{SlackAttachment, SlackWorkspace};
use gpui::ListOffset;

const SLACK_MESSAGE_ATTACHMENT_PREFETCH_ROW_LIMIT: usize = 12;
const SLACK_MESSAGE_REMOTE_IMAGE_ROW_LIMIT: usize = 32;

struct SlackWorkspaceUiIdentity<'a> {
    team_id: &'a str,
    self_user_id: Option<&'a str>,
    conversation_id: &'a str,
}
const SLACK_SIDEBAR_REMOTE_IMAGE_ROW_LIMIT: usize = 40;

#[derive(Clone, Copy, Default)]
struct SlackWorkspaceViewReactivation {
    all_threads: bool,
    activity: bool,
    later: bool,
    files: bool,
    drafts_sent: bool,
    directory: bool,
    new_message: bool,
}

struct SlackMessageListPosition {
    scroll_top: ListOffset,
    anchor_message_id: Option<String>,
    was_following_end: bool,
}

impl SurfaceState {
    pub(crate) fn active_slack_workspace_api(&self) -> Option<Arc<dyn WorkspaceApi>> {
        if !self.is_embedded_workspace() {
            return self.workspace_api.clone();
        }
        match &self.chat_startup {
            ChatStartup::Archive => self.workspace_api.clone(),
            #[cfg(any(test, feature = "test-support"))]
            ChatStartup::Fixture => self.workspace_api.clone(),
            ChatStartup::Ready { connection, .. } => Some(connection.workspace_api.clone()),
            ChatStartup::Loading {
                previous_connection,
                ..
            }
            | ChatStartup::Error {
                previous_connection,
                ..
            } => previous_connection
                .as_ref()
                .map(|connection| connection.workspace_api.clone()),
            ChatStartup::ConnectionRequired { .. } => None,
        }
    }

    pub(crate) fn is_slack_workspace(&self) -> bool {
        self.slack_workspace.is_some()
    }

    pub(crate) fn slack_workspace(&self) -> Option<&SlackWorkspace> {
        self.slack_workspace.as_deref()
    }

    pub(crate) fn slack_workspace_mut(&mut self) -> Option<&mut SlackWorkspace> {
        self.slack_workspace.as_mut().map(Arc::make_mut)
    }

    pub(crate) fn slack_conversation_id(&self) -> Option<&str> {
        self.slack_workspace().and_then(|workspace| {
            (!workspace.conversation_id.is_empty()).then_some(workspace.conversation_id.as_str())
        })
    }

    #[cfg(test)]
    pub(crate) fn slack_media_state(&self, attachment_title: &str) -> SlackMediaState {
        self.slack_media_states
            .get(attachment_title)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn is_slack_section_collapsed(&self, section_label: &str) -> bool {
        self.slack_collapsed_sections.contains(section_label)
    }

    pub(crate) fn can_navigate_slack_back(&self) -> bool {
        !self.slack_conversation_history.is_empty() && self.slack_conversation_history_index > 0
    }

    pub(crate) fn can_navigate_slack_forward(&self) -> bool {
        self.slack_conversation_history_index + 1 < self.slack_conversation_history.len()
    }

    fn reset_slack_message_list_state(&self, message_count: usize) {
        self.slack_message_list_state.reset(message_count);
    }

    fn capture_slack_message_list_position(&self) -> SlackMessageListPosition {
        let scroll_top = self.slack_message_list_state.logical_scroll_top();
        let was_following_end = self.slack_message_list_state.is_following_tail()
            || scroll_top.item_ix >= self.slack_message_rows.len();
        let anchor_message_id = (!was_following_end)
            .then(|| {
                self.slack_message_rows
                    .get(scroll_top.item_ix)
                    .map(|row| row.id.to_string())
            })
            .flatten();
        SlackMessageListPosition {
            scroll_top,
            anchor_message_id,
            was_following_end,
        }
    }

    fn sync_slack_message_list_state(
        &mut self,
        same_conversation: bool,
        previous_position: SlackMessageListPosition,
    ) {
        let next_count = self.slack_message_display_row_count();

        if self.slack_message_list_state.item_count() != next_count {
            self.reset_slack_message_list_state(next_count);
        } else {
            self.slack_message_list_state.remeasure();
        }

        if !same_conversation {
            self.activate_slack_message_list_auto_position();
            return;
        }
        if self.slack_message_list_auto_position_active {
            self.restore_slack_message_list_auto_position();
            return;
        }
        if previous_position.was_following_end {
            self.slack_message_list_state.scroll_to_end();
            return;
        }
        if next_count == 0 {
            return;
        }
        let item_ix = previous_position
            .anchor_message_id
            .as_deref()
            .and_then(|anchor_message_id| {
                self.slack_message_rows
                    .iter()
                    .position(|row| row.id.as_str() == anchor_message_id)
            })
            .unwrap_or_else(|| {
                previous_position
                    .scroll_top
                    .item_ix
                    .min(next_count.saturating_sub(1))
            });
        self.slack_message_list_state.scroll_to(ListOffset {
            item_ix,
            offset_in_item: previous_position.scroll_top.offset_in_item,
        });
    }

    fn activate_slack_message_list_auto_position(&mut self) {
        self.slack_message_list_auto_position_active = true;
        self.slack_new_message_count = 0;
        self.restore_slack_message_list_auto_position();
    }

    pub(super) fn restore_slack_message_list_auto_position(&self) {
        if !self.slack_message_list_auto_position_active {
            return;
        }
        Self::position_slack_message_list_for_conversation(
            &self.slack_message_list_state,
            &self.slack_message_rows,
        );
    }

    pub(crate) fn refresh_slack_message_rows(&mut self) {
        let (message_rows, message_rows_local_today) =
            crate::ui::surface::build_slack_message_rows_with_local_today(
                self.slack_workspace.as_deref(),
            );
        self.slack_message_rows = message_rows;
        self.slack_message_rows_local_today = message_rows_local_today;
        self.slack_message_chunks = build_slack_message_chunks(&self.slack_message_rows);
    }

    pub(in crate::ui::surface::state) fn refresh_slack_message_read_boundary(&mut self) {
        let message_list_position = self.capture_slack_message_list_position();
        self.refresh_slack_message_rows();
        self.sync_slack_message_list_state(true, message_list_position);
    }

    pub(crate) fn refresh_slack_sidebar_rows(&mut self) {
        let sidebar_rows = build_slack_sidebar_rows(
            self.slack_workspace.as_deref(),
            &self.slack_collapsed_sections,
        );
        let mut sidebar_rows = self.project_slack_conversation_read_sidebar_rows(sidebar_rows);
        self.slack_presence_authority
            .overlay_sidebar_rows(&mut sidebar_rows);
        let layout_changed = slack_sidebar_layout_changed(&self.slack_sidebar_rows, &sidebar_rows);
        self.slack_sidebar_rows = sidebar_rows;
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_sidebar(data);
        }
        if self.slack_sidebar_list_state.item_count() != self.slack_sidebar_rows.len() {
            self.slack_sidebar_list_state
                .reset(self.slack_sidebar_rows.len());
        } else if layout_changed {
            self.slack_sidebar_list_state.remeasure();
        }
        if self.slack_home_finder_active() {
            self.rebuild_slack_home_finder_results();
        }
        if let Some(workspace) = self.slack_workspace.as_deref() {
            self.sync_slack_dm_list_state(workspace);
        }
    }

    pub(crate) fn apply_slack_workspace(
        &mut self,
        workspace: SlackWorkspace,
        cx: &mut Context<Self>,
    ) {
        let prepared = prepare_slack_workspace(
            workspace,
            &self.slack_collapsed_sections,
            &self.slack_muted_conversations,
        );
        self.apply_prepared_slack_workspace(prepared, cx);
    }

    fn sync_slack_workspace_metadata(
        &mut self,
        workspace: &SlackWorkspace,
        active_conversation_was_muted: bool,
    ) {
        self.sync_slack_conversation_metadata(
            &workspace.conversation_id,
            active_conversation_was_muted,
        );
        self.sync_slack_conversation_history(workspace);
    }

    fn sync_slack_conversation_metadata(
        &mut self,
        conversation_id: &str,
        active_conversation_was_muted: bool,
    ) {
        if active_conversation_was_muted {
            self.slack_muted_conversations
                .insert(conversation_id.to_string());
        }
    }
}
