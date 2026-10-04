use std::collections::{HashMap, HashSet};

use tokio::sync::broadcast;

use crate::model::{
    SlackRealtimeBatch, SlackRealtimeConnectionState, SlackRealtimeNotification,
    SlackRealtimePresenceChange, SlackRealtimePresenceSnapshot, SlackRealtimeThreadTarget,
    SlackUserPresence,
};

#[derive(Default)]
pub(super) struct SlackRealtimeAccumulator {
    conversation_ids: HashSet<String>,
    threads: HashSet<SlackRealtimeThreadTarget>,
    presence_changes: HashMap<String, SlackUserPresence>,
    presence_revision: Option<u64>,
    presence_snapshot: Option<SlackRealtimePresenceSnapshot>,
    notification_sources: HashSet<crate::model::SlackRealtimeNotificationSource>,
    notifications: Vec<SlackRealtimeNotification>,
    sidebar_changed: bool,
    activity_changed: bool,
    later_changed: bool,
    files_changed: bool,
    all_threads_changed: bool,
    full_resync: bool,
    connection_state: Option<SlackRealtimeConnectionState>,
}

impl SlackRealtimeAccumulator {
    pub(super) fn merge(&mut self, batch: SlackRealtimeBatch) {
        self.conversation_ids.extend(batch.conversation_ids);
        self.threads.extend(batch.threads);
        if let Some(snapshot) = batch.presence_snapshot {
            let accepts_snapshot = self
                .presence_revision
                .is_none_or(|current_revision| snapshot.revision >= current_revision);
            if accepts_snapshot {
                self.presence_changes.clear();
                self.presence_revision = Some(snapshot.revision);
                self.presence_snapshot = Some(snapshot);
            }
        }
        if !batch.presence_changes.is_empty()
            && batch.presence_revision.is_some_and(|revision| {
                self.presence_revision
                    .is_none_or(|current_revision| revision > current_revision)
            })
        {
            self.presence_changes.extend(
                batch
                    .presence_changes
                    .into_iter()
                    .map(|change| (change.user_id, change.presence)),
            );
            self.presence_revision = batch.presence_revision;
        }
        for notification in batch.notifications {
            if self
                .notification_sources
                .insert(notification.source.clone())
            {
                self.notifications.push(notification);
            }
        }
        self.sidebar_changed |= batch.sidebar_changed;
        self.activity_changed |= batch.activity_changed;
        self.later_changed |= batch.later_changed;
        self.files_changed |= batch.files_changed;
        self.all_threads_changed |= batch.all_threads_changed;
        self.full_resync |= batch.full_resync;
        if batch.connection_state.is_some() {
            self.connection_state = batch.connection_state;
        }
    }

    fn take(&mut self) -> Option<SlackRealtimeBatch> {
        if self.is_empty() {
            return None;
        }
        let mut conversation_ids = self.conversation_ids.drain().collect::<Vec<_>>();
        conversation_ids.sort();
        let mut threads = self.threads.drain().collect::<Vec<_>>();
        threads.sort_by(|left, right| {
            left.conversation_id
                .cmp(&right.conversation_id)
                .then_with(|| {
                    left.thread_timestamp
                        .sort_key()
                        .cmp(&right.thread_timestamp.sort_key())
                })
        });
        self.notification_sources.clear();
        let notifications = std::mem::take(&mut self.notifications);
        let mut presence_changes = self
            .presence_changes
            .drain()
            .map(|(user_id, presence)| SlackRealtimePresenceChange { user_id, presence })
            .collect::<Vec<_>>();
        presence_changes.sort_by(|left, right| left.user_id.cmp(&right.user_id));
        let batch = SlackRealtimeBatch {
            conversation_ids,
            threads,
            presence_changes,
            presence_revision: self.presence_revision.take(),
            presence_snapshot: self.presence_snapshot.take(),
            notifications,
            sidebar_changed: self.sidebar_changed,
            activity_changed: self.activity_changed,
            later_changed: self.later_changed,
            files_changed: self.files_changed,
            all_threads_changed: self.all_threads_changed,
            full_resync: self.full_resync,
            connection_state: self.connection_state.take(),
        };
        self.sidebar_changed = false;
        self.activity_changed = false;
        self.later_changed = false;
        self.files_changed = false;
        self.all_threads_changed = false;
        self.full_resync = false;
        Some(batch)
    }

    fn is_empty(&self) -> bool {
        self.conversation_ids.is_empty()
            && self.threads.is_empty()
            && self.presence_changes.is_empty()
            && self.presence_snapshot.is_none()
            && self.notifications.is_empty()
            && !self.sidebar_changed
            && !self.activity_changed
            && !self.later_changed
            && !self.files_changed
            && !self.all_threads_changed
            && !self.full_resync
            && self.connection_state.is_none()
    }
}

pub(super) fn flush_accumulator(
    accumulator: &mut SlackRealtimeAccumulator,
    sender: &broadcast::Sender<SlackRealtimeBatch>,
) {
    if let Some(batch) = accumulator.take() {
        let _ = sender.send(batch);
    }
}
