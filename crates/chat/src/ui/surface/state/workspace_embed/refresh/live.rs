use super::super::{
    prepare_initial_stage, prepare_slack_dm_inbox_snapshot, prepare_slack_shell_snapshot,
    prepare_slack_sidebar_snapshot, Context, PreparedSlackDmInboxSnapshot,
    PreparedSlackShellSnapshot, PreparedSlackSidebarSnapshot, SlackInitialStageLoadResult,
    SurfaceState, WorkspaceApi,
};
use super::{SlackInitialSidebarStage, SlackInitialStageContext};
use crate::ui::surface::SlackInitialRefreshIdentity;

impl SurfaceState {
    pub(super) fn spawn_initial_slack_shell_stage(
        &mut self,
        context: SlackInitialStageContext,
        cx: &mut Context<Self>,
    ) {
        let SlackInitialStageContext {
            identity,
            workspace_api,
            profile_enabled,
        } = context;
        let load_identity = identity.clone();
        self.spawn_background_task(
            (),
            cx,
            move |()| {
                prepare_initial_stage(profile_enabled, || {
                    load_initial_slack_shell(workspace_api.as_ref(), &load_identity)
                })
            },
            move |this, load, cx| {
                this.finish_initial_slack_shell_stage(identity, load, cx);
            },
        );
    }

    fn finish_initial_slack_shell_stage(
        &mut self,
        identity: SlackInitialRefreshIdentity,
        load: SlackInitialStageLoadResult<PreparedSlackShellSnapshot>,
        cx: &mut Context<Self>,
    ) {
        if !self.initial_slack_refresh_is_current(&identity) {
            self.report_stale_initial_slack_refresh(&identity, "live_shell");
            return;
        }
        self.record_slack_activation_live_stage_prepared(
            identity.conversation_id(),
            "shell",
            load.prepared_at,
        );
        match load.result {
            Ok(prepared) => {
                if !self.slack_initial_refresh.record_live_shell(&identity) {
                    return;
                }
                self.apply_prepared_slack_shell_snapshot(prepared, cx);
                self.record_slack_activation_live_stage_applied(
                    identity.conversation_id(),
                    "shell",
                );
                self.finish_initial_slack_activation_if_visible(&identity);
            }
            Err(error) => {
                self.record_slack_activation_live_stage_failed(identity.conversation_id(), "shell");
                self.slack_error = Some(error);
                cx.notify();
            }
        }
    }

    pub(super) fn spawn_initial_slack_sidebar_stage(
        &mut self,
        stage: SlackInitialSidebarStage,
        cx: &mut Context<Self>,
    ) {
        let SlackInitialSidebarStage {
            context:
                SlackInitialStageContext {
                    identity,
                    workspace_api,
                    profile_enabled,
                },
            collapsed_sections,
            muted_conversations,
        } = stage;
        let load_identity = identity.clone();
        self.spawn_background_task(
            (),
            cx,
            move |()| {
                prepare_initial_stage(profile_enabled, || {
                    load_initial_slack_sidebar(
                        workspace_api.as_ref(),
                        &load_identity,
                        &collapsed_sections,
                        &muted_conversations,
                    )
                })
            },
            move |this, load, cx| {
                this.finish_initial_slack_sidebar_stage(identity, load, cx);
            },
        );
    }

    fn finish_initial_slack_sidebar_stage(
        &mut self,
        identity: SlackInitialRefreshIdentity,
        load: SlackInitialStageLoadResult<PreparedSlackSidebarSnapshot>,
        cx: &mut Context<Self>,
    ) {
        if !self.initial_slack_refresh_is_current(&identity) {
            self.report_stale_initial_slack_refresh(&identity, "live_sidebar");
            return;
        }
        self.record_slack_activation_live_stage_prepared(
            identity.conversation_id(),
            "sidebar",
            load.prepared_at,
        );
        match load.result {
            Ok(prepared) => {
                if !self.slack_initial_refresh.record_live_sidebar(&identity) {
                    return;
                }
                self.apply_prepared_slack_sidebar_snapshot(prepared, cx);
                self.record_slack_activation_live_stage_applied(
                    identity.conversation_id(),
                    "sidebar",
                );
            }
            Err(error) => {
                self.record_slack_activation_live_stage_failed(
                    identity.conversation_id(),
                    "sidebar",
                );
                self.slack_error = Some(error);
                cx.notify();
            }
        }
    }

