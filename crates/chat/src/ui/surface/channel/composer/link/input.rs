use std::rc::Rc;

use gpui::Entity;
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputProps, TextInputStyle,
};

use super::{alpha, px, rgb, AppearanceMode, Context, SurfaceState};

impl SurfaceState {
    pub(super) fn slack_composer_link_text_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let dialog = self
            .slack_composer_link_dialog
            .as_ref()
            .expect("Slack link text input requires dialog state");
        let props = TextInputProps::single_line(dialog.text.clone())
            .style(slack_link_dialog_input_style(self.appearance_mode, false))
            .accessibility(self.slack_link_text_accessibility_id.clone(), "Text")
            .on_change(self.slack_composer_link_text_on_change(cx))
            .on_submit(self.slack_composer_link_text_on_submit(cx))
            .on_escape(self.slack_composer_link_on_escape(cx));
        self.slack_link_text_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_link_text_input.clone()
    }

    pub(super) fn slack_composer_link_url_input_entity(
        &self,
        cx: &mut Context<Self>,
    ) -> Entity<TextInput> {
        let dialog = self
            .slack_composer_link_dialog
            .as_ref()
            .expect("Slack link URL input requires dialog state");
        let props = TextInputProps::single_line(dialog.raw_url.clone())
            .placeholder("https://example.com")
            .style(slack_link_dialog_input_style(self.appearance_mode, true))
            .accessibility(self.slack_link_url_accessibility_id.clone(), "Link")
            .on_change(self.slack_composer_link_url_on_change(cx))
            .on_submit(self.slack_composer_link_url_on_submit(cx))
            .on_escape(self.slack_composer_link_on_escape(cx));
        self.slack_link_url_input
            .update(cx, |input, cx| input.apply_props(props, cx));
        self.slack_link_url_input.clone()
    }

    fn slack_composer_link_text_on_change(&self, cx: &mut Context<Self>) -> TextInputChange {
        let surface = cx.entity();
        Rc::new(move |value, _window, cx| {
            surface.update(cx, |surface, cx| {
                surface.set_slack_composer_link_text(value, cx);
            });
        })
    }

    fn slack_composer_link_url_on_change(&self, cx: &mut Context<Self>) -> TextInputChange {
        let surface = cx.entity();
        Rc::new(move |value, _window, cx| {
            surface.update(cx, |surface, cx| {
                surface.set_slack_composer_link_url(value, cx);
            });
        })
    }

    fn slack_composer_link_text_on_submit(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |window, cx| {
            let focus = surface.update(cx, |surface, cx| {
                surface.slack_link_url_input.read(cx).focus_handle_clone()
            });
            window.focus(&focus, cx);
        })
    }

    fn slack_composer_link_url_on_submit(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                surface.save_slack_composer_link_dialog(cx);
            });
        })
    }

    fn slack_composer_link_on_escape(&self, cx: &mut Context<Self>) -> TextInputAction {
        let surface = cx.entity();
        Rc::new(move |_window, cx| {
            surface.update(cx, |surface, cx| {
                surface.close_slack_composer_link_dialog(cx);
            });
        })
    }
}

fn slack_link_dialog_input_style(appearance_mode: AppearanceMode, focused: bool) -> TextInputStyle {
    let (background, border, text, placeholder) = match appearance_mode {
        AppearanceMode::Light => (0xffffff, 0x616061, 0x1d1c1d, 0x6f6f6f),
        AppearanceMode::Dark => (0x1a1d21, 0x85888c, 0xf8f8f8, 0x9a9b9e),
    };
    TextInputStyle {
        height: px(36.0),
        min_height: px(36.0),
        padding_x: px(12.0),
        padding_y: px(7.0),
        radius: px(5.0),
        background: rgb(background).into(),
        border: rgb(if focused { 0x1264a3 } else { border }).into(),
        focused_border: rgb(0x1264a3).into(),
        text: rgb(text).into(),
        placeholder: rgb(placeholder).into(),
        selection: alpha(0x1264a3, 0.25),
        caret: rgb(0x1d1c1d).into(),
        font_size: px(16.0),
        line_height: px(22.0),
        font_family: Some("Lato".into()),
    }
}
