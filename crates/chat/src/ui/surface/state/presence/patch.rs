use std::sync::Arc;

use crate::{
    model::{SlackRealtimePresenceChange, SlackSidebarSection, SlackUserPresence},
    ui::surface::{
        SlackAllThreadRow, SlackDmRow, SlackMemberRow, SlackNewMessageCandidateRow,
        SlackSidebarRow, SlackSidebarRowKind, SurfaceStateData,
    },
};

use super::{SlackPresenceAuthority, SlackPresencePatch};

impl SlackPresenceAuthority {
    pub(in crate::ui::surface) fn apply_self_presence(
        &mut self,
        data: &mut SurfaceStateData,
        presence: SlackUserPresence,
    ) -> bool {
        let Some(user_id) = self.self_user_id.clone() else {
            return false;
        };
        self.latest_by_user_id
            .insert(user_id.clone(), Some(presence));
        self.tombstone_user_ids.remove(&user_id);
        self.patch_user(data, &user_id)
    }

    pub(in crate::ui::surface) fn patch_loaded_views(
        &self,
        data: &mut SurfaceStateData,
        patch: SlackPresencePatch,
        changes: &[SlackRealtimePresenceChange],
    ) -> bool {
        let user_ids = match patch {
            SlackPresencePatch::None => return false,
            SlackPresencePatch::Delta => changes
                .iter()
                .map(|change| change.user_id.as_str())
                .collect::<Vec<_>>(),
            SlackPresencePatch::Full => self
                .latest_by_user_id
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
        };
        let mut changed = false;
        for user_id in user_ids {
            changed |= self.patch_user(data, user_id);
        }
        changed
    }

    fn patch_user(&self, data: &mut SurfaceStateData, user_id: &str) -> bool {
        let Some(presence) = self.latest_by_user_id.get(user_id).copied() else {
            return false;
        };
        let team_id = self
            .team_id
            .as_deref()
            .expect("Slack presence authority must have a team while patching");
        let mut changed = self.patch_sidebar_user(data, team_id, user_id, presence);
        changed |= self.patch_dm_user(data, team_id, user_id, presence);
        changed |= self.patch_all_threads_user(data, team_id, user_id, presence);
        changed |= self.patch_members_user(data, team_id, user_id, presence);
        changed |= self.patch_directory_user(data, team_id, user_id, presence);
        changed |= self.patch_destination_user(data, team_id, user_id, presence);
        changed
    }

    fn patch_sidebar_user(
        &self,
        data: &mut SurfaceStateData,
        team_id: &str,
        user_id: &str,
        presence: Option<SlackUserPresence>,
    ) -> bool {
        let mut changed = false;
        if self.self_user_id.as_deref() == Some(user_id) {
            if let Some(snapshot) = data
                .slack_sidebar_snapshot
                .as_mut()
                .filter(|snapshot| snapshot.team_id == team_id)
            {
                changed |= set_presence(&mut snapshot.rail_badges.self_presence, presence);
            }
        }
        if let Some(snapshot) = data
            .slack_sidebar_snapshot
            .as_mut()
            .filter(|snapshot| snapshot.team_id == team_id)
        {
            if let Some(positions) = self.indexes.sidebar_sections.get(user_id) {
                changed |= patch_sections(&mut snapshot.sections, positions, presence);
            }
            if let Some(positions) = self.indexes.sidebar_rows.get(user_id) {
                changed |= patch_sidebar_rows(&mut data.slack_sidebar_rows, positions, presence);
            }
        }
        changed
    }