    pub(super) fn spawn_initial_slack_dm_stage(
        &mut self,
        context: SlackInitialStageContext,
        cx: &mut Context<Self>,
    ) {
        let SlackInitialStageContext {
            identity,
            workspace_api,
            profile_enabled,
        } = context;
        let load_identity = identity.clone();
        self.spawn_background_task(
            (),
            cx,
            move |()| {
                prepare_initial_stage(profile_enabled, || {
                    load_initial_slack_dm_inbox(workspace_api.as_ref(), &load_identity)
                })
            },
            move |this, load, cx| {
                this.finish_initial_slack_dm_stage(identity, load, cx);
            },
        );
    }

    fn finish_initial_slack_dm_stage(
        &mut self,
        identity: SlackInitialRefreshIdentity,
        load: SlackInitialStageLoadResult<PreparedSlackDmInboxSnapshot>,
        cx: &mut Context<Self>,
    ) {
        if !self.initial_slack_refresh_is_current(&identity) {
            self.report_stale_initial_slack_refresh(&identity, "live_dms");
            return;
        }
        self.record_slack_activation_live_stage_prepared(
            identity.conversation_id(),
            "dms",
            load.prepared_at,
        );
        match load.result {
            Ok(prepared) => {
                if !self.slack_initial_refresh.record_live_dm_inbox(&identity) {
                    return;
                }
                self.apply_prepared_slack_dm_inbox_snapshot(prepared, false, cx);
                self.record_slack_activation_live_stage_applied(identity.conversation_id(), "dms");
            }
            Err(error) => {
                self.record_slack_activation_live_stage_failed(identity.conversation_id(), "dms");
                eprintln!("Slack DMs inbox refresh failed: {error}");
            }
        }
    }
}

fn load_initial_slack_shell(
    workspace_api: &dyn WorkspaceApi,
    identity: &SlackInitialRefreshIdentity,
) -> Result<PreparedSlackShellSnapshot, String> {
    let snapshot = workspace_api.load_slack_shell()?;
    if snapshot.team_id != identity.team_id() {
        return Err("Slack shell targeted a different initial refresh".to_string());
    }
    Ok(prepare_slack_shell_snapshot(snapshot))
}

fn load_initial_slack_sidebar(
    workspace_api: &dyn WorkspaceApi,
    identity: &SlackInitialRefreshIdentity,
    collapsed_sections: &std::collections::HashSet<String>,
    muted_conversations: &std::collections::HashSet<String>,
) -> Result<PreparedSlackSidebarSnapshot, String> {
    let snapshot = workspace_api.load_slack_sidebar(identity.conversation_id())?;
    if snapshot.team_id != identity.team_id()
        || snapshot.conversation_id != identity.conversation_id()
    {
        return Err("Slack sidebar targeted a different initial refresh".to_string());
    }
    Ok(prepare_slack_sidebar_snapshot(
        snapshot,
        collapsed_sections,
        muted_conversations,
    ))
}

fn load_initial_slack_dm_inbox(
    workspace_api: &dyn WorkspaceApi,
    identity: &SlackInitialRefreshIdentity,
) -> Result<PreparedSlackDmInboxSnapshot, String> {
    let snapshot = workspace_api.load_slack_dm_inbox(None)?;
    if snapshot.team_id != identity.team_id() {
        return Err("Slack DM inbox targeted a different initial refresh".to_string());
    }
    Ok(prepare_slack_dm_inbox_snapshot(snapshot))
}
