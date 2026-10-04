use super::super::super::{Context, SlackRailView, SurfaceState};
use crate::ui::surface::{SlackMoreView, SlackRailMenu};

impl SurfaceState {
    pub(crate) fn toggle_slack_rail_menu(&mut self, menu: SlackRailMenu, cx: &mut Context<Self>) {
        self.slack_channel_menu_open = false;
        self.slack_history_menu_open = false;
        self.slack_rail_menu = (self.slack_rail_menu != Some(menu)).then_some(menu);
        cx.notify();
    }

    pub(super) fn close_slack_rail_menu(&mut self, cx: &mut Context<Self>) {
        if self.slack_rail_menu.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn open_slack_more_view(&mut self, view: SlackMoreView, cx: &mut Context<Self>) {
        self.slack_more_view = view;
        self.close_slack_rail_menu(cx);
        self.select_slack_rail_view(SlackRailView::More, cx);
    }

    pub(in super::super) fn toggle_slack_rail_view_visibility(
        &mut self,
        view: SlackRailView,
        cx: &mut Context<Self>,
    ) {
        if !matches!(
            view,
            SlackRailView::Dms
                | SlackRailView::Activity
                | SlackRailView::Files
                | SlackRailView::Later
        ) {
            return;
        }
        if !self.slack_hidden_rail_views.remove(&view) {
            self.slack_hidden_rail_views.insert(view);
        }
        if self.slack_active_rail_view == view {
            self.select_slack_rail_view(SlackRailView::Home, cx);
        } else {
            cx.notify();
        }
    }

    pub(in super::super) fn slack_rail_view_available(&self, view: SlackRailView) -> bool {
        match view {
            SlackRailView::Home => self.slack_workspace_api_capabilities.load_conversation,
            SlackRailView::Dms => self.slack_workspace_api_capabilities.load_dm_inbox,
            SlackRailView::Activity => self.slack_workspace_api_capabilities.load_activity,
            SlackRailView::Files => self.slack_workspace_api_capabilities.load_files,
            SlackRailView::Later => self.slack_workspace_api_capabilities.load_later,
            SlackRailView::DraftsSent => self.slack_workspace_api_capabilities.load_drafts_sent,
            SlackRailView::More | SlackRailView::Admin => true,
        }
    }
}
