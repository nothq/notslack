mod mutations;

use super::{prepare_slack_sidebar_snapshot, Context, PreparedSlackSidebarSnapshot, SurfaceState};
use crate::ui::surface::{
    SlackChannelMenuAction, SlackChannelMenuActions, SlackChannelMenuSubmenu,
    SlackChannelPermalinkRequest, SlackChannelStarRequest, SlackChannelSubmenuAction,
    SlackChannelSubmenuActions, SlackMainRoute,
};
use crate::ui::SlackStarMutation;
use gpui::ClipboardItem;

impl SurfaceState {
    pub(crate) fn slack_channel_menu_available(&self) -> bool {
        self.slack_main_route == SlackMainRoute::Conversation
            && self.slack_active_rail_view == super::SlackRailView::Home
            && self.slack_workspace().is_some()
    }

    pub(crate) fn open_slack_channel_menu(&mut self, cx: &mut Context<Self>) {
        if !self.slack_channel_menu_available() {
            return;
        }
        self.slack_history_menu_open = false;
        self.slack_history_menu_selected_index = None;
        self.slack_channel_notifications_menu_open = false;
        self.slack_channel_notifications_advanced_open = false;
        self.slack_channel_menu_open = true;
        self.slack_channel_menu_focus_pending = true;
        self.slack_channel_submenu_focus_pending = false;
        self.slack_channel_menu_selected_index = None;
        self.slack_channel_menu_submenu = None;
        self.slack_channel_submenu_selected_index = None;
        cx.notify();
    }

    pub(crate) fn close_slack_channel_menu(&mut self, cx: &mut Context<Self>) {
        if !self.slack_channel_menu_open {
            return;
        }
        self.slack_channel_menu_open = false;
        self.slack_channel_menu_focus_pending = false;
        self.slack_channel_submenu_focus_pending = false;
        self.slack_channel_menu_selected_index = None;
        self.slack_channel_menu_submenu = None;
        self.slack_channel_submenu_selected_index = None;
        cx.notify();
    }

    pub(crate) fn slack_channel_menu_actions(&self) -> SlackChannelMenuActions {
        let mut actions = SlackChannelMenuActions::new();
        if !self.slack_channel_menu_available() {
            return actions;
        }
        let workspace = self
            .slack_workspace()
            .expect("Slack conversation menu requires an active workspace");
        if workspace.channel_kind.is_channel()
            && (self.slack_workspace_api_capabilities.load_channel_details
                || self.slack_workspace_api_capabilities.search_messages)
        {
            actions.push(SlackChannelMenuAction::ChannelDetails);
        }
        actions.push(SlackChannelMenuAction::Copy);
        if self.slack_workspace_api_capabilities.mutate_stars
            && self.slack_pending_channel_star.is_none()
        {
            actions.push(SlackChannelMenuAction::ToggleStar(
                self.slack_channel_star_mutation(),
            ));
        }
        actions
    }

    pub(crate) fn slack_channel_submenu_actions(
        &self,
        submenu: SlackChannelMenuSubmenu,
    ) -> SlackChannelSubmenuActions {
        let mut actions = SlackChannelSubmenuActions::new();
        match submenu {
            SlackChannelMenuSubmenu::ChannelDetails => {
                if self.slack_workspace_api_capabilities.load_channel_details {
                    actions.push(SlackChannelSubmenuAction::OpenChannelDetails);
                }
                if self.slack_workspace_api_capabilities.search_messages {
                    actions.push(SlackChannelSubmenuAction::SearchInChannel);
                }
            }
            SlackChannelMenuSubmenu::Copy => {
                actions.push(SlackChannelSubmenuAction::CopyName);
                if self.slack_workspace_api_capabilities.load_channel_permalink {
                    actions.push(SlackChannelSubmenuAction::CopyLink);
                }
            }
        }
        actions
    }

