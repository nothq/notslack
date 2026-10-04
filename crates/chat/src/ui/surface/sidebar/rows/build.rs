use std::collections::HashSet;
use std::sync::Arc;

use gpui::SharedString;

use super::{SlackSidebarRow, SlackSidebarRowKind, SlackSidebarSectionIndicator};
use crate::ui::surface::normalize_slack_dm_finder_text;
use crate::ui::surface::{
    SlackSidebarBoundaryDirection, SlackSidebarBoundaryLabel, SlackSidebarBoundaryTarget,
};
use crate::ui::SlackConversationKind;
use crate::ui::SlackSidebarItem;
use crate::ui::SlackSidebarSnapshot;
use crate::ui::SlackWorkspace;

pub(crate) fn build_slack_sidebar_rows(
    workspace: Option<&SlackWorkspace>,
    collapsed_sections: &HashSet<String>,
) -> Arc<[SlackSidebarRow]> {
    workspace
        .map(|workspace| {
            Arc::<[SlackSidebarRow]>::from(build_generic_sidebar_rows(
                workspace.rail_badges.drafts_sent,
                &workspace.sections,
                workspace.channel_kind,
                collapsed_sections,
            ))
        })
        .unwrap_or_else(|| Arc::<[SlackSidebarRow]>::from(Vec::<SlackSidebarRow>::new()))
}

pub(crate) fn build_slack_sidebar_snapshot_rows(
    sidebar: &SlackSidebarSnapshot,
    collapsed_sections: &HashSet<String>,
) -> Arc<[SlackSidebarRow]> {
    Arc::from(build_generic_sidebar_rows(
        sidebar.rail_badges.drafts_sent,
        &sidebar.sections,
        sidebar.active_conversation_kind,
        collapsed_sections,
    ))
}

fn sidebar_section_indicator(label: &str) -> SlackSidebarSectionIndicator {
    let label = label.trim();
    if label.eq_ignore_ascii_case("Starred") {
        SlackSidebarSectionIndicator::Star
    } else if label.eq_ignore_ascii_case("External connections") {
        SlackSidebarSectionIndicator::ExternalConnections
    } else if label.eq_ignore_ascii_case("Channels") {
        SlackSidebarSectionIndicator::Channels
    } else if label.eq_ignore_ascii_case("Direct messages") {
        SlackSidebarSectionIndicator::DirectMessages
    } else if label.eq_ignore_ascii_case("Apps") {
        SlackSidebarSectionIndicator::Apps
    } else {
        SlackSidebarSectionIndicator::Chevron
    }
}

fn sidebar_section_label(label: &str) -> String {
    if label.trim().eq_ignore_ascii_case("Direct messages") {
        "Direct messages".to_string()
    } else {
        label.to_string()
    }
}

fn build_generic_sidebar_rows(
    drafts_sent: Option<u32>,
    sections: &[crate::model::SlackSidebarSection],
    active_conversation_kind: SlackConversationKind,
    collapsed_sections: &HashSet<String>,
) -> Vec<SlackSidebarRow> {
    let mut rows = slack_default_sidebar_rows(drafts_sent);
    push_slack_starred_rows(&mut rows, sections, collapsed_sections);
    push_existing_sidebar_sections(
        &mut rows,
        sections,
        active_conversation_kind,
        collapsed_sections,
    );
    SlackSidebarRow::refresh_boundary_state(&mut rows);
    rows
}

