mod dm;

use std::{sync::Arc, time::Duration};

use super::{
    SlackConversationLiveTarget, SlackConversationReadRequest, SlackLastReadTimestamp,
    SlackMessageTimestamp, SlackSidebarItem, SLACK_CONVERSATION_READ_DEBOUNCE,
    SLACK_CONVERSATION_REFRESH_INTERVAL,
};
use crate::ui::surface::{
    SlackConversationReadOverlay, SlackSidebarRow, SlackSidebarRowKind, SurfaceState,
};

pub(super) fn slack_conversation_refresh_delay(failures: u8) -> Duration {
    match failures {
        0 => SLACK_CONVERSATION_REFRESH_INTERVAL,
        1 => Duration::from_secs(30),
        2 => Duration::from_secs(60),
        _ => Duration::from_secs(120),
    }
}

pub(super) fn slack_conversation_read_retry_delay(failures: u8) -> Duration {
    match failures {
        0 => SLACK_CONVERSATION_READ_DEBOUNCE,
        1 => Duration::from_secs(15),
        2 => Duration::from_secs(30),
        3 => Duration::from_secs(60),
        _ => Duration::from_secs(120),
    }
}

pub(super) fn slack_conversation_read_at_or_after(
    request: &SlackConversationReadRequest,
    generation: u64,
    target: &SlackConversationLiveTarget,
    timestamp: &SlackMessageTimestamp,
) -> bool {
    request.generation == generation
        && request.target == *target
        && request.message_timestamp.sort_key() >= timestamp.sort_key()
}

pub(super) fn later_slack_message_timestamp(
    left: Option<SlackMessageTimestamp>,
    right: Option<SlackMessageTimestamp>,
) -> Option<SlackMessageTimestamp> {
    match (left, right) {
        (Some(left), Some(right)) if left.sort_key() >= right.sort_key() => Some(left),
        (_, Some(right)) => Some(right),
        (left, None) => left,
    }
}

pub(super) fn advance_slack_last_read(
    current: &mut Option<SlackLastReadTimestamp>,
    confirmed: &SlackLastReadTimestamp,
) -> bool {
    if current
        .as_ref()
        .is_some_and(|current| current.sort_key() >= confirmed.sort_key())
    {
        return false;
    }
    *current = Some(confirmed.clone());
    true
}

fn clear_slack_sidebar_item_unread(item: &mut SlackSidebarItem) -> bool {
    let changed = item.unread || item.count.is_some();
    item.unread = false;
    item.count = None;
    changed
}

