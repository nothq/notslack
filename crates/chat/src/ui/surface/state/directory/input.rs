use std::rc::Rc;

use gpui::{Context, Entity};
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use crate::ui::surface::{alpha, px, rgb, slack_palette, SurfaceState};

impl SurfaceState {
    pub(crate) fn initialize_slack_directory_input(&mut self, cx: &mut Context<Self>) {
        let props = self.slack_directory_input_props(cx);
        self.slack_directory_search_input
            .update(cx, |input, cx| input.apply_props(props, cx));
    }

    pub(crate) fn slack_directory_search_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let props = self.slack_directory_input_props(cx);
        self.slack_directory_search_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_directory_search_input.clone()
    }

    fn slack_directory_input_props(&self, cx: &mut Context<Self>) -> TextInputProps {
        TextInputProps::single_line(self.slack_directory_query.clone())
            .placeholder("Search people")
            .style(self.slack_directory_input_style())
            .bordered(false)
            .accessibility(
                self.slack_directory_search_accessibility_id.clone(),
                "Search people",
            )
            .on_change(self.slack_directory_change_action(cx))
            .on_submit(self.slack_directory_submit_action(cx))
            .on_escape(self.slack_directory_escape_action(cx))
            .on_up(self.slack_directory_move_action(cx, -1))
            .on_down(self.slack_directory_move_action(cx, 1))
    }

    fn slack_directory_change_action(&self, cx: &mut Context<Self>) -> TextInputChange {
        let surface = cx.entity().downgrade();
        Rc::new(move |query, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.set_slack_directory_query(query, cx);
                })
                .ok();
        })
    }

    fn slack_directory_submit_action(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.open_selected_slack_directory_person(cx);
                })
                .ok();
        })
    }

    fn slack_directory_escape_action(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |window, cx| {
            let focus = surface
                .update(cx, |surface, cx| {
                    if surface.slack_profile_panel.is_some() {
                        surface.close_slack_profile_panel(cx);
                        return None;
                    }
                    if !surface.slack_directory_query.is_empty() {
                        surface.clear_slack_directory_query(cx);
                        return None;
                    }
                    surface.leave_slack_directory(cx);
                    surface.record_current_slack_conversation_history();
                    Some(surface.focus_handle.clone())
                })
                .ok()
                .flatten();
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        })
    }

    fn slack_directory_move_action(
        &self,
        cx: &mut Context<Self>,
        direction: i32,
    ) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.move_slack_directory_selection(direction, cx);
                })
                .ok();
        })
    }

    fn slack_directory_input_style(&self) -> TextInputStyle {
        let palette = slack_palette(self.appearance_mode);
        TextInputStyle {
            height: px(38.0),
            min_height: px(38.0),
            padding_x: px(0.0),
            padding_y: px(0.0),
            radius: px(0.0),
            background: alpha(0x000000, 0.0),
            border: alpha(0x000000, 0.0),
            focused_border: alpha(0x000000, 0.0),
            text: rgb(palette.main_text).into(),
            placeholder: rgb(palette.main_secondary_text).into(),
            selection: alpha(0x1264a3, 0.35),
            caret: rgb(palette.main_text).into(),
            font_size: px(15.0),
            line_height: px(22.0),
            font_family: Some("Lato".into()),
        }
    }
}
