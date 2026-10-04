use std::{collections::HashSet, sync::Arc};

use chrono::{DateTime, Days, NaiveDate, Utc};

use super::{
    PreparedSlackDraftsSentSnapshot, SlackDraftsSentFilePresentation, SlackDraftsSentItemRow,
    SlackDraftsSentRow, SlackDraftsSentRowTarget,
};
use crate::ui::surface::SlackComposerDocument;
use crate::ui::{
    initials, slack_avatar_fill, SlackDraftDestination, SlackDraftsSentItem,
    SlackDraftsSentSnapshot, SlackDraftsSentTab,
};

mod target;

use target::{drafts_sent_row_target, SlackDraftsSentRowTargetInput};

type LocalSlackDraftsSentItem<'a> = (&'a SlackDraftsSentItem, DateTime<chrono_tz::Tz>);

struct SlackDraftsSentRowInput<'a> {
    items: &'a [LocalSlackDraftsSentItem<'a>],
    index: usize,
    tab: SlackDraftsSentTab,
    today: NaiveDate,
    authenticated_self_user_id: &'a str,
}

struct SlackDraftsSentItemRowContext<'a> {
    tab: SlackDraftsSentTab,
    timestamp: DateTime<chrono_tz::Tz>,
    sent_group_first: bool,
    sent_group_last: bool,
    authenticated_self_user_id: &'a str,
}

struct SlackDraftsSentItemPresentation<'a> {
    item: &'a SlackDraftsSentItem,
    destination: &'a SlackDraftDestination,
    destination_label: String,
    body: String,
    authenticated_remote_files: Arc<[crate::model::SlackRemoteDraftFileReference]>,
    target: SlackDraftsSentRowTarget,
    context: SlackDraftsSentItemRowContext<'a>,
}

pub(crate) fn prepare_slack_drafts_sent_snapshot(
    snapshot: SlackDraftsSentSnapshot,
    timezone: chrono_tz::Tz,
    authenticated_self_user_id: &str,
) -> Result<PreparedSlackDraftsSentSnapshot, String> {
    validate_slack_drafts_sent_items(&snapshot, authenticated_self_user_id)?;
    let now = Utc::now().with_timezone(&timezone);
    let items = local_slack_drafts_sent_items(&snapshot, timezone)?;
    let mut rows = Vec::with_capacity(snapshot.items.len().saturating_mul(2));
    for index in 0..items.len() {
        append_slack_drafts_sent_item_rows(
            &mut rows,
            SlackDraftsSentRowInput {
                items: &items,
                index,
                tab: snapshot.tab,
                today: now.date_naive(),
                authenticated_self_user_id,
            },
        )?;
    }
    Ok(PreparedSlackDraftsSentSnapshot {
        snapshot,
        authenticated_self_user_id: authenticated_self_user_id.to_string(),
        rows: rows.into(),
    })
}

fn validate_slack_drafts_sent_items(
    snapshot: &SlackDraftsSentSnapshot,
    authenticated_self_user_id: &str,
) -> Result<(), String> {
    let mut seen_ids = HashSet::with_capacity(snapshot.items.len());
    for item in &snapshot.items {
        if item.team_id != snapshot.team_id {
            return Err(format!(
                "Slack Drafts & sent item {} belongs to workspace {}, not {}.",
                item.id, item.team_id, snapshot.team_id
            ));
        }
        if item.user_id != authenticated_self_user_id {
            return Err(format!(
                "Slack Drafts & sent item {} belongs to a different authenticated user.",
                item.id
            ));
        }
        if !seen_ids.insert(item.id.as_str()) {
            return Err(format!(
                "Slack Drafts & sent returned duplicate draft id {}.",
                item.id
            ));
        }
    }
    Ok(())
}

