use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, AppContext, Context, Div, ElementId, Entity, InteractiveElement, IntoElement,
    ParentElement, Render, Role, StatefulInteractiveElement, Styled, WeakEntity, Window,
};
use gpui_components::text_input::{
    TextInput, TextInputAction, TextInputChange, TextInputEnterBehavior, TextInputMode,
    TextInputProps, TextInputStyle,
};

use crate::ui::{alpha, AppearanceMode};

use super::{
    slack_icon, slack_palette, SlackComposerDocument, SlackMessageEditTarget, SlackShellIcon,
    SurfaceState,
};

const SLACK_MESSAGE_EDIT_HEIGHT: f32 = 80.0;
const SLACK_MESSAGE_EDIT_INPUT_HEIGHT: f32 = 38.0;
const SLACK_MESSAGE_EDIT_ACTIONS_HEIGHT: f32 = 40.0;

pub(crate) struct SlackMessageEditView {
    surface: WeakEntity<SurfaceState>,
    target: SlackMessageEditTarget,
    initial_document: SlackComposerDocument,
    document: SlackComposerDocument,
    input: Entity<TextInput>,
    accessibility_id: ElementId,
    focus_pending: bool,
    pending: bool,
    error: Option<String>,
}

impl SlackMessageEditView {
    pub(crate) fn new(
        surface: WeakEntity<SurfaceState>,
        target: SlackMessageEditTarget,
        document: SlackComposerDocument,
        cx: &mut Context<Self>,
    ) -> Self {
        let accessibility_id =
            ElementId::NamedInteger("slack-message-edit".into(), cx.entity_id().as_u64());
        let input_accessibility_id = accessibility_id.clone();
        let text = document.text().to_string();
        let input = cx.new(move |cx| {
            TextInput::new(
                TextInputProps::multiline(text)
                    .accessibility(input_accessibility_id, "Edit message"),
                cx,
            )
        });
        Self {
            surface,
            target,
            initial_document: document.clone(),
            document,
            input,
            accessibility_id,
            focus_pending: true,
            pending: false,
            error: None,
        }
    }

    pub(crate) fn target(&self) -> &SlackMessageEditTarget {
        &self.target
    }

    pub(crate) fn finish_pending(&mut self, error: Option<String>, cx: &mut Context<Self>) {
        self.pending = false;
        self.error = error;
        self.focus_pending = true;
        cx.notify();
    }

    fn can_save(&self) -> bool {
        !self.pending
            && !self.document.text().trim().is_empty()
            && !self.document.same_draft_state(&self.initial_document)
    }

    fn input_props(&mut self, cx: &mut Context<Self>) -> TextInputProps {
        let edit = cx.entity().downgrade();
        let on_change: TextInputChange = Rc::new(move |value, _window, cx| {
            edit.update(cx, |edit, cx| {
                if edit.pending || edit.document.text() == value {
                    return;
                }
                edit.document.apply_text_edit(&value);
                edit.error = None;
                cx.notify();
            })
            .ok();
        });
        let edit = cx.entity().downgrade();
        let on_submit: TextInputAction = Rc::new(move |_window, cx| {
            edit.update(cx, |edit, cx| edit.submit(cx)).ok();
        });
        let edit = cx.entity().downgrade();
        let on_escape: TextInputAction = Rc::new(move |_window, cx| {
            edit.update(cx, |edit, cx| edit.cancel(cx)).ok();
        });
        let palette = slack_palette(AppearanceMode::current(cx));
        let highlights = self.document.highlights(
            rgb(palette.main_text).into(),
            alpha(palette.send_disabled_border, 0.08),
            alpha(palette.send_disabled_border, 0.16),
            rgb(palette.link).into(),
        );
        let request_focus = std::mem::take(&mut self.focus_pending);
        TextInputProps::multiline(self.document.text().to_string())
            .mode(TextInputMode::Multiline {
                max_visible_lines: Some(1),
            })
            .style(slack_message_edit_input_style(AppearanceMode::current(cx)))
            .bordered(false)
            .disabled(self.pending)
            .request_focus(request_focus)
            .highlights(highlights)
            .enter_behavior(TextInputEnterBehavior::SubmitOnEnter)
            .accessibility(self.accessibility_id.clone(), "Edit message")
            .on_change(on_change)
            .on_submit(on_submit)
            .on_escape(on_escape)
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if !self.can_save() {
            return;
        }
        let draft = match self.document.export_message_draft() {
            Ok(draft) => draft,
            Err(error) => {
                self.error = Some(error);
                cx.notify();
                return;
            }
        };
        self.pending = true;
        self.error = None;
        let target = self.target.clone();
        let result = self.surface.update(cx, move |surface, cx| {
            surface.start_slack_message_edit(target, draft, cx)
        });
        match result {
            Ok(Ok(())) => cx.notify(),
            Ok(Err(error)) => self.finish_pending(Some(error), cx),
            Err(_) => self.finish_pending(
                Some("The Slack conversation closed while editing.".to_string()),
                cx,
            ),
        }
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        let target = self.target.clone();
        self.surface
            .update(cx, move |surface, cx| {
                surface.cancel_slack_message_edit(&target, cx);
            })
            .ok();
    }

