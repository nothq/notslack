use super::{
    div, px, rgb, slack_icon, slack_palette, AnyElement, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, SlackShellIcon,
    Styled, SurfaceState, SLACK_SIDEBAR_ROW_HEIGHT,
};
use gpui::{Role, StatefulInteractiveElement};

type SlackSidebarShortcutAction = fn(&mut SurfaceState, &mut Context<SurfaceState>);

struct SlackSidebarShortcutSpec<'a> {
    element_id: &'static str,
    label: &'a str,
    icon: SlackShellIcon,
    badge: Option<&'a str>,
    selected: bool,
    action: Option<SlackSidebarShortcutAction>,
}

impl SurfaceState {
    pub(super) fn render_slack_threads_shortcut(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self.slack_workspace_api_capabilities.load_all_threads {
            return div()
                .w_full()
                .h(px(SLACK_SIDEBAR_ROW_HEIGHT))
                .into_any_element();
        }
        self.render_slack_action_shortcut(
            SlackSidebarShortcutSpec {
                element_id: "slack-sidebar-threads",
                label: "Threads",
                icon: SlackShellIcon::ReplyThread,
                badge: None,
                selected: self.slack_main_route == crate::ui::surface::SlackMainRoute::AllThreads,
                action: Some(Self::activate_slack_all_threads),
            },
            cx,
        )
    }

    pub(super) fn render_slack_huddles_shortcut(&self, cx: &mut Context<Self>) -> AnyElement {
        self.render_slack_action_shortcut(
            SlackSidebarShortcutSpec {
                element_id: "slack-sidebar-huddles",
                label: "Huddles",
                icon: SlackShellIcon::Huddles,
                badge: None,
                selected: false,
                action: None,
            },
            cx,
        )
    }

    pub(super) fn render_slack_drafts_sent_shortcut(
        &self,
        label: &str,
        badge: Option<&str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_slack_action_shortcut(
            SlackSidebarShortcutSpec {
                element_id: "slack-sidebar-drafts-sent",
                label,
                icon: SlackShellIcon::Drafts,
                badge,
                selected: self.slack_active_rail_view == super::super::SlackRailView::DraftsSent,
                action: Some(Self::activate_slack_drafts_sent),
            },
            cx,
        )
    }

    pub(super) fn render_slack_directories_shortcut(&self, cx: &mut Context<Self>) -> AnyElement {
        if !self
            .slack_workspace_api_capabilities
            .load_destination_directory
        {
            return div()
                .w_full()
                .h(px(SLACK_SIDEBAR_ROW_HEIGHT))
                .into_any_element();
        }
        self.render_slack_action_shortcut(
            SlackSidebarShortcutSpec {
                element_id: "slack-sidebar-directories",
                label: "Directories",
                icon: SlackShellIcon::Directories,
                badge: None,
                selected: self.slack_main_route == crate::ui::surface::SlackMainRoute::Directory,
                action: Some(Self::activate_slack_directory),
            },
            cx,
        )
    }

    pub(super) fn render_slack_create_section_shortcut(
        &self,
        badge: Option<&str>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.render_slack_action_shortcut(
            SlackSidebarShortcutSpec {
                element_id: "slack-sidebar-create-section",
                label: "Create a section",
                icon: SlackShellIcon::Plus,
                badge,
                selected: false,
                action: self
                    .slack_workspace_api_capabilities
                    .subscribe_realtime
                    .then_some(Self::open_slack_sidebar_section_dialog),
            },
            cx,
        )
    }

    fn render_slack_action_shortcut(
        &self,
        spec: SlackSidebarShortcutSpec<'_>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (palette, selected) = (slack_palette(self.appearance_mode), spec.selected);
        let row = div()
            .id(spec.element_id)
            .role(Role::TreeItem)
            .aria_label(spec.label)
            .aria_selected(selected)
            .w_full()
            .h(px(SLACK_SIDEBAR_ROW_HEIGHT))
            .px(px(8.0))
            .rounded(px(6.0))
            .when(selected, |this| this.bg(rgb(palette.sidebar_active_bg)))
            .flex()
            .items_center()
            .justify_between()
            .child(self.render_slack_shortcut_leading(&spec, cx))
            .when_some(spec.badge, |this, badge| {
                this.child(self.render_slack_shortcut_badge(badge, palette.sidebar_muted_text))
            });
        let Some(action) = spec.action else {
            return row.into_any_element();
        };
        row.focusable()
            .tab_stop(true)
            .cursor_pointer()
            .hover(|style| {
                style.bg(rgb(if selected {
                    palette.sidebar_active_bg
                } else {
                    palette.sidebar_border
                }))
            })
            .focus_visible(|style| {
                style.bg(rgb(if selected {
                    palette.sidebar_active_bg
                } else {
                    palette.sidebar_border
                }))
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| action(this, cx)),
            )
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space")
                        && !event.keystroke.modifiers.modified()
                    {
                        window.prevent_default();
                        cx.stop_propagation();
                        action(this, cx);
                    }
                }),
            )
            .into_any_element()
    }

    fn render_slack_shortcut_leading(
        &self,
        spec: &SlackSidebarShortcutSpec<'_>,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let text = if spec.selected {
            palette.sidebar_active_text
        } else {
            palette.sidebar_text
        };
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(slack_icon(
                spec.icon,
                if spec.selected {
                    palette.sidebar_active_text
                } else {
                    palette.sidebar_icon
                },
                18.0,
                cx,
            ))
            .child(
                div()
                    .text_size(px(15.0))
                    .font_weight(if spec.selected {
                        FontWeight::MEDIUM
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(rgb(text))
                    .child(spec.label.to_string()),
            )
    }

    fn render_slack_shortcut_badge(&self, badge: &str, text: u32) -> Div {
        if badge == "Tip" {
            return div()
                .h(px(20.0))
                .px(px(5.0))
                .rounded(px(4.0))
                .bg(rgb(0x1264a3))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(13.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child("Tip");
        }
        div()
            .h(px(20.0))
            .px(px(4.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(14.0))
            .text_color(rgb(text))
            .child(format!("✎ {badge}"))
    }
}