    fn patch_dm_user(
        &self,
        data: &mut SurfaceStateData,
        team_id: &str,
        user_id: &str,
        presence: Option<SlackUserPresence>,
    ) -> bool {
        let mut changed = false;
        if let Some(snapshot) = data
            .slack_dm_inbox_snapshot
            .as_mut()
            .filter(|snapshot| snapshot.team_id == team_id)
        {
            if let Some(positions) = self.indexes.dm_snapshot.get(user_id) {
                for &(item_index, participant_index) in positions {
                    if let Some(participant) = snapshot
                        .items
                        .get_mut(item_index)
                        .and_then(|item| item.participants.get_mut(participant_index))
                    {
                        changed |= set_presence(&mut participant.presence, presence);
                    }
                }
            }
            if let Some(positions) = self.indexes.dm_rows.get(user_id) {
                changed |= patch_dm_rows(&mut data.slack_dm_rows, positions, presence);
            }
        }
        changed
    }

    fn patch_all_threads_user(
        &self,
        data: &mut SurfaceStateData,
        team_id: &str,
        user_id: &str,
        presence: Option<SlackUserPresence>,
    ) -> bool {
        let mut changed = false;
        if let Some(snapshot) = data
            .slack_all_threads_snapshot
            .as_mut()
            .filter(|snapshot| snapshot.team_id == team_id)
        {
            if let Some(positions) = self.indexes.all_threads.get(user_id) {
                for &index in positions {
                    if let Some(thread) = snapshot.threads.get_mut(index) {
                        changed |= set_presence(&mut thread.direct_message_presence, presence);
                    }
                }
                changed |=
                    patch_all_thread_rows(&mut data.slack_all_threads_rows, positions, presence);
            }
        }
        changed
    }

    fn patch_members_user(
        &self,
        data: &mut SurfaceStateData,
        team_id: &str,
        user_id: &str,
        presence: Option<SlackUserPresence>,
    ) -> bool {
        let mut changed = false;
        if let Some(snapshot) = data
            .slack_members_snapshot
            .as_mut()
            .filter(|snapshot| snapshot.team_id == team_id)
        {
            if let Some(positions) = self.indexes.members.get(user_id) {
                for &index in positions {
                    if let Some(member) = snapshot.members.get_mut(index) {
                        changed |= set_presence(&mut member.presence, presence);
                    }
                }
                changed |= patch_member_rows(&mut data.slack_members_rows, positions, presence);
            }
        }
        changed
    }

    fn patch_destination_user(
        &self,
        data: &mut SurfaceStateData,
        team_id: &str,
        user_id: &str,
        presence: Option<SlackUserPresence>,
    ) -> bool {
        let mut changed = false;
        if data.slack_new_message_team_id.as_deref() == Some(team_id) {
            if let Some(positions) = self.indexes.new_message.get(user_id) {
                if let Some(snapshot) = data
                    .slack_new_message_directory_snapshot
                    .as_mut()
                    .filter(|snapshot| snapshot.team_id == team_id)
                {
                    for &index in positions {
                        if let Some(candidate) = snapshot.candidates.get_mut(index) {
                            changed |= set_presence(&mut candidate.presence, presence);
                        }
                    }
                }
                changed |=
                    patch_destination_rows(&mut data.slack_new_message_rows, positions, presence);
            }
        }
        if let Some(modal) = data
            .slack_message_forward_modal
            .as_mut()
            .filter(|modal| modal.source.team_id == team_id)
        {
            if let Some(positions) = self.indexes.forward.get(user_id) {
                changed |= patch_destination_rows(&mut modal.rows, positions, presence);
            }
        }
        changed
    }

    fn patch_directory_user(
        &self,
        data: &mut SurfaceStateData,
        team_id: &str,
        user_id: &str,
        presence: Option<SlackUserPresence>,
    ) -> bool {
        if data.slack_directory_team_id.as_deref() != Some(team_id) {
            return false;
        }
        let Some(positions) = self.indexes.directory.get(user_id) else {
            return false;
        };
        let mut changed = false;
        if let Some(snapshot) = data
            .slack_directory_snapshot
            .as_mut()
            .filter(|snapshot| snapshot.team_id == team_id)
        {
            for &index in positions {
                if let Some(candidate) = snapshot.candidates.get_mut(index) {
                    changed |= set_presence(&mut candidate.presence, presence);
                }
            }
        }
        changed |= patch_destination_rows(&mut data.slack_directory_rows, positions, presence);
        changed
    }
}

