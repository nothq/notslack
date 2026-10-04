mod destination;
mod inputs;
mod submit;

use std::{rc::Rc, sync::Arc};

use gpui::{ClipboardItem, Entity, ScrollStrategy};
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::{Context, SurfaceState, WorkspaceApi};
use crate::ui::surface::{
    normalize_slack_dm_finder_text, prepare_slack_destination_directory,
    PreparedSlackDestinationDirectory, SlackMessageActionTarget, SlackMessageForwardDestination,
    SlackMessageForwardModal, SlackMessageForwardSource, SlackMessageRow,
    SlackNewMessageCandidateKind,
};
use crate::ui::{
    alpha, px, rgb, SlackConversationKind, SlackConversationOpenReceipt,
    SlackConversationOpenRequest, SlackDestinationTarget, SlackMessageDraft,
    SlackMessageForwardReceipt,
};

const SLACK_MESSAGE_FORWARD_INITIAL_VISIBLE_ROWS: usize = 8;

impl SurfaceState {
    pub(crate) fn open_slack_message_forward_source(
        &mut self,
        source: SlackMessageForwardSource,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.forward_message
            || !self
                .slack_workspace_api_capabilities
                .load_destination_directory
        {
            return;
        }
        self.slack_message_forward_generation = self
            .slack_message_forward_generation
            .checked_add(1)
            .expect("Slack message forward generation overflowed");
        let generation = self.slack_message_forward_generation;
        self.slack_reaction_picker = None;
        self.slack_message_menu = None;
        self.slack_message_menu_focus_pending = false;
        self.slack_message_forward_modal = Some(SlackMessageForwardModal {
            generation,
            source,
            rows: Arc::default(),
            visible_row_indices: Arc::default(),
            scroll_handle: gpui::UniformListScrollHandle::new(),
            query: String::new(),
            normalized_query: Default::default(),
            selected_index: None,
            destination: None,
            note: String::new(),
            directory_loading: true,
            forwarding: false,
            copying_link: false,
            link_copied: false,
            error: None,
        });
        self.slack_message_forward_focus_pending = true;
        self.slack_message_forward_note_focus_pending = false;
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_forward(data);
        }
        cx.notify();
        self.begin_slack_message_forward_directory_load(generation, cx);
    }

    pub(crate) fn slack_message_forward_source_for_target(
        &self,
        target: &SlackMessageActionTarget,
        row: &SlackMessageRow,
    ) -> Option<SlackMessageForwardSource> {
        let workspace = self.slack_workspace()?;
        if workspace.team_id != target.team_id() || row.action_target.as_deref() != Some(target) {
            return None;
        }
        let private_message = if workspace.conversation_id == target.conversation_id() {
            matches!(
                workspace.channel_kind,
                SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
            )
        } else {
            self.slack_all_threads_snapshot
                .as_ref()
                .and_then(|snapshot| {
                    snapshot.threads.iter().find(|thread| {
                        thread.conversation_id == target.conversation_id()
                            && thread.thread_timestamp == target.root_timestamp().as_str()
                    })
                })
                .is_some_and(|thread| {
                    matches!(
                        thread.conversation_kind,
                        SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
                    )
                })
        };
        Some(SlackMessageForwardSource {
            team_id: target.team_id().to_string(),
            conversation_id: target.conversation_id().to_string(),
            message_timestamp: target.message_timestamp().clone(),
            private_message,
            author: row.author.clone().into(),
            timestamp_label: row.timestamp.clone().into(),
            preview: slack_message_forward_preview(row),
        })
    }

    pub(crate) fn close_slack_message_forward(&mut self, cx: &mut Context<Self>) {
        if self.slack_message_forward_modal.take().is_none() {
            return;
        }
        self.slack_message_forward_generation = self
            .slack_message_forward_generation
            .checked_add(1)
            .expect("Slack message forward generation overflowed");
        self.slack_message_forward_focus_pending = false;
        self.slack_message_forward_note_focus_pending = false;
        let (authority, data) = (&mut self.slack_presence_authority, &self.data);
        authority.reindex_forward(data);
        cx.notify();
    }

    pub(crate) fn reset_slack_message_forward_context(&mut self) {
        if self.slack_message_forward_modal.take().is_some() {
            self.slack_message_forward_generation = self
                .slack_message_forward_generation
                .checked_add(1)
                .expect("Slack message forward generation overflowed");
        }
        self.slack_message_forward_focus_pending = false;
        self.slack_message_forward_note_focus_pending = false;
        let (authority, data) = (&mut self.slack_presence_authority, &self.data);
        authority.reindex_forward(data);
    }

    fn begin_slack_message_forward_directory_load(
        &mut self,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            if let Some(modal) = self.slack_message_forward_modal.as_mut() {
                modal.directory_loading = false;
                modal.error = Some("Slack forwarding requires a connected workspace.".to_string());
            }
            cx.notify();
            return;
        };
        let team_id = self
            .slack_message_forward_modal
            .as_ref()
            .expect("forward modal should exist while loading destinations")
            .source
            .team_id
            .clone();
        self.spawn_background_task(
            (workspace_api, team_id, generation),
            cx,
            |(workspace_api, team_id, generation)| {
                let result = workspace_api
                    .load_slack_destination_directory()
                    .and_then(prepare_slack_destination_directory);
                (team_id, generation, result)
            },
            |this, (team_id, generation, result), cx| {
                this.finish_slack_message_forward_directory_load(&team_id, generation, result, cx);
            },
        );
    }

    fn finish_slack_message_forward_directory_load(
        &mut self,
        team_id: &str,
        generation: u64,
        result: Result<PreparedSlackDestinationDirectory, String>,
        cx: &mut Context<Self>,
    ) {
        if self
            .slack_message_forward_modal
            .as_ref()
            .is_none_or(|modal| modal.generation != generation)
        {
            return;
        }
        self.slack_message_forward_modal
            .as_mut()
            .expect("validated Slack forward modal must remain available")
            .directory_loading = false;
        match result {
            Ok(mut prepared) if prepared.snapshot.team_id == team_id => {
                self.slack_presence_authority
                    .overlay_prepared_destination(&mut prepared);
                let modal = self
                    .slack_message_forward_modal
                    .as_mut()
                    .expect("validated Slack forward modal must remain available");
                modal.rows = prepared.rows;
                modal.error = None;
                let (authority, data) = (&mut self.slack_presence_authority, &self.data);
                authority.reindex_forward(data);
                self.rebuild_slack_message_forward_results();
                self.queue_slack_message_forward_visible_images(
                    0,
                    SLACK_MESSAGE_FORWARD_INITIAL_VISIBLE_ROWS,
                    cx,
                );
            }
            Ok(_) => {
                self.slack_message_forward_modal
                    .as_mut()
                    .expect("validated Slack forward modal must remain available")
                    .error =
                    Some("Slack destination directory returned another workspace.".to_string());
            }
            Err(error) => {
                self.slack_message_forward_modal
                    .as_mut()
                    .expect("validated Slack forward modal must remain available")
                    .error = Some(error);
            }
        }
        cx.notify();
    }
}