fn local_slack_drafts_sent_items(
    snapshot: &SlackDraftsSentSnapshot,
    timezone: chrono_tz::Tz,
) -> Result<Vec<LocalSlackDraftsSentItem<'_>>, String> {
    snapshot
        .items
        .iter()
        .map(|item| {
            item_timestamp(item, snapshot.tab)
                .map(|timestamp| (item, timestamp.with_timezone(&timezone)))
        })
        .collect()
}

fn append_slack_drafts_sent_item_rows(
    rows: &mut Vec<SlackDraftsSentRow>,
    input: SlackDraftsSentRowInput<'_>,
) -> Result<(), String> {
    let SlackDraftsSentRowInput {
        items,
        index,
        tab,
        today,
        authenticated_self_user_id,
    } = input;
    let (item, timestamp) = items
        .get(index)
        .expect("Slack Drafts & sent row index must exist");
    let timestamp = *timestamp;
    let date = timestamp.date_naive();
    let previous_date = index
        .checked_sub(1)
        .and_then(|previous| items.get(previous))
        .map(|(_, timestamp)| timestamp.date_naive());
    let next_date = items
        .get(index + 1)
        .map(|(_, timestamp)| timestamp.date_naive());
    let sent_group_first = tab == SlackDraftsSentTab::Sent && previous_date != Some(date);
    let sent_group_last = tab == SlackDraftsSentTab::Sent && next_date != Some(date);
    if sent_group_first {
        rows.push(SlackDraftsSentRow::DateDivider {
            element_id: format!("slack-drafts-sent-date-{}", timestamp.format("%Y-%m-%d")).into(),
            label: date_divider_label(date, today).into(),
        });
    }
    rows.push(SlackDraftsSentRow::Item(Arc::new(prepare_item_row(
        item,
        SlackDraftsSentItemRowContext {
            tab,
            timestamp,
            sent_group_first,
            sent_group_last,
            authenticated_self_user_id,
        },
    )?)));
    Ok(())
}

fn prepare_item_row(
    item: &SlackDraftsSentItem,
    context: SlackDraftsSentItemRowContext<'_>,
) -> Result<SlackDraftsSentItemRow, String> {
    let destination = item.primary_destination().ok_or_else(|| {
        format!(
            "Slack {} item {} omitted its destination",
            context.tab.label(),
            item.id
        )
    })?;
    let destination_label = slack_drafts_sent_destination_label(destination);
    let body = slack_drafts_sent_body(item);
    let document = slack_drafts_sent_document(item)?;
    let authenticated_remote_files = slack_drafts_sent_authenticated_remote_files(
        item,
        context.tab,
        context.authenticated_self_user_id,
    )?;
    let target = drafts_sent_row_target(SlackDraftsSentRowTargetInput {
        item,
        tab: context.tab,
        destination,
        document,
        authenticated_self_user_id: context.authenticated_self_user_id,
        authenticated_remote_files: authenticated_remote_files.clone(),
    })?;
    Ok(slack_drafts_sent_item_row(
        SlackDraftsSentItemPresentation {
            item,
            destination,
            destination_label,
            body,
            authenticated_remote_files,
            target,
            context,
        },
    ))
}

fn slack_drafts_sent_destination_label(destination: &SlackDraftDestination) -> String {
    if destination.user_ids.is_empty()
        && !destination.label.starts_with('#')
        && destination.label != destination.conversation_id
    {
        format!("# {}", destination.label)
    } else {
        destination.label.clone()
    }
}

fn slack_drafts_sent_body(item: &SlackDraftsSentItem) -> String {
    let body = item.body.trim().replace('\n', " ");
    if body.is_empty() {
        "No message".to_string()
    } else {
        body
    }
}

fn slack_drafts_sent_document(item: &SlackDraftsSentItem) -> Result<SlackComposerDocument, String> {
    item.rich_body
        .as_ref()
        .map(SlackComposerDocument::from_rich_body)
        .transpose()
        .map(|document| {
            document.unwrap_or_else(|| SlackComposerDocument::plain_text(item.body.clone()))
        })
}

