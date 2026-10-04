use super::super::SurfaceState;
use super::{
    ChatReactionPickerCatalogState, ChatReactionPickerCategory, ChatReactionPickerOpenState,
    ChatReactionPickerSkinToneState, ChatReactionPickerState, ChatReactionPickerTarget, Context,
    SurfaceRoot,
};

impl SurfaceRoot {
    pub fn chat_debug_state<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatDebugState, String> {
        let composer_files = self.slack_composer_files(cx);
        let audio_clip_capture = self.slack_audio_clip_capture_state(cx);
        let video_clip_capture = self.slack_video_clip_capture_state(cx);
        let reaction_picker = self.slack_reaction_picker_debug_state(cx)?;
        let file_cleanups = self.slack_file_cleanup_summaries(cx);
        let remote_draft_file_cleanups = self.slack_remote_draft_file_cleanup_summaries(cx);
        let thread_panel = self.slack_thread_panel_summary(cx);
        let media_attachments = self.slack_media_attachments(cx);
        let media_playback = self.slack_media_playback_state(cx);
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            let workspace = surface
                .slack_workspace()
                .ok_or_else(|| "Chat has no loaded Slack workspace".to_string())?;
            Ok(crate::model::ChatDebugState {
                team_id: workspace.team_id.clone(),
                conversation_id: workspace.conversation_id.clone(),
                presence_revision: surface.slack_presence_authority.accepted_revision(),
                presence_known_user_count: surface.slack_presence_authority.known_user_count(),
                self_timezone_id: workspace.self_timezone_id.clone(),
                last_read_boundary_loaded: workspace.last_read_boundary_loaded,
                recent_message_times: recent_chat_message_times(workspace),
                recent_message_row_times: recent_chat_message_row_times(surface),
                composer_text: surface.slack_composer_text.clone(),
                composer_focused: surface.slack_composer_focused,
                composer_placeholder: surface.slack_composer_placeholder(workspace),
                composer_files,
                audio_clip_capture,
                video_clip_capture,
                reaction_picker,
                file_cleanups,
                remote_draft_file_cleanups,
                thread_panel,
                media_attachments,
                media_playback,
                conversations: chat_conversation_summaries(surface, workspace),
            })
        })
    }

    pub fn slack_composer_placeholder<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Option<String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface
                .slack_workspace()
                .map(|workspace| surface.slack_composer_placeholder(workspace))
        })
    }

    pub fn slack_recent_message_row_times<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Vec<(String, String)> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface
                .slack_message_rows
                .iter()
                .rev()
                .take(5)
                .map(|row| (row.id.clone(), row.timestamp.clone()))
                .collect()
        })
    }

    pub fn slack_conversation_id<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Option<String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_conversation_id().map(str::to_string)
        })
    }

    pub fn slack_media_attachments<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Vec<crate::model::ChatMediaAttachmentSummary> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_media_attachment_summaries()
        })
    }

    pub fn slack_media_playback_state<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Option<crate::model::ChatMediaPlaybackState> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.slack_media_playback_state(cx))
    }

    pub fn slack_thread_panel_summary<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Option<crate::model::ChatThreadPanelSummary> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_thread_panel.as_ref().map(|panel| {
                let draft = panel.reply_draft.borrow();
                crate::model::ChatThreadPanelSummary {
                    conversation_id: panel.conversation_id.clone(),
                    parent_message_id: panel.parent_message_id.clone(),
                    loaded_reply_count: panel.reply_rows.len(),
                    expected_reply_count: panel.expected_reply_count,
                    loading: panel.loading,
                    error: panel.error.clone(),
                    draft_text: draft.text().to_string(),
                    broadcast_available: panel.broadcast_label.is_some(),
                    broadcast: draft.broadcast,
                    files: draft.files.control_summaries(),
                    composer_focused: panel.reply_composer_focused,
                    reply_send_pending: surface
                        .slack_thread_reply_is_pending(&panel.reply_draft_key),
                    reply_error: panel.reply_error.clone(),
                }
            })
        })
    }

    pub fn slack_reaction_picker_debug_state<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<ChatReactionPickerState, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            Ok(ChatReactionPickerState {
                open: chat_reaction_picker_open_state(surface)?,
                catalog: chat_reaction_picker_catalog_state(surface),
                skin_tone: chat_reaction_picker_skin_tone_state(surface),
            })
        })
    }
}

