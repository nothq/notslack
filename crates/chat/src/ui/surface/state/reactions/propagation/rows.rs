use std::sync::Arc;

use super::super::super::{build_slack_message_chunks, Context, SlackMessageRow, SurfaceState};
use super::SlackAuthoritativeMessageMutation;
use crate::ui::surface::{SlackLaterRowContent, SlackMessageActionTarget};
use crate::ui::SlackReaction;

use super::super::lookup::top_level_slack_message_row_index;

#[derive(Default)]
pub(in crate::ui::surface::state::reactions) struct SlackAuthoritativeMessageRowTargets {
    main: Option<usize>,
    thread_panel: bool,
    all_threads: Vec<usize>,
    later: Vec<usize>,
    pins: Vec<usize>,
    activity_detail: Option<usize>,
}

impl SurfaceState {
    pub(in crate::ui::surface::state::reactions) fn apply_authoritative_slack_message_row(
        &mut self,
        target: &SlackMessageActionTarget,
        row: &SlackMessageRow,
        mutation: SlackAuthoritativeMessageMutation,
    ) -> SlackAuthoritativeMessageRowTargets {
        let mut affected = SlackAuthoritativeMessageRowTargets {
            main: top_level_slack_message_row_index(&self.slack_message_rows, target),
            ..Default::default()
        };
        if let Some(index) = affected.main {
            mutate_slack_message_rows(
                std::slice::from_mut(&mut Arc::make_mut(&mut self.slack_message_rows)[index]),
                target,
                row,
                mutation,
            );
        }
        affected.thread_panel =
            self.apply_authoritative_slack_thread_panel_row(target, row, mutation);
        affected.all_threads =
            self.apply_authoritative_slack_all_threads_rows(target, row, mutation);
        affected.later = self.apply_authoritative_slack_later_rows(target, row, mutation);
        affected.pins = self.apply_authoritative_slack_pin_rows(target, row, mutation);
        affected.activity_detail =
            self.apply_authoritative_slack_activity_detail_row(target, row, mutation);
        if mutation == SlackAuthoritativeMessageMutation::Reactions {
            mutate_slack_search_reaction_rows(
                Arc::make_mut(&mut self.slack_search_rows),
                target,
                &row.reaction_state,
                &row.reactions,
            );
        }
        affected
    }

    fn apply_authoritative_slack_thread_panel_row(
        &mut self,
        target: &SlackMessageActionTarget,
        row: &SlackMessageRow,
        mutation: SlackAuthoritativeMessageMutation,
    ) -> bool {
        let Some(panel) = self.slack_thread_panel.as_mut().filter(|panel| {
            panel.conversation_id == target.conversation_id()
                && panel.parent_message_id == target.root_timestamp().as_str()
        }) else {
            return false;
        };
        let mut affected = false;
        if panel.parent_row.action_target.as_deref() == Some(target) {
            mutate_slack_message_row(&mut panel.parent_row, row, mutation);
            affected = true;
        }
        affected
            | mutate_slack_message_rows(Arc::make_mut(&mut panel.reply_rows), target, row, mutation)
    }

    fn apply_authoritative_slack_all_threads_rows(
        &mut self,
        target: &SlackMessageActionTarget,
        row: &SlackMessageRow,
        mutation: SlackAuthoritativeMessageMutation,
    ) -> Vec<usize> {
        Arc::make_mut(&mut self.slack_all_threads_rows)
            .iter_mut()
            .enumerate()
            .filter_map(|(index, thread)| {
                let mut affected = false;
                if thread.parent.action_target.as_deref() == Some(target) {
                    mutate_slack_message_row(&mut thread.parent, row, mutation);
                    affected = true;
                }
                affected |= mutate_slack_message_rows(
                    Arc::make_mut(&mut thread.replies),
                    target,
                    row,
                    mutation,
                );
                affected.then_some(index)
            })
            .collect()
    }

    fn apply_authoritative_slack_later_rows(
        &mut self,
        target: &SlackMessageActionTarget,
        row: &SlackMessageRow,
        mutation: SlackAuthoritativeMessageMutation,
    ) -> Vec<usize> {
        self.slack_later_rows
            .iter_mut()
            .enumerate()
            .filter_map(|(index, later)| {
                let SlackLaterRowContent::Message { detail } = &mut later.content else {
                    return None;
                };
                (detail.selected_message_row.action_target.as_deref() == Some(target)).then(|| {
                    mutate_slack_message_row(&mut detail.selected_message_row, row, mutation);
                    index
                })
            })
            .collect()
    }

