use std::{collections::HashMap, sync::Arc};

use super::later_slack_message_timestamp;
use crate::ui::surface::{
    build_slack_dm_rows, SlackConversationReadOverlay, SlackDmRow, SurfaceState,
};
use crate::ui::{
    SlackConversationKind, SlackDirectMessageUnreadState, SlackMessageTimestamp, SlackWorkspace,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn project_slack_conversation_read_dm_rows(
        &self,
        rows: Arc<[SlackDmRow]>,
    ) -> Arc<[SlackDmRow]> {
        let Some(dm_snapshot) = self.slack_dm_inbox_snapshot.as_ref() else {
            return rows;
        };
        let team_id = dm_snapshot.team_id.as_str();
        if self.slack_conversation_read_overlays.is_empty() {
            return rows;
        }
        let authoritative_latest_sort_keys =
            self.slack_direct_message_authoritative_latest_sort_keys(team_id);
        if !rows.iter().any(|row| {
            self.slack_read_overlay_covers_dm_row(
                team_id,
                row,
                authoritative_latest_sort_keys
                    .get(row.conversation_id.as_ref())
                    .copied(),
            ) && (row.unread || row.mention_count.is_some())
        }) {
            return rows;
        }
        rows.iter()
            .cloned()
            .map(|mut row| {
                if self.slack_read_overlay_covers_dm_row(
                    team_id,
                    &row,
                    authoritative_latest_sort_keys
                        .get(row.conversation_id.as_ref())
                        .copied(),
                ) {
                    row.unread = false;
                    row.mention_count = None;
                }
                row
            })
            .collect::<Vec<_>>()
            .into()
    }

    pub(in crate::ui::surface::state) fn refresh_slack_dm_rows_for_read_overlay(&mut self) {
        let Some(snapshot) = self.slack_dm_inbox_snapshot.as_ref() else {
            return;
        };
        let rows = build_slack_dm_rows(snapshot);
        let mut rows = self.project_slack_conversation_read_dm_rows(rows);
        self.slack_presence_authority.overlay_dm_rows(&mut rows);
        self.slack_dm_rows = rows;
        {
            let (authority, data) = (&mut self.slack_presence_authority, &self.data);
            authority.reindex_dm(data);
        }
        self.refresh_slack_dm_visible_rows();
    }

    pub(crate) fn slack_conversation_read_overlay_dm_badges(
        &self,
        workspace: &SlackWorkspace,
    ) -> (Option<u32>, Option<u32>) {
        if self.slack_conversation_read_overlays.is_empty() {
            return (
                workspace.rail_badges.dms,
                workspace.rail_badges.dms_unread_messages,
            );
        }
        let (unread_dm_count, dm_display_count) =
            self.slack_conversation_read_overlay_dm_badge_counts(workspace);
        (
            subtract_slack_badge_value(workspace.rail_badges.dms, unread_dm_count),
            subtract_slack_badge_value(workspace.rail_badges.dms_unread_messages, dm_display_count),
        )
    }

    fn slack_conversation_read_overlay_dm_badge_counts(
        &self,
        workspace: &SlackWorkspace,
    ) -> (u32, u32) {
        let authoritative_latest_sort_keys =
            self.slack_direct_message_authoritative_latest_sort_keys(&workspace.team_id);
        self.slack_conversation_read_overlays
            .values()
            .filter(|overlay| overlay.target.team_id == workspace.team_id)
            .map(|overlay| {
                self.slack_conversation_read_overlay_dm_badge_count(
                    workspace,
                    overlay,
                    &authoritative_latest_sort_keys,
                )
            })
            .fold((0_u32, 0_u32), |counts, next| {
                (
                    counts.0.saturating_add(next.0),
                    counts.1.saturating_add(next.1),
                )
            })
    }

    fn slack_conversation_read_overlay_dm_badge_count(
        &self,
        workspace: &SlackWorkspace,
        overlay: &SlackConversationReadOverlay,
        authoritative_latest_sort_keys: &HashMap<&str, (u64, u32)>,
    ) -> (u32, u32) {
        if let Some(state) = workspace
            .direct_message_unread_states
            .iter()
            .find(|state| state.conversation_id == overlay.target.conversation_id)
        {
            let covered = authoritative_latest_sort_keys
                .get(overlay.target.conversation_id.as_str())
                .copied()
                .is_some_and(|latest| latest <= overlay.read_through.sort_key());
            return if covered {
                (
                    u32::from(state.unread),
                    state.display_count.unwrap_or_default(),
                )
            } else {
                (0, 0)
            };
        }
        let (unread, display_count) = self.slack_fallback_dm_badge_state(workspace, overlay);
        (u32::from(unread), display_count)
    }

    fn slack_fallback_dm_badge_state(
        &self,
        workspace: &SlackWorkspace,
        overlay: &SlackConversationReadOverlay,
    ) -> (bool, u32) {
        let mut unread = false;
        let mut display_count = 0_u32;
        for item in workspace
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .filter(|item| item.target_id == overlay.target.conversation_id)
            .filter(|item| {
                matches!(
                    item.target_kind,
                    SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
                ) && item
                    .latest_message_timestamp
                    .as_ref()
                    .is_some_and(|latest| latest.sort_key() <= overlay.read_through.sort_key())
            })
        {
            unread |= item.unread;
            display_count = display_count.max(item.count.unwrap_or_default());
        }
        if let Some(item) = self
            .slack_dm_inbox_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.team_id == overlay.target.team_id)
            .and_then(|snapshot| {
                snapshot
                    .items
                    .iter()
                    .find(|item| item.conversation_id == overlay.target.conversation_id)
            })
            .filter(|item| item.latest_timestamp.sort_key() <= overlay.read_through.sort_key())
        {
            unread |= item.unread;
            display_count = display_count.max(item.mention_count.unwrap_or_default());
        }
        (unread, display_count)
    }

    pub(super) fn slack_unread_latest_timestamp(
        &self,
        team_id: &str,
        conversation_id: &str,
    ) -> Option<SlackMessageTimestamp> {
        let direct_message_state = self.slack_direct_message_unread_state(team_id, conversation_id);
        let dm_latest = self.slack_dm_unread_latest_timestamp(team_id, conversation_id);
        if let Some(state) = direct_message_state {
            if !state.unread && state.display_count.is_none() {
                return None;
            }
            return later_slack_message_timestamp(
                state.latest_message_timestamp.clone(),
                dm_latest,
            );
        }
        later_slack_message_timestamp(
            self.slack_sidebar_unread_latest_timestamp(team_id, conversation_id),
            dm_latest,
        )
    }

    fn slack_direct_message_unread_state<'a>(
        &'a self,
        team_id: &str,
        conversation_id: &str,
    ) -> Option<&'a SlackDirectMessageUnreadState> {
        self.slack_sidebar_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.team_id == team_id)
            .map(|snapshot| snapshot.direct_message_unread_states.as_slice())
            .or_else(|| {
                self.slack_workspace()
                    .filter(|workspace| workspace.team_id == team_id)
                    .map(|workspace| workspace.direct_message_unread_states.as_slice())
            })?
            .iter()
            .find(|state| state.conversation_id == conversation_id)
    }

    fn slack_dm_unread_latest_timestamp(
        &self,
        team_id: &str,
        conversation_id: &str,
    ) -> Option<SlackMessageTimestamp> {
        self.slack_dm_inbox_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.team_id == team_id)
            .and_then(|snapshot| {
                snapshot.items.iter().find(|item| {
                    item.conversation_id == conversation_id
                        && (item.unread || item.mention_count.is_some())
                })
            })
            .map(|item| item.effective_unread_latest_timestamp().clone())
    }

    fn slack_sidebar_unread_latest_timestamp(
        &self,
        team_id: &str,
        conversation_id: &str,
    ) -> Option<SlackMessageTimestamp> {
        self.slack_sidebar_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.team_id == team_id)
            .map(|snapshot| snapshot.sections.as_slice())
            .or_else(|| {
                self.slack_workspace()
                    .filter(|workspace| workspace.team_id == team_id)
                    .map(|workspace| workspace.sections.as_slice())
            })
            .into_iter()
            .flatten()
            .flat_map(|section| section.items.iter())
            .filter(|item| {
                item.target_id == conversation_id && (item.unread || item.count.is_some())
            })
            .filter_map(|item| item.latest_message_timestamp.clone())
            .fold(None, |latest, timestamp| {
                later_slack_message_timestamp(latest, Some(timestamp))
            })
    }

    pub(super) fn slack_direct_message_authoritative_latest_sort_keys<'a>(
        &'a self,
        team_id: &str,
    ) -> HashMap<&'a str, (u64, u32)> {
        let mut latest_sort_keys = self
            .slack_dm_inbox_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.team_id == team_id)
            .into_iter()
            .flat_map(|snapshot| snapshot.items.iter())
            .map(|item| {
                (
                    item.conversation_id.as_str(),
                    item.effective_unread_latest_timestamp().sort_key(),
                )
            })
            .collect::<HashMap<_, _>>();
        for state in self.slack_direct_message_unread_states(team_id) {
            let Some(latest) = state.latest_message_timestamp.as_ref() else {
                continue;
            };
            latest_sort_keys
                .entry(state.conversation_id.as_str())
                .and_modify(|current| *current = (*current).max(latest.sort_key()))
                .or_insert_with(|| latest.sort_key());
        }
        latest_sort_keys
    }

    fn slack_direct_message_unread_states<'a>(
        &'a self,
        team_id: &str,
    ) -> &'a [SlackDirectMessageUnreadState] {
        self.slack_sidebar_snapshot
            .as_ref()
            .filter(|snapshot| snapshot.team_id == team_id)
            .map(|snapshot| snapshot.direct_message_unread_states.as_slice())
            .or_else(|| {
                self.slack_workspace()
                    .filter(|workspace| workspace.team_id == team_id)
                    .map(|workspace| workspace.direct_message_unread_states.as_slice())
            })
            .unwrap_or_default()
    }

    fn slack_read_overlay_covers_dm_row(
        &self,
        team_id: &str,
        row: &SlackDmRow,
        authoritative_latest_sort_key: Option<(u64, u32)>,
    ) -> bool {
        let latest_sort_key = authoritative_latest_sort_key
            .unwrap_or_else(|| row.latest_message_timestamp.sort_key())
            .max(row.latest_message_timestamp.sort_key());
        self.slack_conversation_read_overlays
            .values()
            .any(|overlay| {
                overlay.target.team_id == team_id
                    && overlay.target.conversation_id == row.conversation_id.as_ref()
                    && latest_sort_key <= overlay.read_through.sort_key()
            })
    }
}

fn subtract_slack_badge_value(value: Option<u32>, amount: u32) -> Option<u32> {
    value
        .map(|value| value.saturating_sub(amount))
        .filter(|value| *value > 0)
}
