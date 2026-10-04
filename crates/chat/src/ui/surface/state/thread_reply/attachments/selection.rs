use gpui::TaskExt;

use super::super::super::{
    load_slack_prepared_upload_files_from_paths, Context, SurfaceState, Window,
};
use crate::ui::surface::{SlackComposerDraftKey, SlackPreparedUploadFile, SlackThreadDraftHandle};

impl SurfaceState {
    pub(crate) fn prompt_for_slack_thread_attachment_files(
        &mut self,
        key: SlackComposerDraftKey,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.upload_files
            || !self.slack_workspace_api_capabilities.stage_file
            || self.slack_thread_reply_is_pending(&key)
            || !self.slack_thread_draft_key_has_current_identity(&key)
        {
            return;
        }
        let handle = self.ensure_slack_thread_draft_handle(key);
        let entity = cx.entity();
        let paths_receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Select files".into()),
        });
        window
            .spawn(cx, async move |cx| {
                let paths = match paths_receiver.await {
                    Ok(Ok(Some(paths))) => paths,
                    Ok(Ok(None)) | Err(_) => {
                        entity.update(cx, |this, _cx| {
                            this.remove_empty_slack_thread_draft_handle(&handle);
                        });
                        return Ok::<(), String>(());
                    }
                    Ok(Err(error)) => {
                        entity.update(cx, |this, cx| {
                            if !this.slack_thread_draft_handle_exists(&handle) {
                                return;
                            }
                            this.show_slack_thread_draft_error(
                                handle.key(),
                                format!("failed to open Slack attachment picker: {error}"),
                                cx,
                            );
                            this.remove_empty_slack_thread_draft_handle(&handle);
                        });
                        return Ok(());
                    }
                };
                entity.update(cx, |this, cx| {
                    this.attach_slack_thread_file_paths(handle, paths, cx);
                });
                Ok(())
            })
            .detach_and_log_err(cx);
    }

    pub(crate) fn attach_slack_thread_file_paths(
        &mut self,
        handle: SlackThreadDraftHandle,
        paths: Vec<std::path::PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.upload_files
            || !self.slack_workspace_api_capabilities.stage_file
            || self.slack_thread_reply_is_pending(handle.key())
            || !self.slack_thread_draft_handle_exists(&handle)
        {
            return;
        }
        let selection = match self.slack_thread_attachment_path_selection(&handle, paths) {
            Ok(selection) => selection,
            Err(message) => {
                self.show_slack_thread_draft_error(handle.key(), message, cx);
                return;
            }
        };
        let (paths, selection_truncated) = selection.into_parts();
        let Some(local_file_api) = self.local_file_api.clone() else {
            self.show_slack_thread_draft_error(
                handle.key(),
                "Slack local file access is unavailable for this surface.".to_string(),
                cx,
            );
            return;
        };
        self.spawn_background_task(
            (handle, paths, selection_truncated, local_file_api),
            cx,
            |(handle, paths, selection_truncated, local_file_api)| {
                (
                    handle,
                    selection_truncated,
                    load_slack_prepared_upload_files_from_paths(local_file_api.as_ref(), paths),
                )
            },
            |this, (handle, selection_truncated, loaded), cx| {
                finish_slack_thread_attachment_load(this, handle, selection_truncated, loaded, cx);
            },
        );
    }
}

fn finish_slack_thread_attachment_load(
    surface: &mut SurfaceState,
    handle: SlackThreadDraftHandle,
    selection_truncated: bool,
    loaded: Result<Vec<SlackPreparedUploadFile>, String>,
    cx: &mut Context<SurfaceState>,
) {
    if !surface.slack_thread_draft_handle_exists(&handle) {
        return;
    }
    match loaded {
        Ok(files) => {
            if let Err(message) = surface.attach_slack_thread_prepared_upload_files_with_selection(
                &handle,
                files,
                selection_truncated,
                cx,
            ) {
                surface.show_slack_thread_draft_error(handle.key(), message, cx);
            }
        }
        Err(message) => {
            surface.show_slack_thread_draft_error(handle.key(), message, cx);
            surface.remove_empty_slack_thread_draft_handle(&handle);
        }
    }
}
