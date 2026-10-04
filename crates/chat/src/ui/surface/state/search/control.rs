use gpui::Context;

use crate::ui::surface::SurfaceState;

impl SurfaceState {
    pub(crate) fn open_slack_search_results(&mut self, query: &str, cx: &mut Context<Self>) {
        self.set_slack_search_query(query.to_string(), cx);
        self.activate_slack_search_query(cx);
    }

    pub(crate) fn control_search_slack_messages(
        &mut self,
        query: &str,
        cx: &mut Context<Self>,
    ) -> Result<String, String> {
        if !self.slack_workspace_api_capabilities.search_messages {
            return Err("Slack message search is unavailable".to_string());
        }
        let query = query.trim();
        if query.is_empty() {
            return Err("Slack message search requires a query".to_string());
        }
        self.open_slack_search_results(query, cx);
        Ok(query.to_string())
    }

    pub(crate) fn control_slack_search_state(&self) -> crate::model::ChatSearchState {
        crate::model::ChatSearchState {
            query: if self.slack_search_results_open {
                self.slack_search_committed_query.clone()
            } else {
                self.slack_search_query.clone()
            },
            open: self.slack_search_open || self.slack_search_results_open,
            loading: self.slack_search_loading,
            total: self.slack_search_total,
            error: self.slack_search_error.clone(),
            results: self
                .slack_search_rows
                .iter()
                .map(|row| crate::model::ChatSearchResultSummary {
                    result_id: row.result_id.to_string(),
                    conversation_id: row.action_target.conversation_id().to_string(),
                    message_id: row.action_target.message_timestamp().as_str().to_string(),
                    thread_root_id: row
                        .show_thread_action
                        .then(|| row.action_target.root_timestamp().as_str().to_string()),
                    conversation_label: row.conversation_label.to_string(),
                    author: row.author.to_string(),
                    timestamp_label: row.full_timestamp_label.to_string(),
                    body_preview: row.body_preview.to_string(),
                    attachment_titles: row
                        .attachments
                        .iter()
                        .map(|attachment| attachment.attachment.title.to_string())
                        .collect(),
                    reactions: row
                        .reaction_state
                        .iter()
                        .map(|reaction| crate::model::ChatReactionSummary {
                            emoji: reaction.emoji.clone(),
                            count: reaction.count,
                            current_user_active: reaction.active,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    pub(crate) fn control_open_slack_search_result(
        &mut self,
        result_id: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let target = self
            .slack_search_rows
            .iter()
            .find(|row| row.result_id.as_ref() == result_id)
            .map(|row| row.action_target.clone())
            .ok_or_else(|| format!("Chat search has no result with ID {result_id}"))?;
        self.activate_slack_search_result(&target, cx);
        Ok(())
    }

    pub(crate) fn control_open_slack_search_result_thread(
        &mut self,
        result_id: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let row = self
            .slack_search_rows
            .iter()
            .find(|row| row.result_id.as_ref() == result_id)
            .ok_or_else(|| format!("Chat search has no result with ID {result_id}"))?;
        if !row.show_thread_action {
            return Err(format!("Chat search result {result_id} has no thread"));
        }
        let target = row.action_target.clone();
        self.activate_slack_search_result_thread(&target, cx);
        Ok(())
    }
}
