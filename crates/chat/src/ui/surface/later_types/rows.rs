use gpui::SharedString;

use super::{
    SlackLaterReminderDetail, SlackLaterRow, SlackLaterRowContent, SlackLaterThreadTarget,
};
use crate::ui::surface::{
    build_slack_thread_parent_row, SlackMessageActionTarget, SlackMessageRow,
};
use crate::ui::{
    initials, slack_avatar_fill, SlackAttachment, SlackConversationKind, SlackLaterContent,
    SlackLaterFile, SlackLaterHydratedItem, SlackLaterItem, SlackLaterItemKey, SlackLaterMessage,
    SlackLaterReferenceContent, SlackLaterReminder, SlackLaterSnapshot, SlackLaterState,
    SlackLaterTombstoneKind,
};

pub(crate) fn build_slack_later_rows(snapshot: &SlackLaterSnapshot) -> Vec<SlackLaterRow> {
    snapshot
        .items
        .iter()
        .map(slack_later_reference_row)
        .collect()
}

pub(crate) fn prepare_slack_later_hydrated_row(
    hydrated: SlackLaterHydratedItem,
    team_id: &str,
) -> SlackLaterRow {
    match hydrated.content {
        SlackLaterContent::Message(message) => {
            slack_later_hydrated_message_row(hydrated.key, *message, team_id)
        }
        SlackLaterContent::File(file) => slack_later_hydrated_file_row(hydrated.key, *file),
    }
}

fn slack_later_hydrated_message_row(
    key: SlackLaterItemKey,
    message: SlackLaterMessage,
    team_id: &str,
) -> SlackLaterRow {
    let expected_reply_count = message.message.reply_count.unwrap_or_default();
    let selected_message_row = slack_later_selected_message_row(&message, team_id);
    let type_label = slack_later_message_type_label(&message);
    let title = message.message.author.clone();
    let preview = (!message.message.body.is_empty()).then(|| message.message.body.clone());
    let accessibility_label = later_accessibility_label(&title, preview.as_deref());
    SlackLaterRow {
        element_id: later_element_id(&key),
        key: key.clone(),
        accessibility_label: accessibility_label.into(),
        type_label: Some(type_label.into()),
        title: title.into(),
        preview: preview.map(Into::into),
        avatar_label: message
            .message
            .avatar_label
            .clone()
            .unwrap_or_else(|| initials(&message.message.author))
            .into(),
        avatar_fill: slack_avatar_fill(&message.message.author),
        avatar_image_url: message.message.avatar_image_url.clone().map(Into::into),
        height: 96.0,
        content: SlackLaterRowContent::Message {
            detail: Box::new(SlackLaterThreadTarget {
                item_key: key,
                conversation_id: message.conversation_id,
                conversation_kind: message.conversation_kind,
                conversation_name: message.conversation_name.into(),
                thread_timestamp: message.thread_timestamp,
                selected_message_id: message.timestamp.into(),
                selected_message_row,
                expected_reply_count,
            }),
        },
    }
}

fn slack_later_selected_message_row(message: &SlackLaterMessage, team_id: &str) -> SlackMessageRow {
    let mut row =
        build_slack_thread_parent_row(&message.message, team_id, &message.conversation_id);
    if message.thread_timestamp != message.timestamp {
        row.action_target = SlackMessageActionTarget::thread_reply(
            team_id,
            &message.conversation_id,
            &message.thread_timestamp,
            &message.timestamp,
        );
    }
    row.compact = false;
    row.divider = None;
    row.unread_boundary_before = false;
    row.date_label = None;
    row.reply_count = None;
    row.latest_reply_timestamp = None;
    row.reply_summary = None;
    row.latest_reply_author = None;
    row.latest_reply_user_id = None;
    row.latest_reply_avatar_text = None;
    row.latest_reply_avatar_image_url = None;
    row.replies.clear();
    row
}