impl SlackSidebarRow {
    pub(crate) fn refresh_boundary_state(rows: &mut [Self]) {
        let mut last_mention = None;
        let mut last_unread = None;
        for (index, row) in rows.iter_mut().enumerate() {
            match row.boundary_label() {
                Some(SlackSidebarBoundaryLabel::UnreadMentions) => last_mention = Some(index),
                Some(SlackSidebarBoundaryLabel::MoreUnreads) => last_unread = Some(index),
                None => {}
            }
            row.boundary_state.last_target_at_or_before = last_mention
                .map(|row_index| SlackSidebarBoundaryTarget {
                    row_index,
                    direction: SlackSidebarBoundaryDirection::Above,
                    label: SlackSidebarBoundaryLabel::UnreadMentions,
                })
                .or_else(|| {
                    last_unread.map(|row_index| SlackSidebarBoundaryTarget {
                        row_index,
                        direction: SlackSidebarBoundaryDirection::Above,
                        label: SlackSidebarBoundaryLabel::MoreUnreads,
                    })
                });
        }

        let mut next_mention = None;
        let mut next_unread = None;
        for (index, row) in rows.iter_mut().enumerate().rev() {
            match row.boundary_label() {
                Some(SlackSidebarBoundaryLabel::UnreadMentions) => next_mention = Some(index),
                Some(SlackSidebarBoundaryLabel::MoreUnreads) => next_unread = Some(index),
                None => {}
            }
            row.boundary_state.next_target_at_or_after = next_mention
                .map(|row_index| SlackSidebarBoundaryTarget {
                    row_index,
                    direction: SlackSidebarBoundaryDirection::Below,
                    label: SlackSidebarBoundaryLabel::UnreadMentions,
                })
                .or_else(|| {
                    next_unread.map(|row_index| SlackSidebarBoundaryTarget {
                        row_index,
                        direction: SlackSidebarBoundaryDirection::Below,
                        label: SlackSidebarBoundaryLabel::MoreUnreads,
                    })
                });
        }
    }

    fn boundary_label(&self) -> Option<SlackSidebarBoundaryLabel> {
        let SlackSidebarRowKind::Item { item, .. } = &self.kind else {
            return None;
        };
        if item.active
            || !matches!(
                item.target_kind,
                SlackConversationKind::Channel
                    | SlackConversationKind::PrivateChannel
                    | SlackConversationKind::DirectMessage
                    | SlackConversationKind::GroupMessage
            )
        {
            return None;
        }
        if item.count.is_some_and(|count| count > 0) {
            Some(SlackSidebarBoundaryLabel::UnreadMentions)
        } else if item.unread {
            Some(SlackSidebarBoundaryLabel::MoreUnreads)
        } else {
            None
        }
    }

    #[cfg(test)]
    fn contributes_unread(&self) -> bool {
        self.boundary_label().is_some()
    }
}

#[cfg(test)]
pub(crate) fn shows_slack_more_unreads_above_pill(
    rows: &[SlackSidebarRow],
    is_row_above_viewport: impl Fn(usize) -> bool,
) -> bool {
    rows.iter()
        .enumerate()
        .any(|(index, row)| is_row_above_viewport(index) && row.contributes_unread())
}

#[cfg(test)]
pub(crate) fn shows_slack_more_unreads_below_pill(
    rows: &[SlackSidebarRow],
    is_row_below_viewport: impl Fn(usize) -> bool,
) -> bool {
    rows.iter()
        .enumerate()
        .any(|(index, row)| is_row_below_viewport(index) && row.contributes_unread())
}

fn push_slack_starred_rows(
    rows: &mut Vec<SlackSidebarRow>,
    sections: &[crate::model::SlackSidebarSection],
    collapsed_sections: &HashSet<String>,
) {
    let starred_items = sections
        .iter()
        .find(|section| section.label == "Starred")
        .map(|section| section.items.clone())
        .unwrap_or_default();

    push_sidebar_section_header(rows, "Starred", SlackSidebarSectionIndicator::Star);
    if collapsed_sections.contains("Starred") {
        return;
    }
    if starred_items.is_empty() {
        rows.push(SlackSidebarRow {
            kind: SlackSidebarRowKind::DropHint,
            boundary_state: Default::default(),
        });
    } else {
        push_sidebar_items(rows, starred_items.into_iter());
    }
}

fn slack_default_sidebar_rows(drafts_sent: Option<u32>) -> Vec<SlackSidebarRow> {
    vec![
        SlackSidebarRow {
            kind: SlackSidebarRowKind::Shortcut {
                label: "Threads".to_string(),
                badge: None,
            },
            boundary_state: Default::default(),
        },
        SlackSidebarRow {
            kind: SlackSidebarRowKind::Shortcut {
                label: "Huddles".to_string(),
                badge: None,
            },
            boundary_state: Default::default(),
        },
        SlackSidebarRow {
            kind: SlackSidebarRowKind::Shortcut {
                label: "Drafts & sent".to_string(),
                badge: drafts_sent
                    .filter(|count| *count > 0)
                    .map(|count| count.to_string()),
            },
            boundary_state: Default::default(),
        },
        SlackSidebarRow {
            kind: SlackSidebarRowKind::Shortcut {
                label: "Directories".to_string(),
                badge: None,
            },
            boundary_state: Default::default(),
        },
        slack_separator_row(),
        SlackSidebarRow {
            kind: SlackSidebarRowKind::Shortcut {
                label: "Create a section".to_string(),
                badge: Some("Tip".to_string()),
            },
            boundary_state: Default::default(),
        },
    ]
}

