use gpui::Role;
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use super::{
    alpha, div, point, px, rgb, slack_icon, AnyElement, BoxShadow, Context, FluentBuilder,
    FontWeight, InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement,
    SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::AppearanceMode;

const SLACK_LINK_DIALOG_WIDTH: f32 = 520.0;
const SLACK_LINK_DIALOG_HEIGHT: f32 = 316.0;
const SLACK_LINK_DIALOG_HORIZONTAL_PADDING: f32 = 24.0;
const SLACK_LINK_DIALOG_INPUT_WIDTH: f32 =
    SLACK_LINK_DIALOG_WIDTH - SLACK_LINK_DIALOG_HORIZONTAL_PADDING * 2.0;

mod header;
mod input;

struct SlackComposerLinkDialogView<'a> {
    title: &'static str,
    error: Option<&'a str>,
    can_save: bool,
    background: u32,
}

impl SurfaceState {
    pub(crate) fn render_slack_composer_link_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        let dialog = self
            .slack_composer_link_dialog
            .as_ref()
            .expect("Slack composer link layer requires dialog state");
        let title = if dialog.editing_existing_link {
            "Edit link"
        } else {
            "Add link"
        };
        let background = match self.appearance_mode {
            AppearanceMode::Light => 0xffffff,
            AppearanceMode::Dark => 0x1a1d21,
        };
        let dialog_view = SlackComposerLinkDialogView {
            title,
            error: dialog.error.as_deref(),
            can_save: dialog.can_save(),
            background,
        };
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .bg(alpha(0x000000, 0.56)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_composer_link_dialog(cx);
            }),
            cx,
        );
        div()
            .id("slack-composer-link-layer")
            .absolute()
            .top(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .child(backdrop)
            .child(self.render_slack_composer_link_dialog(dialog_view, cx))
            .into_any_element()
    }

    fn render_slack_composer_link_dialog(
        &self,
        dialog: SlackComposerLinkDialogView<'_>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("slack-composer-link-dialog")
            .role(Role::Dialog)
            .aria_label(dialog.title)
            .relative()
            .w(px(SLACK_LINK_DIALOG_WIDTH))
            .h(px(SLACK_LINK_DIALOG_HEIGHT))
            .max_w(gpui::relative(0.92))
            .rounded(px(8.0))
            .bg(rgb(dialog.background))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.28),
                offset: point(px(0.0), px(8.0)),
                blur_radius: px(28.0),
                spread_radius: px(0.0),
                inset: false,
            }])
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(self.render_slack_composer_link_header(dialog.title, cx))
            .child(self.render_slack_composer_link_text_control(cx))
            .child(self.render_slack_composer_link_url_control(cx))
            .when_some(dialog.error, |this, error| {
                this.child(
                    div()
                        .absolute()
                        .left(px(SLACK_LINK_DIALOG_HORIZONTAL_PADDING))
                        .right(px(SLACK_LINK_DIALOG_HORIZONTAL_PADDING))
                        .top(px(228.0))
                        .h(px(18.0))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .text_color(rgb(0xe01e5a))
                        .child(error.to_string()),
                )
            })
            .child(self.render_slack_composer_link_actions(dialog.can_save, cx))
    }

    fn render_slack_composer_link_text_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top(px(71.0))
            .left(px(SLACK_LINK_DIALOG_HORIZONTAL_PADDING))
            .w(px(SLACK_LINK_DIALOG_INPUT_WIDTH))
            .child(self.render_slack_composer_link_label("Text"))
            .child(
                div()
                    .mt(px(7.0))
                    .w_full()
                    .child(self.slack_composer_link_text_input_entity(cx)),
            )
    }

    fn render_slack_composer_link_url_control(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .top(px(157.0))
            .left(px(SLACK_LINK_DIALOG_HORIZONTAL_PADDING))
            .w(px(SLACK_LINK_DIALOG_INPUT_WIDTH))
            .child(self.render_slack_composer_link_label("Link"))
            .child(
                div()
                    .mt(px(7.0))
                    .w_full()
                    .rounded(px(5.0))
                    .shadow(vec![BoxShadow {
                        color: alpha(0x1d9bd1, 0.30),
                        offset: point(px(0.0), px(0.0)),
                        blur_radius: px(0.0),
                        spread_radius: px(4.0),
                        inset: false,
                    }])
                    .child(self.slack_composer_link_url_input_entity(cx)),
            )
    }

    fn render_slack_composer_link_label(&self, label: &'static str) -> impl IntoElement {
        div()
            .h(px(19.0))
            .text_size(px(15.0))
            .line_height(px(19.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(slack_link_dialog_text(self.appearance_mode)))
            .child(label)
    }

    fn render_slack_composer_link_actions(
        &self,
        can_save: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let text = slack_link_dialog_text(self.appearance_mode);
        let border = match self.appearance_mode {
            AppearanceMode::Light => 0x616061,
            AppearanceMode::Dark => 0x8b8d8f,
        };
        div()
            .absolute()
            .right(px(SLACK_LINK_DIALOG_HORIZONTAL_PADDING))
            .bottom(px(20.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(
                div()
                    .id("slack-composer-link-cancel")
                    .role(Role::Button)
                    .aria_label("Cancel")
                    .focusable()
                    .tab_stop(true)
                    .w(px(80.0))
                    .h(px(36.0))
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(rgb(border))
                    .cursor_pointer()
                    .hover(|style| style.bg(alpha(text, 0.06)))
                    .focus_visible(|style| style.bg(alpha(text, 0.06)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.close_slack_composer_link_dialog(cx);
                    }))
                    .on_key_down(cx.listener(|this, event, window, cx| {
                        if slack_link_dialog_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.close_slack_composer_link_dialog(cx);
                        }
                    }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(15.0))
                    .line_height(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(text))
                    .child("Cancel"),
            )
            .child(self.render_slack_composer_link_save_button(can_save, cx))
    }

    fn render_slack_composer_link_save_button(
        &self,
        enabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled_background = match self.appearance_mode {
            AppearanceMode::Light => 0xe8e8e8,
            AppearanceMode::Dark => 0x35373b,
        };
        let disabled_text = match self.appearance_mode {
            AppearanceMode::Light => 0x616061,
            AppearanceMode::Dark => 0x9a9b9e,
        };
        let button = div()
            .id("slack-composer-link-save")
            .role(Role::Button)
            .aria_label("Save")
            .w(px(80.0))
            .h(px(36.0))
            .rounded(px(4.0))
            .bg(rgb(if enabled {
                0x007a5a
            } else {
                disabled_background
            }))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(15.0))
            .line_height(px(20.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(if enabled { 0xffffff } else { disabled_text }))
            .child("Save");
        if enabled {
            button
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x148567)))
                .focus_visible(|style| style.bg(rgb(0x148567)))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.save_slack_composer_link_dialog(cx);
                }))
                .on_key_down(cx.listener(|this, event, window, cx| {
                    if slack_link_dialog_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.save_slack_composer_link_dialog(cx);
                    }
                }))
                .into_any_element()
        } else {
            button.into_any_element()
        }
    }
}

fn slack_link_dialog_text(appearance_mode: AppearanceMode) -> u32 {
    match appearance_mode {
        AppearanceMode::Light => 0x1d1c1d,
        AppearanceMode::Dark => 0xf8f8f8,
    }
}

fn slack_link_dialog_action_key(event: &gpui::KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
