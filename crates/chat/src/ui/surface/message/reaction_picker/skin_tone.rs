use crate::ui::surface::{slack_palette, SlackReactionSkinToneSupport, SurfaceState};
use crate::ui::SlackSkinTone;
use crate::ui::{
    alpha, div, point, px, relative, rgb, AnyElement, BoxShadow, Context, Div, FluentBuilder,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    ParentElement, StatefulInteractiveElement, Styled,
};
use gpui::{Role, Stateful, Toggled};
use gpui_components::backdrop::dismissible_click_away;

use super::SLACK_REACTION_PICKER_FOOTER_HEIGHT;

struct SlackSkinToneFooterPresentation {
    explicit_selection: Option<SlackSkinTone>,
    displayed_selection: SlackSkinTone,
    loaded: bool,
    mutation_pending: bool,
    button_label: String,
    error_label: &'static str,
}

impl SurfaceState {
    pub(super) fn render_slack_reaction_picker_footer(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let presentation = self.slack_skin_tone_footer_presentation();
        div()
            .relative()
            .h(px(SLACK_REACTION_PICKER_FOOTER_HEIGHT))
            .flex_none()
            .bg(rgb(palette.reaction_picker_footer_bg))
            .flex()
            .items_center()
            .when(self.slack_skin_tone_error.is_some(), |footer| {
                footer.child(slack_skin_tone_error_label(presentation.error_label))
            })
            .child(self.render_slack_skin_tone_footer_controls(&presentation, cx))
    }

    fn slack_skin_tone_footer_presentation(&self) -> SlackSkinToneFooterPresentation {
        let preference = self.slack_preferred_skin_tone.as_ref();
        let explicit_selection = preference.and_then(|preference| preference.selection);
        let displayed_selection = explicit_selection.unwrap_or_default();
        let loaded = preference.is_some();
        let mutation_pending = self.slack_skin_tone_mutation_request.is_some();
        let current_label = slack_skin_tone_accessibility_label(displayed_selection);
        let error_label = if preference.is_some() {
            "Couldn’t save skin tone"
        } else {
            "Couldn’t load skin tone"
        };
        SlackSkinToneFooterPresentation {
            explicit_selection,
            displayed_selection,
            loaded,
            mutation_pending,
            button_label: format!(
                "Choose your default skin tone, currently selected: {current_label}."
            ),
            error_label,
        }
    }

    fn render_slack_skin_tone_footer_controls(
        &self,
        presentation: &SlackSkinToneFooterPresentation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let controls = div()
            .absolute()
            .right(px(16.0))
            .bottom(px(14.5))
            .flex()
            .flex_col()
            .items_end()
            .when(self.slack_skin_tone_menu_open, |controls| {
                controls.child(self.render_slack_skin_tone_menu(cx))
            })
            .child(
                div()
                    .p(px(3.0))
                    .child(self.render_slack_skin_tone_toggle_button(presentation, cx)),
            );
        if self.slack_skin_tone_menu_open {
            dismissible_click_away(
                controls,
                |this: &mut Self, _, _, cx| this.close_slack_skin_tone_menu(cx),
                cx,
            )
            .into_any_element()
        } else {
            controls.into_any_element()
        }
    }