fn slack_later_message_type_label(message: &SlackLaterMessage) -> String {
    match message.conversation_kind {
        SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage => {
            "Direct Message".to_string()
        }
        SlackConversationKind::Channel
        | SlackConversationKind::PrivateChannel
        | SlackConversationKind::Unknown => message.conversation_name.clone(),
    }
}

fn slack_later_hydrated_file_row(key: SlackLaterItemKey, file: SlackLaterFile) -> SlackLaterRow {
    let type_label = slack_later_file_type_label(&file.attachment);
    let title = file.attachment.title.clone();
    let preview = file.owner_label;
    let accessibility_label = later_accessibility_label(&title, preview.as_deref());
    SlackLaterRow {
        element_id: later_element_id(&key),
        key,
        accessibility_label: accessibility_label.into(),
        type_label: type_label.map(Into::into),
        title: title.into(),
        preview: preview.map(Into::into),
        avatar_label: "▣".into(),
        avatar_fill: 0x1d9bd1,
        avatar_image_url: None,
        height: 96.0,
        content: SlackLaterRowContent::File {
            attachment: Box::new(file.attachment),
        },
    }
}

pub(crate) fn mark_slack_later_hydration_error(row: &mut SlackLaterRow, error: String) {
    let target = match &row.content {
        SlackLaterRowContent::Placeholder { target }
        | SlackLaterRowContent::HydrationError { target } => target.clone(),
        SlackLaterRowContent::Message { .. }
        | SlackLaterRowContent::File { .. }
        | SlackLaterRowContent::Reminder { .. }
        | SlackLaterRowContent::Tombstone
        | SlackLaterRowContent::Unsupported => return,
    };
    let title = "Couldn’t load this saved item.";
    row.title = title.into();
    row.preview = Some(error.clone().into());
    row.accessibility_label = later_accessibility_label(title, Some(&error)).into();
    row.content = SlackLaterRowContent::HydrationError { target };
}

fn slack_later_reference_row(item: &SlackLaterItem) -> SlackLaterRow {
    match &item.content {
        SlackLaterReferenceContent::Message(_) => slack_later_placeholder_row(
            item,
            SlackLaterPlaceholderStyle {
                type_label: "Message",
                title: "Loading saved message…",
                avatar_label: "M",
                avatar_fill: 0x2e3136,
                height: 96.0,
            },
        ),
        SlackLaterReferenceContent::File(_) => slack_later_placeholder_row(
            item,
            SlackLaterPlaceholderStyle {
                type_label: "File",
                title: "Loading saved file…",
                avatar_label: "▣",
                avatar_fill: 0x1d9bd1,
                height: 96.0,
            },
        ),
        SlackLaterReferenceContent::Reminder(reminder) => slack_later_reminder_row(item, reminder),
        SlackLaterReferenceContent::Tombstone { kind } => slack_later_tombstone_row(item, *kind),
        SlackLaterReferenceContent::Unsupported { kind, .. } => {
            slack_later_unsupported_row(item, kind)
        }
    }
}

fn slack_later_tombstone_row(
    item: &SlackLaterItem,
    kind: SlackLaterTombstoneKind,
) -> SlackLaterRow {
    let title = slack_later_tombstone_label(kind);
    SlackLaterRow {
        element_id: later_element_id(&item.key),
        key: item.key.clone(),
        accessibility_label: title.into(),
        type_label: None,
        title: title.into(),
        preview: None,
        avatar_label: "⌫".into(),
        avatar_fill: 0x2e3136,
        avatar_image_url: None,
        height: 64.0,
        content: SlackLaterRowContent::Tombstone,
    }
}