impl SurfaceState {
    pub(in crate::ui::surface::state) fn install_slack_conversation_read_overlay(
        &mut self,
        conversation_id: &str,
    ) -> bool {
        if !self.slack_workspace_api_capabilities.mark_conversation_read {
            return false;
        }
        let team_id = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone())
            .or_else(|| {
                self.slack_shell_snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.team_id.clone())
            })
            .or_else(|| self.slack_shell.as_ref().map(|shell| shell.team_id.clone()));
        let Some(team_id) = team_id else {
            return false;
        };
        let Some(read_through) = self.slack_unread_latest_timestamp(&team_id, conversation_id)
        else {
            return false;
        };
        let target = SlackConversationLiveTarget {
            team_id,
            conversation_id: conversation_id.to_string(),
        };
        let overlay = SlackConversationReadOverlay {
            target: target.clone(),
            read_through,
        };
        if self.slack_conversation_read_overlays.get(&target) == Some(&overlay) {
            return false;
        }
        self.slack_conversation_read_overlays
            .insert(target, overlay);
        self.advance_slack_sidebar_read_epoch();
        self.refresh_slack_sidebar_rows();
        self.refresh_slack_dm_rows_for_read_overlay();
        true
    }

    pub(in crate::ui::surface::state) fn clear_slack_conversation_read_overlay(
        &mut self,
        target: &SlackConversationLiveTarget,
    ) -> bool {
        if self
            .slack_conversation_read_overlays
            .remove(target)
            .is_none()
        {
            return false;
        }
        self.advance_slack_sidebar_read_epoch();
        self.refresh_slack_sidebar_rows();
        self.refresh_slack_dm_rows_for_read_overlay();
        true
    }

    pub(in crate::ui::surface::state) fn clear_slack_conversation_read_overlay_for_id(
        &mut self,
        conversation_id: &str,
    ) -> bool {
        let targets = self
            .slack_conversation_read_overlays
            .keys()
            .filter(|target| target.conversation_id == conversation_id)
            .cloned()
            .collect::<Vec<_>>();
        if targets.is_empty() {
            return false;
        }
        for target in targets {
            self.slack_conversation_read_overlays.remove(&target);
        }
        self.advance_slack_sidebar_read_epoch();
        self.refresh_slack_sidebar_rows();
        self.refresh_slack_dm_rows_for_read_overlay();
        true
    }

    pub(in crate::ui::surface::state) fn project_slack_conversation_read_sidebar_rows(
        &self,
        rows: Arc<[SlackSidebarRow]>,
    ) -> Arc<[SlackSidebarRow]> {
        let team_id = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.as_str());
        let selected_conversation_id = self.slack_pending_conversation_id.as_deref();
        if self.slack_conversation_read_overlays.is_empty() && selected_conversation_id.is_none() {
            return rows;
        }
        let authoritative_latest_sort_keys = team_id
            .filter(|_| !self.slack_conversation_read_overlays.is_empty())
            .map(|team_id| self.slack_direct_message_authoritative_latest_sort_keys(team_id))
            .unwrap_or_default();
        if !rows.iter().any(|row| {
            let SlackSidebarRowKind::Item { item, .. } = &row.kind else {
                return false;
            };
            team_id.is_some_and(|team_id| {
                self.slack_read_overlay_covers_sidebar_item(
                    team_id,
                    item,
                    authoritative_latest_sort_keys
                        .get(item.target_id.as_str())
                        .copied(),
                ) && (item.unread || item.count.is_some())
            }) || selected_conversation_id
                .is_some_and(|conversation_id| item.active != (item.target_id == conversation_id))
        }) {
            return rows;
        }
        let mut projected = rows.to_vec();
        for row in &mut projected {
            let SlackSidebarRowKind::Item { item, .. } = &mut row.kind else {
                continue;
            };
            if let Some(conversation_id) = selected_conversation_id {
                item.active = item.target_id == conversation_id;
            }
            if team_id.is_some_and(|team_id| {
                self.slack_read_overlay_covers_sidebar_item(
                    team_id,
                    item,
                    authoritative_latest_sort_keys
                        .get(item.target_id.as_str())
                        .copied(),
                )
            }) {
                clear_slack_sidebar_item_unread(item);
            }
        }
        SlackSidebarRow::refresh_boundary_state(&mut projected);
        Arc::from(projected)
    }

    fn slack_read_overlay_covers_sidebar_item(
        &self,
        team_id: &str,
        item: &SlackSidebarItem,
        authoritative_latest_sort_key: Option<(u64, u32)>,
    ) -> bool {
        let Some(latest_sort_key) = item
            .latest_message_timestamp
            .as_ref()
            .map(SlackMessageTimestamp::sort_key)
            .into_iter()
            .chain(authoritative_latest_sort_key)
            .max()
        else {
            return false;
        };
        self.slack_conversation_read_overlays
            .values()
            .any(|overlay| {
                overlay.target.team_id == team_id
                    && overlay.target.conversation_id == item.target_id
                    && latest_sort_key <= overlay.read_through.sort_key()
            })
    }

    pub(in crate::ui::surface::state) fn advance_slack_sidebar_read_epoch(&mut self) {
        self.slack_sidebar_read_epoch = self
            .slack_sidebar_read_epoch
            .checked_add(1)
            .expect("Slack sidebar read epoch overflowed");
    }
}
