use super::super::SLACK_REMOTE_IMAGE_PENDING_LIMIT;
use super::image_helpers::{
    slack_attachment_has_remote_preview, slack_attachment_remote_image_urls,
    slack_message_reaction_remote_image_urls,
};
use super::{
    px, Arc, Context, HashSet, Image, SlackMainTab, SlackMessageRow,
    SlackRemoteImagePrefetchRequest, SlackSidebarRow, SlackSidebarRowKind, SlackWorkspace,
    SurfaceState, SLACK_MESSAGE_ATTACHMENT_PREFETCH_ROW_LIMIT,
    SLACK_MESSAGE_REMOTE_IMAGE_ROW_LIMIT, SLACK_SIDEBAR_REMOTE_IMAGE_ROW_LIMIT,
};

impl SurfaceState {
    pub(in crate::ui::surface) fn enqueue_slack_remote_image_url(
        &mut self,
        url: String,
        cx: &mut Context<Self>,
    ) {
        if self.slack_remote_images.contains_key(&url)
            || self.slack_active_remote_image_urls.contains(url.as_str())
            || self
                .slack_pending_remote_image_urls
                .iter()
                .any(|pending| pending == &url)
        {
            return;
        }
        self.slack_pending_remote_image_urls.push(url);
        self.start_next_slack_remote_image_load(cx);
    }

    pub(in crate::ui::surface::state) fn prefetch_slack_message_preview_images(
        &mut self,
        message_rows: &[SlackMessageRow],
        cx: &mut Context<Self>,
    ) {
        for url in message_rows
            .iter()
            .rev()
            .take(SLACK_MESSAGE_REMOTE_IMAGE_ROW_LIMIT)
            .filter_map(|message| message.avatar_image_url.as_ref())
            .cloned()
        {
            self.enqueue_slack_remote_image_url(url, cx);
        }
        for url in message_rows
            .iter()
            .rev()
            .take(SLACK_MESSAGE_REMOTE_IMAGE_ROW_LIMIT)
            .flat_map(|message| message.reply_participants.iter())
            .filter_map(|participant| participant.avatar_image_url.as_ref())
        {
            self.enqueue_slack_remote_image_url(url.to_string(), cx);
        }
        for url in message_rows
            .iter()
            .rev()
            .take(SLACK_MESSAGE_ATTACHMENT_PREFETCH_ROW_LIMIT)
            .flat_map(|message| message.body.remote_image_urls())
        {
            self.enqueue_slack_remote_image_url(url.to_string(), cx);
        }
        for url in message_rows
            .iter()
            .rev()
            .take(SLACK_MESSAGE_ATTACHMENT_PREFETCH_ROW_LIMIT)
            .flat_map(|message| message.attachments.iter())
            .flat_map(|attachment| slack_attachment_remote_image_urls(&attachment.attachment))
            .filter(|url| url.starts_with("http://") || url.starts_with("https://"))
        {
            self.enqueue_slack_remote_image_url(url.to_string(), cx);
        }
    }

    pub(in crate::ui::surface::state) fn prefetch_slack_sidebar_preview_images(
        &mut self,
        sidebar_rows: &[SlackSidebarRow],
        cx: &mut Context<Self>,
    ) {
        for url in sidebar_rows
            .iter()
            .filter(|row| {
                matches!(
                    &row.kind,
                    SlackSidebarRowKind::Item { item, .. } if item.active
                )
            })
            .chain(
                sidebar_rows
                    .iter()
                    .take(SLACK_SIDEBAR_REMOTE_IMAGE_ROW_LIMIT),
            )
            .filter_map(|row| match &row.kind {
                SlackSidebarRowKind::Item { item, .. } => item.avatar_image_url.as_ref(),
                _ => None,
            })
        {
            self.enqueue_slack_remote_image_url(url.clone(), cx);
        }
    }

