use super::{
    alpha, replace_slack_search_display_token, rgb, slack_palette, slack_search_reference_id,
    slack_search_reference_label, Arc, Context, Range, SlackConversationKind, SurfaceState,
    TextInputHighlight,
};
use crate::ui::surface::SlackMessageActionTarget;

impl SurfaceState {
    pub(in crate::ui::surface::state) fn slack_search_input_highlights(
        &self,
        value: &str,
    ) -> Vec<TextInputHighlight> {
        let link_color = slack_palette(self.appearance_mode).link;
        let mut tokens = [
            self.slack_search_current_conversation_display_modifier(),
            self.slack_search_self_display_reference("from"),
            self.slack_search_self_display_reference("with"),
            Some("has:link".to_string()),
            Some("is:thread".to_string()),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
        tokens.sort_unstable();
        tokens.dedup();
        let mut highlights = tokens
            .iter()
            .flat_map(|token| {
                value
                    .match_indices(token)
                    .map(move |(start, matched)| TextInputHighlight {
                        range: start..start + matched.len(),
                        color: rgb(link_color).into(),
                        background: Some(alpha(link_color, 0.14)),
                        ..Default::default()
                    })
            })
            .collect::<Vec<_>>();
        highlights.sort_unstable_by_key(|highlight| highlight.range.start);
        highlights
    }

    pub(crate) fn clear_slack_search(&mut self, cx: &mut Context<Self>) {
        let was_open = self.slack_search_open;
        self.reset_slack_search_state();
        self.slack_search_open = true;
        if was_open {
            self.notify_slack_search_overlay(cx);
        } else {
            cx.notify();
        }
    }

    pub(crate) fn clear_slack_search_results(&mut self, cx: &mut Context<Self>) {
        self.reset_slack_search_state();
        self.slack_search_open = false;
        cx.notify();
    }

    pub(in crate::ui::surface::state) fn reset_slack_search_state(&mut self) {
        self.slack_reaction_picker = None;
        self.slack_search_generation = self.slack_search_generation.wrapping_add(1);
        self.slack_search_query.clear();
        self.slack_search_request_query.clear();
        self.slack_search_committed_query.clear();
        self.slack_search_options = crate::ui::SlackMessageSearchOptions::default();
        self.slack_search_results_open = false;
        self.slack_search_selected_option = 0;
        self.slack_quick_search_rows = Arc::default();
        self.slack_quick_search_message_rows = Arc::default();
        self.slack_quick_search_options_scroll_handle = gpui::ScrollHandle::new();
        self.slack_quick_search_scroll_handle = gpui::UniformListScrollHandle::new();
        self.slack_quick_search_prefetched_range = None;
        self.slack_quick_search_loading = false;
        self.slack_quick_search_error = None;
        self.slack_quick_message_last_started_at = None;
        self.slack_search_rows = Arc::default();
        self.slack_search_list_state.reset(0);
        self.slack_search_total = 0;
        self.slack_search_next_cursor = None;
        self.slack_search_loading = false;
        self.slack_search_error = None;
    }

    pub(crate) fn close_slack_search_results(&mut self, cx: &mut Context<Self>) {
        let reaction_picker_closed = self.slack_reaction_picker.take().is_some();
        if !self.slack_search_results_open {
            if reaction_picker_closed {
                cx.notify();
            }
            return;
        }
        self.slack_search_results_open = false;
        self.slack_search_open = false;
        self.slack_search_generation = self.slack_search_generation.wrapping_add(1);
        self.slack_search_loading = false;
        self.slack_quick_search_loading = false;
        self.slack_quick_message_last_started_at = None;
        cx.notify();
    }

    pub(crate) fn activate_slack_search_result(
        &mut self,
        target: &SlackMessageActionTarget,
        cx: &mut Context<Self>,
    ) {
        self.close_slack_search_results(cx);
        self.open_slack_message_action_target(target, cx);
    }

    pub(crate) fn activate_slack_search_result_thread(
        &mut self,
        target: &SlackMessageActionTarget,
        cx: &mut Context<Self>,
    ) {
        self.close_slack_search_results(cx);
        self.open_slack_message_thread_target(target, cx);
    }

    pub(in crate::ui::surface::state) fn prefetch_slack_search_images_for_range(
        &mut self,
        row_range: Range<usize>,
        cx: &mut Context<Self>,
    ) {
        let urls =
            self.slack_search_rows
                .get(row_range)
                .unwrap_or_default()
                .iter()
                .flat_map(|row| {
                    row.avatar_image_url
                        .iter()
                        .chain(row.reply_avatar_image_urls.iter())
                        .chain(row.attachments.iter().filter_map(|attachment| {
                            attachment.attachment.preview_cache_key.as_ref()
                        }))
                        .chain(
                            row.reactions
                                .iter()
                                .flat_map(|reaction| reaction.variants.iter())
                                .filter_map(|variant| variant.image_cache_key.as_ref()),
                        )
                })
                .map(ToString::to_string)
                .collect::<Vec<_>>();
        for url in urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
    }

    pub(in crate::ui::surface::state) fn slack_search_current_conversation_modifier(
        &self,
    ) -> Option<String> {
        let workspace = self.slack_workspace()?;
        let item = workspace
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .find(|item| item.target_id == workspace.conversation_id);
        let label = slack_search_reference_label(
            item.map_or(workspace.channel_name.as_str(), |item| item.label.as_str()),
        )?;
        match workspace.channel_kind {
            SlackConversationKind::Channel
            | SlackConversationKind::PrivateChannel
            | SlackConversationKind::GroupMessage => {
                slack_search_reference_id(&workspace.conversation_id, &['C', 'G'])
                    .map(|conversation_id| format!("in:<#{conversation_id}|{label}>"))
            }
            SlackConversationKind::DirectMessage => item
                .and_then(|item| item.user_id.as_deref())
                .and_then(|user_id| slack_search_reference_id(user_id, &['U', 'W']))
                .map(|user_id| format!("in:<@{user_id}|@{label}>")),
            SlackConversationKind::Unknown => None,
        }
    }

    pub(in crate::ui::surface::state) fn slack_search_current_conversation_display_modifier(
        &self,
    ) -> Option<String> {
        let workspace = self.slack_workspace()?;
        let item = workspace
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .find(|item| item.target_id == workspace.conversation_id);
        let label = slack_search_reference_label(
            item.map_or(workspace.channel_name.as_str(), |item| item.label.as_str()),
        )?;
        match workspace.channel_kind {
            SlackConversationKind::Channel | SlackConversationKind::PrivateChannel => {
                Some(format!("in:{}", label.trim_start_matches('#')))
            }
            SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage => {
                Some(format!("in:@{}", label.trim_start_matches('@')))
            }
            SlackConversationKind::Unknown => None,
        }
    }

    pub(in crate::ui::surface::state) fn slack_search_self_display_reference(
        &self,
        modifier: &str,
    ) -> Option<String> {
        let label =
            slack_search_reference_label(self.slack_workspace()?.self_display_name.as_deref()?)?;
        Some(format!("{modifier}:@{}", label.trim_start_matches('@')))
    }

    pub(in crate::ui::surface::state) fn slack_search_api_query(
        &self,
        display_query: &str,
    ) -> Option<String> {
        let display_query = display_query.trim();
        let mut query = display_query.to_string();
        if query.is_empty() {
            return None;
        }
        if let (Some(display), Some(request)) = (
            self.slack_search_current_conversation_display_modifier(),
            self.slack_search_current_conversation_modifier(),
        ) {
            query = replace_slack_search_display_token(query, &display, &request);
        }
        if let Some(workspace) = self.slack_workspace() {
            if let (Some(user_id), Some(display_name)) = (
                workspace.self_user_id.as_deref(),
                workspace.self_display_name.as_deref(),
            ) {
                if let (Some(user_id), Some(label)) = (
                    slack_search_reference_id(user_id, &['U', 'W']),
                    slack_search_reference_label(display_name),
                ) {
                    for modifier in ["from", "with"] {
                        query = replace_slack_search_display_token(
                            query,
                            &format!("{modifier}:@{}", label.trim_start_matches('@')),
                            &format!("{modifier}:<@{user_id}|@{label}>"),
                        );
                    }
                }
            }
        }
        Some(query)
    }
}
