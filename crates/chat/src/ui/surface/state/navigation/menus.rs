use super::{Context, KeyDownEvent, SlackRailView, SurfaceState};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn handle_slack_workspace_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.handle_slack_skin_tone_menu_key_down(event, cx) {
            return true;
        }
        if self.handle_slack_date_jump_key_down(event, cx) {
            return true;
        }
        if self.handle_slack_message_menu_key_down(event, cx) {
            return true;
        }
        if self.handle_slack_channel_menu_key_down(event, cx) {
            return true;
        }
        if self.handle_slack_history_menu_key_down(event, cx) {
            return true;
        }
        if self.handle_slack_rail_view_key_down(event, cx) {
            return true;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.handle_slack_escape_key(cx),
            "backspace" => self.handle_slack_backspace_key(cx),
            "enter" => self.handle_slack_enter_key(event, cx),
            "up" if self.slack_search_open => {
                self.move_slack_search_selection(-1, cx);
                true
            }
            "down" if self.slack_search_open => {
                self.move_slack_search_selection(1, cx);
                true
            }
            _ => self.handle_slack_text_input(event, cx),
        }
    }

    fn handle_slack_rail_view_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if event.keystroke.modifiers.modified() {
            return false;
        }
        match (self.slack_active_rail_view, event.keystroke.key.as_str()) {
            (SlackRailView::Activity, "enter" | "space") => {
                self.activate_selected_slack_activity(cx);
            }
            (SlackRailView::Activity, "up") => {
                self.move_slack_activity_selection(-1, cx);
            }
            (SlackRailView::Activity, "down") => {
                self.move_slack_activity_selection(1, cx);
            }
            (SlackRailView::Later, "up") => {
                self.move_slack_later_selection(-1, cx);
            }
            (SlackRailView::Later, "down") => {
                self.move_slack_later_selection(1, cx);
            }
            (SlackRailView::Files, "enter" | "space") => {
                self.open_selected_slack_file(cx);
            }
            (SlackRailView::Files, "up") => {
                self.move_slack_file_selection(-1, cx);
            }
            (SlackRailView::Files, "down") => {
                self.move_slack_file_selection(1, cx);
            }
            _ => return false,
        }
        true
    }

    pub(in crate::ui::surface::state) fn handle_slack_message_menu_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_message_menu.is_none() {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.close_slack_message_menu(cx),
            "up" if !event.keystroke.modifiers.modified() => {
                self.move_slack_message_menu_selection(-1, cx);
            }
            "down" if !event.keystroke.modifiers.modified() => {
                self.move_slack_message_menu_selection(1, cx);
            }
            "home" if !event.keystroke.modifiers.modified() => {
                self.select_slack_message_menu_boundary(false, cx);
            }
            "end" if !event.keystroke.modifiers.modified() => {
                self.select_slack_message_menu_boundary(true, cx);
            }
            "enter" | "space" if !event.keystroke.modifiers.modified() => {
                self.activate_selected_slack_message_menu(cx);
            }
            _ => return false,
        }
        true
    }

    pub(in crate::ui::surface::state) fn handle_slack_channel_menu_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.slack_channel_menu_open {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" if self.slack_channel_menu_submenu.is_some() => {
                self.close_slack_channel_submenu(cx);
            }
            "escape" => self.close_slack_channel_menu(cx),
            "up" if !event.keystroke.modifiers.modified() => {
                self.move_slack_channel_submenu_selection(-1, cx);
            }
            "down" if !event.keystroke.modifiers.modified() => {
                self.move_slack_channel_submenu_selection(1, cx);
            }
            "home" if !event.keystroke.modifiers.modified() => {
                self.select_slack_channel_menu_boundary(false, cx);
            }
            "end" if !event.keystroke.modifiers.modified() => {
                self.select_slack_channel_menu_boundary(true, cx);
            }
            "enter" | "space" if !event.keystroke.modifiers.modified() => {
                self.activate_selected_slack_channel_menu(cx);
            }
            "right" if !event.keystroke.modifiers.modified() => {
                self.open_selected_slack_channel_submenu(cx);
            }
            "left"
                if !event.keystroke.modifiers.modified()
                    && self.slack_channel_menu_submenu.is_some() =>
            {
                self.close_slack_channel_submenu(cx);
            }
            _ => return false,
        }
        true
    }

    pub(in crate::ui::surface::state) fn handle_slack_history_menu_key_down(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.slack_history_menu_open {
            return false;
        }
        match event.keystroke.key.as_str() {
            "escape" => self.close_slack_history_menu(cx),
            "up" if !event.keystroke.modifiers.modified() => {
                self.move_slack_history_menu_selection(-1, cx);
            }
            "down" if !event.keystroke.modifiers.modified() => {
                self.move_slack_history_menu_selection(1, cx);
            }
            "enter" | "space" if !event.keystroke.modifiers.modified() => {
                self.activate_selected_slack_history_menu(cx);
            }
            _ => return false,
        }
        true
    }

    pub(in crate::ui::surface::state) fn handle_slack_all_threads_shortcut(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        if !event.keystroke.key.eq_ignore_ascii_case("t")
            || !event.keystroke.modifiers.platform
            || !event.keystroke.modifiers.shift
            || event.keystroke.modifiers.control
            || event.keystroke.modifiers.alt
            || event.keystroke.modifiers.function
        {
            return false;
        }
        self.activate_slack_all_threads(cx);
        true
    }
}
