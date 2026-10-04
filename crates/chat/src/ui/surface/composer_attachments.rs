use gpui::prelude::FluentBuilder;

use super::{
    div, px, AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    SlackComposerDraftKey, SlackComposerFiles, SlackMainComposerDraftHandle,
    SlackThreadDraftHandle, StatefulInteractiveElement, Styled, SurfaceState,
    SLACK_COMPOSER_FILE_LIMIT,
};

mod card;
mod status;

pub(crate) const SLACK_COMPOSER_ATTACHMENTS_HEIGHT: f32 = 78.0;

#[derive(Clone)]
enum SlackComposerAttachmentOwner {
    Main(SlackMainComposerDraftHandle),
    Thread(SlackThreadDraftHandle),
}

impl SurfaceState {
    pub(crate) fn render_slack_main_draft_attachments(
        &self,
        context_id: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(handle) = self.current_slack_main_composer_draft_handle() else {
            return div().into_any_element();
        };
        self.render_slack_composer_attachment_gallery(
            &self.slack_composer_files,
            SlackComposerAttachmentOwner::Main(handle),
            context_id,
            cx,
        )
    }

    pub(crate) fn render_slack_thread_draft_attachments(
        &self,
        draft_key: &SlackComposerDraftKey,
        context_id: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.with_slack_thread_reply_draft(draft_key, |draft| {
            self.render_slack_composer_attachment_gallery(
                &draft.files,
                SlackComposerAttachmentOwner::Thread(SlackThreadDraftHandle::new(
                    draft_key.clone(),
                    draft.id,
                )),
                context_id,
                cx,
            )
        })
        .unwrap_or_else(|| div().into_any_element())
    }

    fn render_slack_composer_attachment_gallery(
        &self,
        files: &SlackComposerFiles,
        owner: SlackComposerAttachmentOwner,
        context_id: &str,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let scroll_handle = files.scroll_handle();
        let rendered_len = files.len().min(SLACK_COMPOSER_FILE_LIMIT);
        div()
            .id(format!("slack-composer-attachment-gallery-{context_id}"))
            .h(px(SLACK_COMPOSER_ATTACHMENTS_HEIGHT))
            .flex_none()
            .px(px(8.0))
            .py(px(8.0))
            .flex()
            .items_center()
            .overflow_x_scroll()
            .track_scroll(&scroll_handle)
            .children(
                files
                    .iter()
                    .take(rendered_len)
                    .enumerate()
                    .map(|(index, file)| {
                        div()
                            .flex_none()
                            .when(index + 1 < rendered_len, |this| this.mr(px(12.0)))
                            .child(self.render_slack_composer_attachment_card(
                                file.presentation(),
                                owner.clone(),
                                context_id,
                                cx,
                            ))
                    }),
            )
            .into_any_element()
    }
}
