use super::super::{
    prepare_slack_conversation_snapshot, prepare_slack_dm_inbox_snapshot,
    prepare_slack_shell_snapshot, prepare_slack_sidebar_snapshot, Context,
    PreparedSlackConversationSnapshot, PreparedSlackDmInboxSnapshot, PreparedSlackShellSnapshot,
    PreparedSlackSidebarSnapshot, SurfaceState, WorkspaceApi,
};
use super::SlackInitialStageContext;
use crate::ui::surface::{SlackInitialCachedWorkspaceClaim, SlackInitialRefreshIdentity};
use std::collections::HashSet;
use std::time::Instant;

struct SlackInitialCacheStageLoadResult<T> {
    result: Result<Option<T>, String>,
    completed_at: Instant,
}

struct PreparedSlackInitialCachedWorkspace {
    shell: PreparedSlackShellSnapshot,
    sidebar: PreparedSlackSidebarSnapshot,
    conversation: PreparedSlackConversationSnapshot,
}

impl SurfaceState {
    pub(super) fn spawn_initial_slack_cached_workspace_stage(
        &mut self,
        context: SlackInitialStageContext,
        collapsed_sections: HashSet<String>,
        muted_conversations: HashSet<String>,
        cx: &mut Context<Self>,
    ) {
        let SlackInitialStageContext {
            identity,
            workspace_api,
            profile_enabled: _,
        } = context;
        let load_identity = identity.clone();
        self.spawn_background_task(
            (),
            cx,
            move |()| {
                prepare_initial_cache_stage(|| {
                    load_initial_cached_slack_workspace(
                        workspace_api.as_ref(),
                        &load_identity,
                        &collapsed_sections,
                        &muted_conversations,
                    )
                })
            },
            move |this, load, cx| {
                this.finish_initial_slack_cached_workspace_stage(identity, load, cx);
            },
        );
    }

    fn finish_initial_slack_cached_workspace_stage(
        &mut self,
        identity: SlackInitialRefreshIdentity,
        load: SlackInitialCacheStageLoadResult<PreparedSlackInitialCachedWorkspace>,
        cx: &mut Context<Self>,
    ) {
        if !self.initial_slack_refresh_is_current(&identity) {
            self.report_stale_initial_slack_refresh(&identity, "cached_workspace");
            return;
        }
        match load.result {
            Ok(Some(prepared)) => {
                let Some(claim) = self.slack_initial_refresh.claim_cached_workspace(&identity)
                else {
                    self.report_stale_initial_slack_refresh(&identity, "cached_workspace");
                    return;
                };
                if claim.any() {
                    self.record_slack_activation_initial_cache_stage(
                        &identity,
                        "workspace",
                        "applied",
                        load.completed_at,
                    );
                    self.apply_initial_cached_slack_workspace(prepared, claim, cx);
                } else {
                    self.record_slack_activation_initial_cache_stage(
                        &identity,
                        "workspace",
                        "skipped_live",
                        load.completed_at,
                    );
                }
            }
            Ok(None) => self.record_slack_activation_initial_cache_stage(
                &identity,
                "workspace",
                "miss",
                load.completed_at,
            ),
            Err(error) => {
                self.record_slack_activation_initial_cache_stage(
                    &identity,
                    "workspace",
                    "error",
                    load.completed_at,
                );
                report_initial_slack_workspace_cache_error(&identity, &error);
            }
        }
    }

    fn apply_initial_cached_slack_workspace(
        &mut self,
        prepared: PreparedSlackInitialCachedWorkspace,
        claim: SlackInitialCachedWorkspaceClaim,
        cx: &mut Context<Self>,
    ) {
        let PreparedSlackInitialCachedWorkspace {
            shell,
            sidebar,
            conversation,
        } = prepared;
        if claim.shell {
            self.apply_prepared_slack_shell_snapshot(shell, cx);
        }
        if claim.sidebar {
            self.apply_prepared_slack_sidebar_snapshot(sidebar, cx);
        }
        if claim.conversation {
            self.apply_prepared_slack_conversation_snapshot(conversation, cx);
        }
    }

