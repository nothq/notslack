use super::{
    slack_aux_panel_matches_query, slack_normalized_aux_panel_query, SlackAuxPanelRow,
    SlackAuxPanelRowAction, SlackAuxPanelSection, SurfaceState, SLACK_SPECIAL_MENTIONS,
};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn slack_mention_sections(
        &self,
        query: &str,
    ) -> Vec<SlackAuxPanelSection> {
        let normalized_query = slack_normalized_aux_panel_query(query);
        if let Some(sections) = self.slack_workspace_mention_sections(&normalized_query) {
            return sections;
        }
        self.slack_default_mention_sections(&normalized_query)
    }

    pub(in crate::ui::surface::state) fn slack_workspace_mention_sections(
        &self,
        normalized_query: &str,
    ) -> Option<Vec<SlackAuxPanelSection>> {
        let workspace = self.slack_workspace()?;
        if workspace.mention_suggestions.is_empty() {
            return None;
        }
        let rows = workspace
            .mention_suggestions
            .iter()
            .filter(|row| {
                slack_aux_panel_matches_query(normalized_query, &row.label)
                    || row.detail.as_deref().is_some_and(|detail| {
                        slack_aux_panel_matches_query(normalized_query, detail)
                    })
                    || row.accessory.as_deref().is_some_and(|accessory| {
                        slack_aux_panel_matches_query(normalized_query, accessory)
                    })
            })
            .map(|row| {
                Self::slack_aux_row(
                    row.label.clone(),
                    row.detail.clone(),
                    row.accessory.clone(),
                    false,
                    None,
                )
            })
            .collect::<Vec<_>>();
        Some(if rows.is_empty() {
            vec![SlackAuxPanelSection {
                title: None,
                rows: vec![Self::slack_aux_label_row(
                    "No people or mentions match this query.",
                )],
            }]
        } else {
            vec![SlackAuxPanelSection {
                title: Some("People".to_string()),
                rows,
            }]
        })
    }

    pub(in crate::ui::surface::state) fn slack_default_mention_sections(
        &self,
        normalized_query: &str,
    ) -> Vec<SlackAuxPanelSection> {
        let special_rows = self.slack_special_mention_rows(normalized_query);
        let people_rows = self.slack_visible_mention_rows(normalized_query);
        let mut sections = Vec::new();
        if !special_rows.is_empty() {
            sections.push(SlackAuxPanelSection {
                title: Some("Special mentions".to_string()),
                rows: special_rows,
            });
        }
        if !people_rows.is_empty() {
            sections.push(SlackAuxPanelSection {
                title: Some("People".to_string()),
                rows: people_rows,
            });
        }
        if sections.is_empty() {
            sections.push(SlackAuxPanelSection {
                title: None,
                rows: vec![Self::slack_aux_label_row(if normalized_query.is_empty() {
                    "No visible people available."
                } else {
                    "No people or mentions match this query."
                })],
            });
        }
        sections
    }

    pub(in crate::ui::surface::state) fn slack_special_mention_rows(
        &self,
        normalized_query: &str,
    ) -> Vec<SlackAuxPanelRow> {
        SLACK_SPECIAL_MENTIONS
            .into_iter()
            .filter(|(range, detail)| {
                let label = format!("@{}", range.label());
                slack_aux_panel_matches_query(normalized_query, &label)
                    || slack_aux_panel_matches_query(normalized_query, detail)
            })
            .map(|(range, detail)| {
                let label = format!("@{}", range.label());
                Self::slack_aux_row(
                    label,
                    Some(detail.to_string()),
                    Some("Special".to_string()),
                    false,
                    Some(SlackAuxPanelRowAction::InsertComposerBroadcastMention(
                        range,
                    )),
                )
            })
            .collect()
    }

    pub(in crate::ui::surface::state) fn slack_visible_mention_rows(
        &self,
        normalized_query: &str,
    ) -> Vec<SlackAuxPanelRow> {
        self.slack_visible_people_rows()
            .into_iter()
            .filter(|row| {
                slack_aux_panel_matches_query(normalized_query, &row.label)
                    || row.detail.as_deref().is_some_and(|detail| {
                        slack_aux_panel_matches_query(normalized_query, detail)
                    })
            })
            .map(|row| {
                Self::slack_aux_row(
                    row.label.clone(),
                    row.detail,
                    row.accessory,
                    false,
                    Some(SlackAuxPanelRowAction::InsertComposerUserMention {
                        user_id: row.user_id,
                        label: row.label,
                    }),
                )
            })
            .collect()
    }
}
