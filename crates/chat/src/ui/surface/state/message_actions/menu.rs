use super::unread::SlackMessageMarkUnreadTarget;

use super::{
    ClipboardItem, Context, SlackMessageMenuAction, SlackMessageMenuState, SlackMessageTimestamp,
    SurfaceState, SLACK_MESSAGE_MENU_ANCHOR_GAP, SLACK_MESSAGE_MENU_EDGE_INSET,
    SLACK_MESSAGE_MENU_GROUP_GAP, SLACK_MESSAGE_MENU_ROW_HEIGHT,
    SLACK_MESSAGE_MENU_VERTICAL_PADDING, SLACK_MESSAGE_MENU_WIDTH,
};
use crate::ui::surface::{SlackMessageActionTarget, SlackMessageMenuOpenContext};
use gpui::{Pixels, Point};

struct SlackMessageMenuTarget {
    team_id: String,
    conversation_id: String,
    message_timestamp: SlackMessageTimestamp,
    body: gpui::SharedString,
    mark_unread_cursor: Option<SlackMessageTimestamp>,
    own_message: bool,
}

impl SurfaceState {
    pub(crate) fn open_slack_message_menu_for_target(
        &mut self,
        context: SlackMessageMenuOpenContext,
        anchor: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let SlackMessageMenuOpenContext {
            action_target,
            body,
            user_id,
        } = context;
        let Some(target) = self.slack_message_menu_target(&action_target, body, user_id.as_deref())
        else {
            return;
        };
        let actions =
            self.slack_message_menu_actions(action_target.message_timestamp().as_str(), &target);
        if actions.is_empty() {
            return;
        }
        let (top, right) =
            self.slack_message_menu_position(&actions, anchor.x.as_f32(), anchor.y.as_f32());
        self.close_slack_message_menu_conflicts();
        self.slack_message_menu = Some(SlackMessageMenuState {
            team_id: target.team_id,
            conversation_id: target.conversation_id,
            message_timestamp: target.message_timestamp,
            body: target.body,
            mark_unread_cursor: target.mark_unread_cursor,
            actions: actions.into(),
            selected_index: None,
            top,
            right,
        });
        self.slack_message_menu_focus_pending = true;
        self.slack_error = None;
        cx.notify();
    }

    fn slack_message_menu_target(
        &self,
        action_target: &SlackMessageActionTarget,
        body: gpui::SharedString,
        user_id: Option<&str>,
    ) -> Option<SlackMessageMenuTarget> {
        let workspace = self.slack_workspace()?;
        if workspace.team_id != action_target.team_id() {
            return None;
        }
        let own_message = workspace
            .self_user_id
            .as_deref()
            .is_some_and(|self_user_id| user_id == Some(self_user_id));
        let current = self.slack_message_action_target_is_current(
            action_target.team_id(),
            action_target.conversation_id(),
        );
        Some(SlackMessageMenuTarget {
            team_id: action_target.team_id().to_string(),
            conversation_id: action_target.conversation_id().to_string(),
            message_timestamp: action_target.message_timestamp().clone(),
            body,
            mark_unread_cursor: current
                .then(|| {
                    self.slack_message_mark_unread_cursor(
                        action_target.message_timestamp().as_str(),
                    )
                })
                .flatten(),
            own_message,
        })
    }

    fn slack_message_menu_actions(
        &self,
        message_id: &str,
        target: &SlackMessageMenuTarget,
    ) -> Vec<SlackMessageMenuAction> {
        let mut actions = Vec::with_capacity(5);
        if target.own_message && self.slack_message_can_edit(message_id) {
            actions.push(SlackMessageMenuAction::Edit);
        }
        if self.slack_workspace_api_capabilities.mark_conversation_read
            && self.slack_pending_message_mark_unread.is_none()
            && self.slack_conversation_read_request.is_none()
            && target.mark_unread_cursor.is_some()
        {
            actions.push(SlackMessageMenuAction::MarkUnread);
        }
        if self.slack_workspace_api_capabilities.load_message_permalink
            && self.slack_pending_message_permalink.is_none()
        {
            actions.push(SlackMessageMenuAction::CopyLink);
        }
        if !target.body.is_empty() {
            actions.push(SlackMessageMenuAction::CopyMessage);
        }
        if target.own_message && self.slack_message_can_delete(message_id) {
            actions.push(SlackMessageMenuAction::Delete);
        }
        actions
    }

    fn slack_message_menu_position(
        &self,
        actions: &[SlackMessageMenuAction],
        anchor_x: f32,
        anchor_y: f32,
    ) -> (f32, f32) {
        let group_gaps = actions
            .windows(2)
            .filter(|pair| pair[0].group() != pair[1].group())
            .count();
        let menu_height = SLACK_MESSAGE_MENU_VERTICAL_PADDING
            + SLACK_MESSAGE_MENU_ROW_HEIGHT * actions.len() as f32
            + SLACK_MESSAGE_MENU_GROUP_GAP * group_gaps as f32;
        let max_right =
            (self.preview_width - SLACK_MESSAGE_MENU_WIDTH - SLACK_MESSAGE_MENU_EDGE_INSET)
                .max(SLACK_MESSAGE_MENU_EDGE_INSET);
        let right =
            (self.preview_width - anchor_x - 16.0).clamp(SLACK_MESSAGE_MENU_EDGE_INSET, max_right);
        let below = anchor_y + SLACK_MESSAGE_MENU_ANCHOR_GAP;
        let above = anchor_y - menu_height - SLACK_MESSAGE_MENU_ANCHOR_GAP;
        let max_top = (self.viewport_height - menu_height - SLACK_MESSAGE_MENU_EDGE_INSET)
            .max(SLACK_MESSAGE_MENU_EDGE_INSET);
        let top = if below + menu_height <= self.viewport_height {
            below
        } else {
            above
        }
        .clamp(SLACK_MESSAGE_MENU_EDGE_INSET, max_top);
        (top, right)
    }

