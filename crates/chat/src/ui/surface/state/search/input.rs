use super::{
    alpha, px, rgb, slack_palette, Context, Entity, Rc, SurfaceState, TextInput, TextInputAction,
    TextInputChange, TextInputProps, TextInputStyle,
};

impl SurfaceState {
    pub(crate) fn slack_search_input_entity(&self, cx: &mut Context<Self>) -> Entity<TextInput> {
        let props = self.slack_search_input_props(cx);
        self.slack_search_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_search_input.clone()
    }

    fn slack_search_input_props(&self, cx: &mut Context<Self>) -> TextInputProps {
        let palette = slack_palette(self.appearance_mode);
        let open = self.slack_search_open;
        let value = if !open && self.slack_search_results_open {
            format!("Search: {}", self.slack_search_committed_query)
        } else {
            self.slack_search_query.clone()
        };
        let placeholder = if open {
            "Search across people, channels, files, workflows, and more".to_string()
        } else {
            self.slack_workspace()
                .map(|workspace| format!("Search {}", workspace.workspace_name))
                .unwrap_or_else(|| "Search Slack".to_string())
        };
        let highlights = self.slack_search_input_highlights(&value);
        TextInputProps::single_line(value)
            .placeholder(placeholder)
            .request_focus(open)
            .style(TextInputStyle {
                height: px(if open { 38.0 } else { 28.0 }),
                min_height: px(if open { 38.0 } else { 28.0 }),
                padding_x: px(0.0),
                padding_y: px(0.0),
                radius: px(0.0),
                background: alpha(0x000000, 0.0),
                border: alpha(0x000000, 0.0),
                focused_border: alpha(0x000000, 0.0),
                text: rgb(if open { palette.main_text } else { 0xf8f8f8 }).into(),
                placeholder: rgb(if open {
                    palette.main_secondary_text
                } else {
                    0xf8f8f8
                })
                .into(),
                selection: alpha(0x1264a3, if open { 0.28 } else { 0.45 }),
                caret: rgb(if open { palette.main_text } else { 0xf8f8f8 }).into(),
                font_size: px(if open { 16.0 } else { 13.0 }),
                line_height: px(if open { 22.0 } else { 18.0 }),
                font_family: Some("Lato".into()),
            })
            .bordered(false)
            .highlights(highlights)
            .accessibility(
                self.slack_search_accessibility_id.clone(),
                "Search Slack messages",
            )
            .on_change(self.slack_search_on_change(cx))
            .on_submit(self.slack_search_on_submit(cx))
            .on_escape(self.slack_search_on_escape(cx))
            .on_up(self.slack_search_on_up(cx))
            .on_down(self.slack_search_on_down(cx))
            .on_focus(self.slack_search_on_focus(cx))
    }

    fn slack_search_on_change(&self, cx: &mut Context<Self>) -> TextInputChange {
        let surface = cx.entity();
        Rc::new(move |value, _window, cx| {
            surface.update(cx, |surface, cx| {
                surface.set_slack_search_query(value, cx);
            });
        })
    }

    fn slack_search_on_submit(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                surface.activate_selected_slack_search_option(cx);
            });
        })
    }

    fn slack_search_on_escape(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |window, cx| {
            surface.update(cx, |surface, cx| {
                surface.close_slack_search(cx);
                cx.focus_self(window);
            });
        })
    }

    fn slack_search_on_focus(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                surface.open_slack_search(cx);
            });
        })
    }

    fn slack_search_on_up(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                surface.move_slack_search_selection(-1, cx);
            });
        })
    }

    fn slack_search_on_down(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                surface.move_slack_search_selection(1, cx);
            });
        })
    }
}