fn slack_later_reminder_row(item: &SlackLaterItem, reminder: &SlackLaterReminder) -> SlackLaterRow {
    let due_label = slack_later_reminder_due_label(item.state, item.dates.due);
    let accessibility_label = later_accessibility_label(reminder.description(), Some(&due_label));
    SlackLaterRow {
        element_id: later_element_id(&item.key),
        key: item.key.clone(),
        accessibility_label: accessibility_label.into(),
        type_label: Some("Reminder".into()),
        title: reminder.description().to_string().into(),
        preview: Some(due_label.into()),
        avatar_label: "◷".into(),
        avatar_fill: 0x611f69,
        avatar_image_url: None,
        height: 88.0,
        content: SlackLaterRowContent::Reminder {
            detail: SlackLaterReminderDetail {
                reminder_id: reminder.reminder_id().clone(),
                description: reminder.description().to_string().into(),
                due_at: item.dates.due,
                state: item.state,
            },
        },
    }
}

fn slack_later_reminder_due_label(state: SlackLaterState, due_at: u64) -> String {
    match state {
        SlackLaterState::Completed => "Completed".to_string(),
        SlackLaterState::Archived => "Archived".to_string(),
        SlackLaterState::InProgress if due_at == 0 => "No due time".to_string(),
        SlackLaterState::InProgress => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after the Unix epoch")
                .as_secs();
            if due_at <= now {
                "Overdue".to_string()
            } else {
                let minutes = due_at.saturating_sub(now).div_ceil(60);
                if minutes < 60 {
                    format!("Due in {minutes} min")
                } else {
                    let hours = minutes.div_ceil(60);
                    if hours < 24 {
                        format!("Due in {hours} hr")
                    } else {
                        let days = hours.div_ceil(24);
                        format!("Due in {days} days")
                    }
                }
            }
        }
    }
}

fn slack_later_unsupported_row(item: &SlackLaterItem, kind: &str) -> SlackLaterRow {
    let title = format!("This saved {kind} type is not available yet.");
    SlackLaterRow {
        element_id: later_element_id(&item.key),
        key: item.key.clone(),
        accessibility_label: title.clone().into(),
        type_label: Some(kind.to_string().into()),
        title: title.into(),
        preview: None,
        avatar_label: "?".into(),
        avatar_fill: 0x2e3136,
        avatar_image_url: None,
        height: 72.0,
        content: SlackLaterRowContent::Unsupported,
    }
}

struct SlackLaterPlaceholderStyle {
    type_label: &'static str,
    title: &'static str,
    avatar_label: &'static str,
    avatar_fill: u32,
    height: f32,
}

fn slack_later_placeholder_row(
    item: &SlackLaterItem,
    style: SlackLaterPlaceholderStyle,
) -> SlackLaterRow {
    SlackLaterRow {
        element_id: later_element_id(&item.key),
        key: item.key.clone(),
        accessibility_label: style.title.into(),
        type_label: Some(style.type_label.into()),
        title: style.title.into(),
        preview: None,
        avatar_label: style.avatar_label.into(),
        avatar_fill: style.avatar_fill,
        avatar_image_url: None,
        height: style.height,
        content: SlackLaterRowContent::Placeholder {
            target: item
                .hydration_target()
                .expect("message and file Later references are hydratable"),
        },
    }
}

fn later_element_id(key: &SlackLaterItemKey) -> SharedString {
    format!("slack-later-row-{}", key.as_str()).into()
}

fn later_accessibility_label(title: &str, preview: Option<&str>) -> String {
    match preview {
        Some(preview) => format!("{title}: {preview}"),
        None => title.to_string(),
    }
}

fn slack_later_file_type_label(attachment: &SlackAttachment) -> Option<String> {
    attachment
        .mimetype
        .split_once('/')
        .map(|(_, subtype)| subtype)
        .filter(|subtype| !subtype.is_empty())
        .map(|subtype| subtype.to_ascii_uppercase())
}

fn slack_later_tombstone_label(kind: SlackLaterTombstoneKind) -> &'static str {
    match kind {
        SlackLaterTombstoneKind::Message => "A message you saved was deleted.",
        SlackLaterTombstoneKind::File => "A file you saved was deleted.",
        SlackLaterTombstoneKind::Reminder => "A reminder you saved was deleted.",
    }
}