    fn render_toolbar_icon(
        &self,
        icon: SlackShellIcon,
        _label: &'static str,
        circular: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(AppearanceMode::current(cx));
        div()
            .size(px(28.0))
            .rounded(px(if circular { 14.0 } else { 4.0 }))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(icon, palette.composer_icon, 18.0, cx))
    }

    fn render_cancel_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = !self.pending;
        let button = div()
            .id("slack-message-edit-cancel")
            .role(Role::Button)
            .aria_label("Cancel editing")
            .w(px(64.85))
            .h(px(28.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(0x8b8d8f))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(14.0))
            .line_height(px(20.0))
            .font_weight(gpui::FontWeight::BOLD)
            .text_color(rgb(if enabled { 0xf8f8f8 } else { 0x9a9b9e }))
            .child("Cancel");
        if enabled {
            button
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .hover(|style| style.bg(alpha(0xf8f8f8, 0.06)))
                .on_click(cx.listener(|this, _, _, cx| this.cancel(cx)))
        } else {
            button
        }
    }

    fn render_save_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let enabled = self.can_save();
        let button = div()
            .id("slack-message-edit-save")
            .role(Role::Button)
            .aria_label("Save changes")
            .w(px(56.0))
            .h(px(28.0))
            .rounded(px(4.0))
            .bg(rgb(if enabled { 0x007a5a } else { 0x2f3237 }))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(14.0))
            .line_height(px(20.0))
            .font_weight(gpui::FontWeight::BOLD)
            .text_color(rgb(if enabled { 0xffffff } else { 0x9a9b9e }))
            .child("Save");
        if enabled {
            button
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x148567)))
                .on_click(cx.listener(|this, _, _, cx| this.submit(cx)))
        } else {
            button
        }
    }

    fn render_actions(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w_full()
            .h(px(SLACK_MESSAGE_EDIT_ACTIONS_HEIGHT))
            .px(px(8.0))
            .flex_none()
            .flex()
            .items_center()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(self.render_toolbar_icon(SlackShellIcon::Plus, "Attach", true, cx))
                    .child(self.render_toolbar_icon(
                        SlackShellIcon::FormatToggle,
                        "Formatting",
                        false,
                        cx,
                    ))
                    .child(self.render_toolbar_icon(SlackShellIcon::Emoji, "Emoji", false, cx)),
            )
            .when_some(self.error.as_deref(), |this, error| {
                this.child(
                    div()
                        .ml(px(8.0))
                        .min_w(px(0.0))
                        .flex_grow(1.0)
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_size(px(12.0))
                        .text_color(rgb(0xe01e5a))
                        .child(error.to_string()),
                )
            })
            .when(self.error.is_none(), |this| {
                this.child(div().flex_grow(1.0))
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(self.render_cancel_button(cx))
                    .child(self.render_save_button(cx)),
            )
    }
}

impl Render for SlackMessageEditView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let props = self.input_props(cx);
        self.input
            .update(cx, |input, cx| input.apply_props(props, cx));
        let palette = slack_palette(AppearanceMode::current(cx));
        div()
            .id("slack-message-inline-editor")
            .w_full()
            .h(px(SLACK_MESSAGE_EDIT_HEIGHT))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.composer_focused_border))
            .bg(rgb(palette.composer_bg))
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .h(px(SLACK_MESSAGE_EDIT_INPUT_HEIGHT))
                    .flex_none()
                    .child(self.input.clone()),
            )
            .child(self.render_actions(cx))
    }
}

fn slack_message_edit_input_style(appearance_mode: AppearanceMode) -> TextInputStyle {
    let palette = slack_palette(appearance_mode);
    TextInputStyle {
        height: px(SLACK_MESSAGE_EDIT_INPUT_HEIGHT),
        min_height: px(SLACK_MESSAGE_EDIT_INPUT_HEIGHT),
        padding_x: px(12.0),
        padding_y: px(8.0),
        radius: px(7.0),
        background: rgb(palette.composer_bg).into(),
        border: alpha(palette.composer_border, 0.0),
        focused_border: alpha(palette.composer_focused_border, 0.0),
        text: rgb(palette.main_text).into(),
        placeholder: rgb(palette.composer_placeholder).into(),
        selection: alpha(0x1264a3, 0.28),
        caret: rgb(palette.main_text).into(),
        font_size: px(15.0),
        line_height: px(22.0),
        font_family: Some("Lato".into()),
    }
}
