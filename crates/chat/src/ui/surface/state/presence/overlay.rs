use std::sync::Arc;

use crate::{
    model::{SlackSidebarSection, SlackUserPresence},
    ui::surface::{
        PreparedSlackAllThreadsSnapshot, PreparedSlackConversationMembers,
        PreparedSlackDestinationDirectory, PreparedSlackDmInboxSnapshot,
        PreparedSlackSidebarSnapshot, PreparedSlackWorkspace, SlackDmRow,
        SlackNewMessageCandidateRow, SlackSidebarRow, SlackSidebarRowKind, SurfaceStateData,
    },
};

use super::{valid_presence_user_id, SlackPresenceAuthority, SLACK_PRESENCE_AUTHORITY_LIMIT};

impl SlackPresenceAuthority {
    pub(in crate::ui::surface) fn overlay_workspace(
        &mut self,
        workspace: &mut crate::ui::SlackWorkspace,
    ) {
        self.sync_identity(&workspace.team_id, workspace.self_user_id.as_deref());
        if let Some(self_user_id) = workspace.self_user_id.as_deref() {
            self.overlay_value(self_user_id, &mut workspace.rail_badges.self_presence);
        }
        self.overlay_sections(&mut workspace.sections);
    }

    pub(in crate::ui::surface) fn overlay_prepared_workspace(
        &mut self,
        prepared: &mut PreparedSlackWorkspace,
    ) {
        self.overlay_workspace(&mut prepared.workspace);
        self.overlay_sidebar_rows(&mut prepared.sidebar_rows);
    }

    pub(in crate::ui::surface) fn overlay_prepared_sidebar(
        &mut self,
        prepared: &mut PreparedSlackSidebarSnapshot,
    ) {
        self.sync_team(&prepared.snapshot.team_id);
        if let Some(self_user_id) = self.self_user_id.as_deref() {
            Self::overlay_known_value(
                &mut prepared.snapshot.rail_badges.self_presence,
                self.latest_by_user_id.get(self_user_id),
            );
        }
        self.overlay_sections(&mut prepared.snapshot.sections);
        self.overlay_sidebar_rows(&mut prepared.rows);
    }

    pub(in crate::ui::surface) fn overlay_prepared_dm(
        &mut self,
        prepared: &mut PreparedSlackDmInboxSnapshot,
    ) {
        let self_user_id = (!prepared.snapshot.self_user_id.is_empty())
            .then_some(prepared.snapshot.self_user_id.as_str());
        self.sync_identity(&prepared.snapshot.team_id, self_user_id);
        for participant in prepared
            .snapshot
            .items
            .iter_mut()
            .flat_map(|item| item.participants.iter_mut())
        {
            self.overlay_value(&participant.user_id, &mut participant.presence);
        }
        for row in Arc::make_mut(&mut prepared.rows) {
            for participant in Arc::make_mut(&mut row.participants) {
                self.overlay_value(participant.user_id.as_ref(), &mut participant.presence);
            }
        }
    }

    pub(in crate::ui::surface) fn overlay_prepared_all_threads(
        &mut self,
        prepared: &mut PreparedSlackAllThreadsSnapshot,
    ) {
        self.sync_team(&prepared.snapshot.team_id);
        for thread in &mut prepared.snapshot.threads {
            if let Some(user_id) = thread.direct_message_user_id.as_deref() {
                self.overlay_value(user_id, &mut thread.direct_message_presence);
            }
        }
        for row in Arc::make_mut(&mut prepared.rows) {
            if let Some(user_id) = row.direct_message_user_id.as_deref() {
                self.overlay_value(user_id, &mut row.direct_message_presence);
            }
        }
    }

    pub(in crate::ui::surface) fn overlay_prepared_members(
        &mut self,
        prepared: &mut PreparedSlackConversationMembers,
    ) {
        self.sync_team(&prepared.snapshot.team_id);
        for member in &mut prepared.snapshot.members {
            let user_id = member.user_id.clone();
            self.overlay_value(&user_id, &mut member.presence);
        }
        for row in Arc::make_mut(&mut prepared.rows) {
            let user_id = row.user_id.to_string();
            self.overlay_value(&user_id, &mut row.presence);
        }
    }