    fn render_slack_skin_tone_toggle_button(
        &self,
        presentation: &SlackSkinToneFooterPresentation,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-reaction-picker-skin-tone-button")
            .role(Role::Button)
            .aria_label(presentation.button_label.clone())
            .track_focus(&self.slack_skin_tone_toggle_focus_handle)
            .focusable()
            .tab_stop(true)
            .border_1()
            .border_color(alpha(0x000000, 0.0))
            .rounded(px(10.0))
            .px(px(3.0))
            .py(px(2.0))
            .when(
                presentation.loaded && !presentation.mutation_pending,
                |button| {
                    button
                        .cursor_pointer()
                        .hover(move |style| style.bg(rgb(palette.reaction_picker_hover_bg)))
                        .focus_visible(move |style| style.bg(rgb(palette.reaction_picker_focus_bg)))
                },
            )
            .flex()
            .items_center()
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_skin_tone_menu(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if slack_skin_tone_toggle_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_skin_tone_menu(cx);
                }
            }))
            .child(div().text_size(px(23.0)).line_height(relative(1.0)).child(
                slack_reaction_picker_glyph(
                    "👋",
                    SlackReactionSkinToneSupport::Single,
                    Some(presentation.displayed_selection),
                ),
            ))
            .when(presentation.explicit_selection.is_none(), |button| {
                button.child(
                    div()
                        .mx(px(3.0))
                        .text_size(px(14.4))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(palette.main_text))
                        .child("Skin Tone"),
                )
            })
    }

    fn render_slack_skin_tone_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-reaction-picker-skin-tone-dialog")
            .role(Role::Dialog)
            .aria_label("Choose your default skin tone")
            .track_focus(&self.slack_skin_tone_menu_focus_handle)
            .focusable()
            .tab_stop(false)
            .mr(px(3.0))
            .mb(px(-3.0))
            .border_1()
            .border_color(rgb(palette.reaction_picker_border))
            .rounded(px(10.0))
            .px(px(8.0))
            .py(px(4.0))
            .bg(rgb(palette.main_bg))
            .flex()
            .flex_col()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "tab" {
                    window.prevent_default();
                    return;
                }
                if this.handle_slack_skin_tone_menu_key_down(event, cx) {
                    window.prevent_default();
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .mb(px(4.0))
                    .text_size(px(10.88))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child("Choose your default skin tone"),
            )
            .child(
                div()
                    .id("slack-reaction-picker-skin-tone-options")
                    .role(Role::RadioGroup)
                    .aria_label("Choose your default skin tone")
                    .flex()
                    .items_center()
                    .children(
                        self.slack_skin_tone_menu_choices()
                            .into_iter()
                            .map(|selection| self.render_slack_skin_tone_option(selection, cx)),
                    ),
            )
    }

    fn render_slack_skin_tone_option(
        &self,
        selection: SlackSkinTone,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let selected = self.slack_skin_tone_menu_selected == Some(selection);
        div()
            .id(format!(
                "slack-reaction-picker-skin-tone-option-{}",
                selection.preference_value()
            ))
            .role(Role::RadioButton)
            .aria_label(slack_skin_tone_accessibility_label(selection))
            .aria_toggled(if selected {
                Toggled::True
            } else {
                Toggled::False
            })
            .when(selected, |option| option.aria_active_descendant())
            .flex_none()
            .rounded(px(2.0))
            .cursor_pointer()
            .text_size(px(23.0))
            .line_height(relative(1.0))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_skin_tone(selection, cx);
            }))
            .child(
                div()
                    .rounded(px(8.0))
                    .when(selected, |emoji| {
                        emoji.shadow(vec![
                            BoxShadow {
                                color: rgb(palette.reaction_picker_focus_border).into(),
                                offset: point(px(0.0), px(0.0)),
                                blur_radius: px(0.0),
                                spread_radius: px(1.0),
                                inset: false,
                            },
                            BoxShadow {
                                color: alpha(palette.reaction_picker_focus_border, 0.30),
                                offset: point(px(0.0), px(0.0)),
                                blur_radius: px(0.0),
                                spread_radius: px(5.0),
                                inset: false,
                            },
                        ])
                    })
                    .child(slack_reaction_picker_glyph(
                        "👋",
                        SlackReactionSkinToneSupport::Single,
                        Some(selection),
                    )),
            )
    }
}

fn slack_skin_tone_error_label(label: &'static str) -> Div {
    div()
        .min_w(px(0.0))
        .flex_1()
        .pl(px(14.0))
        .text_size(px(11.0))
        .text_color(rgb(0xe01e5a))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .child(label)
}

fn slack_skin_tone_toggle_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}

pub(super) fn slack_reaction_picker_glyph(
    glyph: &str,
    support: SlackReactionSkinToneSupport,
    selection: Option<SlackSkinTone>,
) -> &str {
    if support != SlackReactionSkinToneSupport::Single {
        return glyph;
    }
    let Some(selection) = selection else {
        return glyph;
    };
    let skin_tone = match selection {
        SlackSkinTone::Default => emojis::SkinTone::Default,
        SlackSkinTone::Light => emojis::SkinTone::Light,
        SlackSkinTone::MediumLight => emojis::SkinTone::MediumLight,
        SlackSkinTone::Medium => emojis::SkinTone::Medium,
        SlackSkinTone::MediumDark => emojis::SkinTone::MediumDark,
        SlackSkinTone::Dark => emojis::SkinTone::Dark,
    };
    emojis::get(glyph)
        .and_then(|emoji| emoji.with_skin_tone(skin_tone))
        .map_or(glyph, emojis::Emoji::as_str)
}

fn slack_skin_tone_accessibility_label(selection: SlackSkinTone) -> &'static str {
    match selection {
        SlackSkinTone::Default => "No skin tone",
        SlackSkinTone::Light => "Light skin tone",
        SlackSkinTone::MediumLight => "Medium-light skin tone",
        SlackSkinTone::Medium => "Medium skin tone",
        SlackSkinTone::MediumDark => "Medium-dark skin tone",
        SlackSkinTone::Dark => "Dark skin tone",
    }
}
