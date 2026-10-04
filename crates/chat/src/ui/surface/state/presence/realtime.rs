use std::collections::HashMap;

use crate::model::{
    SlackRealtimePresenceChange, SlackRealtimePresenceSnapshot, SLACK_REALTIME_PRESENCE_USER_LIMIT,
};

use super::{
    valid_presence_user_id, SlackPresenceAuthority, SlackPresencePatch,
    SLACK_PRESENCE_AUTHORITY_LIMIT,
};

impl SlackPresenceAuthority {
    pub(in crate::ui::surface) fn apply_realtime(
        &mut self,
        expected_team_id: &str,
        presence_revision: Option<u64>,
        snapshot: Option<&SlackRealtimePresenceSnapshot>,
        changes: &[SlackRealtimePresenceChange],
    ) -> SlackPresencePatch {
        if snapshot.is_none() && changes.is_empty() {
            return SlackPresencePatch::None;
        }
        if self.team_id.as_deref() != Some(expected_team_id) {
            return SlackPresencePatch::None;
        }
        let revision = presence_revision.expect("Slack presence payload must carry a revision");
        let full = match snapshot {
            Some(snapshot) if snapshot.team_id != expected_team_id => {
                return SlackPresencePatch::None;
            }
            Some(snapshot) => self.apply_snapshot(revision, snapshot),
            None => false,
        };
        let changed = self.apply_changes(revision, changes);
        match (full, changed) {
            (true, _) => SlackPresencePatch::Full,
            (false, true) => SlackPresencePatch::Delta,
            (false, false) => SlackPresencePatch::None,
        }
    }

    fn apply_snapshot(&mut self, revision: u64, snapshot: &SlackRealtimePresenceSnapshot) -> bool {
        assert!(
            revision >= snapshot.revision,
            "Slack presence batch revision must cover its snapshot"
        );
        if snapshot.revision == 0 {
            assert!(
                snapshot.entries.is_empty(),
                "initial Slack presentation presence snapshot must be empty"
            );
        }
        if self
            .accepted_revision
            .is_some_and(|accepted| snapshot.revision <= accepted)
        {
            return false;
        }
        self.replace_from_snapshot(snapshot);
        self.accepted_revision = Some(snapshot.revision);
        true
    }

    fn apply_changes(&mut self, revision: u64, changes: &[SlackRealtimePresenceChange]) -> bool {
        if changes.is_empty()
            || self
                .accepted_revision
                .is_some_and(|accepted| revision <= accepted)
        {
            return false;
        }
        assert!(
            changes.len() <= SLACK_REALTIME_PRESENCE_USER_LIMIT,
            "Slack presence delta exceeded the bounded roster"
        );
        let mut changed = false;
        for change in changes {
            assert!(
                valid_presence_user_id(&change.user_id),
                "Slack presence delta contained an invalid user id"
            );
            self.tombstone_user_ids.remove(&change.user_id);
            changed |= self
                .latest_by_user_id
                .insert(change.user_id.clone(), Some(change.presence))
                != Some(Some(change.presence));
        }
        self.accepted_revision = Some(revision);
        self.compact_tombstones();
        changed
    }

    fn replace_from_snapshot(&mut self, snapshot: &SlackRealtimePresenceSnapshot) {
        assert!(
            snapshot.entries.len() <= SLACK_REALTIME_PRESENCE_USER_LIMIT,
            "Slack presence snapshot exceeded the bounded roster"
        );
        let mut incoming = HashMap::with_capacity(snapshot.entries.len());
        for entry in &snapshot.entries {
            assert!(
                valid_presence_user_id(&entry.user_id),
                "Slack presence snapshot contained an invalid user id"
            );
            assert!(
                incoming
                    .insert(entry.user_id.clone(), Some(entry.presence))
                    .is_none(),
                "Slack presence snapshot repeated a user id"
            );
        }
        for (user_id, presence) in &mut self.latest_by_user_id {
            if presence.is_some() && !incoming.contains_key(user_id) {
                *presence = None;
                self.tombstone_user_ids.insert(user_id.clone());
            }
        }
        for (user_id, presence) in incoming {
            self.tombstone_user_ids.remove(&user_id);
            self.latest_by_user_id.insert(user_id, presence);
        }
        self.compact_tombstones();
    }

    fn compact_tombstones(&mut self) {
        while self.latest_by_user_id.len() > SLACK_PRESENCE_AUTHORITY_LIMIT {
            let Some(user_id) = self.tombstone_user_ids.iter().next().cloned() else {
                break;
            };
            self.tombstone_user_ids.remove(&user_id);
            self.latest_by_user_id.remove(&user_id);
        }
    }
}
