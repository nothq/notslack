use std::collections::HashSet;
use std::sync::Arc;

use gpui::{Entity, FocusHandle, SharedString};
use gpui_components::text_input::TextInput;

use super::{
    build_slack_thread_page_reply_rows_in_timezone, build_slack_thread_parent_row_in_timezone,
    slack_all_threads_timestamp_label, slack_local_today, slack_thread_broadcast_label,
    SlackComposerFormatAction, SlackMessageRow, SlackReplyComposerTarget,
};
use crate::ui::{
    SlackAllThread, SlackAllThreadsSnapshot, SlackAttachment, SlackConversationKind,
    SlackUserPresence,
};

#[derive(Clone)]
pub(crate) struct SlackAllThreadRow {
    pub(crate) key: SharedString,
    pub(crate) conversation_id: SharedString,
    pub(crate) direct_message_user_id: Option<SharedString>,
    pub(crate) conversation_label: SharedString,
    pub(crate) participant_label: SharedString,
    pub(crate) direct_message_presence: Option<SlackUserPresence>,
    pub(crate) broadcast_label: Option<SharedString>,
    pub(crate) accessibility_label: SharedString,
    pub(crate) parent: SlackMessageRow,
    pub(crate) replies: Arc<[SlackMessageRow]>,
    pub(crate) starts_read_section: bool,
    pub(crate) hidden_reply_count: u32,
    pub(crate) remote_image_urls: Arc<[SharedString]>,
}

pub(crate) struct SlackAllThreadsComposerState {
    pub(crate) target: SlackReplyComposerTarget,
    pub(crate) input: Entity<TextInput>,
    pub(crate) formatting_enabled: bool,
    pub(crate) format_roving_target: SlackComposerFormatAction,
    pub(crate) format_focus_handles: [FocusHandle; 10],
}

pub(crate) struct PreparedSlackAllThreadsSnapshot {
    pub(crate) snapshot: SlackAllThreadsSnapshot,
    pub(crate) rows: Arc<[SlackAllThreadRow]>,
}

pub(crate) fn merge_and_prepare_slack_all_threads_snapshot(
    existing: Option<SlackAllThreadsSnapshot>,
    page: SlackAllThreadsSnapshot,
) -> Result<PreparedSlackAllThreadsSnapshot, String> {
    let snapshot = merge_slack_all_threads_snapshot(existing, page)?;
    let rows = build_slack_all_thread_rows(&snapshot);
    Ok(PreparedSlackAllThreadsSnapshot { snapshot, rows })
}

pub(crate) fn prepare_slack_all_threads_snapshot(
    snapshot: SlackAllThreadsSnapshot,
) -> PreparedSlackAllThreadsSnapshot {
    let rows = build_slack_all_thread_rows(&snapshot);
    PreparedSlackAllThreadsSnapshot { snapshot, rows }
}

fn build_slack_all_thread_rows(snapshot: &SlackAllThreadsSnapshot) -> Arc<[SlackAllThreadRow]> {
    let timezone = snapshot.timezone.as_chrono_tz();
    let today = slack_local_today(timezone);
    let mut unread_section_seen = false;
    let mut read_section_started = false;
    snapshot
        .threads
        .iter()
        .map(|thread| {
            let has_unread_replies = !thread.unread_reply_timestamps.is_empty();
            let starts_read_section =
                unread_section_seen && !read_section_started && !has_unread_replies;
            unread_section_seen |= has_unread_replies;
            read_section_started |= starts_read_section;
            build_slack_all_thread_row(
                thread,
                timezone,
                today,
                &snapshot.team_id,
                starts_read_section,
            )
        })
        .collect::<Vec<_>>()
        .into()
}

fn merge_slack_all_threads_snapshot(
    existing: Option<SlackAllThreadsSnapshot>,
    page: SlackAllThreadsSnapshot,
) -> Result<SlackAllThreadsSnapshot, String> {
    let Some(mut existing) = existing else {
        return Ok(page);
    };
    if existing.team_id != page.team_id {
        return Err(format!(
            "Slack All Threads page targeted team {} after team {}",
            page.team_id, existing.team_id
        ));
    }
    if existing.timezone != page.timezone {
        return Err(format!(
            "Slack All Threads page used timezone {} after {}",
            page.timezone, existing.timezone
        ));
    }
    let mut keys = existing
        .threads
        .iter()
        .map(|thread| thread.id.clone())
        .collect::<HashSet<_>>();
    for thread in page.threads {
        if !keys.insert(thread.id.clone()) {
            return Err(format!(
                "Slack All Threads pagination repeated thread {}",
                thread.id
            ));
        }
        existing.threads.push(thread);
    }
    existing.next_cursor = page.next_cursor;
    if page.total_unread_replies.is_some() {
        existing.total_unread_replies = page.total_unread_replies;
    }
    Ok(existing)
}