    fn apply_authoritative_slack_pin_rows(
        &mut self,
        target: &SlackMessageActionTarget,
        row: &SlackMessageRow,
        mutation: SlackAuthoritativeMessageMutation,
    ) -> Vec<usize> {
        Arc::make_mut(&mut self.slack_pins_rows)
            .iter_mut()
            .enumerate()
            .filter_map(|(index, pin)| {
                (pin.message.action_target.as_deref() == Some(target)).then(|| {
                    mutate_slack_message_row(&mut pin.message, row, mutation);
                    index
                })
            })
            .collect()
    }

    fn apply_authoritative_slack_activity_detail_row(
        &mut self,
        target: &SlackMessageActionTarget,
        row: &SlackMessageRow,
        mutation: SlackAuthoritativeMessageMutation,
    ) -> Option<usize> {
        let crate::ui::surface::SlackActivityDetailState::Loaded { rows, .. } =
            &mut self.slack_activity_detail
        else {
            return None;
        };
        let index = top_level_slack_message_row_index(rows.as_ref(), target)?;
        mutate_slack_message_rows(
            std::slice::from_mut(&mut Arc::make_mut(rows)[index]),
            target,
            row,
            mutation,
        );
        Some(index)
    }

    pub(in crate::ui::surface::state::reactions) fn finish_authoritative_slack_message_row_update(
        &mut self,
        row: &SlackMessageRow,
        affected: SlackAuthoritativeMessageRowTargets,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = affected.main {
            self.slack_message_chunks = build_slack_message_chunks(&self.slack_message_rows);
            self.slack_message_list_state
                .remeasure_items(index..index + 1);
        }
        if affected.thread_panel {
            if let Some(panel) = self.slack_thread_panel.as_mut() {
                panel.list_state.remeasure();
            }
        }
        for index in affected.all_threads {
            self.slack_all_threads_list_state
                .remeasure_items(index..index + 1);
        }
        for index in affected.later {
            self.slack_later_list_state
                .remeasure_items(index..index + 1);
        }
        for index in affected.pins {
            self.slack_pins_list_state.remeasure_items(index..index + 1);
        }
        if let Some(index) = affected.activity_detail {
            self.slack_activity_detail_list_state
                .remeasure_items(index..index + 1);
        }
        self.prefetch_slack_message_preview_images(std::slice::from_ref(row), cx);
        let reaction_image_urls =
            super::super::super::workspace::slack_message_reaction_remote_image_urls(row)
                .map(str::to_string)
                .collect::<Vec<_>>();
        for url in reaction_image_urls {
            self.enqueue_slack_remote_image_url(url, cx);
        }
        self.mark_slack_remote_image_queue_dirty();
        self.ensure_slack_remote_image_loads(cx);
    }
}

fn mutate_slack_message_rows(
    rows: &mut [SlackMessageRow],
    target: &SlackMessageActionTarget,
    authoritative: &SlackMessageRow,
    mutation: SlackAuthoritativeMessageMutation,
) -> bool {
    for row in rows {
        if row.action_target.as_deref() == Some(target) {
            mutate_slack_message_row(row, authoritative, mutation);
            return true;
        }
        if mutate_slack_message_rows(&mut row.replies, target, authoritative, mutation) {
            return true;
        }
    }
    false
}

fn mutate_slack_message_row(
    row: &mut SlackMessageRow,
    authoritative: &SlackMessageRow,
    mutation: SlackAuthoritativeMessageMutation,
) {
    match mutation {
        SlackAuthoritativeMessageMutation::Reactions => {
            row.reaction_state.clone_from(&authoritative.reaction_state);
            row.reactions.clone_from(&authoritative.reactions);
        }
        SlackAuthoritativeMessageMutation::SavedState => {
            row.saved_state = authoritative.saved_state;
        }
    }
}

pub(in crate::ui::surface::state::reactions) fn mutate_slack_search_reaction_rows(
    rows: &mut [crate::ui::surface::SlackSearchRow],
    target: &SlackMessageActionTarget,
    reaction_state: &Arc<[SlackReaction]>,
    reactions: &[crate::ui::surface::SlackReactionRow],
) -> bool {
    let mut updated = false;
    for row in rows {
        if row.action_target.as_ref() != target {
            continue;
        }
        row.reaction_state.clone_from(reaction_state);
        row.reactions = reactions.to_vec().into();
        updated = true;
    }
    updated
}

pub(in crate::ui::surface::state::reactions) fn slack_reaction_row_remote_image_urls(
    reactions: &[crate::ui::surface::SlackReactionRow],
) -> impl Iterator<Item = &str> {
    reactions
        .iter()
        .flat_map(|reaction| reaction.variants.iter())
        .filter_map(|variant| variant.image_cache_key.as_deref())
}