fn slack_drafts_sent_authenticated_remote_files(
    item: &SlackDraftsSentItem,
    tab: SlackDraftsSentTab,
    authenticated_self_user_id: &str,
) -> Result<Arc<[crate::model::SlackRemoteDraftFileReference]>, String> {
    match tab {
        SlackDraftsSentTab::Drafts | SlackDraftsSentTab::Scheduled => {
            let target =
                crate::model::SlackDraftTarget::new(item.id.clone(), item.revision.clone());
            crate::model::SlackAuthenticatedRemoteDraftFiles::from_loaded_draft(
                &item.team_id,
                authenticated_self_user_id,
                &target,
                item,
            )
            .map(crate::model::SlackAuthenticatedRemoteDraftFiles::into_references)
        }
        SlackDraftsSentTab::Sent => Ok(Arc::default()),
    }
}

fn slack_drafts_sent_item_row(
    input: SlackDraftsSentItemPresentation<'_>,
) -> SlackDraftsSentItemRow {
    let SlackDraftsSentItemPresentation {
        item,
        destination,
        destination_label,
        body,
        authenticated_remote_files,
        target,
        context,
    } = input;
    SlackDraftsSentItemRow {
        id: item.id.as_str().to_string().into(),
        element_id: format!("slack-drafts-sent-item-{}", item.id).into(),
        accessibility_label: format!(
            "{} in {} at {}: {}",
            context.tab.label(),
            destination_label,
            context.timestamp.format("%-I:%M %p"),
            body
        )
        .into(),
        destination: destination_label.clone().into(),
        body: body.into(),
        timestamp: context.timestamp.format("%-I:%M %p").to_string().into(),
        avatar_label: initials(&destination_label).into(),
        avatar_fill: slack_avatar_fill(&destination_label),
        avatar_image_url: destination.avatar_image_url.clone().map(Into::into),
        files: SlackDraftsSentFilePresentation::from_authenticated_references(
            authenticated_remote_files,
        ),
        target,
        sent_date_key: (context.tab == SlackDraftsSentTab::Sent)
            .then(|| context.timestamp.format("%Y-%m-%d").to_string().into()),
        sent_group_first: context.sent_group_first,
        sent_group_last: context.sent_group_last,
    }
}

fn item_timestamp(
    item: &SlackDraftsSentItem,
    tab: SlackDraftsSentTab,
) -> Result<DateTime<Utc>, String> {
    let unix_seconds = match tab {
        SlackDraftsSentTab::Drafts => item
            .revision
            .as_str()
            .split_once('.')
            .map_or(item.revision.as_str(), |(seconds, _)| seconds)
            .parse::<i64>()
            .map_err(|_| {
                format!(
                    "Slack draft {} returned invalid last_updated_ts {}",
                    item.id, item.revision
                )
            })?,
        SlackDraftsSentTab::Scheduled => i64::try_from(item.scheduled_unix_seconds)
            .map_err(|_| format!("Slack scheduled item {} timestamp overflowed", item.id))?,
        SlackDraftsSentTab::Sent => item
            .primary_destination()
            .and_then(|destination| destination.message_timestamp.as_deref())
            .and_then(|timestamp| timestamp.split_once('.').map(|(seconds, _)| seconds))
            .and_then(|seconds| seconds.parse::<i64>().ok())
            .unwrap_or(
                i64::try_from(item.created_unix_seconds)
                    .map_err(|_| format!("Slack sent item {} timestamp overflowed", item.id))?,
            ),
    };
    DateTime::from_timestamp(unix_seconds, 0).ok_or_else(|| {
        format!(
            "Slack {} item {} timestamp is invalid",
            tab.label(),
            item.id
        )
    })
}

fn date_divider_label(date: NaiveDate, today: NaiveDate) -> String {
    if date == today {
        "Today".to_string()
    } else if today.checked_sub_days(Days::new(1)) == Some(date) {
        "Yesterday".to_string()
    } else {
        date.format("%A, %B %-d").to_string()
    }
}
