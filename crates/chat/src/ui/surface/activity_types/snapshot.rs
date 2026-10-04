use std::collections::{HashMap, HashSet};

use crate::model::{SlackActivitySnapshot, SlackWorkspace};
use time::OffsetDateTime;

use super::row::{
    slack_activity_date_label, slack_activity_row, slack_activity_timestamp, slack_local_offset,
    SlackActivityChannel,
};
use super::{
    PreparedSlackActivitySnapshot, SlackActivityItemMutation, SlackActivityItemMutationRollback,
    SlackActivityRow, SlackActivityWorkspaceContext,
};

pub(crate) fn merge_and_prepare_slack_activity_snapshot(
    existing: Option<SlackActivitySnapshot>,
    page: SlackActivitySnapshot,
    workspace: &SlackActivityWorkspaceContext,
) -> PreparedSlackActivitySnapshot {
    let snapshot = merge_slack_activity_snapshot(existing, page);
    prepare_slack_activity_snapshot(snapshot, workspace)
}

pub(crate) fn reconcile_and_prepare_slack_activity_snapshot(
    mut existing: SlackActivitySnapshot,
    refreshed_page: SlackActivitySnapshot,
    mutations: &[SlackActivityItemMutation],
    workspace: &SlackActivityWorkspaceContext,
) -> PreparedSlackActivitySnapshot {
    let SlackActivitySnapshot {
        activity_views_date_updated,
        items,
        next_cursor: _,
    } = refreshed_page;
    let mut refreshed_items = items
        .into_iter()
        .map(|item| (item.key.clone(), item))
        .collect::<HashMap<_, _>>();
    for item in &mut existing.items {
        if let Some(refreshed) = refreshed_items.remove(&item.key) {
            *item = refreshed;
        }
    }
    existing.activity_views_date_updated = activity_views_date_updated;
    for mutation in mutations {
        apply_slack_activity_item_mutation(&mut existing, mutation);
    }
    prepare_slack_activity_snapshot(existing, workspace)
}

pub(crate) fn mutate_and_prepare_slack_activity_snapshot(
    mut existing: SlackActivitySnapshot,
    mutation: &SlackActivityItemMutation,
    workspace: &SlackActivityWorkspaceContext,
) -> PreparedSlackActivitySnapshot {
    apply_slack_activity_item_mutation(&mut existing, mutation);
    prepare_slack_activity_snapshot(existing, workspace)
}

pub(crate) fn rollback_and_prepare_slack_activity_snapshot(
    mut existing: SlackActivitySnapshot,
    key: &str,
    rollback: SlackActivityItemMutationRollback,
    workspace: &SlackActivityWorkspaceContext,
) -> PreparedSlackActivitySnapshot {
    let item = existing
        .items
        .iter_mut()
        .find(|item| item.key == key)
        .expect("pending Slack Activity mutation must retain its source item");
    item.unread = rollback.unread;
    item.archived = rollback.archived;
    prepare_slack_activity_snapshot(existing, workspace)
}

fn apply_slack_activity_item_mutation(
    snapshot: &mut SlackActivitySnapshot,
    mutation: &SlackActivityItemMutation,
) {
    let item = snapshot
        .items
        .iter_mut()
        .find(|item| item.key == mutation.key())
        .expect("pending Slack Activity mutation must retain its source item");
    if let Some(unread) = mutation.unread() {
        item.unread = unread;
    }
    if let Some(archived) = mutation.archived() {
        item.archived = archived;
    }
}

fn prepare_slack_activity_snapshot(
    snapshot: SlackActivitySnapshot,
    workspace: &SlackActivityWorkspaceContext,
) -> PreparedSlackActivitySnapshot {
    PreparedSlackActivitySnapshot {
        rows: build_slack_activity_rows(&snapshot, workspace).into(),
        snapshot,
    }
}

pub(crate) fn prepare_slack_activity_workspace_context(
    workspace: &SlackWorkspace,
) -> SlackActivityWorkspaceContext {
    let mut channels = workspace
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
        .filter(|item| !item.target_id.is_empty())
        .map(|item| SlackActivityChannel {
            id: item.target_id.clone(),
            label: item.label.clone(),
            kind: item.target_kind,
        })
        .collect::<Vec<_>>();
    if !workspace.conversation_id.is_empty()
        && !channels
            .iter()
            .any(|channel| channel.id == workspace.conversation_id)
    {
        channels.push(SlackActivityChannel {
            id: workspace.conversation_id.clone(),
            label: workspace.channel_name.clone(),
            kind: workspace.channel_kind,
        });
    }
    SlackActivityWorkspaceContext {
        channels: channels.into(),
    }
}

fn merge_slack_activity_snapshot(
    existing: Option<SlackActivitySnapshot>,
    page: SlackActivitySnapshot,
) -> SlackActivitySnapshot {
    let Some(mut existing) = existing else {
        return page;
    };
    let mut keys = existing
        .items
        .iter()
        .map(|item| item.key.clone())
        .collect::<HashSet<_>>();
    existing.items.extend(
        page.items
            .into_iter()
            .filter(|item| keys.insert(item.key.clone())),
    );
    existing.activity_views_date_updated = page.activity_views_date_updated;
    existing.next_cursor = page.next_cursor;
    existing
}

fn build_slack_activity_rows(
    snapshot: &SlackActivitySnapshot,
    workspace: &SlackActivityWorkspaceContext,
) -> Vec<SlackActivityRow> {
    let now = OffsetDateTime::now_utc();
    let local_offset = slack_local_offset();
    let today = now.to_offset(local_offset).date();
    let mut previous_date = None;
    snapshot
        .items
        .iter()
        .filter(|item| !item.archived)
        .filter_map(|item| {
            let mut row = slack_activity_row(item, workspace, now, local_offset)?;
            let date = slack_activity_timestamp(&item.feed_timestamp)
                .map(|timestamp| timestamp.to_offset(local_offset).date());
            if date != previous_date {
                row.divider_label = date
                    .filter(|date| *date != today)
                    .map(|date| slack_activity_date_label(date, today).into());
                if row.divider_label.is_some() {
                    row.card_height += 52.0;
                }
                previous_date = date;
            }
            Some(row)
        })
        .collect()
}