    pub(crate) fn hover_slack_channel_menu_action(
        &mut self,
        index: usize,
        action: SlackChannelMenuAction,
        cx: &mut Context<Self>,
    ) {
        let submenu = action.submenu();
        if self.slack_channel_menu_selected_index == Some(index)
            && self.slack_channel_menu_submenu == submenu
        {
            return;
        }
        self.slack_channel_menu_selected_index = Some(index);
        self.slack_channel_menu_submenu = submenu;
        self.slack_channel_submenu_selected_index = None;
        cx.notify();
    }

    pub(crate) fn hover_slack_channel_submenu_action(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        if self.slack_channel_submenu_selected_index == Some(index) {
            return;
        }
        self.slack_channel_submenu_selected_index = Some(index);
        cx.notify();
    }

    pub(crate) fn move_slack_channel_menu_selection(
        &mut self,
        direction: isize,
        cx: &mut Context<Self>,
    ) {
        let actions = self.slack_channel_menu_actions();
        if actions.is_empty() {
            self.close_slack_channel_menu(cx);
            return;
        }
        let next = self.slack_channel_menu_selected_index.map_or_else(
            || if direction < 0 { actions.len() - 1 } else { 0 },
            |current| (current as isize + direction).rem_euclid(actions.len() as isize) as usize,
        );
        self.slack_channel_menu_selected_index = Some(next);
        self.slack_channel_menu_submenu = None;
        self.slack_channel_submenu_selected_index = None;
        cx.notify();
    }

    pub(crate) fn move_slack_channel_submenu_selection(
        &mut self,
        direction: isize,
        cx: &mut Context<Self>,
    ) {
        let Some(submenu) = self.slack_channel_menu_submenu else {
            self.move_slack_channel_menu_selection(direction, cx);
            return;
        };
        let actions = self.slack_channel_submenu_actions(submenu);
        if actions.is_empty() {
            self.close_slack_channel_submenu(cx);
            return;
        }
        let next = self.slack_channel_submenu_selected_index.map_or_else(
            || if direction < 0 { actions.len() - 1 } else { 0 },
            |current| (current as isize + direction).rem_euclid(actions.len() as isize) as usize,
        );
        self.slack_channel_submenu_selected_index = Some(next);
        cx.notify();
    }

    pub(crate) fn select_slack_channel_menu_boundary(
        &mut self,
        last: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(submenu) = self.slack_channel_menu_submenu {
            let actions = self.slack_channel_submenu_actions(submenu);
            if actions.is_empty() {
                self.close_slack_channel_submenu(cx);
                return;
            }
            self.slack_channel_submenu_selected_index =
                Some(if last { actions.len() - 1 } else { 0 });
            cx.notify();
            return;
        }
        let actions = self.slack_channel_menu_actions();
        if actions.is_empty() {
            self.close_slack_channel_menu(cx);
            return;
        }
        self.slack_channel_menu_selected_index = Some(if last { actions.len() - 1 } else { 0 });
        cx.notify();
    }

    pub(crate) fn open_selected_slack_channel_submenu(&mut self, cx: &mut Context<Self>) -> bool {
        let index = self.slack_channel_menu_selected_index.unwrap_or(0);
        let Some(submenu) = self
            .slack_channel_menu_actions()
            .get(index)
            .and_then(SlackChannelMenuAction::submenu)
        else {
            return false;
        };
        self.slack_channel_menu_submenu = Some(submenu);
        self.slack_channel_submenu_selected_index = Some(0);
        self.slack_channel_submenu_focus_pending = true;
        cx.notify();
        true
    }

    pub(crate) fn close_slack_channel_submenu(&mut self, cx: &mut Context<Self>) {
        if self.slack_channel_menu_submenu.take().is_none() {
            return;
        }
        self.slack_channel_submenu_selected_index = None;
        self.slack_channel_menu_focus_pending = true;
        self.slack_channel_submenu_focus_pending = false;
        cx.notify();
    }

