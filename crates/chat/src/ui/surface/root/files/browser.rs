use super::{slack_files_action_key, SLACK_FILES_HEADER_HEIGHT};
use crate::ui::surface::{
    slack_icon, SlackFilesMenu, SlackFilesScope, SlackShellIcon, SurfaceState,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, Context, Div, FontWeight, InteractiveElement, IntoElement, ParentElement, Role,
    StatefulInteractiveElement, Styled,
};

impl SurfaceState {
    pub(super) fn render_slack_files_browser(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .h_full()
            .flex()
            .flex_col()
            .relative()
            .child(
                div()
                    .h(px(SLACK_FILES_HEADER_HEIGHT))
                    .flex_none()
                    .px(px(39.0))
                    .flex()
                    .items_center()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xd1d2d3))
                    .child("All files"),
            )
            .child(
                div()
                    .flex_grow(1.0)
                    .min_h(px(0.0))
                    .px(px(32.0))
                    .pb(px(12.0))
                    .flex()
                    .flex_col()
                    .child(self.render_slack_files_search(cx))
                    .child(self.render_slack_files_controls(cx))
                    .child(self.render_slack_files_content(cx)),
            )
            .when(
                self.slack_files_menu == Some(SlackFilesMenu::Types),
                |this| this.child(self.render_slack_files_type_menu(cx)),
            )
            .when(
                self.slack_files_menu == Some(SlackFilesMenu::Sort),
                |this| this.child(self.render_slack_files_sort_menu(cx)),
            )
    }

    fn render_slack_files_search(&self, cx: &mut Context<Self>) -> Div {
        div()
            .h(px(42.0))
            .flex_none()
            .rounded(px(9.0))
            .border_2()
            .border_color(rgb(0x1d9bd1))
            .bg(rgb(0x1a1d21))
            .flex()
            .items_center()
            .child(
                div()
                    .w(px(48.0))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(SlackShellIcon::Search, 0xb9babd, 20.0, cx)),
            )
            .child(
                div()
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .child(self.slack_files_search_input_entity(cx)),
            )
            .when(!self.slack_files_search_query.is_empty(), |this| {
                this.child(
                    div()
                        .id("slack-files-search-clear")
                        .role(Role::Button)
                        .aria_label("Clear file search")
                        .focusable()
                        .tab_stop(true)
                        .w(px(42.0))
                        .h_full()
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x25282d)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.clear_slack_files_search(cx);
                        }))
                        .on_key_down(cx.listener(|this, event, window, cx| {
                            if slack_files_action_key(event) {
                                window.prevent_default();
                                cx.stop_propagation();
                                this.clear_slack_files_search(cx);
                            }
                        }))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(20.0))
                        .text_color(rgb(0xb9babd))
                        .child("×"),
                )
            })
    }

    fn render_slack_files_controls(&self, cx: &mut Context<Self>) -> Div {
        let compact = self.preview_width < 1050.0;
        div()
            .h(px(if compact { 104.0 } else { 70.0 }))
            .flex_none()
            .pt(px(24.0))
            .flex()
            .when(compact, |this| this.flex_wrap())
            .items_start()
            .justify_between()
            .gap(px(8.0))
            .child(self.render_slack_files_scope_group(cx))
            .child(self.render_slack_files_filter_group(cx))
    }

    fn render_slack_files_scope_group(&self, cx: &mut Context<Self>) -> Div {
        div().flex().gap(px(8.0)).children(
            SlackFilesScope::ALL
                .into_iter()
                .map(|scope| self.render_slack_files_scope_button(scope, cx)),
        )
    }

    fn render_slack_files_filter_group(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .when(
                self.slack_files_loading && !self.slack_files_rows.is_empty(),
                |this| {
                    this.child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(0x9ca2a9))
                            .child("Loading more…"),
                    )
                },
            )
            .when(
                self.slack_files_error.is_some() && !self.slack_files_rows.is_empty(),
                |this| this.child(self.render_slack_files_page_retry(cx)),
            )
            .child(self.render_slack_files_type_button(cx))
            .when(
                self.slack_files_scope == SlackFilesScope::All
                    && self.slack_files_committed_query.is_empty(),
                |this| this.child(self.render_slack_files_sort_button(cx)),
            )
    }

    fn render_slack_files_page_retry(&self, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        div()
            .id("slack-files-page-retry")
            .role(Role::Button)
            .aria_label("Retry loading more Slack files")
            .focusable()
            .tab_stop(true)
            .h(px(28.0))
            .px(px(10.0))
            .rounded(px(4.0))
            .bg(rgb(0x4a2527))
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
                this.retry_slack_files(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_files_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.retry_slack_files(cx);
                }
            }))
            .flex()
            .items_center()
            .font_weight(FontWeight::BOLD)
            .text_size(px(12.0))
            .text_color(rgb(0xf2d8d6))
            .child("Retry")
    }

    fn render_slack_files_scope_button(
        &self,
        scope: SlackFilesScope,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected = self.slack_files_scope == scope;
        div()
            .id(format!(
                "slack-files-scope-{}",
                scope.label().to_ascii_lowercase().replace(' ', "-")
            ))
            .role(Role::Button)
            .aria_label(scope.label())
            .aria_selected(selected)
            .focusable()
            .tab_stop(true)
            .h(px(28.0))
            .px(px(12.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(if selected { 0x36c5f0 } else { 0x56585e }))
            .bg(rgb(if selected { 0x36c5f0 } else { 0x1b1d21 }))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(if selected { 0x36c5f0 } else { 0x27292d })))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_files_scope(scope, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_files_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.select_slack_files_scope(scope, cx);
                }
            }))
            .flex()
            .items_center()
            .text_size(px(14.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(if selected { 0x101114 } else { 0xf8f8f8 }))
            .child(scope.label())
    }

    fn render_slack_files_type_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.slack_files_type_filters.len();
        let selected = self.slack_files_menu == Some(SlackFilesMenu::Types);
        div()
            .id("slack-files-types")
            .role(Role::Button)
            .aria_label(format!("Filter file types, {count} selected"))
            .aria_expanded(selected)
            .focusable()
            .tab_stop(true)
            .h(px(28.0))
            .px(px(12.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(0x36c5f0))
            .bg(rgb(0x36c5f0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x56d0f3)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_files_menu(SlackFilesMenu::Types, cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_files_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_files_menu(SlackFilesMenu::Types, cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(7.0))
            .text_size(px(14.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0x101114))
            .child("☰")
            .child(format!("{count} Types"))
            .child("⌄")
    }

    fn render_slack_files_sort_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let expanded = self.slack_files_menu == Some(SlackFilesMenu::Sort);
        div()
            .id("slack-files-sort")
            .role(Role::Button)
            .aria_label(format!("Sort files by {}", self.slack_files_sort.label()))
            .aria_expanded(expanded)
            .focusable()
            .tab_stop(true)
            .h(px(28.0))
            .px(px(12.0))
            .rounded(px(4.0))
            .border_1()
            .border_color(rgb(0x36c5f0))
            .bg(rgb(0x36c5f0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x56d0f3)))
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_files_menu(SlackFilesMenu::Sort, cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_files_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_files_menu(SlackFilesMenu::Sort, cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(7.0))
            .text_size(px(14.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0x101114))
            .child(self.slack_files_sort.label())
            .child("⌄")
    }
}