    fn close_slack_message_menu_conflicts(&mut self) {
        self.slack_reaction_picker = None;
        self.slack_channel_menu_open = false;
        self.slack_channel_menu_selected_index = None;
        self.slack_channel_menu_submenu = None;
        self.slack_channel_submenu_selected_index = None;
        self.slack_history_menu_open = false;
        self.slack_history_menu_selected_index = None;
    }

    pub(crate) fn close_slack_message_menu(&mut self, cx: &mut Context<Self>) {
        if self.slack_message_menu.take().is_none() {
            return;
        }
        self.slack_message_menu_focus_pending = false;
        cx.notify();
    }

    pub(crate) fn hover_slack_message_menu_action(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(menu) = self.slack_message_menu.as_mut() else {
            return;
        };
        if menu.selected_index == Some(index) {
            return;
        }
        menu.selected_index = Some(index);
        cx.notify();
    }

    pub(crate) fn move_slack_message_menu_selection(
        &mut self,
        direction: isize,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.slack_message_menu.as_mut() else {
            return;
        };
        let len = menu.actions.len();
        if len == 0 {
            self.close_slack_message_menu(cx);
            return;
        }
        let next = menu.selected_index.map_or_else(
            || if direction < 0 { len - 1 } else { 0 },
            |index| {
                if direction < 0 {
                    index.checked_sub(1).unwrap_or(len - 1)
                } else {
                    (index + 1) % len
                }
            },
        );
        menu.selected_index = Some(next);
        cx.notify();
    }

    pub(crate) fn select_slack_message_menu_boundary(
        &mut self,
        last: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(menu) = self.slack_message_menu.as_mut() else {
            return;
        };
        if menu.actions.is_empty() {
            self.close_slack_message_menu(cx);
            return;
        }
        menu.selected_index = Some(if last { menu.actions.len() - 1 } else { 0 });
        cx.notify();
    }

    pub(crate) fn activate_selected_slack_message_menu(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.slack_message_menu.as_ref() else {
            return;
        };
        let index = menu.selected_index.unwrap_or(0);
        let Some(action) = menu.actions.get(index).copied() else {
            self.close_slack_message_menu(cx);
            return;
        };
        self.activate_slack_message_menu_action(action, cx);
    }

    pub(crate) fn activate_slack_message_menu_action(
        &mut self,
        action: SlackMessageMenuAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            SlackMessageMenuAction::Edit => self.activate_slack_message_edit(cx),
            SlackMessageMenuAction::MarkUnread => self.activate_slack_message_mark_unread(cx),
            SlackMessageMenuAction::CopyLink => self.activate_slack_message_copy_link(cx),
            SlackMessageMenuAction::CopyMessage => self.activate_slack_message_copy(cx),
            SlackMessageMenuAction::Delete => self.activate_slack_message_delete(cx),
        }
    }

    fn activate_slack_message_edit(&mut self, cx: &mut Context<Self>) {
        let Some(message_id) = self.slack_message_menu_id() else {
            return;
        };
        self.close_slack_message_menu(cx);
        self.open_slack_message_edit(&message_id, cx);
    }

    fn activate_slack_message_mark_unread(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.slack_message_menu.clone() else {
            return;
        };
        let Some(cursor) = menu.mark_unread_cursor else {
            self.close_slack_message_menu(cx);
            return;
        };
        self.close_slack_message_menu(cx);
        self.start_slack_message_mark_unread(
            SlackMessageMarkUnreadTarget {
                team_id: menu.team_id,
                conversation_id: menu.conversation_id,
                target_timestamp: menu.message_timestamp,
                cursor,
            },
            cx,
        );
    }

    fn activate_slack_message_copy_link(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.slack_message_menu.clone() else {
            return;
        };
        self.close_slack_message_menu(cx);
        self.start_slack_message_permalink_copy(
            menu.team_id,
            menu.conversation_id,
            menu.message_timestamp,
            cx,
        );
    }

    fn activate_slack_message_copy(&mut self, cx: &mut Context<Self>) {
        let Some(body) = self
            .slack_message_menu
            .as_ref()
            .map(|menu| menu.body.to_string())
        else {
            return;
        };
        self.close_slack_message_menu(cx);
        cx.write_to_clipboard(ClipboardItem::new_string(body));
    }

    fn activate_slack_message_delete(&mut self, cx: &mut Context<Self>) {
        let Some(message_id) = self.slack_message_menu_id() else {
            return;
        };
        self.close_slack_message_menu(cx);
        self.open_slack_message_delete(&message_id, cx);
    }

    fn slack_message_menu_id(&self) -> Option<String> {
        self.slack_message_menu
            .as_ref()
            .map(|menu| menu.message_timestamp.as_str().to_string())
    }
}
