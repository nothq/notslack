use std::{collections::HashSet, sync::Arc};

use crate::model::{SlackFileMetadataEntry, SlackFileMetadataRequest};

use super::rows::slack_drafts_sent_tab_index;
use super::{Context, SurfaceState};
use crate::ui::surface::{
    SlackDraftsSentFileCard, SlackDraftsSentFileMetadataTarget, SlackDraftsSentFilePresentation,
    SlackDraftsSentRow,
};

const SLACK_DRAFTS_SENT_FILE_METADATA_OVERDRAW: usize = 2;
const SLACK_DRAFTS_SENT_FILE_METADATA_MAX_ACTIVE: usize = 4;

impl SurfaceState {
    pub(super) fn queue_slack_drafts_sent_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        let urls = self.slack_drafts_sent_rows[visible_start.min(self.slack_drafts_sent_rows.len())
            ..visible_end.min(self.slack_drafts_sent_rows.len())]
            .iter()
            .filter_map(|row| match row {
                SlackDraftsSentRow::Item(row) => row.avatar_image_url.as_deref(),
                SlackDraftsSentRow::DateDivider { .. } => None,
            })
            .map(str::to_string)
            .collect::<HashSet<_>>();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }

    pub(super) fn queue_slack_drafts_sent_file_metadata(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_workspace_api_capabilities.load_file_metadata {
            return;
        }
        let Some(team_id) = self.slack_drafts_sent_team_id.clone() else {
            return;
        };
        let Some(self_user_id) = self.slack_drafts_sent_self_user_id.clone() else {
            return;
        };
        let start = visible_start.saturating_sub(SLACK_DRAFTS_SENT_FILE_METADATA_OVERDRAW);
        let end = visible_end
            .saturating_add(SLACK_DRAFTS_SENT_FILE_METADATA_OVERDRAW)
            .min(self.slack_drafts_sent_rows.len());
        let targets = self.slack_drafts_sent_rows[start.min(end)..end]
            .iter()
            .filter_map(|row| match row {
                SlackDraftsSentRow::Item(row) => row.files.pending_references().map(|references| {
                    SlackDraftsSentFileMetadataTarget {
                        team_id: team_id.clone(),
                        self_user_id: self_user_id.clone(),
                        tab: self.slack_drafts_sent_tab,
                        row_id: row.id.clone(),
                        references: references.clone(),
                    }
                }),
                SlackDraftsSentRow::DateDivider { .. } => None,
            })
            .collect::<Vec<_>>();
        for target in targets {
            self.slack_drafts_sent_file_metadata.enqueue(target);
        }
        self.dispatch_slack_drafts_sent_file_metadata(cx);
    }

    fn dispatch_slack_drafts_sent_file_metadata(&mut self, cx: &mut Context<Self>) {
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        while let Some(target) = self
            .slack_drafts_sent_file_metadata
            .start_next(SLACK_DRAFTS_SENT_FILE_METADATA_MAX_ACTIVE)
        {
            if self.slack_drafts_sent_team_id.as_deref() != Some(target.team_id.as_str())
                || self.slack_drafts_sent_self_user_id.as_deref()
                    != Some(target.self_user_id.as_str())
                || !self.mark_slack_drafts_sent_files_loading(&target)
            {
                self.slack_drafts_sent_file_metadata.finish(&target);
                continue;
            }
            let request = SlackFileMetadataRequest::new(target.references.to_vec())
                .expect("authenticated non-empty Slack draft file references must form a request");
            let request_api = workspace_api.clone();
            self.spawn_background_task(
                (target, request),
                cx,
                move |(target, request)| {
                    let result = request_api.load_slack_file_metadata(&request);
                    (target, result)
                },
                |this, (target, result), cx| {
                    this.finish_slack_drafts_sent_file_metadata(target, result, cx);
                },
            );
        }
    }

    fn finish_slack_drafts_sent_file_metadata(
        &mut self,
        target: SlackDraftsSentFileMetadataTarget,
        result: Result<crate::model::SlackFileMetadataBatch, String>,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_drafts_sent_file_metadata.finish(&target) {
            return;
        }
        if self.slack_drafts_sent_team_id.as_deref() == Some(target.team_id.as_str())
            && self.slack_drafts_sent_self_user_id.as_deref() == Some(target.self_user_id.as_str())
        {
            let cards = match result {
                Ok(batch) => batch
                    .into_entries()
                    .into_iter()
                    .map(|entry| match entry {
                        SlackFileMetadataEntry::Loaded { metadata, .. } => {
                            SlackDraftsSentFileCard::Loaded(metadata.attachment_arc())
                        }
                        SlackFileMetadataEntry::Failed { .. } => {
                            SlackDraftsSentFileCard::Unavailable
                        }
                    })
                    .collect::<Vec<_>>(),
                Err(_) => target
                    .references
                    .iter()
                    .map(|_| SlackDraftsSentFileCard::Unavailable)
                    .collect(),
            };
            let preview_urls = cards
                .iter()
                .filter_map(|card| match card {
                    SlackDraftsSentFileCard::Loaded(attachment) => {
                        attachment.preview_image_url.clone()
                    }
                    SlackDraftsSentFileCard::Loading | SlackDraftsSentFileCard::Unavailable => None,
                })
                .collect::<Vec<_>>();
            self.apply_slack_drafts_sent_file_cards(&target, cards.into());
            for url in preview_urls {
                self.enqueue_slack_remote_image_url(url, cx);
            }
        }
        self.dispatch_slack_drafts_sent_file_metadata(cx);
        cx.notify();
    }

    fn mark_slack_drafts_sent_files_loading(
        &mut self,
        target: &SlackDraftsSentFileMetadataTarget,
    ) -> bool {
        self.update_slack_drafts_sent_file_presentation(target, |files| match files {
            SlackDraftsSentFilePresentation::Pending { references }
                if references == &target.references =>
            {
                Some(SlackDraftsSentFilePresentation::Loading {
                    references: references.clone(),
                })
            }
            SlackDraftsSentFilePresentation::None
            | SlackDraftsSentFilePresentation::Pending { .. }
            | SlackDraftsSentFilePresentation::Loading { .. }
            | SlackDraftsSentFilePresentation::Loaded { .. } => None,
        })
    }

    fn apply_slack_drafts_sent_file_cards(
        &mut self,
        target: &SlackDraftsSentFileMetadataTarget,
        cards: Arc<[SlackDraftsSentFileCard]>,
    ) -> bool {
        self.update_slack_drafts_sent_file_presentation(target, |files| {
            let references = files.references()?;
            (references == &target.references).then(|| SlackDraftsSentFilePresentation::Loaded {
                references: references.clone(),
                cards,
            })
        })
    }

    fn update_slack_drafts_sent_file_presentation(
        &mut self,
        target: &SlackDraftsSentFileMetadataTarget,
        update: impl FnOnce(&SlackDraftsSentFilePresentation) -> Option<SlackDraftsSentFilePresentation>,
    ) -> bool {
        let index = slack_drafts_sent_tab_index(target.tab);
        let rows: &mut [SlackDraftsSentRow] =
            Arc::make_mut(&mut self.slack_drafts_sent_cached_rows[index]);
        let Some(row) = rows.iter_mut().find_map(|row| match row {
            SlackDraftsSentRow::Item(row) if row.id == target.row_id => Some(Arc::make_mut(row)),
            SlackDraftsSentRow::Item(_) | SlackDraftsSentRow::DateDivider { .. } => None,
        }) else {
            return false;
        };
        let Some(files) = update(&row.files) else {
            return false;
        };
        row.files = files;
        if self.slack_drafts_sent_tab == target.tab {
            self.slack_drafts_sent_rows = self.slack_drafts_sent_cached_rows[index].clone();
        }
        true
    }
}
