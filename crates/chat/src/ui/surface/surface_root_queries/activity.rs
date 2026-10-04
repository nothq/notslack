use super::{
    ChatActivityFilter, ChatActivityReadTargetKind, ChatActivityRowSummary, ChatActivityState,
    ChatRailView, Context, SlackActivityFilter, SlackActivityReadKind, SlackRailView, SurfaceRoot,
};

impl SurfaceRoot {
    pub fn slack_activity_state<AppState: 'static>(
        &mut self,
        cx: &mut Context<AppState>,
    ) -> ChatActivityState {
        let surface = self.ensure_surface(cx);
        surface.update(cx, |surface, _cx| {
            let selected_item_key = surface
                .slack_activity_selected_key
                .as_ref()
                .map(ToString::to_string);
            let pending_item_keys = surface
                .slack_activity_item_mutation_request
                .iter()
                .map(|request| request.mutation.key().to_string())
                .chain(
                    surface
                        .slack_activity_item_mutation_queue
                        .iter()
                        .map(|queued| queued.mutation.key().to_string()),
                )
                .collect::<Vec<_>>();
            let rows = surface
                .slack_activity_visible_row_indices
                .iter()
                .filter_map(|index| surface.slack_activity_rows.get(*index))
                .map(|row| ChatActivityRowSummary {
                    item_key: row.key.to_string(),
                    unread: row.unread,
                    selected: selected_item_key.as_deref() == Some(row.key.as_ref()),
                    read_pending: surface.slack_activity_item_mutation_is_pending(row.key.as_ref()),
                    read_target_kind: row
                        .read_target
                        .as_ref()
                        .map(|target| chat_activity_read_target_kind(target.kind())),
                })
                .collect();
            ChatActivityState {
                active_rail_view: chat_rail_view(surface.slack_active_rail_view),
                activity_count: surface
                    .slack_workspace()
                    .and_then(|workspace| workspace.rail_badges.activity),
                filter: chat_activity_filter(surface.slack_activity_filter),
                unread_only: surface.slack_activity_unread_only,
                loading: surface.slack_activity_loading,
                load_error: surface.slack_activity_error.clone(),
                read_pending: !pending_item_keys.is_empty(),
                pending_item_keys,
                read_error: surface
                    .slack_activity_mutation_error
                    .as_ref()
                    .map(ToString::to_string),
                selected_item_key,
                rows,
            }
        })
    }
}

fn chat_rail_view(view: SlackRailView) -> ChatRailView {
    match view {
        SlackRailView::Home => ChatRailView::Home,
        SlackRailView::Dms => ChatRailView::Dms,
        SlackRailView::Activity => ChatRailView::Activity,
        SlackRailView::Files => ChatRailView::Files,
        SlackRailView::Later => ChatRailView::Later,
        SlackRailView::DraftsSent => ChatRailView::DraftsSent,
        SlackRailView::More => ChatRailView::More,
        SlackRailView::Admin => ChatRailView::Admin,
    }
}

fn chat_activity_filter(filter: SlackActivityFilter) -> ChatActivityFilter {
    match filter {
        SlackActivityFilter::All => ChatActivityFilter::All,
        SlackActivityFilter::Dms => ChatActivityFilter::Dms,
        SlackActivityFilter::Mentions => ChatActivityFilter::Mentions,
        SlackActivityFilter::Threads => ChatActivityFilter::Threads,
    }
}

fn chat_activity_read_target_kind(kind: SlackActivityReadKind) -> ChatActivityReadTargetKind {
    match kind {
        SlackActivityReadKind::ThreadV2 => ChatActivityReadTargetKind::ThreadV2,
        SlackActivityReadKind::AtUser => ChatActivityReadTargetKind::AtUser,
        SlackActivityReadKind::Dm => ChatActivityReadTargetKind::Dm,
        SlackActivityReadKind::BotDmBundle => ChatActivityReadTargetKind::BotDmBundle,
    }
}