fn slack_message_forward_preview(row: &crate::ui::surface::SlackMessageRow) -> gpui::SharedString {
    let source = if row.body.trim().is_empty() {
        "Shared message"
    } else {
        row.body.trim()
    };
    let compact = source.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = compact.chars();
    let preview = characters.by_ref().take(240).collect::<String>();
    if characters.next().is_some() {
        format!("{preview}…").into()
    } else {
        preview.into()
    }
}

fn slack_message_forward_open_receipt_matches(
    request: &SlackConversationOpenRequest,
    receipt: &SlackConversationOpenReceipt,
) -> bool {
    receipt.team_id == request.team_id() && receipt.user_ids == request.user_ids()
}

fn slack_message_forward_destination_input_style() -> TextInputStyle {
    TextInputStyle {
        height: px(34.0),
        min_height: px(34.0),
        padding_x: px(12.0),
        padding_y: px(9.0),
        radius: px(6.0),
        background: alpha(0x000000, 0.0),
        border: alpha(0x000000, 0.0),
        focused_border: alpha(0x000000, 0.0),
        text: rgb(0xf8f8f8).into(),
        placeholder: rgb(0x9a9b9e).into(),
        selection: alpha(0x1d9bd1, 0.35),
        caret: rgb(0xf8f8f8).into(),
        font_size: px(16.0),
        line_height: px(22.0),
        font_family: Some("Lato".into()),
    }
}

fn slack_message_forward_note_input_style() -> TextInputStyle {
    TextInputStyle {
        height: px(124.0),
        min_height: px(124.0),
        padding_x: px(12.0),
        padding_y: px(9.0),
        radius: px(6.0),
        background: rgb(0x1a1d21).into(),
        border: rgb(0x565856).into(),
        focused_border: rgb(0x1d9bd1).into(),
        text: rgb(0xf8f8f8).into(),
        placeholder: rgb(0x9a9b9e).into(),
        selection: alpha(0x1d9bd1, 0.35),
        caret: rgb(0xf8f8f8).into(),
        font_size: px(15.0),
        line_height: px(21.0),
        font_family: Some("Lato".into()),
    }
}
