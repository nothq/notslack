mod realtime;

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{atomic::Ordering, Arc},
};

use crate::model::{
    SlackAllThreadsSnapshot, SlackConversationMembersSnapshot, SlackDestinationDirectorySnapshot,
    SlackDmInboxSnapshot, SlackRealtimePresenceChange, SlackRealtimePresenceSnapshot,
    SlackSidebarSnapshot, SlackUserPresence, SlackWorkspace,
    SLACK_REALTIME_PRESENCE_PAYLOAD_MAX_BYTES, SLACK_REALTIME_PRESENCE_USER_ID_MAX_BYTES,
    SLACK_REALTIME_PRESENCE_USER_LIMIT,
};

use super::SlackWorkspaceRuntime;

const SLACK_SYSTEM_USER_ID: &str = "USLACK";
#[derive(Default)]
pub(super) struct SlackPresenceState {
    self_user_id: Option<String>,
    dm_user_ids: Option<BTreeSet<String>>,
    latest_by_user_id: HashMap<String, SlackUserPresence>,
    accepted_source_revision: Option<u64>,
    presentation_revision: u64,
    accepted_roster_generation: u64,
}

impl SlackPresenceState {
    fn desired_ids(&self) -> Option<Arc<[String]>> {
        let self_user_id = self.self_user_id.as_ref()?;
        let dm_user_ids = self.dm_user_ids.as_ref()?;
        let mut ids = Vec::with_capacity(
            dm_user_ids
                .len()
                .saturating_add(1)
                .min(SLACK_REALTIME_PRESENCE_USER_LIMIT),
        );
        let mut serialized_bytes = 64_usize;
        for user_id in std::iter::once(self_user_id).chain(
            dm_user_ids
                .iter()
                .filter(|user_id| user_id.as_str() != self_user_id.as_str()),
        ) {
            let next_bytes = serialized_bytes
                .saturating_add(user_id.len())
                .saturating_add(3);
            if ids.len() == SLACK_REALTIME_PRESENCE_USER_LIMIT
                || !is_valid_slack_presence_user_id(user_id)
                || next_bytes > SLACK_REALTIME_PRESENCE_PAYLOAD_MAX_BYTES
            {
                continue;
            }
            serialized_bytes = next_bytes;
            ids.push(user_id.clone());
        }
        Some(ids.into())
    }

    fn retain_desired_presence(&mut self, desired_ids: &[String]) {
        let desired_ids = desired_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        self.latest_by_user_id
            .retain(|user_id, _| desired_ids.contains(user_id.as_str()));
    }

    fn snapshot(&self, team_id: &str) -> SlackRealtimePresenceSnapshot {
        let mut entries = self
            .latest_by_user_id
            .iter()
            .map(|(user_id, presence)| SlackRealtimePresenceChange {
                user_id: user_id.clone(),
                presence: *presence,
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.user_id.cmp(&right.user_id));
        SlackRealtimePresenceSnapshot {
            team_id: team_id.to_string(),
            revision: self.presentation_revision,
            entries,
        }
    }

    fn configured_roster_size(&self) -> usize {
        self.dm_user_ids
            .as_ref()
            .map(BTreeSet::len)
            .unwrap_or_default()
            .saturating_add(usize::from(self.self_user_id.is_some()))
    }
}

struct SlackDmPresenceRoster {
    user_ids: BTreeSet<String>,
    exceeded_bound: bool,
}

impl SlackDmPresenceRoster {
    fn from_sidebar(sidebar: &SlackSidebarSnapshot) -> Self {
        let mut user_ids = BTreeSet::new();
        let mut exceeded_bound = false;
        for user_id in sidebar
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .filter_map(|item| item.user_id.as_deref())
            .map(str::trim)
            .filter(|user_id| {
                *user_id != SLACK_SYSTEM_USER_ID && is_valid_slack_presence_user_id(user_id)
            })
        {
            if user_ids.contains(user_id) {
                continue;
            }
            if user_ids.len() >= SLACK_REALTIME_PRESENCE_USER_LIMIT {
                exceeded_bound = true;
                break;
            }
            user_ids.insert(user_id.to_string());
        }
        Self {
            user_ids,
            exceeded_bound,
        }
    }
}

impl SlackWorkspaceRuntime {
    pub(super) fn update_presence_self_user(
        &self,
        self_user_id: Option<&str>,
    ) -> Result<(), String> {
        let self_user_id = self_user_id
            .map(str::trim)
            .filter(|user_id| {
                *user_id != SLACK_SYSTEM_USER_ID && is_valid_slack_presence_user_id(user_id)
            })
            .map(str::to_string);
        let _commit = self
            .presence_roster_commit
            .lock()
            .map_err(|_| "Slack presence roster commit mutex poisoned".to_string())?;
        let desired_ids = {
            let mut state = self.presence_state.lock().map_err(|_| {
                "Slack presence state mutex poisoned while applying self user".to_string()
            })?;
            state.self_user_id = self_user_id;
            let desired_ids = state.desired_ids();
            if let Some(desired_ids) = desired_ids.as_ref() {
                state.retain_desired_presence(desired_ids);
            }
            desired_ids
        };
        self.realtime.set_presence_subscription_ids(desired_ids);
        Ok(())
    }

    pub(super) fn next_presence_roster_generation(&self) -> u64 {
        let _commit = self
            .presence_roster_commit
            .lock()
            .expect("Slack presence roster commit mutex poisoned while issuing a load");
        self.presence_roster_generation
            .fetch_add(1, Ordering::AcqRel)
            .checked_add(1)
            .expect("Slack presence roster generation overflowed")
    }

