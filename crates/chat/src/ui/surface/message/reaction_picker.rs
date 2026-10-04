use crate::ui::surface::{
    slack_palette, SlackMessageRenderContext, SlackPalette, SlackReactionPickerAnchor,
    SlackReactionPickerState, SlackReactionSkinToneSupport, SurfaceState,
};
use crate::ui::{
    alpha, div, point, px, relative, rgb, BoxShadow, Context, Div, FontWeight, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, StatefulInteractiveElement, Styled,
};
use gpui::{anchored, deferred, Anchor, Role, Stateful};
use gpui_components::backdrop::dismissible_click_away_with_key;

mod categories;
mod grid;
mod skin_tone;

use skin_tone::slack_reaction_picker_glyph;

const SLACK_REACTION_PICKER_WIDTH: f32 = 361.0;
const SLACK_REACTION_PICKER_HEIGHT: f32 = 540.0;
const SLACK_REACTION_PICKER_CATEGORY_HEIGHT: f32 = 42.0;
const SLACK_REACTION_PICKER_SEARCH_HEIGHT: f32 = 60.0;
const SLACK_REACTION_PICKER_LIST_HEIGHT: f32 = 334.0;
const SLACK_REACTION_PICKER_HANDY_HEIGHT: f32 = 42.0;
const SLACK_REACTION_PICKER_FOOTER_HEIGHT: f32 = 62.0;
#[derive(Clone, Copy)]
struct SlackReactionPickerHandySpec {
    reaction_name: &'static str,
    glyph: &'static str,
    accessibility_label: &'static str,
    skin_tone_support: SlackReactionSkinToneSupport,
}

const SLACK_REACTION_PICKER_HANDY_REACTIONS: [SlackReactionPickerHandySpec; 5] = [
    SlackReactionPickerHandySpec {
        reaction_name: "slightly_smiling_face",
        glyph: "🙂",
        accessibility_label: "React with slightly smiling face",
        skin_tone_support: SlackReactionSkinToneSupport::None,
    },
    SlackReactionPickerHandySpec {
        reaction_name: "+1",
        glyph: "👍",
        accessibility_label: "React with thumbs up",
        skin_tone_support: SlackReactionSkinToneSupport::Single,
    },
    SlackReactionPickerHandySpec {
        reaction_name: "heavy_check_mark",
        glyph: "✔️",
        accessibility_label: "React with check mark",
        skin_tone_support: SlackReactionSkinToneSupport::None,
    },
    SlackReactionPickerHandySpec {
        reaction_name: "heart",
        glyph: "❤️",
        accessibility_label: "React with heart",
        skin_tone_support: SlackReactionSkinToneSupport::None,
    },
    SlackReactionPickerHandySpec {
        reaction_name: "eyes",
        glyph: "👀",
        accessibility_label: "React with eyes",
        skin_tone_support: SlackReactionSkinToneSupport::None,
    },
];

impl SurfaceState {
    pub(in crate::ui::surface) fn render_slack_reaction_picker(
        &self,
        picker: &SlackReactionPickerState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let picker_surface = self.render_slack_reaction_picker_surface(picker, palette, cx);
        match picker.identity.anchor() {
            SlackReactionPickerAnchor::ReactionBar => div()
                .absolute()
                .bottom(px(31.0))
                .left(px(0.0))
                .size_0()
                .child(
                    deferred(anchored().anchor(Anchor::BottomLeft).child(picker_surface))
                        .with_priority(50),
                ),
            SlackReactionPickerAnchor::HoverAction
                if matches!(
                    picker.identity.render_context(),
                    SlackMessageRenderContext::Thread | SlackMessageRenderContext::Later
                ) =>
            {
                div()
                    .absolute()
                    .bottom(px(41.0))
                    .left(px(11.0))
                    .size_0()
                    .child(
                        deferred(anchored().anchor(Anchor::BottomLeft).child(picker_surface))
                            .with_priority(50),
                    )
            }
            SlackReactionPickerAnchor::HoverAction => div()
                .absolute()
                .bottom(px(41.0))
                .right(px(-6.0))
                .size_0()
                .child(
                    deferred(anchored().anchor(Anchor::BottomRight).child(picker_surface))
                        .with_priority(50),
                ),
        }
    }

