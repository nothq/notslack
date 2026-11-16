//! Slack's "Set a status" dialog: a status box with an emoji button,
//! suggestions while it is empty, "Clear after" once it has a status, and
//! Cancel and Save.

use gpui::prelude::FluentBuilder;
use gpui::{
    deferred, div, point, px, relative, rgb, AnyElement, BoxShadow, Context, Div, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, ParentElement,
    Role, Stateful, StatefulInteractiveElement, Styled,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use crate::ui::alpha;
use crate::ui::surface::{
    slack_icon, slack_palette, slack_status_emoji_glyph, SlackPalette, SlackSelfStatusDialog,
    SlackShellIcon, SlackStatusExpirationPreset, SurfaceState, SLACK_STATUS_SUGGESTIONS,
};

const DIALOG_WIDTH: f32 = 520.0;
const EMOJI_PICKER_COLUMNS: usize = 9;
const PRIMARY_GREEN: u32 = 0x007a5a;
const ROW_HOVER_BG: u32 = 0x1264a3;

impl SurfaceState {
    pub(super) fn render_slack_self_status_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        let dialog = self
            .slack_self_status_dialog
            .as_ref()
            .expect("Slack status layer requires dialog state");
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .bg(alpha(0x000000, 0.56)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_self_status_dialog(cx);
            }),
            cx,
        );
        div()
            .id("slack-self-status-layer")
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
            .child(self.render_slack_self_status_dialog(dialog, cx))
            .into_any_element()
    }

    fn render_slack_self_status_dialog(
        &self,
        dialog: &SlackSelfStatusDialog,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let body = div()
            .px(px(28.0))
            .pb(px(20.0))
            .flex()
            .flex_col()
            .gap(px(16.0))
            .child(self.slack_status_input_row(dialog, palette, cx))
            .map(|this| {
                if dialog.is_empty() {
                    this.child(self.slack_status_suggestions(palette, cx))
                } else {
                    this.child(self.slack_status_clear_after(dialog, palette, cx))
                }
            })
            .when_some(dialog.error.as_deref(), |this, error| {
                this.child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(0xe01e5a))
                        .child(error.to_string()),
                )
            });
        slack_status_dialog_container(palette, cx)
            .child(slack_status_dialog_header(palette, cx))
            .child(body)
            .child(slack_status_dialog_footer(dialog, palette, cx))
    }

    fn slack_status_input_row(
        &self,
        dialog: &SlackSelfStatusDialog,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Div {
        let emoji = slack_status_emoji_glyph(&dialog.emoji);
        let emoji_button = div()
            .id("slack-self-status-emoji")
            .role(Role::Button)
            .aria_label("Choose an emoji")
            .size(px(32.0))
            .flex_none()
            .rounded(px(6.0))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|style| style.bg(rgb(palette.composer_icon_bg)))
            .on_click(cx.listener(|this, _, _, cx| this.toggle_slack_status_emoji_picker(cx)))
            .child(match emoji {
                Some(glyph) => div().text_size(px(20.0)).child(glyph).into_any_element(),
                None => slack_icon(
                    SlackShellIcon::Emoji,
                    palette.composer_placeholder,
                    20.0,
                    cx,
                ),
            });
        div()
            .relative()
            .child(
                div()
                    .h(px(44.0))
                    .pl(px(6.0))
                    .pr(px(8.0))
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgb(palette.composer_border))
                    .bg(rgb(palette.composer_bg))
                    .child(emoji_button)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(self.slack_self_status_text_input_entity(cx)),
                    ),
            )
            .when(dialog.emoji_picker_open, |this| {
                // Deferred, so it paints over the rest of the dialog.
                this.child(deferred(
                    self.slack_status_emoji_picker(dialog, palette, cx),
                ))
            })
    }

    fn slack_status_emoji_picker(
        &self,
        dialog: &SlackSelfStatusDialog,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let choices = self.slack_status_emoji_choices(&dialog.emoji_query);
        let rows = choices
            .chunks(EMOJI_PICKER_COLUMNS)
            .map(|row| {
                div().flex().children(row.iter().map(|choice| {
                    let shortcode = choice.shortcode.clone();
                    div()
                        .id(format!("slack-status-emoji-{shortcode}"))
                        .role(Role::Button)
                        .aria_label(format!(":{shortcode}:"))
                        .size(px(36.0))
                        .rounded(px(6.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(22.0))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(palette.composer_icon_bg)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.choose_slack_status_emoji(&shortcode, cx);
                        }))
                        .child(choice.glyph.clone())
                }))
            })
            .collect::<Vec<_>>();
        div()
            .id("slack-self-status-emoji-picker")
            .absolute()
            .top(px(50.0))
            .left(px(0.0))
            .w(px(36.0 * EMOJI_PICKER_COLUMNS as f32 + 26.0))
            .h(px(340.0))
            .p(px(12.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(palette.main_border))
            .bg(rgb(palette.main_bg))
            .shadow(slack_status_shadow())
            .occlude()
            .child(self.slack_self_status_emoji_input_entity(cx))
            .child(
                div()
                    .id("slack-self-status-emoji-grid")
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_y_scroll()
                    .when(rows.is_empty(), |this| {
                        this.child(
                            div()
                                .p(px(8.0))
                                .text_size(px(13.0))
                                .text_color(rgb(palette.main_secondary_text))
                                .child("No emoji match your search."),
                        )
                    })
                    .children(rows),
            )
    }

    fn slack_status_suggestions(&self, palette: SlackPalette, cx: &mut Context<Self>) -> Div {
        let workspace_name = self
            .slack_workspace()
            .map(|workspace| workspace.workspace_name.clone())
            .filter(|name| !name.is_empty());
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .pb(px(6.0))
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child(match workspace_name {
                        Some(name) => format!("For {name}"),
                        None => "Suggestions".to_string(),
                    }),
            )
            .children(SLACK_STATUS_SUGGESTIONS.into_iter().map(|suggestion| {
                div()
                    .id(format!("slack-status-suggestion-{}", suggestion.emoji))
                    .role(Role::Button)
                    .aria_label(suggestion.text)
                    .h(px(36.0))
                    .mx(px(-12.0))
                    .px(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .rounded(px(4.0))
                    .text_size(px(15.0))
                    .text_color(rgb(palette.main_text))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(ROW_HOVER_BG)).text_color(rgb(0xffffff)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.choose_slack_status_suggestion(suggestion, cx);
                    }))
                    .child(
                        div()
                            .text_size(px(18.0))
                            .child(slack_status_emoji_glyph(suggestion.emoji).unwrap_or_default()),
                    )
                    .child(
                        div()
                            .flex()
                            .child(div().font_weight(FontWeight::BOLD).child(suggestion.text))
                            .child(format!(" — {}", suggestion.expiration.label())),
                    )
            }))
    }

    fn slack_status_clear_after(
        &self,
        dialog: &SlackSelfStatusDialog,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Div {
        let button = div()
            .id("slack-self-status-expiration")
            .role(Role::Button)
            .aria_label("Clear after")
            .h(px(36.0))
            .px(px(12.0))
            .flex()
            .items_center()
            .justify_between()
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(palette.composer_border))
            .bg(rgb(palette.composer_bg))
            .text_size(px(15.0))
            .text_color(rgb(palette.main_text))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| this.toggle_slack_status_expiration_menu(cx)))
            .child(dialog.expiration.label())
            .child(slack_icon(
                SlackShellIcon::ChevronDown,
                palette.main_text,
                14.0,
                cx,
            ));
        div()
            .flex()
            .flex_col()
            .gap(px(6.0))
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child("Clear after"),
            )
            .child(
                div()
                    .relative()
                    .child(button)
                    .when(dialog.expiration_menu_open, |this| {
                        this.child(deferred(slack_status_expiration_menu(dialog, palette, cx)))
                    }),
            )
    }
}