    pub(in crate::ui::surface) fn overlay_prepared_destination(
        &mut self,
        prepared: &mut PreparedSlackDestinationDirectory,
    ) {
        self.sync_identity(
            &prepared.snapshot.team_id,
            Some(&prepared.snapshot.self_user_id),
        );
        for candidate in &mut prepared.snapshot.candidates {
            if let Some(user_id) = destination_presence_user_id(candidate).map(str::to_string) {
                self.overlay_value(&user_id, &mut candidate.presence);
            }
        }
        self.overlay_destination_rows(&mut prepared.rows);
    }

    pub(in crate::ui::surface) fn overlay_destination_rows(
        &mut self,
        rows: &mut Arc<[SlackNewMessageCandidateRow]>,
    ) {
        for row in Arc::make_mut(rows) {
            if let Some(user_id) = row.presence_user_id.as_deref() {
                self.overlay_value(user_id, &mut row.presence);
            }
        }
    }

    pub(in crate::ui::surface) fn overlay_dm_rows(&mut self, rows: &mut Arc<[SlackDmRow]>) {
        for row in Arc::make_mut(rows) {
            for participant in Arc::make_mut(&mut row.participants) {
                let user_id = participant.user_id.to_string();
                self.overlay_value(&user_id, &mut participant.presence);
            }
        }
    }

    pub(in crate::ui::surface) fn overlay_sidebar_rows(
        &mut self,
        rows: &mut Arc<[SlackSidebarRow]>,
    ) {
        for row in Arc::make_mut(rows) {
            let SlackSidebarRowKind::Item { item, .. } = &mut row.kind else {
                continue;
            };
            if let Some(user_id) = item.user_id.as_deref() {
                self.overlay_value(user_id, &mut item.presence);
            }
        }
    }

    pub(super) fn seed_loaded_data(&mut self, data: &mut SurfaceStateData) {
        if let Some(workspace) = data.slack_workspace.as_mut() {
            self.overlay_workspace(Arc::make_mut(workspace));
        }
        if let Some(snapshot) = data.slack_sidebar_snapshot.as_mut() {
            if let Some(self_user_id) = self.self_user_id.as_deref() {
                Self::overlay_known_value(
                    &mut snapshot.rail_badges.self_presence,
                    self.latest_by_user_id.get(self_user_id),
                );
            }
            self.overlay_sections(&mut snapshot.sections);
        }
        self.overlay_sidebar_rows(&mut data.slack_sidebar_rows);
        if let Some(snapshot) = data.slack_dm_inbox_snapshot.as_mut() {
            for participant in snapshot
                .items
                .iter_mut()
                .flat_map(|item| item.participants.iter_mut())
            {
                self.overlay_value(&participant.user_id, &mut participant.presence);
            }
        }
        for row in Arc::make_mut(&mut data.slack_dm_rows) {
            for participant in Arc::make_mut(&mut row.participants) {
                self.overlay_value(participant.user_id.as_ref(), &mut participant.presence);
            }
        }
    }

    fn overlay_sections(&mut self, sections: &mut [SlackSidebarSection]) {
        for item in sections
            .iter_mut()
            .flat_map(|section| section.items.iter_mut())
        {
            if let Some(user_id) = item.user_id.as_deref() {
                self.overlay_value(user_id, &mut item.presence);
            }
        }
    }

    fn overlay_value(&mut self, user_id: &str, current: &mut Option<SlackUserPresence>) {
        if !valid_presence_user_id(user_id) {
            return;
        }
        if let Some(known) = self.latest_by_user_id.get(user_id) {
            *current = *known;
        } else if current.is_some() && self.latest_by_user_id.len() < SLACK_PRESENCE_AUTHORITY_LIMIT
        {
            self.latest_by_user_id.insert(user_id.to_string(), *current);
        }
    }

    fn overlay_known_value(
        current: &mut Option<SlackUserPresence>,
        known: Option<&Option<SlackUserPresence>>,
    ) {
        if let Some(known) = known {
            *current = *known;
        }
    }
}

fn destination_presence_user_id(candidate: &crate::ui::SlackDestinationCandidate) -> Option<&str> {
    match &candidate.target {
        crate::ui::SlackDestinationTarget::Person { user_id } => Some(user_id),
        crate::ui::SlackDestinationTarget::Conversation { kind, .. }
            if *kind == crate::ui::SlackConversationKind::DirectMessage =>
        {
            candidate.participant_user_ids.first().map(String::as_str)
        }
        _ => None,
    }
}
