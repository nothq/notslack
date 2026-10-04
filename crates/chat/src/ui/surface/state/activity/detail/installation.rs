use std::{collections::HashSet, sync::Arc};

use super::{
    collect_slack_activity_detail_row_urls, find_activity_anchor_index, px,
    slack_activity_conversation_composer_context, Context, ListOffset,
    PreparedSlackConversationSnapshot, PreparedSlackThreadSnapshot, SharedString,
    SlackActivityDetailInstallation, SlackActivityDetailRequest, SlackActivityDetailState,
    SlackActivityDetailTarget, SlackActivityRow, SlackMessageRow, SlackMessageTimestamp,
    SurfaceState, SLACK_ACTIVITY_DETAIL_IMAGE_VISIBLE_OVERDRAW,
    SLACK_ACTIVITY_INITIAL_VISIBLE_ROWS,
};

type SlackActivityThreadRowsResult = Result<Arc<[SlackMessageRow]>, &'static str>;

impl SurfaceState {
    pub(super) fn apply_loaded_slack_activity_conversation_detail(
        &mut self,
        request: SlackActivityDetailRequest,
        prepared: PreparedSlackConversationSnapshot,
        cx: &mut Context<Self>,
    ) {
        if request.thread_timestamp.is_some() {
            self.fail_loaded_slack_activity_detail(
                request,
                "Slack returned a conversation window for a thread reply.",
                cx,
            );
            return;
        }
        if prepared.snapshot.team_id != request.team_id
            || prepared.snapshot.conversation_id != request.channel_id.as_ref()
        {
            self.fail_loaded_slack_activity_detail(
                request,
                "Slack returned a different conversation for this activity item.",
                cx,
            );
            return;
        }
        let Some(anchor_index) =
            find_activity_anchor_index(&prepared.message_rows, &request.message_timestamp)
        else {
            self.fail_loaded_slack_activity_detail(
                request,
                "Slack did not return the requested activity message.",
                cx,
            );
            return;
        };
        let channel_label = request
            .channel_label
            .clone()
            .unwrap_or_else(|| prepared.snapshot.channel_name.clone().into());
        self.slack_remote_images.extend(prepared.remote_images);
        let composer = slack_activity_conversation_composer_context(
            &prepared.snapshot,
            request.self_user_id.clone(),
            request.key.clone(),
        );
        self.install_slack_activity_detail(
            SlackActivityDetailInstallation {
                key: request.key,
                channel_label,
                message_timestamp: request.message_timestamp,
                target: SlackActivityDetailTarget::Conversation {
                    composer: Box::new(composer),
                },
                rows: prepared.message_rows,
                anchor_index,
            },
            cx,
        );
    }

    pub(super) fn apply_loaded_slack_activity_thread_detail(
        &mut self,
        request: SlackActivityDetailRequest,
        mut prepared: PreparedSlackThreadSnapshot,
        cx: &mut Context<Self>,
    ) {
        let installation = prepare_loaded_slack_activity_thread_detail(&request, &mut prepared)
            .map(|rows| {
                let channel_label = request
                    .channel_label
                    .clone()
                    .unwrap_or_else(|| prepared.snapshot.conversation_name.clone().into());
                SlackActivityDetailInstallation {
                    key: request.key.clone(),
                    channel_label,
                    message_timestamp: request.message_timestamp.clone(),
                    target: SlackActivityDetailTarget::Thread {
                        team_id: request.team_id.clone(),
                        self_user_id: request.self_user_id.clone(),
                        conversation_id: request.channel_id.to_string(),
                        item_key: request.key.clone(),
                        thread_timestamp: request
                            .thread_timestamp
                            .clone()
                            .expect("validated activity thread must retain its parent timestamp"),
                    },
                    rows,
                    anchor_index: 1,
                }
            });
        match installation {
            Ok(installation) => self.install_slack_activity_detail(installation, cx),
            Err(message) => self.fail_loaded_slack_activity_detail(request, message, cx),
        }
    }

    fn fail_loaded_slack_activity_detail(
        &mut self,
        request: SlackActivityDetailRequest,
        message: &'static str,
        cx: &mut Context<Self>,
    ) {
        self.park_slack_main_composer(cx);
        self.slack_activity_detail = SlackActivityDetailState::Error {
            key: request.key,
            channel_label: request.channel_label,
            message: message.into(),
        };
        self.rebuild_slack_activity_local_delivery_rows();
        cx.notify();
    }

