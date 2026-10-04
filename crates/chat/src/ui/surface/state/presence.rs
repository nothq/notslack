use std::collections::{HashMap, HashSet};

use crate::{
    model::{
        SlackUserPresence, SLACK_REALTIME_PRESENCE_USER_ID_MAX_BYTES,
        SLACK_REALTIME_PRESENCE_USER_LIMIT,
    },
    ui::surface::SurfaceStateData,
};

mod index;
mod overlay;
mod patch;
mod realtime;

use index::SlackPresenceProjectionIndexes;

const SLACK_PRESENCE_AUTHORITY_LIMIT: usize = SLACK_REALTIME_PRESENCE_USER_LIMIT * 2;

#[derive(Clone, Copy)]
pub(in crate::ui::surface) enum SlackPresencePatch {
    None,
    Delta,
    Full,
}

#[derive(Default)]
pub(in crate::ui::surface) struct SlackPresenceAuthority {
    team_id: Option<String>,
    self_user_id: Option<String>,
    accepted_revision: Option<u64>,
    latest_by_user_id: HashMap<String, Option<SlackUserPresence>>,
    tombstone_user_ids: HashSet<String>,
    indexes: SlackPresenceProjectionIndexes,
}

impl SlackPresenceAuthority {
    pub(in crate::ui::surface) fn initialize(&mut self, data: &mut SurfaceStateData) {
        if let Some(workspace) = data.slack_workspace.as_ref() {
            self.sync_identity(&workspace.team_id, workspace.self_user_id.as_deref());
        } else if let Some(shell) = data.slack_shell_snapshot.as_ref() {
            self.sync_identity(&shell.team_id, shell.self_user_id.as_deref());
        } else if let Some(shell) = data.slack_shell.as_ref() {
            self.sync_team(&shell.team_id);
        }
        self.seed_loaded_data(data);
        self.reindex_all(data);
    }

    pub(in crate::ui::surface) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(in crate::ui::surface) fn rebase_source(&mut self) {
        self.accepted_revision = None;
    }

    pub(in crate::ui::surface) const fn accepted_revision(&self) -> Option<u64> {
        self.accepted_revision
    }

    pub(in crate::ui::surface) fn known_user_count(&self) -> usize {
        self.latest_by_user_id
            .values()
            .filter(|presence| presence.is_some())
            .count()
    }

    pub(in crate::ui::surface) fn resolve_self_presence(
        &self,
        workspace: &crate::ui::SlackWorkspace,
    ) -> Option<SlackUserPresence> {
        if self.team_id.as_deref() != Some(workspace.team_id.as_str())
            || self.self_user_id.as_deref() != workspace.self_user_id.as_deref()
        {
            return workspace.rail_badges.self_presence;
        }
        let Some(user_id) = workspace.self_user_id.as_deref() else {
            return workspace.rail_badges.self_presence;
        };
        self.resolve_presence(
            &workspace.team_id,
            user_id,
            workspace.rail_badges.self_presence,
        )
    }

    pub(in crate::ui::surface) fn resolve_presence(
        &self,
        team_id: &str,
        user_id: &str,
        fallback: Option<SlackUserPresence>,
    ) -> Option<SlackUserPresence> {
        if self.team_id.as_deref() != Some(team_id) {
            return fallback;
        }
        self.latest_by_user_id
            .get(user_id)
            .copied()
            .unwrap_or(fallback)
    }

    pub(in crate::ui::surface) fn sync_team(&mut self, team_id: &str) {
        if self
            .team_id
            .as_deref()
            .is_some_and(|current| current != team_id)
        {
            self.clear();
        }
        self.team_id = Some(team_id.to_string());
    }

    pub(in crate::ui::surface) fn sync_identity(
        &mut self,
        team_id: &str,
        self_user_id: Option<&str>,
    ) {
        let self_changed = self
            .self_user_id
            .as_deref()
            .is_some_and(|current| Some(current) != self_user_id);
        if self
            .team_id
            .as_deref()
            .is_some_and(|current| current != team_id)
            || self_changed
        {
            self.clear();
        }
        self.team_id = Some(team_id.to_string());
        if self_user_id.is_some() || self.self_user_id.is_some() {
            self.self_user_id = self_user_id.map(str::to_string);
        }
    }
}

fn valid_presence_user_id(user_id: &str) -> bool {
    !user_id.is_empty()
        && user_id.len() <= SLACK_REALTIME_PRESENCE_USER_ID_MAX_BYTES
        && user_id.bytes().all(|byte| byte.is_ascii_alphanumeric())
}
