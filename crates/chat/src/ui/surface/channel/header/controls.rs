use crate::ui::surface::{
    slack_icon, slack_palette, SlackHeaderControl, SlackRailView, SlackShellIcon, SurfaceState,
};
use crate::ui::SlackWorkspace;
use crate::ui::{
    div, px, rgb, AppearanceMode, Context, Div, FluentBuilder, FocusHandle, InteractiveElement,
    IntoElement, ParentElement, StatefulInteractiveElement, Styled, Window,
};
use gpui::{AppContext, Render, Role, Stateful};

use super::slack_header_action_key;

mod star;

pub(super) struct SlackHeaderSquareButtonSpec {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) icon: SlackShellIcon,
    pub(super) fill: u32,
    pub(super) bordered: bool,
}

pub(super) struct SlackHeaderTooltip {
    pub(super) label: &'static str,
}

impl Render for SlackHeaderTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.0))
            .py(px(5.0))
            .rounded(px(4.0))
            .bg(rgb(0x2a2d31))
            .text_size(px(12.0))
            .text_color(rgb(0xffffff))
            .child(self.label)
    }
}

impl SurfaceState {
    pub(crate) fn render_slack_main_actions(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> Div {
        if !workspace.channel_kind.is_channel() {
            return self.render_slack_non_channel_actions(cx);
        }
        let members = self.slack_members_header_data(workspace);
        div()
            .w(px(225.0))
            .flex_none()
            .flex()
            .items_center()
            .child(
                div()
                    .w(px(49.0))
                    .h(px(28.0))
                    .flex_none()
                    .when_some(members, |this, data| {
                        this.child(self.render_slack_members_header_button(data, cx))
                    }),
            )
            .child(div().w(px(9.0)).h(px(1.0)).flex_none())
            .child(self.render_slack_header_huddle_control(cx))
            .child(div().w(px(9.0)).h(px(1.0)).flex_none())
            .child(div().size(px(28.0)).flex_none().when(
                self.slack_channel_notifications_control_available(),
                |this| this.child(self.render_slack_header_notifications_button(cx)),
            ))
            .child(div().w(px(8.0)).h(px(1.0)).flex_none())
            .child(div().size(px(28.0)).flex_none().when(
                self.slack_workspace_api_capabilities.search_messages,
                |this| this.child(self.render_slack_header_search_button(cx)),
            ))
            .child(div().w(px(8.0)).h(px(1.0)).flex_none())
            .child(
                div()
                    .size(px(28.0))
                    .flex_none()
                    .when(self.slack_channel_menu_available(), |this| {
                        this.child(self.render_slack_header_more_button(cx))
                    }),
            )
    }

    fn render_slack_non_channel_actions(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(self.render_slack_header_huddle_control(cx))
            .child(div().size(px(28.0)).flex_none().when(
                self.slack_channel_notifications_control_available(),
                |this| this.child(self.render_slack_header_notifications_button(cx)),
            ))
            .when(
                self.slack_workspace_api_capabilities.search_messages,
                |this| this.child(self.render_slack_header_search_button(cx)),
            )
            .when(self.slack_channel_menu_available(), |this| {
                this.child(self.render_slack_header_more_button(cx))
            })
            .when(self.slack_active_rail_view == SlackRailView::Dms, |this| {
                this.child(self.render_slack_dms_close_button(cx))
            })
    }

    fn render_slack_header_huddle_control(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let border = match self.appearance_mode {
            AppearanceMode::Dark => 0x34363b,
            AppearanceMode::Light => palette.main_border,
        };
        div()
            .w(px(58.0))
            .h(px(28.0))
            .flex_none()
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(border))
            .flex()
            .items_center()
            .child(
                div()
                    .w(px(36.0))
                    .h(px(26.0))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(
                        SlackShellIcon::Huddles,
                        palette.main_secondary_text,
                        20.0,
                        cx,
                    )),
            )
            .child(div().w(px(1.0)).h(px(18.0)).bg(rgb(border)))
            .child(
                div()
                    .flex_grow(1.0)
                    .h(px(26.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(slack_icon(
                        SlackShellIcon::ChevronDown,
                        palette.main_secondary_text,
                        12.0,
                        cx,
                    )),
            )
    }

    fn render_slack_header_more_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        let workspace = self
            .slack_workspace()
            .expect("Slack conversation menu header requires an active workspace");
        let tab_stop =
            self.slack_header_active_control(workspace) == Some(SlackHeaderControl::More);
        let label = if workspace.channel_kind.is_channel() {
            "More channel actions"
        } else {
            "More conversation actions"
        };
        self.render_slack_header_square_button(
            SlackHeaderSquareButtonSpec {
                id: "slack-channel-more",
                label,
                icon: SlackShellIcon::MoreVertical,
                fill: palette.main_secondary_text,
                bordered: false,
            },
            &self.slack_header_more_focus_handle,
            tab_stop,
            cx,
        )
        .aria_expanded(self.slack_channel_menu_open)
        .on_click(cx.listener(|this, _, _, cx| {
            this.slack_header_roving_target = SlackHeaderControl::More;
            if this.slack_channel_menu_open {
                this.close_slack_channel_menu(cx);
            } else {
                this.open_slack_channel_menu(cx);
            }
        }))
        .on_key_down(cx.listener(|this, event, window, cx| {
            if this.handle_slack_channel_header_roving_key(
                SlackHeaderControl::More,
                event,
                window,
                cx,
            ) {
                return;
            }
            if !slack_header_action_key(event) {
                return;
            }
            window.prevent_default();
            cx.stop_propagation();
            if this.slack_channel_menu_open {
                this.activate_selected_slack_channel_menu(cx);
            } else {
                this.open_slack_channel_menu(cx);
            }
        }))
    }

