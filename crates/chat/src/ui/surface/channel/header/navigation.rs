use crate::ui::surface::{SlackHeaderControl, SurfaceState};
use crate::ui::SlackWorkspace;
use crate::ui::{Context, FocusHandle, KeyDownEvent, Window};

impl SurfaceState {
    pub(super) fn slack_channel_members_control_available(
        &self,
        workspace: &SlackWorkspace,
    ) -> bool {
        self.slack_workspace_api_capabilities
            .load_conversation_members
            && workspace.channel_kind.is_channel()
            && (workspace.member_count.is_some()
                || self
                    .slack_members_snapshot
                    .as_ref()
                    .is_some_and(|snapshot| {
                        snapshot.team_id == workspace.team_id
                            && snapshot.conversation_id == workspace.conversation_id
                    }))
    }

    pub(super) fn slack_channel_move_menu_available(&self, workspace: &SlackWorkspace) -> bool {
        workspace.channel_kind.is_channel() && self.slack_workspace_api_capabilities.mutate_stars
    }

    fn slack_channel_header_controls(
        &self,
        workspace: &SlackWorkspace,
    ) -> [Option<SlackHeaderControl>; 6] {
        [
            self.slack_channel_move_menu_available(workspace)
                .then_some(SlackHeaderControl::ChannelMove),
            (workspace.channel_kind.is_channel()
                && self.slack_workspace_api_capabilities.load_channel_details)
                .then_some(SlackHeaderControl::ChannelDetails),
            self.slack_channel_members_control_available(workspace)
                .then_some(SlackHeaderControl::Members),
            self.slack_channel_notifications_control_available()
                .then_some(SlackHeaderControl::Notifications),
            self.slack_workspace_api_capabilities
                .search_messages
                .then_some(SlackHeaderControl::Search),
            self.slack_channel_menu_available()
                .then_some(SlackHeaderControl::More),
        ]
    }

    pub(super) fn slack_header_active_control(
        &self,
        workspace: &SlackWorkspace,
    ) -> Option<SlackHeaderControl> {
        let mut controls = self
            .slack_channel_header_controls(workspace)
            .into_iter()
            .flatten();
        let first = controls.next()?;
        Some(
            std::iter::once(first)
                .chain(controls)
                .find(|control| *control == self.slack_header_roving_target)
                .unwrap_or(first),
        )
    }

    pub(super) fn handle_slack_channel_header_roving_key(
        &mut self,
        current: SlackHeaderControl,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let direction = match event.keystroke.key.as_str() {
            "left" if !event.keystroke.modifiers.modified() => -1,
            "right" if !event.keystroke.modifiers.modified() => 1,
            _ => return false,
        };
        let Some(workspace) = self.slack_workspace() else {
            return false;
        };
        let controls = self.slack_channel_header_controls(workspace);
        let mut available = [SlackHeaderControl::ChannelMove; 6];
        let mut len = 0;
        for control in controls.into_iter().flatten() {
            available[len] = control;
            len += 1;
        }
        let Some(position) = available[..len]
            .iter()
            .position(|control| *control == current)
        else {
            return false;
        };
        let next_position = (position as isize + direction).rem_euclid(len as isize) as usize;
        let next = available[next_position];
        let focus_handle = self.slack_header_control_focus_handle(next).clone();
        self.slack_header_roving_target = next;
        window.prevent_default();
        cx.stop_propagation();
        window.focus(&focus_handle, cx);
        cx.notify();
        true
    }

    fn slack_header_control_focus_handle(&self, control: SlackHeaderControl) -> &FocusHandle {
        match control {
            SlackHeaderControl::ChannelMove => &self.slack_header_move_focus_handle,
            SlackHeaderControl::ChannelDetails => &self.slack_header_details_focus_handle,
            SlackHeaderControl::Members => &self.slack_header_members_focus_handle,
            SlackHeaderControl::Notifications => &self.slack_header_notifications_focus_handle,
            SlackHeaderControl::Search => &self.slack_header_search_focus_handle,
            SlackHeaderControl::More => &self.slack_header_more_focus_handle,
        }
    }
}