    pub(crate) fn activate_selected_slack_channel_menu(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(submenu) = self.slack_channel_menu_submenu {
            let index = self.slack_channel_submenu_selected_index.unwrap_or(0);
            let Some(action) = self.slack_channel_submenu_actions(submenu).get(index) else {
                self.close_slack_channel_submenu(cx);
                return true;
            };
            self.activate_slack_channel_submenu_action(action, cx);
            return true;
        }
        let index = self.slack_channel_menu_selected_index.unwrap_or(0);
        let Some(action) = self.slack_channel_menu_actions().get(index) else {
            self.close_slack_channel_menu(cx);
            return true;
        };
        self.activate_slack_channel_menu_action(action, cx);
        true
    }

    pub(crate) fn activate_slack_channel_menu_action(
        &mut self,
        action: SlackChannelMenuAction,
        cx: &mut Context<Self>,
    ) {
        if let Some(submenu) = action.submenu() {
            self.slack_channel_menu_submenu = Some(submenu);
            self.slack_channel_menu_selected_index = Some(submenu.root_index());
            self.slack_channel_submenu_selected_index = Some(0);
            self.slack_channel_submenu_focus_pending = true;
            cx.notify();
            return;
        }
        let SlackChannelMenuAction::ToggleStar(mutation) = action else {
            return;
        };
        self.start_slack_channel_star_mutation(mutation, cx);
    }

    pub(crate) fn activate_slack_channel_submenu_action(
        &mut self,
        action: SlackChannelSubmenuAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            SlackChannelSubmenuAction::OpenChannelDetails => {
                self.close_slack_channel_menu(cx);
                self.open_slack_channel_details(cx);
            }
            SlackChannelSubmenuAction::SearchInChannel => {
                self.close_slack_channel_menu(cx);
                self.open_slack_conversation_search(cx);
            }
            SlackChannelSubmenuAction::CopyName => {
                let Some(name) = self
                    .slack_workspace()
                    .map(|workspace| workspace.channel_name.clone())
                else {
                    return;
                };
                self.close_slack_channel_menu(cx);
                cx.write_to_clipboard(ClipboardItem::new_string(name));
            }
            SlackChannelSubmenuAction::CopyLink => {
                self.start_slack_channel_permalink_copy(cx);
            }
        }
    }

    pub(crate) fn close_slack_channel_details(&mut self, cx: &mut Context<Self>) {
        if !self.slack_channel_details_open {
            return;
        }
        self.slack_channel_details_open = false;
        self.clear_slack_channel_details_view();
        cx.notify();
    }

    pub(crate) fn open_slack_channel_details_members(&mut self, cx: &mut Context<Self>) {
        self.slack_channel_details_open = false;
        self.clear_slack_channel_details_view();
        self.open_slack_members_panel(cx);
    }

    pub(crate) fn reset_slack_channel_menu_context(&mut self) {
        self.slack_channel_move_menu_open = false;
        self.slack_channel_move_menu_focus_pending = false;
        self.slack_header_move_focus_pending = false;
        self.slack_channel_menu_open = false;
        self.slack_channel_menu_focus_pending = false;
        self.slack_channel_submenu_focus_pending = false;
        self.slack_channel_menu_selected_index = None;
        self.slack_channel_menu_submenu = None;
        self.slack_channel_submenu_selected_index = None;
        self.slack_channel_details_open = false;
        self.clear_slack_channel_details_view();
        if self.slack_pending_channel_star.take().is_some() {
            self.slack_channel_star_generation = self
                .slack_channel_star_generation
                .checked_add(1)
                .expect("Slack channel star request generation overflowed");
        }
        if self.slack_pending_channel_permalink.take().is_some() {
            self.slack_channel_permalink_generation = self
                .slack_channel_permalink_generation
                .checked_add(1)
                .expect("Slack channel permalink request generation overflowed");
        }
    }
}

fn slack_channel_is_starred(
    sections: &[crate::ui::SlackSidebarSection],
    conversation_id: &str,
) -> bool {
    sections.iter().any(|section| {
        section.label.eq_ignore_ascii_case("starred")
            && section
                .items
                .iter()
                .any(|item| item.target_id == conversation_id)
    })
}
