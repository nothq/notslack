use super::super::super::{
    alpha, div, px, rgb, AnyElement, Context, Div, FluentBuilder, FontWeight, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, SlackRailView,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use super::slack_rail_action_key;
use crate::ui::surface::{SlackMoreView, SlackRailMenu};
use crate::ui::SlackWorkspace;
use gpui::{point, BoxShadow, Role, Stateful};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

struct SlackRailMenuSpec {
    id: &'static str,
    label: &'static str,
    left: f32,
    top: f32,
}

#[derive(Clone, Copy)]
struct SlackRailMenuActionSpec {
    id: &'static str,
    label: &'static str,
    detail: Option<&'static str>,
    height: f32,
}

impl SurfaceState {
    pub(in super::super) fn render_slack_rail_menu_layer(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let menu = self
            .slack_rail_menu
            .expect("Slack rail menu layer requires an open menu");
        let spec = slack_rail_menu_spec(menu, self.slack_rail_width());
        let body = self.render_slack_rail_menu_body(menu, workspace, cx);
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_rail_menu(cx);
            }),
            cx,
        );
        div()
            .absolute()
            .top(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .occlude()
            .child(backdrop)
            .child(self.render_slack_rail_menu(spec, body, cx))
            .into_any_element()
    }

    fn render_slack_rail_menu_body(
        &self,
        menu: SlackRailMenu,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        match menu {
            SlackRailMenu::Workspace => self.render_slack_workspace_switcher_menu(workspace, cx),
            SlackRailMenu::More => self.render_slack_more_menu(cx),
            SlackRailMenu::Admin => self.render_slack_admin_menu(cx),
        }
    }

    fn render_slack_rail_menu(
        &self,
        spec: SlackRailMenuSpec,
        body: Div,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        div()
            .id(spec.id)
            .role(Role::Menu)
            .aria_label(spec.label)
            .absolute()
            .left(px(spec.left))
            .top(px(spec.top))
            .w(px(360.0))
            .rounded(px(12.0))
            .border_1()
            .border_color(rgb(0x34373b))
            .bg(rgb(0x212428))
            .shadow(slack_rail_menu_shadow())
            .overflow_hidden()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(body)
    }

    fn render_slack_workspace_switcher_menu(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .py(px(4.0))
            .flex()
            .flex_col()
            .child(self.render_slack_workspace_switcher_current(workspace, cx))
            .child(self.render_slack_rail_menu_action(
                SlackRailMenuActionSpec {
                    id: "slack-workspace-switcher-add",
                    label: "Add a workspace",
                    detail: None,
                    height: 52.0,
                },
                cx,
                |this, cx| {
                    this.close_slack_rail_menu(cx);
                    this.reconnect_chat_workspace(cx);
                },
            ))
    }

    fn render_slack_workspace_switcher_current(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("slack-workspace-switcher-current")
            .role(Role::MenuItem)
            .aria_label(format!("{}, current workspace", workspace.workspace_name))
            .focusable()
            .tab_stop(true)
            .h(px(56.0))
            .px(px(16.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x1264a3)))
            .focus_visible(|style| style.bg(rgb(0x1264a3)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    this.close_slack_rail_menu(cx);
                }),
            )
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_rail_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.close_slack_rail_menu(cx);
                }
            }))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(self.render_slack_workspace_badge(workspace))
            .child(slack_workspace_switcher_identity(workspace))
            .child(slack_workspace_switcher_shortcut())
    }

    fn render_slack_more_menu(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .child(self.render_slack_rail_menu_header("More", 38.0))
            .child(div().h(px(56.0)))
            .child(self.render_slack_rail_menu_action(
                SlackRailMenuActionSpec {
                    id: "slack-rail-customize-navigation",
                    label: "Customize navigation bar",
                    detail: None,
                    height: 42.0,
                },
                cx,
                |this, cx| {
                    this.open_slack_more_view(SlackMoreView::CustomizeNavigation, cx);
                },
            ))
    }

    fn render_slack_admin_menu(&self, cx: &mut Context<Self>) -> Div {
        div()
            .pb(px(8.0))
            .flex()
            .flex_col()
            .child(self.render_slack_rail_menu_header("Admin Tools", 44.0))
            .child(self.render_slack_rail_menu_action(
                SlackRailMenuActionSpec {
                    id: "slack-admin-workspace-settings",
                    label: "Workspace settings",
                    detail: None,
                    height: 36.0,
                },
                cx,
                |this, cx| {
                    this.close_slack_rail_menu(cx);
                    this.select_slack_rail_view(SlackRailView::Admin, cx);
                },
            ))
            .when(
                self.slack_workspace_api_capabilities
                    .load_conversation_members,
                |this| {
                    this.child(self.render_slack_rail_menu_action(
                        SlackRailMenuActionSpec {
                            id: "slack-admin-manage-members",
                            label: "Manage members",
                            detail: None,
                            height: 36.0,
                        },
                        cx,
                        |this, cx| {
                            this.close_slack_rail_menu(cx);
                            this.open_slack_members_panel(cx);
                        },
                    ))
                },
            )
            .child(self.render_slack_rail_menu_action(
                SlackRailMenuActionSpec {
                    id: "slack-admin-apps-workflows",
                    label: "Apps & workflows",
                    detail: None,
                    height: 36.0,
                },
                cx,
                |this, cx| {
                    this.open_slack_more_view(SlackMoreView::Tools, cx);
                },
            ))
    }

    fn render_slack_rail_menu_header(&self, label: &'static str, height: f32) -> Div {
        div()
            .h(px(height))
            .px(px(16.0))
            .flex()
            .items_center()
            .text_size(px(15.0))
            .font_weight(FontWeight::BLACK)
            .text_color(rgb(0xf8f8f8))
            .child(label)
    }

    fn render_slack_rail_menu_action<F>(
        &self,
        spec: SlackRailMenuActionSpec,
        cx: &mut Context<Self>,
        action: F,
    ) -> impl IntoElement
    where
        F: Fn(&mut SurfaceState, &mut Context<SurfaceState>) + Clone + 'static,
    {
        let keyboard_action = action.clone();
        div()
            .id(spec.id)
            .role(Role::MenuItem)
            .aria_label(spec.label)
            .focusable()
            .tab_stop(true)
            .h(px(spec.height))
            .px(px(24.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .focus_visible(|style| style.bg(rgb(0x1264a3)).text_color(rgb(0xffffff)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| action(this, cx)),
            )
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_rail_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    keyboard_action(this, cx);
                }
            }))
            .flex()
            .items_center()
            .text_size(px(15.0))
            .line_height(px(spec.height))
            .text_color(rgb(0xf8f8f8))
            .child(spec.label)
            .when_some(spec.detail, |this, detail| {
                this.child(slack_rail_menu_detail(detail))
            })
    }
}

