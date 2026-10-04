use super::{
    Arc, ClipboardItem, Context, SlackMessageDraft, SlackMessageForwardReceipt,
    SlackMessageForwardSource, SurfaceState, WorkspaceApi,
};

struct SlackMessageForwardSubmission {
    generation: u64,
    workspace_api: Arc<dyn WorkspaceApi>,
    source: SlackMessageForwardSource,
    destination_conversation_id: String,
    note: Option<SlackMessageDraft>,
}

impl SurfaceState {
    pub(crate) fn submit_slack_message_forward(&mut self, cx: &mut Context<Self>) {
        let Some(submission) = self.prepare_slack_message_forward_submission(cx) else {
            return;
        };
        let generation = submission.generation;
        let completion_destination = submission.destination_conversation_id.clone();
        self.spawn_background_task(
            (
                submission.workspace_api,
                submission.source,
                submission.destination_conversation_id,
                submission.note,
            ),
            cx,
            |(workspace_api, source, destination_conversation_id, note)| {
                workspace_api.forward_slack_message(
                    &source.conversation_id,
                    &source.message_timestamp,
                    &destination_conversation_id,
                    note.as_ref(),
                )
            },
            move |this, result: Result<SlackMessageForwardReceipt, String>, cx| {
                this.finish_slack_message_forward(generation, &completion_destination, result, cx);
            },
        );
    }

    fn prepare_slack_message_forward_submission(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<SlackMessageForwardSubmission> {
        let (generation, source, destination_conversation_id, note) = self
            .slack_message_forward_modal
            .as_ref()
            .filter(|modal| !modal.forwarding)
            .and_then(|modal| {
                Some((
                    modal.generation,
                    modal.source.clone(),
                    modal
                        .destination
                        .as_ref()?
                        .conversation_id
                        .as_ref()?
                        .to_string(),
                    modal.note.trim().to_string(),
                ))
            })?;
        let note = self.prepare_slack_message_forward_note(note, cx)?;
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.set_slack_message_forward_error(
                "Slack forwarding requires a connected workspace.".to_string(),
                cx,
            );
            return None;
        };
        let modal = self
            .slack_message_forward_modal
            .as_mut()
            .expect("forward modal disappeared while preparing submission");
        modal.forwarding = true;
        modal.error = None;
        cx.notify();
        Some(SlackMessageForwardSubmission {
            generation,
            workspace_api,
            source,
            destination_conversation_id,
            note,
        })
    }

    fn prepare_slack_message_forward_note(
        &mut self,
        note: String,
        cx: &mut Context<Self>,
    ) -> Option<Option<SlackMessageDraft>> {
        if note.is_empty() {
            return Some(None);
        }
        match SlackMessageDraft::plain_text(note) {
            Ok(note) => Some(Some(note)),
            Err(error) => {
                self.set_slack_message_forward_error(error, cx);
                None
            }
        }
    }

    fn set_slack_message_forward_error(&mut self, error: String, cx: &mut Context<Self>) {
        if let Some(modal) = self.slack_message_forward_modal.as_mut() {
            modal.error = Some(error);
        }
        cx.notify();
    }

    fn finish_slack_message_forward(
        &mut self,
        generation: u64,
        destination_conversation_id: &str,
        result: Result<SlackMessageForwardReceipt, String>,
        cx: &mut Context<Self>,
    ) {
        let Some(modal) = self
            .slack_message_forward_modal
            .as_mut()
            .filter(|modal| modal.generation == generation)
        else {
            return;
        };
        modal.forwarding = false;
        match result {
            Ok(receipt) if receipt.destination_conversation_id == destination_conversation_id => {
                self.slack_message_forward_modal = None;
                self.slack_message_forward_focus_pending = false;
                self.slack_message_forward_note_focus_pending = false;
            }
            Ok(_) => {
                modal.error =
                    Some("Slack forwarded the message to another destination.".to_string());
            }
            Err(error) => modal.error = Some(error),
        }
        cx.notify();
    }

    pub(crate) fn copy_slack_message_forward_link(&mut self, cx: &mut Context<Self>) {
        if !self.slack_workspace_api_capabilities.load_message_permalink {
            return;
        }
        let Some((generation, source)) = self
            .slack_message_forward_modal
            .as_ref()
            .filter(|modal| !modal.copying_link)
            .map(|modal| (modal.generation, modal.source.clone()))
        else {
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            if let Some(modal) = self.slack_message_forward_modal.as_mut() {
                modal.error = Some("Slack forwarding requires a connected workspace.".to_string());
            }
            cx.notify();
            return;
        };
        let Some(modal) = self.slack_message_forward_modal.as_mut() else {
            return;
        };
        modal.copying_link = true;
        modal.link_copied = false;
        modal.error = None;
        cx.notify();

        self.spawn_background_task(
            (workspace_api, source),
            cx,
            |(workspace_api, source): (Arc<dyn WorkspaceApi>, SlackMessageForwardSource)| {
                workspace_api.load_slack_message_permalink(
                    &source.conversation_id,
                    &source.message_timestamp,
                )
            },
            move |this, result, cx| {
                let Some(modal) = this
                    .slack_message_forward_modal
                    .as_mut()
                    .filter(|modal| modal.generation == generation)
                else {
                    return;
                };
                modal.copying_link = false;
                match result {
                    Ok(permalink) => {
                        cx.write_to_clipboard(ClipboardItem::new_string(permalink));
                        modal.link_copied = true;
                        modal.error = None;
                    }
                    Err(error) => modal.error = Some(error),
                }
                cx.notify();
            },
        );
    }

    pub(crate) fn queue_slack_message_forward_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        let urls = self
            .slack_message_forward_modal
            .as_ref()
            .map(|modal| {
                let start = visible_start.min(modal.visible_row_indices.len());
                let end = visible_end.min(modal.visible_row_indices.len());
                modal.visible_row_indices[start..end]
                    .iter()
                    .filter_map(|row_index| modal.rows.get(*row_index))
                    .filter_map(|row| row.avatar_image_url.as_ref())
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }
}
