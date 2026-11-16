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

/// The editor's border and its one-line input.
const SLACK_MESSAGE_EDIT_FRAME_HEIGHT: f32 = 2.0;
const SLACK_MESSAGE_EDIT_INPUT_HEIGHT: f32 = 38.0;
const SLACK_MESSAGE_EDIT_LINE_HEIGHT: f32 = 22.0;
const SLACK_MESSAGE_EDIT_ACTIONS_HEIGHT: f32 = 40.0;
/// The editor grows with the message up to this many lines, then scrolls.
const SLACK_MESSAGE_EDIT_MAX_VISIBLE_LINES: usize = 10;

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
    /// How many lines the input showed last render; the message list is
    /// remeasured when it changes.
    visible_lines: usize,
    /// Whether the message list has scrolled the editor into view yet.
    revealed: bool,
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
        // The input measures its lines while laying out; render again then
        // so the editor's height follows.
        cx.observe(&input, |_, _, cx| cx.notify()).detach();
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
            visible_lines: 1,
            revealed: false,
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
                max_visible_lines: Some(SLACK_MESSAGE_EDIT_MAX_VISIBLE_LINES),
            })
            .style(slack_message_edit_input_style(
                AppearanceMode::current(cx),
                slack_message_edit_input_height(self.visible_lines),
            ))
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
        cx.notify();
        let target = self.target.clone();
        let surface = self.surface.clone();
        let edit = cx.entity().downgrade();
        // The surface reads this editor, so it runs once this update ends;
        // reading an entity mid-update panics.
        cx.defer(move |cx| {
            let result = surface.update(cx, move |surface, cx| {
                surface.start_slack_message_edit(target, draft, cx)
            });
            edit.update(cx, |edit, cx| match result {
                Ok(Ok(())) => cx.notify(),
                Ok(Err(error)) => edit.finish_pending(Some(error), cx),
                Err(_) => edit.finish_pending(
                    Some("The Slack conversation closed while editing.".to_string()),
                    cx,
                ),
            })
            .ok();
        });
    }

    pub(crate) fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        let target = self.target.clone();
        let surface = self.surface.clone();
        // Deferred like submit: the surface reads this editor to check it is
        // the one being cancelled.
        cx.defer(move |cx| {
            surface
                .update(cx, move |surface, cx| {
                    surface.cancel_slack_message_edit(&target, cx);
                })
                .ok();
        });
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let visible_lines = self
            .input
            .read(cx)
            .measured_line_count()
            .clamp(1, SLACK_MESSAGE_EDIT_MAX_VISIBLE_LINES);
        if visible_lines != self.visible_lines || !self.revealed {
            self.visible_lines = visible_lines;
            self.revealed = true;
            let surface = self.surface.clone();
            cx.defer({
                let surface = surface.clone();
                move |cx| {
                    surface
                        .update(cx, |surface, cx| {
                            surface.slack_message_list_state.remeasure();
                            cx.notify();
                        })
                        .ok();
                }
            });
            // Keep the whole editor in view as it opens and grows, even on
            // the last message. The list only knows the new height once it
            // has laid the row out again, so reveal it the frame after.
            let message_id = self.target.message_timestamp.as_str().to_string();
            window.on_next_frame(move |window, _| {
                window.on_next_frame(move |_, cx| {
                    surface
                        .update(cx, |surface, cx| {
                            if let Some(index) = surface
                                .slack_message_rows
                                .iter()
                                .position(|row| row.id == message_id)
                            {
                                surface
                                    .slack_message_list_state
                                    .scroll_to_reveal_item(index);
                                cx.notify();
                            }
                        })
                        .ok();
                });
                window.refresh();
            });
        }
        let input_height = slack_message_edit_input_height(self.visible_lines);
        let props = self.input_props(cx);
        self.input
            .update(cx, |input, cx| input.apply_props(props, cx));
        let palette = slack_palette(AppearanceMode::current(cx));
        div()
            .id("slack-message-inline-editor")
            .w_full()
            .h(px(input_height
                + SLACK_MESSAGE_EDIT_ACTIONS_HEIGHT
                + SLACK_MESSAGE_EDIT_FRAME_HEIGHT))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.composer_focused_border))
            .bg(rgb(palette.composer_bg))
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .h(px(input_height))
                    .flex_none()
                    .child(self.input.clone()),
            )
            .child(self.render_actions(cx))
    }
}

fn slack_message_edit_input_height(visible_lines: usize) -> f32 {
    SLACK_MESSAGE_EDIT_INPUT_HEIGHT
        + visible_lines.saturating_sub(1) as f32 * SLACK_MESSAGE_EDIT_LINE_HEIGHT
}

fn slack_message_edit_input_style(appearance_mode: AppearanceMode, height: f32) -> TextInputStyle {
    let palette = slack_palette(appearance_mode);
    TextInputStyle {
        height: px(height),
        min_height: px(height),
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
        line_height: px(SLACK_MESSAGE_EDIT_LINE_HEIGHT),
        font_family: Some("Lato".into()),
    }
}