    fn render_slack_reaction_picker_surface(
        &self,
        picker: &SlackReactionPickerState,
        palette: SlackPalette,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = picker.identity().element_id();
        let surface = div()
            .w(px(SLACK_REACTION_PICKER_WIDTH))
            .h(px(SLACK_REACTION_PICKER_HEIGHT))
            .rounded(px(6.0))
            .bg(rgb(palette.main_bg))
            .overflow_hidden()
            .shadow(slack_reaction_picker_shadow(palette))
            .flex()
            .flex_col()
            .child(self.render_slack_reaction_picker_categories(picker, cx))
            .child(self.render_slack_reaction_picker_search(cx))
            .child(self.render_slack_reaction_picker_list(picker, cx))
            .child(self.render_slack_reaction_picker_handy_reactions(picker, cx))
            .child(self.render_slack_reaction_picker_footer(cx));
        dismissible_click_away_with_key(
            surface,
            id,
            |this: &mut Self, _, _, cx| {
                this.close_slack_reaction_picker(cx);
            },
            cx,
        )
        .role(Role::Dialog)
        .aria_label("Emoji picker")
        .track_focus(&self.slack_reaction_picker_focus_handle)
        .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
            this.handle_slack_reaction_picker_surface_key(event, window, cx);
        }))
    }

    fn handle_slack_reaction_picker_surface_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut crate::ui::Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .slack_reaction_picker_search_input
            .read(cx)
            .focus_handle_clone()
            .is_focused(window)
        {
            return;
        }
        if self.handle_slack_key_down(event, cx) {
            cx.stop_propagation();
        }
    }

    fn render_slack_reaction_picker_search(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_REACTION_PICKER_SEARCH_HEIGHT))
            .flex_none()
            .px(px(17.5))
            .pt(px(14.0))
            .pb(px(8.0))
            .bg(rgb(palette.main_bg))
            .child(
                div()
                    .w_full()
                    .h(px(38.0))
                    .rounded(px(8.0))
                    .shadow(vec![BoxShadow {
                        color: alpha(palette.reaction_picker_focus_border, 0.30),
                        offset: point(px(0.0), px(0.0)),
                        blur_radius: px(0.0),
                        spread_radius: px(4.0),
                        inset: false,
                    }])
                    .child(self.slack_reaction_picker_search_input_entity(cx)),
            )
    }

    fn render_slack_reaction_picker_handy_reactions(
        &self,
        picker: &SlackReactionPickerState,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_REACTION_PICKER_HANDY_HEIGHT))
            .flex_none()
            .border_t_1()
            .border_b_1()
            .border_color(rgb(palette.reaction_picker_border))
            .bg(rgb(palette.reaction_picker_footer_bg))
            .flex()
            .items_center()
            .child(
                div()
                    .pl(px(15.0))
                    .flex_grow(1.0)
                    .text_size(px(13.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child("Handy Reactions"),
            )
            .child(
                div()
                    .id("slack-reaction-picker-handy-grid")
                    .role(Role::Grid)
                    .aria_label("Handy reactions")
                    .h_full()
                    .pr(px(5.0))
                    .flex()
                    .items_center()
                    .children(
                        SLACK_REACTION_PICKER_HANDY_REACTIONS
                            .iter()
                            .copied()
                            .enumerate()
                            .map(|(index, spec)| {
                                self.render_slack_reaction_picker_handy_reaction(
                                    picker, index, spec, cx,
                                )
                            }),
                    ),
            )
    }

    fn render_slack_reaction_picker_handy_reaction(
        &self,
        picker: &SlackReactionPickerState,
        index: usize,
        spec: SlackReactionPickerHandySpec,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let identity = picker.identity();
        let keyboard_identity = identity.clone();
        div()
            .id(("slack-reaction-picker-handy", index))
            .role(Role::GridCell)
            .aria_label(spec.accessibility_label)
            .focusable()
            .tab_stop(true)
            .w(px(38.0))
            .h(px(36.0))
            .flex_none()
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(palette.reaction_picker_hover_bg)))
            .focus_visible(move |style| style.bg(rgb(palette.reaction_picker_focus_bg)))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_reaction_picker_emoji(
                    &identity,
                    spec.reaction_name,
                    spec.skin_tone_support,
                    cx,
                );
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.activate_slack_reaction_picker_emoji(
                    &keyboard_identity,
                    spec.reaction_name,
                    spec.skin_tone_support,
                    cx,
                );
            }))
            .child(div().text_size(px(22.0)).line_height(relative(1.0)).child(
                slack_reaction_picker_glyph(
                    spec.glyph,
                    spec.skin_tone_support,
                    self.slack_preferred_skin_tone_selection(),
                ),
            ))
    }
}

fn slack_reaction_picker_shadow(palette: SlackPalette) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: rgb(palette.reaction_picker_border).into(),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
            inset: false,
        },
        BoxShadow {
            color: alpha(0x000000, 0.08),
            offset: point(px(0.0), px(4.0)),
            blur_radius: px(12.0),
            spread_radius: px(0.0),
            inset: false,
        },
    ]
}
