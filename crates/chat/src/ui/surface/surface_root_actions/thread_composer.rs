use super::{
    Context, SlackAttachmentPathSelection, SlackComposerFileId, SlackThreadDraftHandle, SurfaceRoot,
};

impl SurfaceRoot {
    pub fn focus_slack_thread_composer<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            if !surface.can_send_slack_thread_reply() {
                return Err("no active writable Slack thread".to_string());
            }
            surface
                .slack_thread_panel
                .as_mut()
                .expect("validated Slack thread disappeared before focus")
                .reply_composer_focused = true;
            cx.notify();
            Ok(())
        })
    }

    pub fn set_slack_thread_composer_text<AppState: 'static>(
        &mut self,
        text: String,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            if !surface.can_send_slack_thread_reply() {
                return Err("no active writable Slack thread".to_string());
            }
            surface.set_slack_thread_reply_text(text, cx);
            Ok(())
        })
    }

    pub fn set_slack_thread_reply_broadcast<AppState: 'static>(
        &mut self,
        broadcast: bool,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            if !surface.can_send_slack_thread_reply() {
                return Err("no active writable Slack thread".to_string());
            }
            let panel = surface
                .slack_thread_panel
                .as_ref()
                .expect("validated Slack thread disappeared before broadcast update");
            if broadcast && panel.broadcast_label.is_none() {
                return Err("the active Slack thread cannot broadcast replies".to_string());
            }
            surface.set_slack_thread_reply_broadcast(broadcast, cx);
            Ok(())
        })
    }

    pub fn current_slack_thread_draft_handle<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<SlackThreadDraftHandle, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            if !surface.can_send_slack_thread_reply()
                || !surface.slack_workspace_api_capabilities.upload_files
                || !surface.slack_workspace_api_capabilities.stage_file
            {
                return Err("no active Slack thread accepts file attachments".to_string());
            }
            let key = surface
                .current_slack_thread_reply_draft_key()
                .ok_or_else(|| "no active Slack thread draft".to_string())?;
            surface
                .slack_thread_draft_handle(&key)
                .ok_or_else(|| "no active Slack thread draft".to_string())
        })
    }

    pub fn attach_slack_thread_upload_files<AppState: 'static>(
        &mut self,
        handle: SlackThreadDraftHandle,
        files: Vec<crate::ui::SlackUploadFile>,
        path_selection_truncated: bool,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.attach_slack_thread_upload_files_with_selection(
                &handle,
                files,
                path_selection_truncated,
                cx,
            )
        })
    }

    pub fn attach_slack_thread_prepared_upload_files<AppState: 'static>(
        &mut self,
        handle: SlackThreadDraftHandle,
        files: Vec<super::super::SlackPreparedUploadFile>,
        path_selection_truncated: bool,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatComposerFilesAttached, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            surface.attach_slack_thread_prepared_upload_files_with_selection(
                &handle,
                files,
                path_selection_truncated,
                cx,
            )
        })
    }

    pub fn attach_slack_thread_files_from_paths<AppState, Completion>(
        &mut self,
        paths: Vec<std::path::PathBuf>,
        completion: Completion,
        cx: &mut Context<AppState>,
    ) where
        AppState: 'static,
        Completion:
            FnOnce(Result<crate::model::ChatComposerFilesAttached, String>) + Send + 'static,
    {
        let handle = match self.current_slack_thread_draft_handle(cx) {
            Ok(handle) => handle,
            Err(error) => {
                completion(Err(error));
                return;
            }
        };
        let selection = match self.slack_thread_attachment_path_selection(&handle, paths, cx) {
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
                        surface.attach_slack_thread_prepared_upload_files_with_selection(
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

    pub fn slack_thread_attachment_path_selection<AppState: 'static>(
        &mut self,
        handle: &SlackThreadDraftHandle,
        paths: Vec<std::path::PathBuf>,
        cx: &mut Context<AppState>,
    ) -> Result<SlackAttachmentPathSelection, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            surface.slack_thread_attachment_path_selection(handle, paths)
        })
    }

    pub fn remove_slack_thread_composer_file<AppState: 'static>(
        &mut self,
        file_id: &str,
        cx: &mut Context<AppState>,
    ) -> Result<Vec<crate::model::ChatComposerFileSummary>, String> {
        let file_id = SlackComposerFileId::parse(file_id)?;
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            let key = surface
                .current_slack_thread_reply_draft_key()
                .ok_or_else(|| "no active Slack thread draft".to_string())?;
            if !surface.remove_slack_thread_reply_attachment(&key, file_id, cx) {
                return Err(format!(
                    "Slack thread composer file {file_id} was not found."
                ));
            }
            surface
                .with_slack_thread_reply_draft(&key, |draft| draft.files.control_summaries())
                .ok_or_else(|| "the active Slack thread draft moved after removal".to_string())
        })
    }

    pub fn retry_slack_thread_composer_file<AppState: 'static>(
        &mut self,
        file_id: &str,
        cx: &mut Context<AppState>,
    ) -> Result<crate::model::ChatComposerFileSummary, String> {
        let file_id = SlackComposerFileId::parse(file_id)?;
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            let key = surface
                .current_slack_thread_reply_draft_key()
                .ok_or_else(|| "no active Slack thread draft".to_string())?;
            surface.retry_slack_thread_reply_attachment(&key, file_id, cx)
        })
    }

    pub fn slack_thread_staged_files<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<Vec<crate::ui::SlackStagedFile>, String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            let key = surface
                .current_slack_thread_reply_draft_key()
                .ok_or_else(|| "no active Slack thread draft".to_string())?;
            surface
                .with_slack_thread_reply_draft(&key, |draft| {
                    if draft.files.is_empty() {
                        return Ok(Vec::new());
                    }
                    if !draft.files.all_staged() {
                        return Err(
                            "Slack thread composer files have not all completed staging."
                                .to_string(),
                        );
                    }
                    draft.files.staged_files()
                })
                .ok_or_else(|| "the active Slack thread draft moved".to_string())?
        })
    }

    pub fn send_slack_thread_reply<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> Result<(), String> {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, cx| {
            if !surface.can_send_slack_thread_reply() {
                return Err("no active writable Slack thread".to_string());
            }
            let draft_key = surface
                .slack_thread_panel
                .as_ref()
                .expect("validated Slack thread disappeared before reply submission")
                .reply_draft_key
                .clone();
            if !surface.slack_thread_reply_draft_has_content(&draft_key) {
                return Err("add a reply or attachment before sending".to_string());
            }
            if !surface.slack_thread_reply_draft_is_sendable(&draft_key) {
                return Err("Slack thread attachments are not ready to share yet.".to_string());
            }
            surface.send_slack_thread_reply(cx);
            if surface.slack_thread_reply_is_pending(&draft_key) {
                return Ok(());
            }
            let error = surface
                .slack_thread_panel
                .as_ref()
                .and_then(|panel| panel.reply_error.clone())
                .unwrap_or_else(|| "Slack thread reply did not start".to_string());
            Err(error)
        })
    }
}
