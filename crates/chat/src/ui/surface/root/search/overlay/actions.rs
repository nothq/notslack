use super::super::super::super::{
    div, px, rgb, slack_icon, Context, Div, FluentBuilder, FontWeight, InteractiveElement,
    IntoElement, ParentElement, SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};
use super::super::{consume_slack_search_action_key, slack_search_overlay_palette};
use super::{SLACK_SEARCH_ACTION_HEIGHT, SLACK_SEARCH_FOOTER_HEIGHT};
use crate::ui::surface::SlackSearchUnreadDraftPosition;
use gpui::Role;

#[derive(Clone, Copy)]
struct SlackSearchSelectionButtonSpec {
    id: &'static str,
    label: &'static str,
    icon: SlackShellIcon,
    delta: i32,
    margin: bool,
}

impl SurfaceState {
    pub(super) fn render_slack_search_dialog_content(
        &self,
        unread_drafts: &[SlackSearchUnreadDraftPosition],
        recent_history_indices: &[usize],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("slack-search-options")
            .role(Role::ListBox)
            .aria_label("Search options")
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_y_scroll()
            .track_scroll(&self.slack_quick_search_options_scroll_handle)
            .flex()
            .flex_col()
            .when(self.slack_search_has_api_query(), |this| {
                this.child(self.render_slack_search_query_action(cx))
            })
            .when(
                self.slack_search_has_api_query()
                    && self.slack_search_has_current_conversation_action(),
                |this| this.child(self.render_slack_search_current_conversation_action(cx)),
            )
            .when(self.slack_search_has_api_query(), |this| {
                this.child(self.render_slack_quick_search_results(cx))
                    .child(self.render_slack_quick_search_message_results(cx))
            })
            .when(!self.slack_search_has_api_query(), |this| {
                this.child(self.render_slack_search_empty_sections(
                    unread_drafts,
                    recent_history_indices,
                    cx,
                ))
            })
    }

    fn render_slack_search_query_action(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        let query = self.slack_search_query.trim().to_string();
        let option_index = 0;
        let selected = self.slack_search_selected_option == option_index;
        div()
            .id("slack-search-primary-option")
            .role(Role::ListBoxOption)
            .aria_label(format!("Show search results for {query}"))
            .aria_selected(selected)
            .aria_position_in_set(option_index + 1)
            .aria_size_of_set(self.slack_search_option_count())
            .focusable()
            .tab_stop(true)
            .h(px(SLACK_SEARCH_ACTION_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .when(selected, |this| this.bg(rgb(palette.selected_bg)))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(palette.hover_bg)))
            .focus_visible(move |style| style.bg(rgb(palette.hover_bg)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.select_slack_search_option(option_index, cx);
                }
            }))
            .on_click(cx.listener(|this, _, _, cx| {
                this.activate_slack_search_query(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if consume_slack_search_action_key(event, window, cx) {
                    this.activate_slack_search_query(cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(14.0))
            .child(slack_icon(SlackShellIcon::Search, palette.text, 18.0, cx))
            .child(slack_search_action_label(
                "Show results for: ",
                query,
                palette.text,
                palette.strong_text,
            ))
            .when(selected, |this| {
                this.child(slack_search_enter_keycap(
                    palette.key_bg,
                    palette.strong_text,
                ))
            })
    }

    fn render_slack_search_current_conversation_action(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        let query = self.slack_search_query.trim().to_string();
        let option_index = 1;
        let selected = self.slack_search_selected_option == option_index;
        div()
            .id("slack-search-current-conversation-option")
            .role(Role::ListBoxOption)
            .aria_label(format!("Show search results in this channel for {query}"))
            .aria_selected(selected)
            .aria_position_in_set(option_index + 1)
            .aria_size_of_set(self.slack_search_option_count())
            .focusable()
            .tab_stop(true)
            .h(px(SLACK_SEARCH_ACTION_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .when(selected, |this| this.bg(rgb(palette.selected_bg)))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(palette.hover_bg)))
            .focus_visible(move |style| style.bg(rgb(palette.hover_bg)))
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                if *hovered {
                    this.select_slack_search_option(option_index, cx);
                }
            }))
            .on_click(cx.listener(|this, _, _, cx| {
                this.activate_slack_search_in_current_conversation(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if consume_slack_search_action_key(event, window, cx) {
                    this.activate_slack_search_in_current_conversation(cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(14.0))
            .child(slack_icon(
                SlackShellIcon::ListSearch,
                palette.text,
                18.0,
                cx,
            ))
            .child(slack_search_action_label(
                "Show results in this channel for: ",
                query,
                palette.text,
                palette.strong_text,
            ))
            .when(selected, |this| {
                this.child(slack_search_enter_keycap(
                    palette.key_bg,
                    palette.strong_text,
                ))
            })
    }

    pub(super) fn render_slack_search_footer(
        &self,
        has_options: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        div()
            .h(px(SLACK_SEARCH_FOOTER_HEIGHT))
            .flex_none()
            .border_t_1()
            .border_color(rgb(palette.border))
            .px(px(16.0))
            .flex()
            .items_center()
            .when(has_options, |this| {
                this.child(self.render_slack_search_selection_button(
                    SlackSearchSelectionButtonSpec {
                        id: "slack-search-select-previous",
                        label: "Select previous search option",
                        icon: SlackShellIcon::ArrowUp,
                        delta: -1,
                        margin: false,
                    },
                    cx,
                ))
                .child(self.render_slack_search_selection_button(
                    SlackSearchSelectionButtonSpec {
                        id: "slack-search-select-next",
                        label: "Select next search option",
                        icon: SlackShellIcon::ArrowDown,
                        delta: 1,
                        margin: true,
                    },
                    cx,
                ))
                .child(slack_search_select_label(palette.secondary_text))
            })
            .child(div().flex_grow(1.0))
    }

    fn render_slack_search_selection_button(
        &self,
        spec: SlackSearchSelectionButtonSpec,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_search_overlay_palette(self.appearance_mode);
        div()
            .id(spec.id)
            .role(Role::Button)
            .aria_label(spec.label)
            .focusable()
            .tab_stop(true)
            .size(px(20.0))
            .when(spec.margin, |this| this.ml(px(4.0)))
            .rounded(px(4.0))
            .bg(rgb(palette.key_bg))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(palette.key_hover_bg)))
            .focus_visible(move |style| style.bg(rgb(palette.key_hover_bg)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.move_slack_search_selection(spec.delta, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if consume_slack_search_action_key(event, window, cx) {
                    this.move_slack_search_selection(spec.delta, cx);
                }
            }))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(spec.icon, palette.secondary_text, 14.0, cx))
    }
}

fn slack_search_action_label(
    prefix: &'static str,
    query: String,
    text_color: u32,
    strong_text_color: u32,
) -> Div {
    div()
        .flex_grow(1.0)
        .min_w(px(0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .flex()
        .items_center()
        .text_size(px(16.0))
        .line_height(px(22.0))
        .text_color(rgb(text_color))
        .child(prefix)
        .child(
            div()
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(strong_text_color))
                .child(query),
        )
}

fn slack_search_enter_keycap(background: u32, text_color: u32) -> Div {
    div()
        .h(px(24.0))
        .px(px(5.0))
        .flex_none()
        .rounded(px(4.0))
        .bg(rgb(background))
        .flex()
        .items_center()
        .text_size(px(12.0))
        .text_color(rgb(text_color))
        .child("Enter")
}

fn slack_search_select_label(text_color: u32) -> Div {
    div()
        .ml(px(6.0))
        .text_size(px(12.0))
        .text_color(rgb(text_color))
        .child("Select")
}
