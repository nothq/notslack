use gpui::{AppContext, TaskExt};

use super::super::{load_slack_prepared_upload_files_from_paths, Context, SurfaceState, Window};
use crate::ui::surface::{
    SlackAttachmentPathSelection, SlackComposerCaptureGeneration, SlackMainComposerDraftHandle,
    SlackPreparedUploadFile, SlackVideoClipCaptureState,
};

#[derive(Clone, Copy)]
enum SlackAttachmentSelectionPolicy {
    General,
    SingleVideoClip {
        generation: SlackComposerCaptureGeneration,
    },
}

struct SlackAttachmentPickerLoad {
    policy: SlackAttachmentSelectionPolicy,
    handle: SlackMainComposerDraftHandle,
    selection_truncated: bool,
    loaded: Result<Vec<SlackPreparedUploadFile>, String>,
}

impl SurfaceState {
    pub(crate) fn prompt_for_slack_attachment_files(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.prompt_for_slack_attachment_files_with_policy(
            SlackAttachmentSelectionPolicy::General,
            window,
            cx,
        )
        .ok();
    }

    pub(crate) fn prompt_for_single_slack_video_attachment(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.prompt_for_slack_attachment_files_with_policy(
            SlackAttachmentSelectionPolicy::SingleVideoClip { generation },
            window,
            cx,
        )
    }

    fn prompt_for_slack_attachment_files_with_policy(
        &mut self,
        policy: SlackAttachmentSelectionPolicy,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let handle = self.prepare_slack_attachment_picker()?;
        let local_file_api = self.local_file_api.clone().ok_or_else(|| {
            "Slack local file access is unavailable for this surface.".to_string()
        })?;
        let entity = cx.entity();
        let paths_receiver = cx.prompt_for_paths(policy.path_prompt_options());
        window
            .spawn(cx, async move |cx| {
                let paths = match paths_receiver.await {
                    Ok(Ok(Some(paths))) => paths,
                    Ok(Ok(None)) | Err(_) => return Ok::<(), String>(()),
                    Ok(Err(error)) => {
                        entity.update(cx, |this, cx| {
                            policy.report_error(
                                this,
                                &handle,
                                format!("failed to open Slack attachment picker: {error}"),
                                cx,
                            );
                        });
                        return Ok(());
                    }
                };
                let Some(selection) = entity.update(cx, |this, cx| {
                    select_slack_attachment_paths(this, policy, &handle, paths, cx)
                }) else {
                    return Ok(());
                };
                let (paths, selection_truncated) = selection.into_parts();
                let loaded = cx
                    .background_spawn(async move {
                        load_slack_prepared_upload_files_from_paths(local_file_api.as_ref(), paths)
                    })
                    .await;
                entity.update(cx, |this, cx| {
                    finish_slack_attachment_picker_load(
                        this,
                        SlackAttachmentPickerLoad {
                            policy,
                            handle,
                            selection_truncated,
                            loaded,
                        },
                        cx,
                    );
                });
                Ok(())
            })
            .detach_and_log_err(cx);
        Ok(())
    }

    fn prepare_slack_attachment_picker(&self) -> Result<SlackMainComposerDraftHandle, String> {
        if self.slack_schedule_blocks_current_composer_mutation() {
            return Err("Wait for Slack scheduling to finish before attaching files.".to_string());
        }
        if !self.is_slack_workspace()
            || !self.has_current_slack_send_target()
            || !self.slack_workspace_api_capabilities.upload_files
            || !self.slack_workspace_api_capabilities.stage_file
        {
            return Err("File uploads are unavailable for this Slack workspace.".to_string());
        }
        self.current_slack_main_composer_draft_handle()
            .ok_or_else(|| "There is no active Slack composer draft.".to_string())
    }
}

fn select_slack_attachment_paths(
    surface: &mut SurfaceState,
    policy: SlackAttachmentSelectionPolicy,
    handle: &SlackMainComposerDraftHandle,
    paths: Vec<std::path::PathBuf>,
    cx: &mut Context<SurfaceState>,
) -> Option<SlackAttachmentPathSelection> {
    match policy.select_paths(surface, handle, paths) {
        Ok(selection) => Some(selection),
        Err(message) => {
            policy.report_error(surface, handle, message, cx);
            None
        }
    }
}

