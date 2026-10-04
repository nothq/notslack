use super::{slack_files_action_key, SLACK_FILES_HEADER_HEIGHT};
use crate::ui::surface::{
    slack_icon, SlackFilesSidebarSelection, SlackFilesTypeFilter, SlackShellIcon, SurfaceState,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Role, StatefulInteractiveElement, Styled,
};

#[derive(Clone, Copy)]
enum SlackFilesSidebarAction {
    All,
    Type(SlackFilesTypeFilter),
    RecentlyViewed,
}

struct SlackFilesSidebarButtonSpec {
    id: gpui::ElementId,
    label: &'static str,
    selected: bool,
    icon: AnyElement,
    action: SlackFilesSidebarAction,
}

impl SurfaceState {
    pub(super) fn render_slack_files_sidebar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .w(px(self.slack_sidebar_width()))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(rgb(0x34363a))
            .bg(rgb(0x101114))
            .child(
                div()
                    .h(px(SLACK_FILES_HEADER_HEIGHT))
                    .flex_none()
                    .px(px(16.0))
                    .flex()
                    .items_center()
                    .text_size(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xf8f8f8))
                    .child("Files"),
            )
            .child(
                div()
                    .px(px(8.0))
                    .pt(px(3.0))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(self.render_slack_files_sidebar_all(cx))
                    .child(self.render_slack_files_sidebar_type(
                        "Canvases",
                        "▣",
                        SlackFilesTypeFilter::CanvasesAndDocuments,
                        cx,
                    ))
                    .child(self.render_slack_files_sidebar_type(
                        "Lists",
                        "☷",
                        SlackFilesTypeFilter::Lists,
                        cx,
                    ))
                    .child(
                        div()
                            .mt(px(20.0))
                            .child(self.render_slack_files_recently_viewed(cx)),
                    ),
            )
    }

    fn render_slack_files_sidebar_all(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected = self.slack_files_sidebar_selection == SlackFilesSidebarSelection::All;
        self.render_slack_files_sidebar_button(
            SlackFilesSidebarButtonSpec {
                id: "slack-files-sidebar-all".into(),
                label: "All files",
                selected,
                icon: slack_icon(
                    SlackShellIcon::Files,
                    if selected { 0x1d1c1d } else { 0xb9babd },
                    18.0,
                    cx,
                ),
                action: SlackFilesSidebarAction::All,
            },
            cx,
        )
    }

    fn render_slack_files_sidebar_type(
        &self,
        label: &'static str,
        glyph: &'static str,
        filter: SlackFilesTypeFilter,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected =
            self.slack_files_sidebar_selection == SlackFilesSidebarSelection::Type(filter);
        self.render_slack_files_sidebar_button(
            SlackFilesSidebarButtonSpec {
                id: format!("slack-files-sidebar-{}", label.to_ascii_lowercase()).into(),
                label,
                selected,
                icon: div()
                    .w(px(18.0))
                    .text_size(px(17.0))
                    .text_color(rgb(if selected { 0x1d1c1d } else { 0xb9babd }))
                    .child(glyph)
                    .into_any_element(),
                action: SlackFilesSidebarAction::Type(filter),
            },
            cx,
        )
    }

    fn render_slack_files_recently_viewed(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected =
            self.slack_files_sidebar_selection == SlackFilesSidebarSelection::RecentlyViewed;
        self.render_slack_files_sidebar_button(
            SlackFilesSidebarButtonSpec {
                id: "slack-files-sidebar-recent".into(),
                label: "Recently viewed",
                selected,
                icon: div()
                    .w(px(18.0))
                    .text_size(px(17.0))
                    .text_color(rgb(if selected { 0x1d1c1d } else { 0xb9babd }))
                    .child("◷")
                    .into_any_element(),
                action: SlackFilesSidebarAction::RecentlyViewed,
            },
            cx,
        )
    }

    fn render_slack_files_sidebar_button(
        &self,
        spec: SlackFilesSidebarButtonSpec,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let SlackFilesSidebarButtonSpec {
            id,
            label,
            selected,
            icon,
            action,
        } = spec;
        let key_action = action;
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(label)
            .aria_selected(selected)
            .focusable()
            .tab_stop(true)
            .h(px(28.0))
            .w_full()
            .px(px(12.0))
            .rounded(px(4.0))
            .when(selected, |this| this.bg(rgb(0xf8f8f8)))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(if selected { 0xf8f8f8 } else { 0x27292d })))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.activate_slack_files_sidebar_action(action, cx);
            }))
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_files_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.activate_slack_files_sidebar_action(key_action, cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(10.0))
            .text_size(px(15.0))
            .text_color(rgb(if selected { 0x1d1c1d } else { 0xb9babd }))
            .child(icon)
            .child(label)
    }

    fn activate_slack_files_sidebar_action(
        &mut self,
        action: SlackFilesSidebarAction,
        cx: &mut Context<Self>,
    ) {
        match action {
            SlackFilesSidebarAction::All => self.select_slack_files_all(cx),
            SlackFilesSidebarAction::Type(filter) => {
                self.select_slack_files_type_preset(filter, cx);
            }
            SlackFilesSidebarAction::RecentlyViewed => {
                self.select_slack_files_recently_viewed(cx);
            }
        }
    }
}
