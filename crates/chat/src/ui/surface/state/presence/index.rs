use std::collections::HashMap;

use crate::{
    model::SlackSidebarSection,
    ui::surface::{SlackSidebarRow, SlackSidebarRowKind, SurfaceStateData},
};

use super::{valid_presence_user_id, SlackPresenceAuthority};

pub(super) type Positions = HashMap<String, Vec<usize>>;
pub(super) type NestedPositions = HashMap<String, Vec<(usize, usize)>>;

#[derive(Default)]
pub(super) struct SlackPresenceProjectionIndexes {
    pub(super) sidebar_sections: NestedPositions,
    pub(super) sidebar_rows: Positions,
    pub(super) dm_snapshot: NestedPositions,
    pub(super) dm_rows: NestedPositions,
    pub(super) all_threads: Positions,
    pub(super) members: Positions,
    pub(super) directory: Positions,
    pub(super) new_message: Positions,
    pub(super) forward: Positions,
}

impl SlackPresenceAuthority {
    pub(in crate::ui::surface) fn reindex_workspace(&mut self, data: &SurfaceStateData) {
        self.reindex_sidebar(data);
    }

    pub(in crate::ui::surface) fn reindex_sidebar(&mut self, data: &SurfaceStateData) {
        self.indexes.sidebar_sections = data
            .slack_sidebar_snapshot
            .as_ref()
            .map(|snapshot| index_sections(&snapshot.sections))
            .unwrap_or_default();
        self.indexes.sidebar_rows = index_sidebar_rows(&data.slack_sidebar_rows);
    }

    pub(in crate::ui::surface) fn reindex_dm(&mut self, data: &SurfaceStateData) {
        self.indexes.dm_snapshot = data
            .slack_dm_inbox_snapshot
            .as_ref()
            .map(|snapshot| {
                index_nested(snapshot.items.iter().map(|item| {
                    item.participants
                        .iter()
                        .map(|participant| participant.user_id.as_str())
                }))
            })
            .unwrap_or_default();
        self.indexes.dm_rows = index_nested(data.slack_dm_rows.iter().map(|row| {
            row.participants
                .iter()
                .map(|participant| participant.user_id.as_ref())
        }));
    }

    pub(in crate::ui::surface) fn reindex_all_threads(&mut self, data: &SurfaceStateData) {
        self.indexes.all_threads = index_optional_ids(
            data.slack_all_threads_rows
                .iter()
                .map(|row| row.direct_message_user_id.as_deref()),
        );
    }

    pub(in crate::ui::surface) fn reindex_members(&mut self, data: &SurfaceStateData) {
        self.indexes.members = index_ids(
            data.slack_members_rows
                .iter()
                .map(|row| row.user_id.as_ref()),
        );
    }

    pub(in crate::ui::surface) fn reindex_new_message(&mut self, data: &SurfaceStateData) {
        self.indexes.new_message = index_optional_ids(
            data.slack_new_message_rows
                .iter()
                .map(|row| row.presence_user_id.as_deref()),
        );
    }

    pub(in crate::ui::surface) fn reindex_directory(&mut self, data: &SurfaceStateData) {
        self.indexes.directory = index_optional_ids(
            data.slack_directory_rows
                .iter()
                .map(|row| row.presence_user_id.as_deref()),
        );
    }

    pub(in crate::ui::surface) fn reindex_forward(&mut self, data: &SurfaceStateData) {
        self.indexes.forward = data
            .slack_message_forward_modal
            .as_ref()
            .map(|modal| {
                index_optional_ids(modal.rows.iter().map(|row| row.presence_user_id.as_deref()))
            })
            .unwrap_or_default();
    }

    pub(super) fn reindex_all(&mut self, data: &SurfaceStateData) {
        self.reindex_workspace(data);
        self.reindex_dm(data);
        self.reindex_all_threads(data);
        self.reindex_members(data);
        self.reindex_directory(data);
        self.reindex_new_message(data);
        self.reindex_forward(data);
    }
}

fn index_sections(sections: &[SlackSidebarSection]) -> NestedPositions {
    index_nested(sections.iter().map(|section| {
        section
            .items
            .iter()
            .map(|item| item.user_id.as_deref().unwrap_or_default())
    }))
}

fn index_sidebar_rows(rows: &[SlackSidebarRow]) -> Positions {
    index_optional_ids(rows.iter().map(|row| match &row.kind {
        SlackSidebarRowKind::Item { item, .. } => item.user_id.as_deref(),
        _ => None,
    }))
}

fn index_ids<'a>(ids: impl Iterator<Item = &'a str>) -> Positions {
    index_optional_ids(ids.map(Some))
}

fn index_optional_ids<'a>(ids: impl Iterator<Item = Option<&'a str>>) -> Positions {
    let mut positions = Positions::new();
    for (index, user_id) in ids.enumerate() {
        let Some(user_id) = user_id.filter(|user_id| valid_presence_user_id(user_id)) else {
            continue;
        };
        positions
            .entry(user_id.to_string())
            .or_default()
            .push(index);
    }
    positions
}

fn index_nested<'a, I>(rows: impl Iterator<Item = I>) -> NestedPositions
where
    I: Iterator<Item = &'a str>,
{
    let mut positions = NestedPositions::new();
    for (row_index, row) in rows.enumerate() {
        for (item_index, user_id) in row.enumerate() {
            if !valid_presence_user_id(user_id) {
                continue;
            }
            positions
                .entry(user_id.to_string())
                .or_default()
                .push((row_index, item_index));
        }
    }
    positions
}
