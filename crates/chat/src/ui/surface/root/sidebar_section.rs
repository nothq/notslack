use gpui::prelude::FluentBuilder;
use gpui::{
    div, point, px, rgb, uniform_list, AnyElement, BoxShadow, Context, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, ListSizingBehavior, MouseButton, MouseDownEvent,
    ParentElement, Role, StatefulInteractiveElement, Styled,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use crate::ui::surface::{
    slack_palette, SlackSidebarSectionChoice, SlackSidebarSectionDialog, SlackSurfaceActionButton,
    SurfaceState,
};
use crate::ui::{alpha, SlackConversationKind, SlackSidebarSectionSort};

impl SurfaceState {
    pub(super) fn render_slack_sidebar_section_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        let dialog = self
            .slack_sidebar_section_dialog
            .as_ref()
            .expect("Slack sidebar section layer requires dialog state");
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .bg(alpha(0x000000, 0.56)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_sidebar_section_dialog(cx);
            }),
            cx,
        );
        div()
            .id("slack-sidebar-section-layer")
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
            .child(self.render_slack_sidebar_section_dialog(dialog, cx))
            .into_any_element()
    }

    fn render_slack_sidebar_section_dialog(
        &self,
        dialog: &SlackSidebarSectionDialog,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        self.slack_sidebar_section_dialog_container(&palette, cx)
            .child(self.render_slack_sidebar_section_dialog_header(&palette, cx))
            .child(self.slack_sidebar_section_name_input_entity(cx))
            .child(self.render_slack_sidebar_section_selection(dialog, &palette, cx))
            .child(self.render_slack_sidebar_section_choices(dialog, cx))
            .when_some(dialog.error.as_deref(), |this, error| {
                this.child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(0xe01e5a))
                        .child(error.to_string()),
                )
            })
            .child(self.render_slack_sidebar_section_dialog_footer(cx))
    }

    fn slack_sidebar_section_dialog_container(
        &self,
        palette: &crate::ui::surface::SlackPalette,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id("slack-sidebar-section-dialog")
            .role(Role::Dialog)
            .aria_label("Create a section")
            .relative()
            .w(px(540.0))
            .h(px(590.0))
            .max_w(gpui::relative(0.94))
            .max_h(gpui::relative(0.92))
            .rounded(px(10.0))
            .border_1()
            .border_color(rgb(palette.main_border))
            .bg(rgb(palette.main_bg))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.48),
                offset: point(px(0.0), px(8.0)),
                blur_radius: px(28.0),
                spread_radius: px(0.0),
                inset: false,
            }])
            .p(px(24.0))
            .flex()
            .flex_col()
            .gap(px(14.0))
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
    }

    fn render_slack_sidebar_section_dialog_header(
        &self,
        palette: &crate::ui::surface::SlackPalette,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(22.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.main_text))
                    .child("Create a section"),
            )
            .child(self.slack_sidebar_section_button(
                SlackSurfaceActionButton {
                    id: "slack-sidebar-section-close",
                    label: "Close",
                    action: SurfaceState::close_slack_sidebar_section_dialog,
                },
                false,
                cx,
            ))
    }

    fn render_slack_sidebar_section_selection(
        &self,
        dialog: &SlackSidebarSectionDialog,
        palette: &crate::ui::surface::SlackPalette,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(px(13.0))
                    .text_color(rgb(palette.main_secondary_text))
                    .child(format!("{} selected", dialog.selected_count())),
            )
            .child(self.slack_sidebar_section_button(
                SlackSurfaceActionButton {
                    id: "slack-sidebar-section-sort",
                    label: match dialog.sort {
                        SlackSidebarSectionSort::Recent => "Sidebar order",
                        SlackSidebarSectionSort::Alphabetical => "A–Z",
                    },
                    action: SurfaceState::toggle_slack_sidebar_section_sort,
                },
                false,
                cx,
            ))
    }

    fn render_slack_sidebar_section_dialog_footer(&self, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex_none()
            .flex()
            .justify_end()
            .child(self.slack_sidebar_section_button(
                SlackSurfaceActionButton {
                    id: "slack-sidebar-section-create",
                    label: "Create",
                    action: SurfaceState::submit_slack_sidebar_section,
                },
                true,
                cx,
            ))
    }

    fn render_slack_sidebar_section_choices(
        &self,
        dialog: &SlackSidebarSectionDialog,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        if dialog.choices.is_empty() {
            return div()
                .flex_grow(1.0)
                .min_h(px(0.0))
                .rounded(px(7.0))
                .border_1()
                .border_color(rgb(palette.main_border))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(14.0))
                .text_color(rgb(palette.main_secondary_text))
                .child("No conversations are available")
                .into_any_element();
        }
        let choices = dialog.choices.clone();
        let count = choices.len();
        let scroll_handle = dialog.scroll_handle.clone();
        let view = cx.entity();
        uniform_list(
            "slack-sidebar-section-choices",
            count,
            move |range, _window, cx| {
                let choices = choices.clone();
                view.update(cx, move |this, cx| {
                    range
                        .map(|index| {
                            this.render_slack_sidebar_section_choice(
                                index,
                                &choices[index],
                                count,
                                cx,
                            )
                        })
                        .collect::<Vec<_>>()
                })
            },
        )
        .with_sizing_behavior(ListSizingBehavior::Auto)
        .track_scroll(&scroll_handle)
        .flex_grow(1.0)
        .min_h(px(0.0))
        .rounded(px(7.0))
        .border_1()
        .border_color(rgb(palette.main_border))
        .into_any_element()
    }

    fn render_slack_sidebar_section_choice(
        &self,
        index: usize,
        choice: &SlackSidebarSectionChoice,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let keyboard_index = index;
        div()
            .id(format!(
                "slack-sidebar-section-choice-{}",
                choice.conversation_id
            ))
            .role(Role::ListBoxOption)
            .aria_label(format!(
                "{} {}",
                choice_kind_label(choice.kind),
                choice.label
            ))
            .aria_selected(choice.selected)
            .aria_position_in_set(index + 1)
            .aria_size_of_set(count)
            .focusable()
            .tab_stop(true)
            .h(px(42.0))
            .px(px(10.0))
            .cursor_pointer()
            .flex()
            .items_center()
            .gap(px(10.0))
            .hover(|style| style.bg(rgb(palette.attachment_bg)))
            .focus_visible(|style| style.bg(rgb(palette.attachment_bg)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_slack_sidebar_section_choice(index, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space")
                    && !event.keystroke.modifiers.modified()
                {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_sidebar_section_choice(keyboard_index, cx);
                }
            }))
            .child(self.slack_sidebar_section_choice_checkbox(choice.selected, &palette))
            .child(self.slack_sidebar_section_choice_kind(choice.kind, &palette))
            .child(self.slack_sidebar_section_choice_label(choice, &palette))
            .into_any_element()
    }

    fn slack_sidebar_section_choice_checkbox(
        &self,
        selected: bool,
        palette: &crate::ui::surface::SlackPalette,
    ) -> gpui::Div {
        div()
            .size(px(18.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(if selected {
                palette.link
            } else {
                palette.main_border
            }))
            .when(selected, |this| {
                this.bg(rgb(palette.link))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.0))
                    .text_color(rgb(0xffffff))
                    .child("✓")
            })
    }

    fn slack_sidebar_section_choice_kind(
        &self,
        kind: SlackConversationKind,
        palette: &crate::ui::surface::SlackPalette,
    ) -> gpui::Div {
        div()
            .w(px(74.0))
            .text_size(px(12.0))
            .text_color(rgb(palette.main_secondary_text))
            .child(choice_kind_label(kind))
    }

    fn slack_sidebar_section_choice_label(
        &self,
        choice: &SlackSidebarSectionChoice,
        palette: &crate::ui::surface::SlackPalette,
    ) -> gpui::Div {
        div()
            .min_w(px(0.0))
            .overflow_hidden()
            .text_ellipsis()
            .whitespace_nowrap()
            .text_size(px(14.0))
            .text_color(rgb(palette.main_text))
            .child(choice.label.clone())
    }

    fn slack_sidebar_section_button(
        &self,
        button: SlackSurfaceActionButton,
        primary: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let SlackSurfaceActionButton { id, label, action } = button;
        let palette = slack_palette(self.appearance_mode);
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(label)
            .focusable()
            .tab_stop(true)
            .h(px(36.0))
            .px(px(14.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(if primary {
                0x007a5a
            } else {
                palette.main_border
            }))
            .bg(rgb(if primary { 0x007a5a } else { palette.main_bg }))
            .cursor_pointer()
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(14.0))
            .font_weight(FontWeight::MEDIUM)
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
}

fn choice_kind_label(kind: SlackConversationKind) -> &'static str {
    match kind {
        SlackConversationKind::Channel => "Channel",
        SlackConversationKind::PrivateChannel => "Private",
        SlackConversationKind::DirectMessage => "DM",
        SlackConversationKind::GroupMessage => "Group DM",
        SlackConversationKind::Unknown => "Conversation",
    }
}