fn finish_slack_attachment_picker_load(
    surface: &mut SurfaceState,
    load: SlackAttachmentPickerLoad,
    cx: &mut Context<SurfaceState>,
) {
    let SlackAttachmentPickerLoad {
        policy,
        handle,
        selection_truncated,
        loaded,
    } = load;
    let files = match loaded {
        Ok(files) => files,
        Err(message) => {
            policy.report_error(surface, &handle, message, cx);
            return;
        }
    };
    let attachment = policy.validate_loaded_files(&files).and_then(|()| {
        policy.before_attachment(surface, &handle, cx)?;
        surface
            .attach_slack_prepared_upload_files_to_main_draft_with_selection(
                &handle,
                files,
                selection_truncated,
                cx,
            )
            .map(|_| ())
    });
    if let Err(message) = attachment {
        policy.report_error(surface, &handle, message, cx);
    }
}

impl SlackAttachmentSelectionPolicy {
    fn path_prompt_options(self) -> gpui::PathPromptOptions {
        match self {
            Self::General => gpui::PathPromptOptions {
                files: true,
                directories: false,
                multiple: true,
                prompt: Some("Select files".into()),
            },
            Self::SingleVideoClip { .. } => gpui::PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: Some("Select a video".into()),
            },
        }
    }

    fn select_paths(
        self,
        surface: &SurfaceState,
        handle: &SlackMainComposerDraftHandle,
        paths: Vec<std::path::PathBuf>,
    ) -> Result<SlackAttachmentPathSelection, String> {
        if matches!(self, Self::SingleVideoClip { .. }) && paths.len() != 1 {
            return Err("Select exactly one video file.".to_string());
        }
        let selection = surface.slack_main_attachment_path_selection(handle, paths)?;
        if matches!(self, Self::SingleVideoClip { .. })
            && (selection.path_count() != 1 || selection.was_truncated())
        {
            return Err("The composer has no room for another video attachment.".to_string());
        }
        Ok(selection)
    }

    fn validate_loaded_files(self, files: &[SlackPreparedUploadFile]) -> Result<(), String> {
        if matches!(self, Self::General) {
            return Ok(());
        }
        let [file] = files else {
            return Err("Select exactly one video file.".to_string());
        };
        if !file.mimetype().starts_with("video/") {
            return Err("The selected file is not a supported video.".to_string());
        }
        Ok(())
    }

    fn before_attachment(
        self,
        surface: &mut SurfaceState,
        handle: &SlackMainComposerDraftHandle,
        cx: &mut Context<SurfaceState>,
    ) -> Result<(), String> {
        let Self::SingleVideoClip { generation } = self else {
            return Ok(());
        };
        let capture_is_current = matches!(
            surface.slack_composer_capture.video(),
            Some(
                SlackVideoClipCaptureState::Preparing { operation }
                    | SlackVideoClipCaptureState::Prepared { operation, .. }
            ) if operation.generation == generation
                && &operation.owner == handle
                && surface.slack_composer_capture_operation_is_current(operation)
        );
        if !capture_is_current {
            return Err("The video clip recorder no longer owns the active draft.".to_string());
        }
        surface
            .cancel_slack_video_clip_capture_for_generation(generation, cx)
            .map(|_| ())
    }

    fn report_error(
        self,
        surface: &mut SurfaceState,
        handle: &SlackMainComposerDraftHandle,
        message: String,
        cx: &mut Context<SurfaceState>,
    ) {
        if let Self::SingleVideoClip { generation } = self {
            if let Some(modal) = surface
                .slack_video_clip_modal
                .as_ref()
                .filter(|modal| modal.read(cx).generation() == generation)
            {
                modal.update(cx, |modal, cx| {
                    modal.set_interaction_error(message, cx);
                });
                return;
            }
        }
        if surface.current_slack_main_composer_draft_handle().as_ref() == Some(handle) {
            surface.slack_error = Some(message);
            surface.slack_composer_focused = true;
            cx.notify();
        }
    }
}
