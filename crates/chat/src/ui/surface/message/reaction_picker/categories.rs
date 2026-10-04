use crate::ui::surface::{
    slack_icon, slack_palette, SlackReactionPickerState, SlackShellIcon, SurfaceState,
    SLACK_REACTION_PICKER_CATEGORY_COUNT,
};
use crate::ui::{
    div, px, rgb, AnyElement, Context, Div, FluentBuilder, InteractiveElement, IntoElement,
    KeyDownEvent, ParentElement, StatefulInteractiveElement, Styled,
};
use gpui::Role;

use super::SLACK_REACTION_PICKER_CATEGORY_HEIGHT;

const SLACK_REACTION_PICKER_CATEGORY_WIDTH: f32 = 34.3;

#[derive(Clone, Copy)]
struct SlackReactionPickerCategorySpec {
    label: &'static str,
    icon: SlackShellIcon,
}

const SLACK_REACTION_PICKER_CATEGORIES: [SlackReactionPickerCategorySpec;
    SLACK_REACTION_PICKER_CATEGORY_COUNT] = [
    SlackReactionPickerCategorySpec {
        label: "Search",
        icon: SlackShellIcon::Search,
    },
    SlackReactionPickerCategorySpec {
        label: "Smileys & People",
        icon: SlackShellIcon::ReactionCategorySmileys,
    },
    SlackReactionPickerCategorySpec {
        label: "Animals & Nature",
        icon: SlackShellIcon::ReactionCategoryAnimals,
    },
    SlackReactionPickerCategorySpec {
        label: "Food & Drink",
        icon: SlackShellIcon::ReactionCategoryFood,
    },
    SlackReactionPickerCategorySpec {
        label: "Travel & Places",
        icon: SlackShellIcon::ReactionCategoryTravel,
    },
    SlackReactionPickerCategorySpec {
        label: "Activities",
        icon: SlackShellIcon::ReactionCategoryActivities,
    },
    SlackReactionPickerCategorySpec {
        label: "Objects",
        icon: SlackShellIcon::ReactionCategoryObjects,
    },
    SlackReactionPickerCategorySpec {
        label: "Symbols",
        icon: SlackShellIcon::ReactionCategorySymbols,
    },
    SlackReactionPickerCategorySpec {
        label: "Flags",
        icon: SlackShellIcon::ReactionCategoryFlags,
    },
    SlackReactionPickerCategorySpec {
        label: "Custom",
        icon: SlackShellIcon::ReactionCategoryCustom,
    },
];

impl SurfaceState {
    pub(super) fn render_slack_reaction_picker_categories(
        &self,
        picker: &SlackReactionPickerState,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-reaction-picker-categories")
            .role(Role::TabList)
            .aria_label("Category")
            .h(px(SLACK_REACTION_PICKER_CATEGORY_HEIGHT))
            .flex_none()
            .pt(px(4.0))
            .pl(px(7.0))
            .border_b_1()
            .border_color(rgb(palette.reaction_picker_border))
            .bg(rgb(palette.main_bg))
            .flex()
            .items_start()
            .children(
                SLACK_REACTION_PICKER_CATEGORIES
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(index, spec)| {
                        self.render_slack_reaction_picker_category(
                            index,
                            spec,
                            if picker.query.is_empty() {
                                picker.category_index == index
                            } else {
                                index == 0
                            },
                            cx,
                        )
                    }),
            )
    }

    fn render_slack_reaction_picker_category(
        &self,
        index: usize,
        spec: SlackReactionPickerCategorySpec,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id(("slack-reaction-picker-category", index))
            .role(Role::Tab)
            .aria_label(spec.label)
            .aria_selected(selected)
            .focusable()
            .tab_stop(true)
            .relative()
            .w(px(SLACK_REACTION_PICKER_CATEGORY_WIDTH))
            .h(px(38.0))
            .flex_none()
            .rounded_t(px(9.6))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(palette.reaction_picker_hover_bg)))
            .focus_visible(|style| style.bg(rgb(palette.reaction_picker_focus_bg)))
            .flex()
            .items_start()
            .justify_center()
            .pt(px(4.0))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_reaction_picker_category(index, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                let next_index = match event.keystroke.key.as_str() {
                    "left" if !event.keystroke.modifiers.modified() => {
                        Some(index.saturating_sub(1))
                    }
                    "right" if !event.keystroke.modifiers.modified() => {
                        Some((index + 1).min(SLACK_REACTION_PICKER_CATEGORIES.len() - 1))
                    }
                    "enter" | "space" if !event.keystroke.modifiers.modified() => Some(index),
                    _ => None,
                };
                let Some(next_index) = next_index else {
                    return;
                };
                window.prevent_default();
                cx.stop_propagation();
                this.select_slack_reaction_picker_category(next_index, cx);
            }))
            .when(selected, |this| {
                this.child(slack_reaction_picker_category_selection(palette.main_text))
            })
            .child(slack_reaction_picker_category_icon(
                spec,
                palette.main_secondary_text,
                cx,
            ))
    }
}

fn slack_reaction_picker_category_selection(color: u32) -> Div {
    div()
        .absolute()
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .h(px(3.0))
        .bg(rgb(color))
}

fn slack_reaction_picker_category_icon(
    spec: SlackReactionPickerCategorySpec,
    color: u32,
    cx: &mut Context<SurfaceState>,
) -> AnyElement {
    slack_icon(spec.icon, color, 22.0, cx)
}
