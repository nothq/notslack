use crate::ui::surface::{slack_icon, slack_palette, SlackShellIcon, SurfaceState};
use crate::ui::{
    div, px, rgb, AppearanceMode, Context, Div, FluentBuilder, InteractiveElement, ParentElement,
    StatefulInteractiveElement, Styled,
};
use crate::ui::{SlackChannelNotificationMode, SlackChannelNotificationMutation};
use gpui::{Role, Stateful, Toggled};

pub(super) struct SlackChannelNotificationRowSpec {
    id: &'static str,
    label: &'static str,
    detail: &'static str,
    icon: SlackShellIcon,
    height: f32,
    checked: bool,
    selected: bool,
    mutation: SlackChannelNotificationMutation,
}

struct SlackChannelNotificationRowVisual {
    keyboard_active: bool,
    text: u32,
    detail: u32,
    hover_group: String,
}

impl SlackChannelNotificationRowSpec {
    pub(super) fn everything(checked: bool, selected: bool) -> Self {
        Self {
            id: "slack-channel-notifications-everything",
            label: "All new posts",
            detail: "Messages and threads you follow",
            icon: SlackShellIcon::BellAlerting,
            height: 46.25,
            checked,
            selected,
            mutation: SlackChannelNotificationMutation::SetMode(
                SlackChannelNotificationMode::Everything,
            ),
        }
    }

    pub(super) fn mentions(checked: bool, selected: bool) -> Self {
        Self {
            id: "slack-channel-notifications-mentions",
            label: "Just mentions",
            detail: "@you, @channel, @here",
            icon: SlackShellIcon::Bell,
            height: 46.25,
            checked,
            selected,
            mutation: SlackChannelNotificationMutation::SetMode(
                SlackChannelNotificationMode::Mentions,
            ),
        }
    }

    pub(super) fn muted(muted: bool, selected: bool) -> Self {
        Self {
            id: "slack-channel-notifications-muted",
            label: if muted {
                "Unmute channel"
            } else {
                "Mute and hide"
            },
            detail: "Only badge the channel when someone @mentions you",
            icon: SlackShellIcon::BellOff,
            height: 62.5,
            checked: muted,
            selected,
            mutation: SlackChannelNotificationMutation::SetMuted(!muted),
        }
    }

    pub(super) fn nothing(checked: bool, selected: bool) -> Self {
        Self {
            id: "slack-channel-notifications-nothing",
            label: "No notifications",
            detail: "Do not notify you about new posts",
            icon: SlackShellIcon::BellOff,
            height: 62.5,
            checked,
            selected,
            mutation: SlackChannelNotificationMutation::SetMode(
                SlackChannelNotificationMode::Nothing,
            ),
        }
    }
}

impl SurfaceState {
    pub(super) fn render_slack_channel_notification_row(
        &self,
        spec: SlackChannelNotificationRowSpec,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let visual = self.slack_channel_notification_row_visual(&spec);
        let mutation = spec.mutation;
        div()
            .id(spec.id)
            .role(Role::MenuItemRadio)
            .aria_label(format!("{}. {}", spec.label, spec.detail))
            .aria_toggled(if spec.checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .h(px(spec.height))
            .pl(px(if spec.checked { 4.0 } else { 24.0 }))
            .pr(px(24.0))
            .group(visual.hover_group.clone())
            .cursor_pointer()
            .when(visual.keyboard_active, |style| style.bg(rgb(0x1264a3)))
            .hover(|style| style.bg(rgb(0x1264a3)))
            .flex()
            .items_center()
            .when(spec.checked, |this| {
                this.child(self.render_slack_channel_notification_check(&visual, cx))
            })
            .child(
                slack_channel_notification_hover_icon(
                    spec.icon,
                    visual.text,
                    20.0,
                    &visual.hover_group,
                    cx,
                )
                .mr(px(8.0)),
            )
            .child(slack_channel_notification_row_text(&spec, &visual))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.start_slack_channel_notification_mutation(mutation, cx);
            }))
    }

    fn slack_channel_notification_row_visual(
        &self,
        spec: &SlackChannelNotificationRowSpec,
    ) -> SlackChannelNotificationRowVisual {
        let palette = slack_palette(self.appearance_mode);
        let keyboard_active =
            self.slack_channel_notifications_menu_keyboard_highlighted && spec.selected;
        let (main_text, secondary_text) = match self.appearance_mode {
            AppearanceMode::Dark => (0xf8f8f8, 0xb9babd),
            AppearanceMode::Light => (palette.main_text, palette.main_secondary_text),
        };
        SlackChannelNotificationRowVisual {
            keyboard_active,
            text: slack_channel_notification_row_color(keyboard_active, spec.checked, main_text),
            detail: slack_channel_notification_row_color(
                keyboard_active,
                spec.checked,
                secondary_text,
            ),
            hover_group: format!("{}-hover", spec.id),
        }
    }

    fn render_slack_channel_notification_check(
        &self,
        visual: &SlackChannelNotificationRowVisual,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .w(px(20.0))
            .h(px(28.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(slack_channel_notification_hover_icon(
                SlackShellIcon::MenuCheck,
                visual.text,
                16.0,
                &visual.hover_group,
                cx,
            ))
    }
}

pub(super) fn slack_channel_notification_hover_icon(
    icon: SlackShellIcon,
    fill: u32,
    size: f32,
    hover_group: &str,
    cx: &mut Context<SurfaceState>,
) -> Div {
    div()
        .relative()
        .size(px(size))
        .flex_none()
        .child(
            div()
                .absolute()
                .size(px(size))
                .opacity(1.0)
                .group_hover(hover_group.to_string(), |style| style.opacity(0.0))
                .child(slack_icon(icon, fill, size, cx)),
        )
        .child(
            div()
                .absolute()
                .size(px(size))
                .opacity(0.0)
                .group_hover(hover_group.to_string(), |style| style.opacity(1.0))
                .child(slack_icon(icon, 0xf8f8f8, size, cx)),
        )
}

fn slack_channel_notification_row_text(
    spec: &SlackChannelNotificationRowSpec,
    visual: &SlackChannelNotificationRowVisual,
) -> Div {
    div()
        .min_w(px(0.0))
        .flex_1()
        .flex()
        .flex_col()
        .child(
            div()
                .ml(px(8.0))
                .text_size(px(15.0))
                .line_height(px(28.0))
                .text_color(rgb(visual.text))
                .group_hover(visual.hover_group.clone(), |style| {
                    style.text_color(rgb(0xf8f8f8))
                })
                .child(spec.label),
        )
        .child(
            div()
                .pb(px(4.0))
                .text_size(px(13.0))
                .line_height(px(16.25))
                .text_color(rgb(visual.detail))
                .group_hover(visual.hover_group.clone(), |style| {
                    style.text_color(rgb(0xf8f8f8))
                })
                .child(div().ml(px(8.0)).child(spec.detail)),
        )
}

fn slack_channel_notification_row_color(keyboard_active: bool, checked: bool, default: u32) -> u32 {
    if keyboard_active {
        0xf8f8f8
    } else if checked {
        0x1d9bd1
    } else {
        default
    }
}
