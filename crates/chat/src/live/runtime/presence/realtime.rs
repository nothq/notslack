use std::{collections::HashSet, sync::atomic::Ordering};

use crate::model::{
    SlackRealtimeBatch, SlackRealtimeConnectionState, SLACK_REALTIME_PRESENCE_USER_LIMIT,
};

use super::super::SlackWorkspaceRuntime;
use super::{is_valid_slack_presence_user_id, SlackPresenceState};

impl SlackWorkspaceRuntime {
    pub(crate) fn publish_presentation_batch(&self, mut batch: SlackRealtimeBatch) {
        self.apply_realtime_presence(&mut batch);
        if batch.connection_state == Some(SlackRealtimeConnectionState::Closed) {
            self.presentation_realtime_closed
                .store(true, Ordering::Release);
        }
        let _ = self.presentation_realtime.send(batch);
    }

    fn apply_realtime_presence(&self, batch: &mut SlackRealtimeBatch) {
        if batch.presence_changes.is_empty() && batch.presence_snapshot.is_none() {
            return;
        }
        let Ok(mut state) = self.presence_state.lock() else {
            eprintln!("Slack presence state mutex poisoned while applying realtime changes");
            discard_presence_payload(batch);
            return;
        };
        let Some(source_revision) = batch.presence_revision.take() else {
            eprintln!("discarded Slack presence payload without a source revision");
            discard_presence_payload(batch);
            return;
        };
        let snapshot_disposition =
            apply_presence_snapshot(&mut state, batch, source_revision, self.loader.team_id());
        if snapshot_disposition == SlackPresenceSnapshotDisposition::Rejected {
            return;
        }
        apply_presence_changes(&mut state, batch, source_revision);
        let accepted_snapshot = snapshot_disposition == SlackPresenceSnapshotDisposition::Accepted;
        if !accepted_snapshot && batch.presence_changes.is_empty() {
            return;
        }
        self.publish_presence_snapshot(&mut state, batch, accepted_snapshot);
    }

    fn publish_presence_snapshot(
        &self,
        state: &mut SlackPresenceState,
        batch: &mut SlackRealtimeBatch,
        accepted_snapshot: bool,
    ) {
        state.presentation_revision = state
            .presentation_revision
            .checked_add(1)
            .expect("Slack presentation presence revision overflowed");
        let presentation_snapshot = state.snapshot(self.loader.team_id());
        batch.presence_revision = Some(presentation_snapshot.revision);
        if accepted_snapshot {
            batch.presence_changes.clear();
            batch.presence_snapshot = Some(presentation_snapshot.clone());
        }
        self.presentation_presence_snapshot
            .send_replace(presentation_snapshot);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SlackPresenceSnapshotDisposition {
    Accepted,
    Ignored,
    Rejected,
}

fn apply_presence_snapshot(
    state: &mut SlackPresenceState,
    batch: &mut SlackRealtimeBatch,
    source_revision: u64,
    team_id: &str,
) -> SlackPresenceSnapshotDisposition {
    let Some(snapshot) = batch.presence_snapshot.take() else {
        return SlackPresenceSnapshotDisposition::Ignored;
    };
    if snapshot.team_id != team_id {
        eprintln!(
            "discarded Slack presence snapshot for team {} on runtime {team_id}",
            snapshot.team_id
        );
        batch.presence_changes.clear();
        return SlackPresenceSnapshotDisposition::Rejected;
    }
    if source_revision < snapshot.revision {
        eprintln!("discarded Slack presence snapshot newer than its batch revision");
        batch.presence_changes.clear();
        return SlackPresenceSnapshotDisposition::Rejected;
    }
    if snapshot.revision == 0 {
        assert!(
            snapshot.entries.is_empty(),
            "initial Slack presence snapshot must be empty"
        );
    }
    if state
        .accepted_source_revision
        .is_some_and(|accepted_revision| snapshot.revision <= accepted_revision)
    {
        return SlackPresenceSnapshotDisposition::Ignored;
    }
    let desired_ids = state.desired_ids().unwrap_or_default();
    let desired_ids = desired_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    state.latest_by_user_id = snapshot
        .entries
        .into_iter()
        .filter(|entry| {
            desired_ids.contains(entry.user_id.as_str())
                && is_valid_slack_presence_user_id(&entry.user_id)
        })
        .take(SLACK_REALTIME_PRESENCE_USER_LIMIT)
        .map(|entry| (entry.user_id, entry.presence))
        .collect();
    state.accepted_source_revision = Some(snapshot.revision);
    SlackPresenceSnapshotDisposition::Accepted
}

fn apply_presence_changes(
    state: &mut SlackPresenceState,
    batch: &mut SlackRealtimeBatch,
    source_revision: u64,
) {
    if state
        .accepted_source_revision
        .is_some_and(|accepted_revision| source_revision <= accepted_revision)
    {
        batch.presence_changes.clear();
        return;
    }
    let Some(desired_ids) = state.desired_ids() else {
        state.accepted_source_revision = Some(source_revision);
        batch.presence_changes.clear();
        return;
    };
    let desired_ids = desired_ids
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    batch.presence_changes.retain(|change| {
        if !desired_ids.contains(change.user_id.as_str())
            || !is_valid_slack_presence_user_id(&change.user_id)
        {
            return false;
        }
        if state.latest_by_user_id.get(&change.user_id) == Some(&change.presence) {
            return false;
        }
        state
            .latest_by_user_id
            .insert(change.user_id.clone(), change.presence);
        true
    });
    state.accepted_source_revision = Some(source_revision);
}

fn discard_presence_payload(batch: &mut SlackRealtimeBatch) {
    batch.presence_changes.clear();
    batch.presence_revision = None;
    batch.presence_snapshot = None;
}