fn build_slack_all_thread_row(
    thread: &SlackAllThread,
    timezone: chrono_tz::Tz,
    today: time::Date,
    team_id: &str,
    starts_read_section: bool,
) -> SlackAllThreadRow {
    let parent = prepare_all_thread_parent(thread, timezone, today, team_id);
    let replies = prepare_all_thread_replies(thread, timezone, today, team_id);
    let participant_label = participant_label(&thread.participant_names);
    let conversation_label =
        conversation_label(thread.conversation_kind, thread.conversation_name.as_str());
    let hidden_reply_count = thread
        .reply_count
        .saturating_sub(u32::try_from(thread.visible_replies.len()).unwrap_or(u32::MAX));
    let accessibility_label = if participant_label.is_empty() {
        format!("Thread in {conversation_label}")
    } else {
        format!("Thread in {conversation_label}, with {participant_label}")
    };
    let remote_image_urls = collect_thread_remote_image_urls(&parent, &replies).into();
    SlackAllThreadRow {
        key: thread.id.clone().into(),
        conversation_id: thread.conversation_id.clone().into(),
        direct_message_user_id: thread.direct_message_user_id.clone().map(Into::into),
        conversation_label: conversation_label.clone().into(),
        participant_label: participant_label.into(),
        direct_message_presence: thread.direct_message_presence,
        broadcast_label: slack_thread_broadcast_label(
            thread.conversation_kind,
            &thread.conversation_name,
        ),
        accessibility_label: accessibility_label.into(),
        parent,
        replies,
        starts_read_section,
        hidden_reply_count,
        remote_image_urls,
    }
}

fn prepare_all_thread_parent(
    thread: &SlackAllThread,
    timezone: chrono_tz::Tz,
    today: time::Date,
    team_id: &str,
) -> SlackMessageRow {
    let mut parent = build_slack_thread_parent_row_in_timezone(
        &thread.parent,
        timezone,
        team_id,
        &thread.conversation_id,
    );
    parent.timestamp = slack_all_threads_timestamp_label(&thread.parent, timezone, today);
    parent.reply_count = None;
    parent.latest_reply_timestamp = None;
    parent.reply_summary = None;
    parent.latest_reply_author = None;
    parent.latest_reply_user_id = None;
    parent.latest_reply_avatar_text = None;
    parent.latest_reply_avatar_image_url = None;
    parent.replies.clear();
    parent
}

fn prepare_all_thread_replies(
    thread: &SlackAllThread,
    timezone: chrono_tz::Tz,
    today: time::Date,
    team_id: &str,
) -> Arc<[SlackMessageRow]> {
    let mut replies = build_slack_thread_page_reply_rows_in_timezone(
        &thread.visible_replies,
        timezone,
        team_id,
        &thread.conversation_id,
        &thread.thread_timestamp,
    );
    Arc::make_mut(&mut replies)
        .iter_mut()
        .zip(&thread.visible_replies)
        .for_each(|(row, message)| {
            row.timestamp = slack_all_threads_timestamp_label(message, timezone, today);
        });
    let unread_reply_timestamps = thread
        .unread_reply_timestamps
        .iter()
        .map(|timestamp| timestamp.as_str())
        .collect::<HashSet<_>>();
    if let Some(first_unread_reply) = Arc::make_mut(&mut replies)
        .iter_mut()
        .find(|row| unread_reply_timestamps.contains(row.id.as_str()))
    {
        first_unread_reply.unread_boundary_before = true;
        first_unread_reply.compact = false;
    }
    replies
}

fn conversation_label(kind: SlackConversationKind, name: &str) -> String {
    if kind.is_channel() && !name.starts_with('#') {
        format!("# {name}")
    } else {
        name.to_string()
    }
}

fn participant_label(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [name] => name.clone(),
        [left, right] => format!("{left} and {right}"),
        names => {
            let (last, prefix) = names.split_last().expect("non-empty checked by pattern");
            format!("{}, and {last}", prefix.join(", "))
        }
    }
}

fn collect_thread_remote_image_urls(
    parent: &SlackMessageRow,
    replies: &[SlackMessageRow],
) -> Vec<SharedString> {
    let mut urls = HashSet::new();
    for message in std::iter::once(parent).chain(replies.iter()) {
        collect_message_remote_image_urls(message, &mut urls);
    }
    urls.into_iter().map(Into::into).collect()
}

fn collect_message_remote_image_urls(message: &SlackMessageRow, urls: &mut HashSet<String>) {
    if let Some(url) = message.avatar_image_url.as_ref() {
        urls.insert(url.clone());
    }
    urls.extend(message.reactions.iter().flat_map(|reaction| {
        reaction
            .variants
            .iter()
            .filter_map(|variant| variant.image_cache_key.as_ref().map(ToString::to_string))
    }));
    for attachment in &message.attachments {
        collect_attachment_remote_image_urls(&attachment.attachment, urls);
    }
}

fn collect_attachment_remote_image_urls(attachment: &SlackAttachment, urls: &mut HashSet<String>) {
    if let Some(url) = attachment.preview_image_url.as_ref() {
        urls.insert(url.clone());
    }
    let Some(metadata) = attachment.legacy_metadata() else {
        return;
    };
    urls.extend(metadata.service_icon_url.iter().cloned());
    urls.extend(metadata.thumbnail_url.iter().cloned());
    urls.extend(metadata.author_avatar_image_url.iter().cloned());
    urls.extend(
        metadata
            .files
            .iter()
            .filter_map(|file| file.preview_image_url.clone()),
    );
}