    pub(super) fn install_slack_activity_detail(
        &mut self,
        installation: SlackActivityDetailInstallation,
        cx: &mut Context<Self>,
    ) {
        let SlackActivityDetailInstallation {
            key,
            channel_label,
            message_timestamp,
            target,
            rows,
            anchor_index,
        } = installation;
        let composer = target.composer().cloned();
        let authoritative_row_count = rows.len();
        self.slack_activity_detail = SlackActivityDetailState::Loaded {
            key,
            channel_label,
            message_timestamp,
            target,
            rows,
        };
        if let Some(composer) = composer {
            self.activate_slack_activity_main_composer(composer, cx);
        }
        self.rebuild_slack_activity_local_delivery_rows();
        let row_count =
            authoritative_row_count.saturating_add(self.slack_activity_local_delivery_rows.len());
        self.slack_activity_detail_list_state.reset(row_count);
        self.slack_activity_detail_list_state.scroll_to(ListOffset {
            item_ix: anchor_index,
            offset_in_item: px(0.0),
        });
        self.queue_slack_activity_detail_visible_images(
            anchor_index,
            anchor_index.saturating_add(SLACK_ACTIVITY_INITIAL_VISIBLE_ROWS),
            cx,
        );
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn fail_slack_activity_detail(
        &mut self,
        row: SlackActivityRow,
        message: SharedString,
        cx: &mut Context<Self>,
    ) {
        self.park_slack_main_composer(cx);
        self.slack_activity_detail = SlackActivityDetailState::Error {
            key: row.key,
            channel_label: row.channel_label,
            message,
        };
        self.rebuild_slack_activity_local_delivery_rows();
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn queue_slack_activity_detail_visible_images(
        &mut self,
        visible_start: usize,
        visible_end: usize,
        cx: &mut Context<Self>,
    ) {
        let SlackActivityDetailState::Loaded { rows, .. } = &self.slack_activity_detail else {
            return;
        };
        let start = visible_start.saturating_sub(SLACK_ACTIVITY_DETAIL_IMAGE_VISIBLE_OVERDRAW);
        let end = visible_end
            .saturating_add(SLACK_ACTIVITY_DETAIL_IMAGE_VISIBLE_OVERDRAW)
            .min(rows.len());
        let mut urls = HashSet::new();
        for row in &rows[start..end] {
            collect_slack_activity_detail_row_urls(row, &mut urls);
        }
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }
}

fn prepare_loaded_slack_activity_thread_detail(
    request: &SlackActivityDetailRequest,
    prepared: &mut PreparedSlackThreadSnapshot,
) -> SlackActivityThreadRowsResult {
    let thread_timestamp = validate_slack_activity_thread_detail(request, prepared)?;
    let mut parent = prepared
        .parent_row
        .take()
        .ok_or("Slack omitted the requested activity thread parent.")?;
    if parent.id != thread_timestamp.as_str() {
        return Err("Slack returned a different parent for this activity thread.");
    }
    let mut reply = prepared
        .reply_rows
        .iter()
        .find(|reply| reply.id == request.message_timestamp.as_str())
        .cloned()
        .ok_or("Slack omitted the requested activity thread reply.")?;
    parent.replies.clear();
    reply.replies.clear();
    Ok(vec![parent, reply].into())
}

fn validate_slack_activity_thread_detail<'a>(
    request: &'a SlackActivityDetailRequest,
    prepared: &PreparedSlackThreadSnapshot,
) -> Result<&'a SlackMessageTimestamp, &'static str> {
    let thread_timestamp = request
        .thread_timestamp
        .as_ref()
        .ok_or("Slack returned a thread window for a top-level activity message.")?;
    if prepared.snapshot.team_id != request.team_id
        || prepared.snapshot.conversation_id != request.channel_id.as_ref()
        || prepared.snapshot.thread_timestamp != thread_timestamp.as_str()
    {
        return Err("Slack returned a different thread for this activity item.");
    }
    Ok(thread_timestamp)
}