fn slack_status_expiration_menu(
    dialog: &SlackSelfStatusDialog,
    palette: SlackPalette,
    cx: &mut Context<SurfaceState>,
) -> Stateful<Div> {
    div()
        .id("slack-self-status-expiration-menu")
        .absolute()
        .top(px(40.0))
        .left(px(0.0))
        .right(px(0.0))
        .py(px(8.0))
        .rounded(px(8.0))
        .border_1()
        .border_color(rgb(palette.main_border))
        .bg(rgb(palette.main_bg))
        .shadow(slack_status_shadow())
        .occlude()
        .children(SlackStatusExpirationPreset::ALL.into_iter().map(|preset| {
            let selected = preset == dialog.expiration;
            div()
                .id(format!("slack-status-clear-{}", preset.label()))
                .role(Role::MenuItem)
                .aria_label(preset.label())
                .h(px(32.0))
                .px(px(16.0))
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(15.0))
                .text_color(rgb(palette.main_text))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(ROW_HOVER_BG)).text_color(rgb(0xffffff)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.choose_slack_status_expiration(preset, cx);
                }))
                .child(preset.label())
                .when(selected, |this| this.child("✓"))
        }))
}

fn slack_status_dialog_container(
    palette: SlackPalette,
    cx: &mut Context<SurfaceState>,
) -> Stateful<Div> {
    div()
        .id("slack-self-status-dialog")
        .role(Role::Dialog)
        .aria_label("Set a status")
        .relative()
        .w(px(DIALOG_WIDTH))
        .max_w(relative(0.92))
        .rounded(px(10.0))
        .bg(rgb(palette.main_bg))
        .border_1()
        .border_color(rgb(palette.main_border))
        .shadow(slack_status_shadow())
        .flex()
        .flex_col()
        .occlude()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|_: &mut SurfaceState, _: &MouseDownEvent, _, cx| {
                cx.stop_propagation();
            }),
        )
}