    pub(in crate::ui::surface::state) fn prefetch_slack_visible_images(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let message_rows = self.slack_message_rows.clone();
        let sidebar_rows = self.slack_sidebar_rows.clone();
        self.prefetch_slack_message_preview_images(&message_rows, cx);
        self.prefetch_slack_sidebar_preview_images(&sidebar_rows, cx);
        if let Some(url) = self
            .slack_workspace()
            .and_then(|workspace| workspace.workspace_logo_url.clone())
        {
            self.enqueue_slack_remote_image_url(url, cx);
        }
        if let Some(url) = self
            .slack_workspace()
            .and_then(|workspace| workspace.self_avatar_image_url.clone())
        {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }

    pub fn apply_slack_remote_image(
        &mut self,
        url: &str,
        image: Arc<Image>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_remote_images.contains_key(url) {
            return;
        }
        let affected_message_rows =
            self.slack_message_rows
                .iter()
                .enumerate()
                .filter_map(|(index, row)| {
                    (row.body
                        .remote_image_urls()
                        .any(|image_url| image_url == url)
                        || row.attachments.iter().any(|attachment| {
                            attachment.preview_cache_key.as_deref() == Some(url)
                                || attachment.shared_message.as_ref().is_some_and(
                                    |shared_message| {
                                        shared_message.files.iter().any(|file| {
                                            file.preview_cache_key.as_deref() == Some(url)
                                        })
                                    },
                                )
                        }))
                    .then_some(index)
                })
                .collect::<Vec<_>>();
        self.slack_remote_images.insert(url.to_string(), image);
        for row_index in affected_message_rows.iter().copied() {
            self.slack_message_list_state
                .remeasure_items(row_index..row_index + 1);
        }
        if !affected_message_rows.is_empty() {
            self.restore_slack_message_list_auto_position();
        }
        cx.notify();
    }

    pub fn slack_remote_image_prefetch_request(&self) -> Option<SlackRemoteImagePrefetchRequest> {
        let workspace = self.slack_workspace()?;
        let api = self.active_slack_workspace_api()?;
        let mut seen = HashSet::new();
        let missing_urls = self
            .slack_workspace_remote_image_urls(workspace)
            .into_iter()
            .filter(|url| !self.slack_remote_images.contains_key(url))
            .filter(|url| seen.insert(url.clone()))
            .take(SLACK_REMOTE_IMAGE_PENDING_LIMIT)
            .collect::<Vec<_>>();
        (!missing_urls.is_empty()).then_some((api, missing_urls))
    }

    pub(crate) fn slack_workspace_remote_image_urls(
        &self,
        workspace: &SlackWorkspace,
    ) -> Vec<String> {
        let mut urls = workspace
            .workspace_logo_url
            .iter()
            .cloned()
            .chain(workspace.self_avatar_image_url.iter().cloned())
            .collect::<Vec<_>>();
        self.extend_slack_message_remote_image_urls(&mut urls);
        self.extend_slack_sidebar_remote_image_urls(&mut urls);
        self.extend_slack_aux_panel_remote_image_urls(&mut urls);
        self.extend_slack_expanded_attachment_remote_image_urls(&mut urls);
        urls
    }

    fn extend_slack_message_remote_image_urls(&self, urls: &mut Vec<String>) {
        let message_start = self.slack_primary_message_row_start();
        let message_rows = self
            .slack_message_rows
            .iter()
            .skip(message_start)
            .take(SLACK_MESSAGE_REMOTE_IMAGE_ROW_LIMIT);
        for row in message_rows {
            urls.extend(row.avatar_image_url.iter().cloned());
            urls.extend(row.body.remote_image_urls().map(str::to_string));
            urls.extend(slack_message_reaction_remote_image_urls(row).map(str::to_string));
            urls.extend(row.reply_participants.iter().filter_map(|participant| {
                participant
                    .avatar_image_url
                    .as_ref()
                    .map(ToString::to_string)
            }));
            urls.extend(
                row.replies
                    .iter()
                    .take(SLACK_MESSAGE_REMOTE_IMAGE_ROW_LIMIT)
                    .filter_map(|reply| reply.avatar_image_url.clone()),
            );
            urls.extend(
                row.replies
                    .iter()
                    .take(SLACK_MESSAGE_REMOTE_IMAGE_ROW_LIMIT)
                    .flat_map(slack_message_reaction_remote_image_urls)
                    .map(str::to_string),
            );
            urls.extend(
                row.attachments
                    .iter()
                    .flat_map(|attachment| {
                        slack_attachment_remote_image_urls(&attachment.attachment)
                    })
                    .map(str::to_string),
            );
        }
    }

    fn extend_slack_sidebar_remote_image_urls(&self, urls: &mut Vec<String>) {
        let sidebar_start = self.slack_primary_sidebar_row_start();
        urls.extend(
            self.slack_sidebar_rows
                .iter()
                .skip(sidebar_start)
                .take(SLACK_SIDEBAR_REMOTE_IMAGE_ROW_LIMIT)
                .filter_map(|row| match &row.kind {
                    SlackSidebarRowKind::Item { item, .. } => item.avatar_image_url.clone(),
                    _ => None,
                }),
        );
        urls.extend(
            self.slack_sidebar_rows
                .iter()
                .filter_map(|row| match &row.kind {
                    SlackSidebarRowKind::Item { item, .. } if item.active => {
                        item.avatar_image_url.clone()
                    }
                    _ => None,
                }),
        );
    }

    fn extend_slack_aux_panel_remote_image_urls(&self, urls: &mut Vec<String>) {
        urls.extend(
            self.slack_aux_panel
                .iter()
                .flat_map(|panel| panel.sections.iter())
                .take(4)
                .flat_map(|section| section.rows.iter())
                .take(60)
                .filter_map(|row| row.image_url.clone()),
        );
    }

    fn extend_slack_expanded_attachment_remote_image_urls(&self, urls: &mut Vec<String>) {
        if let Some(selection) = self.slack_expanded_attachment.as_ref() {
            urls.extend(
                slack_attachment_remote_image_urls(&selection.attachment).map(str::to_string),
            );
        }
    }

    pub(in crate::ui::surface::state) fn slack_primary_message_row_start(&self) -> usize {
        let row_count = self.slack_message_rows.len();
        if self.slack_message_list_state.viewport_bounds().size.height <= px(0.0) {
            return row_count.saturating_sub(SLACK_MESSAGE_REMOTE_IMAGE_ROW_LIMIT);
        }
        self.slack_message_list_state
            .logical_scroll_top()
            .item_ix
            .saturating_sub(4)
            .min(row_count.saturating_sub(SLACK_MESSAGE_REMOTE_IMAGE_ROW_LIMIT))
    }

    pub(in crate::ui::surface::state) fn slack_primary_sidebar_row_start(&self) -> usize {
        let row_count = self.slack_sidebar_rows.len();
        if self.slack_sidebar_list_state.viewport_bounds().size.height <= px(0.0) {
            return self
                .slack_sidebar_rows
                .iter()
                .position(|row| {
                    matches!(
                        &row.kind,
                        SlackSidebarRowKind::Item { item, .. } if item.active
                    )
                })
                .unwrap_or(0)
                .saturating_sub(8);
        }
        self.slack_sidebar_list_state
            .logical_scroll_top()
            .item_ix
            .saturating_sub(4)
            .min(row_count.saturating_sub(SLACK_SIDEBAR_REMOTE_IMAGE_ROW_LIMIT))
    }

    pub(in crate::ui::surface::state) fn should_load_slack_attachment_previews(&self) -> bool {
        self.slack_expanded_attachment.is_some()
            || (self.slack_active_tab == SlackMainTab::Messages
                && self
                    .slack_message_rows
                    .iter()
                    .rev()
                    .take(SLACK_MESSAGE_ATTACHMENT_PREFETCH_ROW_LIMIT)
                    .flat_map(|row| row.attachments.iter())
                    .any(|attachment| slack_attachment_has_remote_preview(&attachment.attachment)))
    }

    pub(crate) fn mark_slack_remote_image_queue_dirty(&mut self) {
        self.slack_remote_image_queue_dirty = true;
    }
}