    pub(super) fn spawn_initial_slack_cached_dm_stage(
        &mut self,
        context: SlackInitialStageContext,
        cx: &mut Context<Self>,
    ) {
        let SlackInitialStageContext {
            identity,
            workspace_api,
            profile_enabled: _,
        } = context;
        let load_identity = identity.clone();
        self.spawn_background_task(
            (),
            cx,
            move |()| {
                prepare_initial_cache_stage(|| {
                    load_initial_cached_slack_dm_inbox(workspace_api.as_ref(), &load_identity)
                })
            },
            move |this, load, cx| {
                this.finish_initial_slack_cached_dm_stage(identity, load, cx);
            },
        );
    }

    fn finish_initial_slack_cached_dm_stage(
        &mut self,
        identity: SlackInitialRefreshIdentity,
        load: SlackInitialCacheStageLoadResult<PreparedSlackDmInboxSnapshot>,
        cx: &mut Context<Self>,
    ) {
        if !self.initial_slack_refresh_is_current(&identity) {
            self.report_stale_initial_slack_refresh(&identity, "cached_dms");
            return;
        }
        match load.result {
            Ok(Some(prepared)) => {
                if self.slack_initial_refresh.claim_cached_dm_inbox(&identity) {
                    self.record_slack_activation_initial_cache_stage(
                        &identity,
                        "dms",
                        "applied",
                        load.completed_at,
                    );
                    self.apply_prepared_slack_dm_inbox_snapshot(prepared, false, cx);
                } else {
                    self.record_slack_activation_initial_cache_stage(
                        &identity,
                        "dms",
                        "skipped_live",
                        load.completed_at,
                    );
                }
            }
            Ok(None) => self.record_slack_activation_initial_cache_stage(
                &identity,
                "dms",
                "miss",
                load.completed_at,
            ),
            Err(error) => {
                self.record_slack_activation_initial_cache_stage(
                    &identity,
                    "dms",
                    "error",
                    load.completed_at,
                );
                report_initial_slack_dm_cache_error(&identity, &error);
            }
        }
    }
}

fn prepare_initial_cache_stage<T>(
    load: impl FnOnce() -> Result<Option<T>, String>,
) -> SlackInitialCacheStageLoadResult<T> {
    SlackInitialCacheStageLoadResult {
        result: load(),
        completed_at: Instant::now(),
    }
}

fn load_initial_cached_slack_workspace(
    workspace_api: &dyn WorkspaceApi,
    identity: &SlackInitialRefreshIdentity,
    collapsed_sections: &HashSet<String>,
    muted_conversations: &HashSet<String>,
) -> Result<Option<PreparedSlackInitialCachedWorkspace>, String> {
    let Some(workspace) = workspace_api.load_cached_slack_workspace(identity.conversation_id())?
    else {
        return Ok(None);
    };
    if workspace.team_id != identity.team_id()
        || workspace.conversation_id != identity.conversation_id()
    {
        return Err("Slack cached workspace targeted a different initial refresh".to_string());
    }
    Ok(Some(PreparedSlackInitialCachedWorkspace {
        shell: prepare_slack_shell_snapshot(workspace.shell_snapshot()),
        sidebar: prepare_slack_sidebar_snapshot(
            workspace.sidebar_snapshot(),
            collapsed_sections,
            muted_conversations,
        ),
        conversation: prepare_slack_conversation_snapshot(workspace.conversation_snapshot()),
    }))
}

fn load_initial_cached_slack_dm_inbox(
    workspace_api: &dyn WorkspaceApi,
    identity: &SlackInitialRefreshIdentity,
) -> Result<Option<PreparedSlackDmInboxSnapshot>, String> {
    let Some(snapshot) = workspace_api.load_cached_slack_dm_inbox()? else {
        return Ok(None);
    };
    if snapshot.team_id != identity.team_id() {
        return Err("Slack cached DM inbox targeted a different initial refresh".to_string());
    }
    Ok(Some(prepare_slack_dm_inbox_snapshot(snapshot)))
}

fn report_initial_slack_workspace_cache_error(identity: &SlackInitialRefreshIdentity, error: &str) {
    eprintln!(
        "Slack initial workspace cache load failed for team={} conversation={} generation={}: {error}",
        identity.team_id(),
        identity.conversation_id(),
        identity.generation(),
    );
}

fn report_initial_slack_dm_cache_error(identity: &SlackInitialRefreshIdentity, error: &str) {
    eprintln!(
        "Slack initial DM cache load failed for team={} conversation={} generation={}: {error}",
        identity.team_id(),
        identity.conversation_id(),
        identity.generation(),
    );
}