fn slack_status_dialog_header(palette: SlackPalette, cx: &mut Context<SurfaceState>) -> Div {
    div()
        .px(px(28.0))
        .pt(px(20.0))
        .pb(px(16.0))
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(px(22.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(palette.main_text))
                .child("Set a status"),
        )
        .child(
            div()
                .id("slack-self-status-close")
                .role(Role::Button)
                .aria_label("Close")
                .size(px(36.0))
                .rounded(px(6.0))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|style| style.bg(rgb(palette.composer_icon_bg)))
                .on_click(cx.listener(|this, _, _, cx| this.close_slack_self_status_dialog(cx)))
                .child(slack_icon(
                    SlackShellIcon::Close,
                    palette.main_text,
                    18.0,
                    cx,
                )),
        )
}

fn slack_status_dialog_footer(
    dialog: &SlackSelfStatusDialog,
    palette: SlackPalette,
    cx: &mut Context<SurfaceState>,
) -> Div {
    div()
        .px(px(28.0))
        .py(px(16.0))
        .flex()
        .items_center()
        .gap(px(12.0))
        .border_t_1()
        .border_color(rgb(palette.main_border))
        .when(dialog.had_status, |this| {
            this.child(slack_status_button(
                "slack-self-status-clear",
                "Clear status",
                false,
                palette,
                cx,
                SurfaceState::clear_slack_self_status,
            ))
        })
        .child(div().flex_1())
        .child(slack_status_button(
            "slack-self-status-cancel",
            "Cancel",
            false,
            palette,
            cx,
            SurfaceState::close_slack_self_status_dialog,
        ))
        .child(slack_status_button(
            "slack-self-status-save",
            if dialog.saving { "Saving…" } else { "Save" },
            !dialog.is_empty(),
            palette,
            cx,
            SurfaceState::submit_slack_self_status,
        ))
}

fn slack_status_button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    palette: SlackPalette,
    cx: &mut Context<SurfaceState>,
    action: fn(&mut SurfaceState, &mut Context<SurfaceState>),
) -> Stateful<Div> {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .focusable()
        .tab_stop(true)
        .h(px(36.0))
        .px(px(16.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(rgb(if primary {
            PRIMARY_GREEN
        } else {
            palette.main_border
        }))
        .bg(rgb(if primary {
            PRIMARY_GREEN
        } else {
            palette.main_bg
        }))
        .cursor_pointer()
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(15.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(if primary { 0xffffff } else { palette.main_text }))
        .hover(move |style| style.bg(rgb(if primary { 0x148567 } else { 0x303337 })))
        .focus_visible(|style| style.border_color(rgb(0x1d9bd1)))
        .on_click(cx.listener(move |this, _, _, cx| action(this, cx)))
        .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space")
                && !event.keystroke.modifiers.modified()
            {
                window.prevent_default();
                cx.stop_propagation();
                action(this, cx);
            }
        }))
        .child(label)
}

fn slack_status_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: alpha(0x000000, 0.48),
        offset: point(px(0.0), px(8.0)),
        blur_radius: px(28.0),
        spread_radius: px(0.0),
        inset: false,
    }]
}