fn set_presence(
    current: &mut Option<SlackUserPresence>,
    presence: Option<SlackUserPresence>,
) -> bool {
    if *current == presence {
        return false;
    }
    *current = presence;
    true
}

fn patch_sections(
    sections: &mut [SlackSidebarSection],
    positions: &[(usize, usize)],
    presence: Option<SlackUserPresence>,
) -> bool {
    positions.iter().fold(false, |changed, &(section, item)| {
        sections
            .get_mut(section)
            .and_then(|section| section.items.get_mut(item))
            .is_some_and(|item| set_presence(&mut item.presence, presence))
            || changed
    })
}

fn patch_sidebar_rows(
    rows: &mut Arc<[SlackSidebarRow]>,
    positions: &[usize],
    presence: Option<SlackUserPresence>,
) -> bool {
    let needs_patch = positions.iter().any(|&index| {
        rows.get(index).is_some_and(|row| {
            matches!(&row.kind, SlackSidebarRowKind::Item { item, .. } if item.presence != presence)
        })
    });
    if !needs_patch {
        return false;
    }
    for &index in positions {
        let Some(SlackSidebarRow {
            kind: SlackSidebarRowKind::Item { item, .. },
            ..
        }) = Arc::make_mut(rows).get_mut(index)
        else {
            continue;
        };
        item.presence = presence;
    }
    true
}

fn patch_dm_rows(
    rows: &mut Arc<[SlackDmRow]>,
    positions: &[(usize, usize)],
    presence: Option<SlackUserPresence>,
) -> bool {
    let needs_patch = positions.iter().any(|&(row_index, participant_index)| {
        rows.get(row_index)
            .and_then(|row| row.participants.get(participant_index))
            .is_some_and(|participant| participant.presence != presence)
    });
    if !needs_patch {
        return false;
    }
    let rows = Arc::make_mut(rows);
    for &(row_index, participant_index) in positions {
        if let Some(participant) = rows
            .get_mut(row_index)
            .and_then(|row| Arc::make_mut(&mut row.participants).get_mut(participant_index))
        {
            participant.presence = presence;
        }
    }
    true
}

fn patch_all_thread_rows(
    rows: &mut Arc<[SlackAllThreadRow]>,
    positions: &[usize],
    presence: Option<SlackUserPresence>,
) -> bool {
    patch_rows(
        rows,
        positions,
        |row| row.direct_message_presence,
        |row| row.direct_message_presence = presence,
        presence,
    )
}

fn patch_member_rows(
    rows: &mut Arc<[SlackMemberRow]>,
    positions: &[usize],
    presence: Option<SlackUserPresence>,
) -> bool {
    patch_rows(
        rows,
        positions,
        |row| row.presence,
        |row| row.presence = presence,
        presence,
    )
}

fn patch_destination_rows(
    rows: &mut Arc<[SlackNewMessageCandidateRow]>,
    positions: &[usize],
    presence: Option<SlackUserPresence>,
) -> bool {
    patch_rows(
        rows,
        positions,
        |row| row.presence,
        |row| row.presence = presence,
        presence,
    )
}

fn patch_rows<T: Clone>(
    rows: &mut Arc<[T]>,
    positions: &[usize],
    current: impl Fn(&T) -> Option<SlackUserPresence>,
    mut apply: impl FnMut(&mut T),
    presence: Option<SlackUserPresence>,
) -> bool {
    if !positions
        .iter()
        .any(|&index| rows.get(index).is_some_and(|row| current(row) != presence))
    {
        return false;
    }
    let rows = Arc::make_mut(rows);
    for &index in positions {
        if let Some(row) = rows.get_mut(index) {
            apply(row);
        }
    }
    true
}
