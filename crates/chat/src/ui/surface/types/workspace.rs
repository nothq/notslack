use super::{
    build_slack_dm_rows, build_slack_message_chunks, build_slack_message_rows_with_local_today,
    build_slack_shell_remote_images, build_slack_sidebar_remote_images, build_slack_sidebar_rows,
    build_slack_sidebar_snapshot_rows, HashSet, PreparedSlackDmInboxSnapshot,
    PreparedSlackShellSnapshot, PreparedSlackSidebarSnapshot, PreparedSlackWorkspace,
    SlackDmInboxSnapshot, SlackShellSnapshot, SlackSidebarSnapshot, SlackWorkspace, SurfaceState,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackInitialRefreshIdentity {
    generation: u64,
    team_id: String,
    conversation_id: String,
}

impl SlackInitialRefreshIdentity {
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn team_id(&self) -> &str {
        &self.team_id
    }

    pub(crate) fn conversation_id(&self) -> &str {
        &self.conversation_id
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SlackInitialRefreshContentState {
    #[default]
    Pending,
    CacheApplied,
    LiveApplied,
}

#[derive(Debug)]
struct SlackInitialRefreshRun {
    identity: SlackInitialRefreshIdentity,
    shell: SlackInitialRefreshContentState,
    sidebar: SlackInitialRefreshContentState,
    conversation: SlackInitialRefreshContentState,
    dm_inbox: SlackInitialRefreshContentState,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SlackInitialCachedWorkspaceClaim {
    pub(crate) shell: bool,
    pub(crate) sidebar: bool,
    pub(crate) conversation: bool,
}

impl SlackInitialCachedWorkspaceClaim {
    pub(crate) fn any(self) -> bool {
        self.shell || self.sidebar || self.conversation
    }
}

#[derive(Debug, Default)]
pub(crate) struct SlackInitialRefreshState {
    generation: u64,
    active: Option<SlackInitialRefreshRun>,
}

impl SlackInitialRefreshState {
    pub(crate) fn begin(
        &mut self,
        team_id: String,
        conversation_id: String,
    ) -> SlackInitialRefreshIdentity {
        self.generation = self
            .generation
            .checked_add(1)
            .expect("Slack initial refresh generation overflowed");
        let identity = SlackInitialRefreshIdentity {
            generation: self.generation,
            team_id,
            conversation_id,
        };
        self.active = Some(SlackInitialRefreshRun {
            identity: identity.clone(),
            shell: SlackInitialRefreshContentState::Pending,
            sidebar: SlackInitialRefreshContentState::Pending,
            conversation: SlackInitialRefreshContentState::Pending,
            dm_inbox: SlackInitialRefreshContentState::Pending,
        });
        identity
    }

    pub(crate) fn is_current(&self, identity: &SlackInitialRefreshIdentity) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.identity == *identity)
    }

    pub(crate) fn claim_cached_workspace(
        &mut self,
        identity: &SlackInitialRefreshIdentity,
    ) -> Option<SlackInitialCachedWorkspaceClaim> {
        let active = self
            .active
            .as_mut()
            .filter(|active| active.identity == *identity)?;
        let claim = SlackInitialCachedWorkspaceClaim {
            shell: active.shell != SlackInitialRefreshContentState::LiveApplied,
            sidebar: active.sidebar != SlackInitialRefreshContentState::LiveApplied,
            conversation: active.conversation != SlackInitialRefreshContentState::LiveApplied,
        };
        if claim.shell {
            active.shell = SlackInitialRefreshContentState::CacheApplied;
        }
        if claim.sidebar {
            active.sidebar = SlackInitialRefreshContentState::CacheApplied;
        }
        if claim.conversation {
            active.conversation = SlackInitialRefreshContentState::CacheApplied;
        }
        Some(claim)
    }

    pub(crate) fn claim_cached_dm_inbox(&mut self, identity: &SlackInitialRefreshIdentity) -> bool {
        let Some(active) = self
            .active
            .as_mut()
            .filter(|active| active.identity == *identity)
        else {
            return false;
        };
        if active.dm_inbox == SlackInitialRefreshContentState::LiveApplied {
            return false;
        }
        active.dm_inbox = SlackInitialRefreshContentState::CacheApplied;
        true
    }

    pub(crate) fn record_live_conversation(
        &mut self,
        identity: &SlackInitialRefreshIdentity,
    ) -> bool {
        let Some(active) = self
            .active
            .as_mut()
            .filter(|active| active.identity == *identity)
        else {
            return false;
        };
        active.conversation = SlackInitialRefreshContentState::LiveApplied;
        true
    }

    pub(crate) fn record_live_shell(&mut self, identity: &SlackInitialRefreshIdentity) -> bool {
        let Some(active) = self
            .active
            .as_mut()
            .filter(|active| active.identity == *identity)
        else {
            return false;
        };
        active.shell = SlackInitialRefreshContentState::LiveApplied;
        true
    }

    pub(crate) fn record_live_sidebar(&mut self, identity: &SlackInitialRefreshIdentity) -> bool {
        let Some(active) = self
            .active
            .as_mut()
            .filter(|active| active.identity == *identity)
        else {
            return false;
        };
        active.sidebar = SlackInitialRefreshContentState::LiveApplied;
        true
    }

    pub(crate) fn record_live_dm_inbox(&mut self, identity: &SlackInitialRefreshIdentity) -> bool {
        let Some(active) = self
            .active
            .as_mut()
            .filter(|active| active.identity == *identity)
        else {
            return false;
        };
        active.dm_inbox = SlackInitialRefreshContentState::LiveApplied;
        true
    }

    pub(crate) fn invalidate(&mut self) {
        self.active = None;
    }
}

pub(crate) fn prepare_slack_workspace(
    mut workspace: SlackWorkspace,
    collapsed_sections: &HashSet<String>,
    muted_conversations: &HashSet<String>,
) -> PreparedSlackWorkspace {
    let active_conversation_was_muted = workspace
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
        .find(|item| item.target_id == workspace.conversation_id)
        .is_some_and(|item| item.muted);
    let conversation_id = workspace.conversation_id.clone();
    let conversation_muted =
        active_conversation_was_muted || muted_conversations.contains(conversation_id.as_str());

    for item in workspace
        .sections
        .iter_mut()
        .flat_map(|section| section.items.iter_mut())
    {
        item.active = item.target_id == conversation_id;
        if item.target_id == conversation_id {
            item.muted = conversation_muted;
        }
    }

    let (message_rows, message_rows_local_today) =
        build_slack_message_rows_with_local_today(Some(&workspace));
    let message_rows_local_today =
        message_rows_local_today.expect("prepared Slack workspace must have a local date");
    let message_chunks = build_slack_message_chunks(&message_rows);
    let sidebar_rows = build_slack_sidebar_rows(Some(&workspace), collapsed_sections);
    let remote_images = SurfaceState::build_slack_remote_images(Some(&workspace));

    PreparedSlackWorkspace {
        workspace,
        message_rows,
        message_rows_local_today,
        message_chunks,
        sidebar_rows,
        remote_images,
        active_conversation_was_muted,
    }
}

pub(crate) fn prepare_slack_shell_snapshot(
    snapshot: SlackShellSnapshot,
) -> PreparedSlackShellSnapshot {
    let remote_images = build_slack_shell_remote_images(&snapshot);
    PreparedSlackShellSnapshot {
        snapshot,
        remote_images,
    }
}

pub(crate) fn prepare_slack_sidebar_snapshot(
    mut snapshot: SlackSidebarSnapshot,
    collapsed_sections: &HashSet<String>,
    muted_conversations: &HashSet<String>,
) -> PreparedSlackSidebarSnapshot {
    let active_conversation_was_muted = snapshot
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
        .find(|item| item.target_id == snapshot.conversation_id)
        .is_some_and(|item| item.muted);
    let conversation_muted = active_conversation_was_muted
        || muted_conversations.contains(snapshot.conversation_id.as_str());
    for item in snapshot
        .sections
        .iter_mut()
        .flat_map(|section| section.items.iter_mut())
    {
        item.active = item.target_id == snapshot.conversation_id;
        if item.active {
            item.muted = conversation_muted;
        }
    }
    let rows = build_slack_sidebar_snapshot_rows(&snapshot, collapsed_sections);
    let remote_images = build_slack_sidebar_remote_images(&snapshot);
    PreparedSlackSidebarSnapshot {
        snapshot,
        rows,
        remote_images,
        active_conversation_was_muted,
    }
}

pub(crate) fn prepare_slack_dm_inbox_snapshot(
    snapshot: SlackDmInboxSnapshot,
) -> PreparedSlackDmInboxSnapshot {
    let rows = build_slack_dm_rows(&snapshot);
    PreparedSlackDmInboxSnapshot { snapshot, rows }
}
