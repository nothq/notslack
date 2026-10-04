use super::{
    prepare_slack_members_page, spawn_background_task_for_entity, Context, Instant,
    PreparedSlackConversationMembers, ScrollStrategy, SlackConversationMembersCursor,
    SlackMembersBackgroundRequest, SlackMembersLoad, SurfaceState,
    SLACK_MEMBERS_INITIAL_VISIBLE_ROWS,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn begin_slack_members_page(
        &mut self,
        cursor: Option<SlackConversationMembersCursor>,
        cx: &mut Context<Self>,
    ) {
        if self.slack_members_request.is_some() {
            return;
        }
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            self.slack_members_error =
                Some("Slack members require a connected workspace.".to_string());
            cx.notify();
            return;
        };
        let Some(conversation_id) = self.slack_members_conversation_id.clone() else {
            return;
        };
        let request = SlackMembersLoad {
            generation: self.slack_members_generation,
            conversation_id,
            cursor,
            started_at: Instant::now(),
            profile_enabled: Self::slack_load_profiling_enabled(),
        };
        if request.profile_enabled {
            eprintln!(
                "[notslack-slack-members-profile] stage=request_started conversation={} generation={} paginated={}",
                request.conversation_id,
                request.generation,
                request.cursor.is_some(),
            );
        }
        self.slack_members_request = Some(request.clone());
        self.slack_members_loading = true;
        self.slack_members_error = None;
        cx.notify();
        let existing_members = self
            .slack_members_snapshot
            .as_ref()
            .map(|snapshot| snapshot.members.clone())
            .unwrap_or_default();
        spawn_background_task_for_entity(
            (workspace_api, request, existing_members),
            cx,
            |(workspace_api, request, existing_members): SlackMembersBackgroundRequest| {
                let result = workspace_api
                    .load_slack_conversation_members(
                        &request.conversation_id,
                        request.cursor.as_ref(),
                    )
                    .and_then(|snapshot| {
                        prepare_slack_members_page(existing_members, snapshot, &request)
                    });
                (request, result)
            },
            |this, (request, result), cx| {
                this.finish_slack_members_page(request, result, cx);
            },
        );
    }

    pub(in crate::ui::surface::state) fn finish_slack_members_page(
        &mut self,
        request: SlackMembersLoad,
        result: Result<PreparedSlackConversationMembers, String>,
        cx: &mut Context<Self>,
    ) {
        let current_request = self.slack_members_request_is_current(&request);
        self.log_slack_members_request_completion(&request, &result, current_request);
        if !current_request {
            return;
        }
        self.slack_members_request = None;
        self.slack_members_loading = false;
        let Some(prepared) = self.validate_slack_members_page(&request, result, cx) else {
            return;
        };
        self.install_prepared_slack_members_page(&request, prepared, cx);
        let next_cursor = self
            .slack_members_snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.next_cursor.clone());
        let should_continue = self.slack_members_panel_open
            && !self.slack_members_query.trim().is_empty()
            && next_cursor.is_some();
        cx.notify();
        if should_continue {
            self.begin_slack_members_page(next_cursor, cx);
        }
    }

    fn slack_members_request_is_current(&self, request: &SlackMembersLoad) -> bool {
        self.slack_members_generation == request.generation
            && self.slack_members_conversation_id.as_deref()
                == Some(request.conversation_id.as_str())
            && self.slack_members_request.as_ref().is_some_and(|current| {
                current.generation == request.generation
                    && current.conversation_id == request.conversation_id
                    && current.cursor == request.cursor
            })
    }

    fn log_slack_members_request_completion(
        &self,
        request: &SlackMembersLoad,
        result: &Result<PreparedSlackConversationMembers, String>,
        current_request: bool,
    ) {
        if !request.profile_enabled {
            return;
        }
        let (outcome, member_count, external_member_count, external_organization_count) =
            match (result, current_request) {
                (_, false) => ("stale", 0, 0, 0),
                (Ok(prepared), true) => (
                    "ready",
                    prepared.snapshot.members.len(),
                    prepared
                        .external_summary
                        .as_ref()
                        .map_or(0, |summary| summary.external_member_count),
                    prepared
                        .external_summary
                        .as_ref()
                        .map_or(0, |summary| summary.external_organization_count),
                ),
                (Err(_), true) => ("failed", 0, 0, 0),
            };
        eprintln!(
            "[notslack-slack-members-profile] stage=request_completed conversation={} generation={} elapsed_ms={:.2} outcome={} members={} external_members={} external_organizations={}",
            request.conversation_id,
            request.generation,
            request.started_at.elapsed().as_secs_f64() * 1_000.0,
            outcome,
            member_count,
            external_member_count,
            external_organization_count,
        );
    }

    fn validate_slack_members_page(
        &mut self,
        request: &SlackMembersLoad,
        result: Result<PreparedSlackConversationMembers, String>,
        cx: &mut Context<Self>,
    ) -> Option<PreparedSlackConversationMembers> {
        match result {
            Ok(prepared)
                if prepared.snapshot.conversation_id == request.conversation_id
                    && self.slack_workspace().is_some_and(|workspace| {
                        workspace.team_id == prepared.snapshot.team_id
                    }) =>
            {
                Some(prepared)
            }
            Ok(_) => {
                self.slack_members_error = Some(
                    "Slack members response did not match the active conversation.".to_string(),
                );
                cx.notify();
                None
            }
            Err(error) => {
                self.slack_members_error = Some(error);
                cx.notify();
                None
            }
        }
    }

    fn install_prepared_slack_members_page(
        &mut self,
        request: &SlackMembersLoad,
        mut prepared: PreparedSlackConversationMembers,
        cx: &mut Context<Self>,
    ) {
        self.slack_presence_authority
            .overlay_prepared_members(&mut prepared);
        self.slack_members_snapshot = Some(prepared.snapshot);
        self.slack_external_members_summary = prepared.external_summary;
        self.slack_external_organization_badges_by_user_id =
            prepared.external_organization_badges_by_user_id;
        self.slack_members_rows = prepared.rows;
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_members(data);
        }
        self.rebuild_slack_members_visible_rows();
        self.slack_members_error = None;
        self.slack_members_prefetched_range = None;
        if request.profile_enabled {
            eprintln!(
                "[notslack-slack-members-profile] stage=snapshot_applied conversation={} generation={} elapsed_ms={:.2} members={}",
                request.conversation_id,
                request.generation,
                request.started_at.elapsed().as_secs_f64() * 1_000.0,
                self.slack_members_rows.len(),
            );
        }
        self.queue_slack_external_members_summary_images(cx);
        if self.slack_members_panel_open {
            self.queue_slack_members_visible_images(0, SLACK_MEMBERS_INITIAL_VISIBLE_ROWS, cx);
        }
    }

    pub(crate) fn select_slack_member_row(&mut self, visible_index: usize, cx: &mut Context<Self>) {
        let Some(row_index) = self
            .slack_members_visible_row_indices
            .get(visible_index)
            .copied()
        else {
            return;
        };
        let Some(user_id) = self
            .slack_members_rows
            .get(row_index)
            .map(|row| row.user_id.to_string())
        else {
            return;
        };
        self.slack_members_scroll_handle
            .scroll_to_item(visible_index, ScrollStrategy::Nearest);
        self.open_slack_member_profile(&user_id, cx);
    }

    pub(in crate::ui::surface::state) fn queue_slack_external_members_summary_images(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        let applies = self
            .slack_workspace()
            .zip(self.slack_members_snapshot.as_ref())
            .zip(self.slack_external_members_summary.as_ref())
            .is_some_and(|((workspace, snapshot), summary)| {
                snapshot.next_cursor.is_none()
                    && snapshot.team_id == workspace.team_id
                    && snapshot.conversation_id == workspace.conversation_id
                    && summary.team_id.as_ref() == workspace.team_id
                    && summary.conversation_id.as_ref() == workspace.conversation_id
            });
        if !applies {
            return;
        }
        let urls = self
            .slack_external_members_summary
            .as_ref()
            .expect("applicable Slack external-members summary must exist")
            .organizations
            .iter()
            .filter_map(|organization| organization.image_url.as_ref())
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }
}
