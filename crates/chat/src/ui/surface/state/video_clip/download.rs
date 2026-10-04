use gpui::{TaskExt, WeakEntity};

use super::{
    Context, SlackComposerCaptureGeneration, SlackVideoClipCaptureState, SlackVideoClipModal,
    SurfaceState, Window, GENERIC_VIDEO_CLIP_FILENAME,
};
use crate::ui::AppContext;

impl SurfaceState {
    pub(crate) fn prompt_download_reviewed_slack_video_clip(
        &mut self,
        generation: SlackComposerCaptureGeneration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let clip = match self.slack_composer_capture.video() {
            Some(SlackVideoClipCaptureState::Reviewing {
                operation,
                artifact,
                ..
            }) if operation.generation == generation
                && self.slack_composer_capture_operation_is_current(operation) =>
            {
                artifact.clip().clone()
            }
            _ => return Err("There is no reviewed Slack video clip to download.".to_string()),
        };
        let modal = self
            .slack_video_clip_modal
            .as_ref()
            .filter(|modal| modal.read(cx).generation() == generation)
            .map(|modal| modal.downgrade())
            .ok_or_else(|| "The Slack video clip recorder is no longer open.".to_string())?;
        let local_file_api = self.local_file_api.clone().ok_or_else(|| {
            "Slack local file access is unavailable for this surface.".to_string()
        })?;
        let download_directory = local_file_api.default_download_directory()?;
        let path_receiver =
            cx.prompt_for_new_path(&download_directory, Some(GENERIC_VIDEO_CLIP_FILENAME));
        window
            .spawn(cx, async move |cx| {
                let path = match path_receiver.await {
                    Ok(Ok(Some(path))) => path,
                    Ok(Ok(None)) | Err(_) => {
                        finish_slack_video_clip_download(&modal, Ok(()), cx);
                        return Ok::<(), String>(());
                    }
                    Ok(Err(error)) => {
                        finish_slack_video_clip_download(
                            &modal,
                            Err(format!("failed to open video save dialog: {error}")),
                            cx,
                        );
                        return Ok(());
                    }
                };
                let result = cx
                    .background_spawn(async move {
                        local_file_api.save_video_file(clip.file().path(), path)
                    })
                    .await;
                finish_slack_video_clip_download(&modal, result, cx);
                Ok(())
            })
            .detach_and_log_err(cx);
        Ok(())
    }
}

fn finish_slack_video_clip_download(
    modal: &WeakEntity<SlackVideoClipModal>,
    result: Result<(), String>,
    cx: &mut gpui::AsyncWindowContext,
) {
    modal
        .update(cx, |modal, cx| modal.finish_download(result, cx))
        .ok();
}
