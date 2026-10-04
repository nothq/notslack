use super::{
    Context, SlackAttachmentPathSelection, SlackComposerFileId, SlackComposerFormatAction,
    SlackMainComposerDraftHandle, SurfaceRoot,
};

impl SurfaceRoot {
    pub fn focus_slack_composer<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.focus_slack_composer(cx));
    }

    pub fn apply_slack_composer_format<AppState: 'static>(
        &mut self,
        action: SlackComposerFormatAction,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.apply_slack_composer_format(action, cx);
        });
    }

    #[cfg(test)]
    pub fn attach_slack_draft_attachment<AppState: 'static>(
        &mut self,
        attachment: crate::ui::SlackAttachment,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.attach_slack_draft_attachment(attachment, cx);
        });
    }

    pub fn attach_slack_upload_files<AppState: 'static>(
        &mut self,
        files: Vec<crate::ui::SlackUploadFile>,
        cx: &mut Context<AppState>,
    ) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.attach_slack_upload_files(files, cx);
        });
    }

    pub fn current_slack_main_draft_handle<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<SlackMainComposerDraftHandle, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            if !surface.slack_workspace_api_capabilities.upload_files
                || !surface.slack_workspace_api_capabilities.stage_file
            {
                return Err(
                    "the active Slack composer does not accept file attachments".to_string()
                );
            }
            surface
                .current_slack_main_composer_draft_handle()
                .ok_or_else(|| "no active Slack composer draft".to_string())
        })
    }

    pub fn attach_slack_main_upload_files<AppState: 'static>(
        &mut self,
        handle: SlackMainComposerDraftHandle,
        files: Vec<crate::ui::SlackUploadFile>,
        path_selection_truncated: bool,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.attach_slack_upload_files_to_main_draft_with_selection(
                &handle,
                files,
                path_selection_truncated,
                cx,
            )
        })
    }

    pub fn attach_slack_main_prepared_upload_files<AppState: 'static>(
        &mut self,
        handle: SlackMainComposerDraftHandle,
        files: Vec<super::super::SlackPreparedUploadFile>,
        path_selection_truncated: bool,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.attach_slack_prepared_upload_files_to_main_draft_with_selection(
                &handle,
                files,
                path_selection_truncated,
                cx,
            )
        })
    }

    pub fn attach_slack_main_files_from_paths<AppState, Completion>(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        completion: Completion,
        cx: &mut Context<AppState>,
    ) where
        AppState: 'static,
        Completion:
            FnOnce(Result<crate::model::ChatComposerFilesAttached, String>) + Send + 'static,
    {
        let handle = match self.current_slack_main_draft_handle(cx) {
            Ok(handle) => handle,
            Err(error) => {
                completion(Err(error));
                return;
            }
        };
        let selection = match self.slack_main_attachment_path_selection(&handle, paths, cx) {
            Ok(selection) => selection,
            Err(error) => {
                completion(Err(error));
                return;
            }
        };
        let (paths, path_selection_truncated) = selection.into_parts();
        let surface = self.ensure_surface(cx);
        surface.update(cx, move |surface, cx| {
            let Some(local_file_api) = surface.local_file_api.clone() else {
                completion(Err(
                    "Slack local file access is unavailable for this surface.".to_string(),
                ));
                return;
            };
            crate::ui::spawn_background_task_for_entity(
                (handle, paths, path_selection_truncated, local_file_api),
                cx,
                |(handle, paths, path_selection_truncated, local_file_api)| {
                    (
                        handle,
                        path_selection_truncated,
                        crate::ui::load_slack_prepared_upload_files_from_paths(
                            local_file_api.as_ref(),
                            paths,
                        ),
                    )
                },
                move |surface, (handle, path_selection_truncated, loaded), cx| {
                    let result = loaded.and_then(|files| {
                        surface.attach_slack_prepared_upload_files_to_main_draft_with_selection(
                            &handle,
                            files,
                            path_selection_truncated,
                            cx,
                        )
                    });
                    completion(result);
                },
            );
        });
    }

    pub fn slack_main_attachment_path_selection<AppState: 'static>(
        &mut self,
        handle: &SlackMainComposerDraftHandle,
        paths: Vec<std::path::PathBuf>,
        cx: &mut Context<AppState>,
    ) -> Result<SlackAttachmentPathSelection, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_main_attachment_path_selection(handle, paths)
        })
    }

    pub fn remove_slack_main_composer_file<AppState: 'static>(
        &mut self,
        file_id: &str,
        cx: &mut Context<AppState>,
    ) -> Result<Vec<crate::model::ChatComposerFileSummary>, String> {
        let file_id = SlackComposerFileId::parse(file_id)?;
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            if !surface.remove_slack_draft_attachment(file_id, cx) {
                return Err(format!("Slack composer file {file_id} was not found."));
            }
            Ok(surface.slack_composer_files.control_summaries())
        })
    }

    pub fn retry_slack_main_composer_file<AppState: 'static>(
        &mut self,
        file_id: &str,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatComposerFileSummary, String> {
        let file_id = SlackComposerFileId::parse(file_id)?;
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.retry_slack_draft_attachment(file_id, cx)
        })
    }

    pub fn slack_main_staged_files<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<Vec<crate::ui::SlackStagedFile>, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            if surface.slack_composer_files.is_empty() {
                return Ok(Vec::new());
            }
            if !surface.slack_composer_files.all_staged() {
                return Err("Slack composer files have not all completed staging.".to_string());
            }
            surface.slack_composer_files.staged_files()
        })
    }

    pub fn slack_file_cleanup_summaries<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Vec<crate::model::ChatFileCleanupSummary> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_file_staging.file_cleanup_summaries()
        })
    }

    pub fn slack_remote_draft_file_cleanup_summaries<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Vec<crate::model::ChatRemoteDraftFileCleanupSummary> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_remote_draft_files.cleanup_summaries()
        })
    }

    pub fn send_slack_message<AppState: 'static>(&mut self, cx: &mut Context<AppState>) {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| surface.submit_slack_composer(cx));
    }
}