fn recent_chat_message_times(
    workspace: &crate::ui::SlackWorkspace,
) -> Vec<crate::model::ChatMessageTiming> {
    workspace
        .messages
        .iter()
        .rev()
        .take(5)
        .map(|message| crate::model::ChatMessageTiming {
            id: message.id.clone(),
            timestamp: message.timestamp.clone(),
            latest_reply_timestamp: message.latest_reply_timestamp.clone(),
        })
        .collect()
}

fn recent_chat_message_row_times(
    surface: &SurfaceState,
) -> Vec<crate::model::ChatMessageRowTiming> {
    surface
        .slack_message_rows
        .iter()
        .rev()
        .take(5)
        .map(|row| crate::model::ChatMessageRowTiming {
            id: row.id.clone(),
            timestamp: row.timestamp.clone(),
        })
        .collect()
}

fn chat_conversation_summaries(
    surface: &SurfaceState,
    workspace: &crate::ui::SlackWorkspace,
) -> Vec<crate::model::ChatConversationSummary> {
    workspace
        .sections
        .iter()
        .flat_map(|section| {
            section.items.iter().map(|item| {
                let presence = item.user_id.as_deref().and_then(|user_id| {
                    surface.slack_presence_authority.resolve_presence(
                        &workspace.team_id,
                        user_id,
                        item.presence,
                    )
                });
                crate::model::ChatConversationSummary {
                    id: item.target_id.clone(),
                    label: item.label.clone(),
                    kind: item.target_kind,
                    user_id: item.user_id.clone(),
                    presence,
                    count: item.count,
                }
            })
        })
        .collect()
}

fn chat_reaction_picker_open_state(
    surface: &SurfaceState,
) -> Result<Option<ChatReactionPickerOpenState>, String> {
    surface
        .slack_reaction_picker
        .as_ref()
        .map(|picker| {
            let active_category_index = if picker.query.is_empty() {
                picker.category_index
            } else {
                0
            };
            Ok(ChatReactionPickerOpenState {
                target: ChatReactionPickerTarget {
                    team_id: picker.identity.target().team_id().to_string(),
                    conversation_id: picker.identity.target().conversation_id().to_string(),
                    message_id: picker
                        .identity
                        .target()
                        .message_timestamp()
                        .as_str()
                        .to_string(),
                    thread_parent_message_id: picker
                        .identity
                        .target()
                        .thread_timestamp()
                        .map(|timestamp| timestamp.as_str().to_string()),
                },
                active_category: ChatReactionPickerCategory::try_from(active_category_index)?,
                query: picker.query.clone(),
            })
        })
        .transpose()
}

fn chat_reaction_picker_catalog_state(surface: &SurfaceState) -> ChatReactionPickerCatalogState {
    ChatReactionPickerCatalogState {
        request_pending: surface.slack_reaction_catalog_request.is_some(),
        installed: surface.slack_reaction_picker_catalog.is_installed(),
        frequent_count: surface.slack_reaction_picker_catalog.frequent().len(),
        custom_count: surface.slack_reaction_picker_catalog.custom().len(),
        error: surface
            .slack_reaction_catalog_error
            .as_ref()
            .map(ToString::to_string),
    }
}

fn chat_reaction_picker_skin_tone_state(surface: &SurfaceState) -> ChatReactionPickerSkinToneState {
    ChatReactionPickerSkinToneState {
        loaded: surface.slack_preferred_skin_tone.is_some(),
        load_pending: surface.slack_skin_tone_load_request.is_some(),
        active: surface
            .slack_preferred_skin_tone
            .as_ref()
            .and_then(|preference| preference.selection)
            .map(Into::into),
        pending: surface
            .slack_skin_tone_mutation_request
            .as_ref()
            .map(|request| request.selected.into()),
        menu_open: surface.slack_skin_tone_menu_open,
        menu_selection: if surface.slack_skin_tone_menu_open {
            surface.slack_skin_tone_menu_selected.map(Into::into)
        } else {
            None
        },
        error: surface
            .slack_skin_tone_error
            .as_ref()
            .map(ToString::to_string),
    }
}
