use super::{
    alpha, div, px, rgb, slack_conversation_files_action_key, slack_icon, slack_palette, Context,
    Div, FluentBuilder, FontWeight, InteractiveElement, IntoElement, ParentElement, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::surface::{SlackConversationFilesMenu, SLACK_CONVERSATION_FILES_CONTROL_HEIGHT};
use crate::ui::{SlackConversationFilesFilter, SlackConversationFilesSort};
use gpui::Role;

impl SurfaceState {
    pub(super) fn render_slack_conversation_files_search(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div().h(px(52.0)).flex_none().px(px(32.0)).child(
            div()
                .h(px(36.0))
                .w_full()
                .rounded(px(8.0))
                .border_1()
                .border_color(alpha(palette.main_secondary_text, 0.28))
                .bg(rgb(palette.attachment_bg))
                .flex()
                .items_center()
                .child(
                    div()
                        .w(px(47.0))
                        .h_full()
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(slack_icon(
                            SlackShellIcon::Search,
                            palette.main_secondary_text,
                            17.0,
                            cx,
                        )),
                )
                .child(
                    div()
                        .flex_grow(1.0)
                        .min_w(px(0.0))
                        .child(self.slack_conversation_files_search_input_entity(cx)),
                )
                .when(
                    !self.slack_conversation_files_search_query.is_empty(),
                    |this| {
                        this.child(self.render_slack_conversation_files_search_clear(
                            palette.main_secondary_text,
                            cx,
                        ))
                    },
                ),
        )
    }

    fn render_slack_conversation_files_search_clear(
        &self,
        text_color: u32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("slack-conversation-files-search-clear")
            .role(Role::Button)
            .aria_label("Clear files and links search")
            .focusable()
            .tab_stop(true)
            .size(px(34.0))
            .flex_none()
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0x5e5d60, 0.12)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.clear_slack_conversation_files_search(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_conversation_files_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.clear_slack_conversation_files_search(cx);
                }
            }))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(20.0))
            .text_color(rgb(text_color))
            .child("×")
    }

    pub(super) fn render_slack_conversation_files_controls(&self, cx: &mut Context<Self>) -> Div {
        div()
            .h(px(SLACK_CONVERSATION_FILES_CONTROL_HEIGHT))
            .flex_none()
            .px(px(32.0))
            .pt(px(5.0))
            .pb(px(8.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div().flex().items_center().gap(px(8.0)).children(
                    SlackConversationFilesFilter::ALL
                        .into_iter()
                        .map(|filter| self.render_slack_conversation_files_filter(filter, cx)),
                ),
            )
            .child(self.render_slack_conversation_files_sort_button(cx))
    }

    fn render_slack_conversation_files_filter(
        &self,
        filter: SlackConversationFilesFilter,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let selected = self.slack_conversation_files_filter == filter;
        let label = filter.label();
        let (border, background, text) = slack_conversation_files_filter_colors(&palette, selected);
        div()
            .id(format!(
                "slack-conversation-files-filter-{}",
                label.to_ascii_lowercase()
            ))
            .role(Role::Button)
            .aria_label(label)
            .aria_selected(selected)
            .focusable()
            .tab_stop(true)
            .h(px(28.0))
            .px(px(11.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(border)
            .bg(background)
            .cursor_pointer()
            .hover(move |style| {
                style.bg(if selected {
                    alpha(0x1264a3, 1.0)
                } else {
                    alpha(palette.main_text, 0.07)
                })
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_conversation_files_filter(filter, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_conversation_files_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.select_slack_conversation_files_filter(filter, cx);
                }
            }))
            .flex()
            .items_center()
            .font_weight(FontWeight::BOLD)
            .text_size(px(13.0))
            .text_color(text)
            .child(label)
    }

    fn render_slack_conversation_files_sort_button(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let expanded = self.slack_conversation_files_menu == Some(SlackConversationFilesMenu::Sort);
        let label = self.slack_conversation_files_sort.label();
        div()
            .id("slack-conversation-files-sort")
            .role(Role::Button)
            .aria_label(format!("Sort files and links by {label}"))
            .aria_expanded(expanded)
            .focusable()
            .tab_stop(true)
            .h(px(28.0))
            .px(px(12.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(alpha(palette.main_secondary_text, 0.45))
            .bg(rgb(palette.attachment_bg))
            .cursor_pointer()
            .hover(move |style| style.bg(alpha(palette.main_text, 0.07)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_conversation_files_sort_menu(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_conversation_files_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_conversation_files_sort_menu(cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(7.0))
            .font_weight(FontWeight::BOLD)
            .text_size(px(13.0))
            .text_color(rgb(palette.main_text))
            .child(label)
            .child(slack_icon(
                SlackShellIcon::ChevronDown,
                palette.main_secondary_text,
                11.0,
                cx,
            ))
    }

    pub(super) fn render_slack_conversation_files_sort_menu(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let sorts: &[SlackConversationFilesSort] =
            if self.slack_conversation_files_committed_query.is_empty() {
                &SlackConversationFilesSort::CHRONOLOGICAL
            } else {
                &SlackConversationFilesSort::SEARCH
            };
        div()
            .absolute()
            .top(px(108.0))
            .right(px(32.0))
            .w(px(168.0))
            .py(px(6.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(alpha(palette.main_text, 0.2))
            .bg(rgb(palette.attachment_bg))
            .children(sorts.iter().copied().map(|sort| {
                let selected = sort == self.slack_conversation_files_sort;
                div()
                    .id(format!(
                        "slack-conversation-files-sort-{}",
                        sort.label().to_ascii_lowercase()
                    ))
                    .role(Role::MenuItem)
                    .aria_label(sort.label())
                    .aria_selected(selected)
                    .focusable()
                    .tab_stop(true)
                    .h(px(32.0))
                    .px(px(12.0))
                    .cursor_pointer()
                    .hover(move |style| style.bg(alpha(palette.main_text, 0.08)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_slack_conversation_files_sort(sort, cx);
                    }))
                    .on_key_down(cx.listener(move |this, event, window, cx| {
                        if slack_conversation_files_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.select_slack_conversation_files_sort(sort, cx);
                        }
                    }))
                    .flex()
                    .items_center()
                    .justify_between()
                    .font_weight(if selected {
                        FontWeight::BOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_size(px(14.0))
                    .text_color(rgb(palette.main_text))
                    .child(sort.label())
                    .when(selected, |this| this.child("✓"))
            }))
    }
}

fn slack_conversation_files_filter_colors(
    palette: &super::super::SlackPalette,
    selected: bool,
) -> (gpui::Hsla, gpui::Hsla, gpui::Hsla) {
    if selected {
        (
            alpha(0x1264a3, 1.0),
            rgb(0x1264a3).into(),
            rgb(0xffffff).into(),
        )
    } else {
        (
            alpha(palette.main_text, 0.3),
            rgb(palette.attachment_bg).into(),
            rgb(palette.main_text).into(),
        )
    }
}
