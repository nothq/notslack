use std::collections::HashSet;
use std::sync::Arc;

use crate::ui::surface::SlackDraftsSentRow;
use crate::ui::{SlackDraftsSentSnapshot, SlackDraftsSentTab};

pub(in crate::ui::surface::state) fn slack_drafts_sent_tab_index(tab: SlackDraftsSentTab) -> usize {
    match tab {
        SlackDraftsSentTab::Drafts => 0,
        SlackDraftsSentTab::Scheduled => 1,
        SlackDraftsSentTab::Sent => 2,
    }
}

pub(in crate::ui::surface::state) fn merge_slack_drafts_sent_snapshots(
    existing: Option<SlackDraftsSentSnapshot>,
    page: SlackDraftsSentSnapshot,
) -> SlackDraftsSentSnapshot {
    let Some(existing) = existing else {
        return page;
    };
    assert_eq!(existing.team_id, page.team_id);
    assert_eq!(existing.tab, page.tab);
    let mut ids = existing
        .items
        .iter()
        .map(|item| item.id.clone())
        .collect::<HashSet<_>>();
    let mut items = existing.items;
    items.extend(
        page.items
            .into_iter()
            .filter(|item| ids.insert(item.id.clone())),
    );
    SlackDraftsSentSnapshot {
        team_id: page.team_id,
        tab: page.tab,
        items,
        next_cursor: page.next_cursor,
    }
}

pub(in crate::ui::surface::state) fn append_slack_drafts_sent_rows(
    existing: &[SlackDraftsSentRow],
    page: &[SlackDraftsSentRow],
) -> Arc<[SlackDraftsSentRow]> {
    let mut element_ids = existing
        .iter()
        .map(|row| match row {
            SlackDraftsSentRow::DateDivider { element_id, .. } => element_id.clone(),
            SlackDraftsSentRow::Item(row) => row.element_id.clone(),
        })
        .collect::<HashSet<_>>();
    let mut additions = page
        .iter()
        .filter_map(|row| {
            let element_id = match row {
                SlackDraftsSentRow::DateDivider { element_id, .. } => element_id,
                SlackDraftsSentRow::Item(row) => &row.element_id,
            };
            element_ids.insert(element_id.clone()).then(|| row.clone())
        })
        .collect::<Vec<_>>();
    let mut rows = existing.to_vec();
    let existing_last_item = rows
        .iter()
        .rposition(|row| matches!(row, SlackDraftsSentRow::Item(_)));
    let addition_first_item = additions
        .iter()
        .position(|row| matches!(row, SlackDraftsSentRow::Item(_)));
    if let (Some(existing_index), Some(addition_index)) = (existing_last_item, addition_first_item)
    {
        let same_sent_date = match (&rows[existing_index], &additions[addition_index]) {
            (SlackDraftsSentRow::Item(existing), SlackDraftsSentRow::Item(addition)) => {
                existing.sent_date_key.is_some() && existing.sent_date_key == addition.sent_date_key
            }
            _ => false,
        };
        if same_sent_date {
            if let SlackDraftsSentRow::Item(existing) = &mut rows[existing_index] {
                Arc::make_mut(existing).sent_group_last = false;
            }
            if let SlackDraftsSentRow::Item(addition) = &mut additions[addition_index] {
                Arc::make_mut(addition).sent_group_first = false;
            }
        }
    }
    rows.extend(additions);
    rows.into()
}

pub(in crate::ui::surface::state) fn next_slack_drafts_sent_generation(generation: u64) -> u64 {
    generation
        .checked_add(1)
        .expect("Slack Drafts & sent request generation overflowed")
}
