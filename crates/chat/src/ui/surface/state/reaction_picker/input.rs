use std::rc::Rc;

use gpui::Entity;
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use crate::ui::surface::{slack_palette, Context, SurfaceState};
use crate::ui::{alpha, px, rgb};

impl SurfaceState {
    pub(in crate::ui::surface) fn slack_reaction_picker_search_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let picker = self
            .slack_reaction_picker
            .as_ref()
            .expect("reaction picker search input requires an open picker");
        let props = TextInputProps::single_line(picker.query.clone())
            .placeholder("Search all emoji")
            .style(self.slack_reaction_picker_search_input_style())
            .accessibility(
                self.slack_reaction_picker_search_accessibility_id.clone(),
                "Emoji name",
            )
            .on_change(self.slack_reaction_picker_search_on_change(cx))
            .on_submit(self.slack_reaction_picker_search_on_submit(cx))
            .on_escape(self.slack_reaction_picker_search_on_escape(cx));
        self.slack_reaction_picker_search_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_reaction_picker_search_input.clone()
    }

    fn slack_reaction_picker_search_input_style(&self) -> TextInputStyle {
        let palette = slack_palette(self.appearance_mode);
        TextInputStyle {
            height: px(38.0),
            min_height: px(38.0),
            padding_x: px(12.0),
            padding_y: px(9.0),
            radius: px(8.0),
            background: rgb(palette.main_bg).into(),
            border: rgb(palette.reaction_picker_focus_border).into(),
            focused_border: rgb(palette.reaction_picker_focus_border).into(),
            text: rgb(palette.main_text).into(),
            placeholder: rgb(palette.main_secondary_text).into(),
            selection: alpha(0x1264a3, 0.28),
            caret: rgb(palette.main_text).into(),
            font_size: px(15.0),
            line_height: px(20.0),
            font_family: Some("Lato".into()),
        }
    }

    fn slack_reaction_picker_search_on_change(&self, cx: &mut Context<Self>) -> TextInputChange {
        let surface = cx.entity().downgrade();
        Rc::new(move |query, _window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.set_slack_reaction_picker_query(query, cx);
                })
                .ok();
        })
    }

    fn slack_reaction_picker_search_on_submit(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |_window, cx| {
            surface
                .update(cx, |surface, cx| {
                    surface.activate_first_slack_reaction_picker_emoji(cx);
                })
                .ok();
        })
    }

    fn slack_reaction_picker_search_on_escape(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity().downgrade();
        Rc::new(move |window, cx| {
            let focus = surface
                .update(cx, |surface, cx| {
                    surface.close_slack_reaction_picker(cx);
                    surface.focus_handle.clone()
                })
                .ok();
            if let Some(focus) = focus {
                window.focus(&focus, cx);
            }
        })
    }
}
