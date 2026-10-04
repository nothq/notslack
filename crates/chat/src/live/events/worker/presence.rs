use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use futures_util::SinkExt;
use serde::Serialize;
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;

use crate::model::{
    SlackRealtimeBatch, SlackRealtimePresenceChange, SlackRealtimePresenceSnapshot,
    SlackUserPresence, SLACK_REALTIME_PRESENCE_PAYLOAD_MAX_BYTES,
    SLACK_REALTIME_PRESENCE_USER_ID_MAX_BYTES, SLACK_REALTIME_PRESENCE_USER_LIMIT,
};

use super::{super::SlackPresenceSubscriptionIds, SlackRealtimeSocketWriter};

pub(super) struct SlackSocketPresence<'a> {
    pub(super) subscription_ids: &'a mut watch::Receiver<SlackPresenceSubscriptionIds>,
    pub(super) snapshot: &'a watch::Sender<SlackRealtimePresenceSnapshot>,
    pub(super) commit: &'a Mutex<()>,
}

#[derive(Serialize)]
struct SlackPing {
    id: u64,
    #[serde(rename = "type")]
    kind: &'static str,
}

#[derive(Serialize)]
struct SlackPresenceSubscription<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    id: u64,
    ids: &'a [String],
}

fn take_slack_outbound_id(next: &mut u64) -> Result<u64, ()> {
    let id = *next;
    *next = next.checked_add(1).ok_or(())?;
    Ok(id)
}

fn slack_ping_payload(id: u64) -> Result<String, ()> {
    serde_json::to_string(&SlackPing { id, kind: "ping" }).map_err(|_| ())
}

pub(super) async fn send_slack_ping(
    write: &mut SlackRealtimeSocketWriter,
    outbound_id: &mut u64,
) -> Result<(), ()> {
    let id = take_slack_outbound_id(outbound_id)?;
    let ping = slack_ping_payload(id)?;
    write.send(Message::Text(ping)).await.map_err(|_| ())
}

fn slack_presence_subscription_payload(id: u64, ids: &[String]) -> Result<String, ()> {
    if ids.len() > SLACK_REALTIME_PRESENCE_USER_LIMIT
        || ids.iter().any(|user_id| {
            user_id.is_empty() || user_id.len() > SLACK_REALTIME_PRESENCE_USER_ID_MAX_BYTES
        })
    {
        return Err(());
    }
    let payload = serde_json::to_string(&SlackPresenceSubscription {
        kind: "presence_sub",
        id,
        ids,
    })
    .map_err(|_| ())?;
    (payload.len() <= SLACK_REALTIME_PRESENCE_PAYLOAD_MAX_BYTES)
        .then_some(payload)
        .ok_or(())
}

pub(super) async fn send_slack_presence_subscription(
    write: &mut SlackRealtimeSocketWriter,
    outbound_id: &mut u64,
    ids: &[String],
) -> Result<(), ()> {
    let id = take_slack_outbound_id(outbound_id)?;
    let payload = slack_presence_subscription_payload(id, ids)?;
    write.send(Message::Text(payload)).await.map_err(|_| ())
}

pub(super) fn refresh_presence_roster(
    commit: &Mutex<()>,
    receiver: &mut watch::Receiver<SlackPresenceSubscriptionIds>,
    snapshot: &watch::Sender<SlackRealtimePresenceSnapshot>,
) -> (
    SlackPresenceSubscriptionIds,
    Option<SlackRealtimePresenceSnapshot>,
) {
    let _commit = commit
        .lock()
        .expect("Slack realtime presence commit mutex poisoned while refreshing the roster");
    let desired_ids = receiver.borrow_and_update().clone();
    let pruned_snapshot =
        retain_presence_roster(snapshot, desired_ids.as_deref().unwrap_or_default());
    (desired_ids, pruned_snapshot)
}

pub(super) fn record_desired_presence_changes(
    commit: &Mutex<()>,
    receiver: &mut watch::Receiver<SlackPresenceSubscriptionIds>,
    snapshot: &watch::Sender<SlackRealtimePresenceSnapshot>,
    mut batch: SlackRealtimeBatch,
) -> (SlackPresenceSubscriptionIds, SlackRealtimeBatch) {
    let _commit = commit
        .lock()
        .expect("Slack realtime presence commit mutex poisoned while recording an event");
    let desired_ids = receiver.borrow().clone();
    filter_slack_presence_changes(&mut batch.presence_changes, desired_ids.as_deref());
    record_presence_changes(snapshot, &mut batch);
    (desired_ids, batch)
}

fn filter_slack_presence_changes(
    changes: &mut Vec<SlackRealtimePresenceChange>,
    desired_ids: Option<&[String]>,
) {
    let Some(desired_ids) = desired_ids else {
        changes.clear();
        return;
    };
    let desired_ids = desired_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    changes.retain(|change| desired_ids.contains(change.user_id.as_str()));
}

fn record_presence_changes(
    sender: &watch::Sender<SlackRealtimePresenceSnapshot>,
    batch: &mut SlackRealtimeBatch,
) {
    if batch.presence_changes.is_empty() {
        return;
    }
    let mut committed_revision = None;
    sender.send_if_modified(|latest| {
        let mut entries = std::mem::take(&mut latest.entries)
            .into_iter()
            .map(|entry| (entry.user_id, entry.presence))
            .collect::<HashMap<_, _>>();
        let mut changed = false;
        for change in &batch.presence_changes {
            changed |=
                entries.insert(change.user_id.clone(), change.presence) != Some(change.presence);
        }
        latest.entries = sorted_presence_entries(entries);
        if !changed {
            return false;
        }
        latest.revision = latest
            .revision
            .checked_add(1)
            .expect("Slack raw presence revision overflowed");
        committed_revision = Some(latest.revision);
        true
    });
    let Some(revision) = committed_revision else {
        batch.presence_changes.clear();
        return;
    };
    batch.presence_revision = Some(revision);
}

fn retain_presence_roster(
    sender: &watch::Sender<SlackRealtimePresenceSnapshot>,
    desired_ids: &[String],
) -> Option<SlackRealtimePresenceSnapshot> {
    let desired_ids = desired_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut committed = None;
    sender.send_if_modified(|latest| {
        let previous_len = latest.entries.len();
        latest
            .entries
            .retain(|entry| desired_ids.contains(entry.user_id.as_str()));
        if latest.entries.len() == previous_len {
            return false;
        }
        latest.revision = latest
            .revision
            .checked_add(1)
            .expect("Slack raw presence revision overflowed");
        committed = Some(latest.clone());
        true
    });
    committed
}

fn sorted_presence_entries(
    entries: HashMap<String, SlackUserPresence>,
) -> Vec<SlackRealtimePresenceChange> {
    let mut entries = entries
        .into_iter()
        .map(|(user_id, presence)| SlackRealtimePresenceChange { user_id, presence })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.user_id.cmp(&right.user_id));
    entries
}

pub(super) fn presence_snapshot_batch(
    snapshot: SlackRealtimePresenceSnapshot,
) -> SlackRealtimeBatch {
    SlackRealtimeBatch {
        presence_revision: Some(snapshot.revision),
        presence_snapshot: Some(snapshot),
        ..SlackRealtimeBatch::default()
    }
}