    pub(super) fn update_and_overlay_sidebar_presence(
        &self,
        sidebar: &mut SlackSidebarSnapshot,
        roster_generation: u64,
    ) -> Result<bool, String> {
        let roster = SlackDmPresenceRoster::from_sidebar(sidebar);
        let accepted_desired_ids = {
            let mut state = self.presence_state.lock().map_err(|_| {
                "Slack presence state mutex poisoned while applying sidebar users".to_string()
            })?;
            let accepted = self.presence_roster_generation.load(Ordering::Acquire)
                == roster_generation
                && roster_generation > state.accepted_roster_generation;
            if accepted {
                state.accepted_roster_generation = roster_generation;
                state.dm_user_ids = Some(roster.user_ids);
            }
            let desired_ids = state.desired_ids();
            if roster.exceeded_bound
                || state.configured_roster_size() > SLACK_REALTIME_PRESENCE_USER_LIMIT
            {
                eprintln!(
                    "Slack presence subscription exceeded the {SLACK_REALTIME_PRESENCE_USER_LIMIT}-user bound"
                );
            }
            if let Some(desired_ids) = desired_ids.as_ref() {
                state.retain_desired_presence(desired_ids);
            }
            overlay_sidebar_presence(sidebar, &state);
            accepted.then_some(desired_ids)
        };
        if let Some(desired_ids) = accepted_desired_ids {
            self.realtime.set_presence_subscription_ids(desired_ids);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(super) fn overlay_workspace_presence(&self, workspace: &mut SlackWorkspace) {
        let Ok(state) = self.presence_state.lock() else {
            eprintln!("Slack presence state mutex poisoned while applying cached workspace");
            return;
        };
        overlay_workspace_presence(workspace, &state);
    }

    pub(super) fn overlay_dm_inbox_presence(&self, snapshot: &mut SlackDmInboxSnapshot) {
        let Ok(state) = self.presence_state.lock() else {
            eprintln!("Slack presence state mutex poisoned while applying the DM inbox");
            return;
        };
        for participant in snapshot
            .items
            .iter_mut()
            .flat_map(|item| item.participants.iter_mut())
        {
            if let Some(presence) = state.latest_by_user_id.get(&participant.user_id) {
                participant.presence = Some(*presence);
            }
        }
    }

    pub(super) fn overlay_all_threads_presence(&self, snapshot: &mut SlackAllThreadsSnapshot) {
        let Ok(state) = self.presence_state.lock() else {
            eprintln!("Slack presence state mutex poisoned while applying All Threads");
            return;
        };
        for thread in &mut snapshot.threads {
            if let Some(presence) = thread
                .direct_message_user_id
                .as_ref()
                .and_then(|user_id| state.latest_by_user_id.get(user_id))
            {
                thread.direct_message_presence = Some(*presence);
            }
        }
    }

    pub(super) fn overlay_members_presence(&self, snapshot: &mut SlackConversationMembersSnapshot) {
        let Ok(state) = self.presence_state.lock() else {
            eprintln!("Slack presence state mutex poisoned while applying channel members");
            return;
        };
        for member in &mut snapshot.members {
            if let Some(presence) = state.latest_by_user_id.get(&member.user_id) {
                member.presence = Some(*presence);
            }
        }
    }

    pub(super) fn overlay_destination_directory_presence(
        &self,
        snapshot: &mut SlackDestinationDirectorySnapshot,
    ) {
        let Ok(state) = self.presence_state.lock() else {
            eprintln!("Slack presence state mutex poisoned while applying destination directory");
            return;
        };
        for candidate in &mut snapshot.candidates {
            let user_id = match &candidate.target {
                crate::model::SlackDestinationTarget::Person { user_id } => Some(user_id.as_str()),
                crate::model::SlackDestinationTarget::Conversation { kind, .. }
                    if *kind == crate::model::SlackConversationKind::DirectMessage =>
                {
                    candidate.participant_user_ids.first().map(String::as_str)
                }
                _ => None,
            };
            if let Some(presence) = user_id.and_then(|user_id| state.latest_by_user_id.get(user_id))
            {
                candidate.presence = Some(*presence);
            }
        }
    }
}

fn is_valid_slack_presence_user_id(user_id: &str) -> bool {
    !user_id.is_empty()
        && user_id.len() <= SLACK_REALTIME_PRESENCE_USER_ID_MAX_BYTES
        && user_id.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

pub(super) fn overlay_sidebar_presence(
    sidebar: &mut SlackSidebarSnapshot,
    state: &SlackPresenceState,
) {
    if let Some(self_user_id) = state.self_user_id.as_ref() {
        if let Some(presence) = state.latest_by_user_id.get(self_user_id) {
            sidebar.rail_badges.self_presence = Some(*presence);
        }
    }
    for item in sidebar
        .sections
        .iter_mut()
        .flat_map(|section| section.items.iter_mut())
    {
        if let Some(presence) = item
            .user_id
            .as_ref()
            .and_then(|user_id| state.latest_by_user_id.get(user_id))
        {
            item.presence = Some(*presence);
        }
    }
}

fn overlay_workspace_presence(workspace: &mut SlackWorkspace, state: &SlackPresenceState) {
    if let Some(self_user_id) = workspace.self_user_id.as_ref() {
        if let Some(presence) = state.latest_by_user_id.get(self_user_id) {
            workspace.rail_badges.self_presence = Some(*presence);
        }
    }
    for item in workspace
        .sections
        .iter_mut()
        .flat_map(|section| section.items.iter_mut())
    {
        if let Some(presence) = item
            .user_id
            .as_ref()
            .and_then(|user_id| state.latest_by_user_id.get(user_id))
        {
            item.presence = Some(*presence);
        }
    }
}