    fn render_slack_header_notifications_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        let workspace = self
            .slack_workspace()
            .expect("Slack notification header requires an active workspace");
        let preference = self
            .slack_channel_notification_preference
            .as_ref()
            .expect("Slack notification header requires loaded preferences");
        let muted = preference.muted;
        let label = if muted {
            "Unmute conversation"
        } else {
            "Mute conversation"
        };
        let tab_stop =
            self.slack_header_active_control(workspace) == Some(SlackHeaderControl::Notifications);
        self.render_slack_header_square_button(
            SlackHeaderSquareButtonSpec {
                id: "slack-channel-notifications",
                label,
                icon: slack_channel_notification_icon(preference),
                fill: palette.main_secondary_text,
                bordered: true,
            },
            &self.slack_header_notifications_focus_handle,
            tab_stop,
            cx,
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.slack_header_roving_target = SlackHeaderControl::Notifications;
            this.start_slack_channel_notification_mutation(
                crate::ui::SlackChannelNotificationMutation::SetMuted(!muted),
                cx,
            );
        }))
        .on_key_down(cx.listener(move |this, event, window, cx| {
            if this.handle_slack_channel_header_roving_key(
                SlackHeaderControl::Notifications,
                event,
                window,
                cx,
            ) {
                return;
            }
            if !slack_header_action_key(event) {
                return;
            }
            window.prevent_default();
            cx.stop_propagation();
            this.start_slack_channel_notification_mutation(
                crate::ui::SlackChannelNotificationMutation::SetMuted(!muted),
                cx,
            );
        }))
    }

    fn render_slack_header_search_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        let workspace = self
            .slack_workspace()
            .expect("Slack conversation search header requires an active workspace");
        let tab_stop =
            self.slack_header_active_control(workspace) == Some(SlackHeaderControl::Search);
        let label = "Search in channel";
        self.render_slack_header_square_button(
            SlackHeaderSquareButtonSpec {
                id: "slack-conversation-search",
                label,
                icon: SlackShellIcon::Search,
                fill: palette.main_secondary_text,
                bordered: true,
            },
            &self.slack_header_search_focus_handle,
            tab_stop,
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.slack_header_roving_target = SlackHeaderControl::Search;
            this.open_slack_conversation_search(cx);
        }))
        .on_key_down(cx.listener(|this, event, window, cx| {
            if this.handle_slack_channel_header_roving_key(
                SlackHeaderControl::Search,
                event,
                window,
                cx,
            ) {
                return;
            }
            if slack_header_action_key(event) {
                window.prevent_default();
                cx.stop_propagation();
                this.open_slack_conversation_search(cx);
            }
        }))
    }

    fn render_slack_dms_close_button(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        self.render_slack_header_square_button(
            SlackHeaderSquareButtonSpec {
                id: "slack-dms-close",
                label: "Close direct messages",
                icon: SlackShellIcon::Close,
                fill: palette.main_secondary_text,
                bordered: true,
            },
            &self.slack_header_dms_close_focus_handle,
            true,
            cx,
        )
        .on_click(cx.listener(|this, _, _, cx| {
            this.close_slack_dms_view(cx);
        }))
        .on_key_down(cx.listener(|this, event, window, cx| {
            if slack_header_action_key(event) {
                window.prevent_default();
                cx.stop_propagation();
                this.close_slack_dms_view(cx);
            }
        }))
    }

    pub(super) fn render_slack_header_square_button(
        &self,
        spec: SlackHeaderSquareButtonSpec,
        focus_handle: &FocusHandle,
        tab_stop: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let border = match self.appearance_mode {
            AppearanceMode::Dark => 0x34363b,
            AppearanceMode::Light => slack_palette(self.appearance_mode).main_border,
        };
        div()
            .id(spec.id)
            .role(Role::Button)
            .aria_label(spec.label)
            .track_focus(focus_handle)
            .focusable()
            .tab_stop(tab_stop)
            .size(px(28.0))
            .rounded(px(8.0))
            .when(spec.bordered, |this| {
                this.border_1().border_color(rgb(border))
            })
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x2a2d31)))
            .focus_visible(|style| style.bg(rgb(0x2a2d31)))
            .tooltip(move |_, cx| cx.new(|_| SlackHeaderTooltip { label: spec.label }).into())
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(spec.icon, spec.fill, 20.0, cx))
    }
}

fn slack_channel_notification_icon(
    preference: &crate::ui::SlackChannelNotificationPreference,
) -> SlackShellIcon {
    if preference.muted {
        SlackShellIcon::BellOff
    } else {
        SlackShellIcon::Bell
    }
}

fn slack_header_button_hover(appearance_mode: AppearanceMode) -> u32 {
    match appearance_mode {
        AppearanceMode::Dark => 0x2a2d31,
        AppearanceMode::Light => 0xf1f2f3,
    }
}