fn push_existing_sidebar_sections(
    rows: &mut Vec<SlackSidebarRow>,
    sections: &[crate::model::SlackSidebarSection],
    active_conversation_kind: SlackConversationKind,
    collapsed_sections: &HashSet<String>,
) {
    for section in sections.iter().filter(|section| section.label != "Starred") {
        push_sidebar_section_header(
            rows,
            &section.label,
            sidebar_section_indicator(&section.label),
        );
        if collapsed_sections.contains(section.label.as_str()) {
            continue;
        }
        push_sidebar_items(rows, section.items.iter().cloned());
        push_add_channels_if_needed(rows, active_conversation_kind, section);
    }
}

fn push_add_channels_if_needed(
    rows: &mut Vec<SlackSidebarRow>,
    active_conversation_kind: SlackConversationKind,
    section: &crate::model::SlackSidebarSection,
) {
    if !matches!(
        active_conversation_kind,
        SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
    ) || !section.label.is_empty()
        || !section
            .items
            .iter()
            .all(|item| item.target_kind.is_channel())
    {
        return;
    }
    rows.push(slack_spacer_row(28.0));
}

fn slack_separator_row() -> SlackSidebarRow {
    SlackSidebarRow {
        kind: SlackSidebarRowKind::Separator,
        boundary_state: Default::default(),
    }
}

fn slack_spacer_row(height: f32) -> SlackSidebarRow {
    SlackSidebarRow {
        kind: SlackSidebarRowKind::Spacer { height },
        boundary_state: Default::default(),
    }
}

fn push_sidebar_section_header(
    rows: &mut Vec<SlackSidebarRow>,
    label: &str,
    indicator: SlackSidebarSectionIndicator,
) {
    let label = sidebar_section_label(label);
    rows.push(SlackSidebarRow {
        kind: SlackSidebarRowKind::SectionHeader {
            label,
            indicator,
            collapsible: true,
        },
        boundary_state: Default::default(),
    });
}

fn push_sidebar_items(
    rows: &mut Vec<SlackSidebarRow>,
    items: impl Iterator<Item = SlackSidebarItem>,
) {
    rows.extend(items.map(|item| {
        let label = SharedString::from(item.label.clone());
        let secondary_context = item
            .secondary_context
            .as_ref()
            .map(|context| SharedString::from(context.clone()));
        let finder_element_id = format!("slack-home-finder-row-{}", item.target_id).into();
        let finder_accessibility_label = item.label.clone().into();
        let finder_search_key = sidebar_item_finder_search_key(&item).into();
        let finder_group_count_label = (item.target_kind == SlackConversationKind::GroupMessage)
            .then(|| {
                item.label
                    .split(',')
                    .map(str::trim)
                    .filter(|segment| !segment.is_empty())
                    .count()
                    .max(2)
                    .to_string()
                    .into()
            });
        SlackSidebarRow {
            kind: SlackSidebarRowKind::Item {
                item: Box::new(item),
                label,
                secondary_context,
                finder_element_id,
                finder_accessibility_label,
                finder_search_key,
                finder_group_count_label,
            },
            boundary_state: Default::default(),
        }
    }));
}

fn sidebar_item_finder_search_key(item: &SlackSidebarItem) -> String {
    let mut searchable = String::with_capacity(
        item.label.len()
            + item
                .secondary_context
                .as_ref()
                .map_or(0, |context| context.len() + 1),
    );
    searchable.push_str(&item.label);
    if let Some(context) = item.secondary_context.as_deref() {
        searchable.push(' ');
        searchable.push_str(context);
    }
    normalize_slack_dm_finder_text(&searchable)
}
