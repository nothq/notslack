use super::slack_files_action_key;
use crate::ui::surface::{SlackFilesSort, SlackFilesTypeFilter, SurfaceState};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, Context, InteractiveElement, IntoElement, ParentElement, Role,
    StatefulInteractiveElement, Styled, Toggled,
};

impl SurfaceState {
    pub(super) fn render_slack_files_type_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("slack-files-types-menu")
            .role(Role::Menu)
            .aria_label("File type filters")
            .absolute()
            .right(px(196.0))
            .top(px(150.0))
            .w(px(260.0))
            .py(px(8.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(rgb(0x34363a))
            .bg(rgb(0x1a1d21))
            .flex()
            .flex_col()
            .children(
                SlackFilesTypeFilter::ALL
                    .into_iter()
                    .map(|filter| self.render_slack_files_type_menu_item(filter, cx)),
            )
    }

    fn render_slack_files_type_menu_item(
        &self,
        filter: SlackFilesTypeFilter,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let checked = self.slack_files_type_filters.contains(&filter);
        div()
            .id(format!(
                "slack-files-type-{}",
                filter.label().to_ascii_lowercase().replace(' ', "-")
            ))
            .role(Role::MenuItemCheckBox)
            .aria_label(filter.label())
            .aria_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .focusable()
            .tab_stop(true)
            .h(px(32.0))
            .px(px(12.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x1264a3)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_slack_files_type_filter(filter, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_files_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_files_type_filter(filter, cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(10.0))
            .text_size(px(14.0))
            .text_color(rgb(0xf8f8f8))
            .child(
                div()
                    .size(px(16.0))
                    .rounded(px(3.0))
                    .border_1()
                    .border_color(rgb(if checked { 0x1d9bd1 } else { 0x7c7f85 }))
                    .when(checked, |this| this.bg(rgb(0x1d9bd1)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(11.0))
                    .child(if checked { "✓" } else { "" }),
            )
            .child(filter.label())
    }

    pub(super) fn render_slack_files_sort_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("slack-files-sort-menu")
            .role(Role::Menu)
            .aria_label("File sort order")
            .absolute()
            .right(px(32.0))
            .top(px(150.0))
            .w(px(180.0))
            .py(px(8.0))
            .rounded(px(7.0))
            .border_1()
            .border_color(rgb(0x34363a))
            .bg(rgb(0x1a1d21))
            .flex()
            .flex_col()
            .children(
                SlackFilesSort::ALL
                    .into_iter()
                    .map(|sort| self.render_slack_files_sort_menu_item(sort, cx)),
            )
    }

    fn render_slack_files_sort_menu_item(
        &self,
        sort: SlackFilesSort,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let checked = self.slack_files_sort == sort;
        div()
            .id(format!(
                "slack-files-sort-{}",
                sort.label().to_ascii_lowercase().replace(' ', "-")
            ))
            .role(Role::MenuItemRadio)
            .aria_label(sort.label())
            .aria_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .focusable()
            .tab_stop(true)
            .h(px(32.0))
            .px(px(12.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x1264a3)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_slack_files_sort(sort, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_files_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.select_slack_files_sort(sort, cx);
                }
            }))
            .flex()
            .items_center()
            .justify_between()
            .text_size(px(14.0))
            .text_color(rgb(0xf8f8f8))
            .child(sort.label())
            .child(if checked { "✓" } else { "" })
    }
}