fn slack_rail_menu_spec(menu: SlackRailMenu, rail_width: f32) -> SlackRailMenuSpec {
    match menu {
        SlackRailMenu::Workspace => SlackRailMenuSpec {
            id: "slack-workspace-switcher-menu",
            label: "Workspaces",
            left: 4.0,
            top: 53.0,
        },
        SlackRailMenu::More => SlackRailMenuSpec {
            id: "slack-rail-more-menu",
            label: "More",
            left: rail_width - 3.0,
            top: 301.0,
        },
        SlackRailMenu::Admin => SlackRailMenuSpec {
            id: "slack-rail-admin-menu",
            label: "Admin Tools",
            left: rail_width - 3.0,
            top: 437.0,
        },
    }
}

fn slack_workspace_switcher_identity(workspace: &SlackWorkspace) -> Div {
    div()
        .flex_grow(1.0)
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .child(
            div()
                .text_size(px(15.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xf8f8f8))
                .child(workspace.workspace_name.clone()),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(rgb(0xb9babd))
                .child("Current workspace"),
        )
}

fn slack_workspace_switcher_shortcut() -> Div {
    div()
        .text_size(px(12.0))
        .text_color(rgb(0xb9babd))
        .child("⌘1")
}

fn slack_rail_menu_detail(detail: &'static str) -> Div {
    div()
        .ml_auto()
        .text_size(px(12.0))
        .text_color(rgb(0xb9babd))
        .child(detail)
}

fn slack_rail_menu_shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: alpha(0x000000, 0.42),
        offset: point(px(0.0), px(8.0)),
        blur_radius: px(28.0),
        spread_radius: px(-8.0),
        inset: false,
    }]
}
