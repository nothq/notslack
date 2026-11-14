//! The bar at the foot of the sidebar while notslack is in a huddle, like
//! Slack's: where the huddle is, how it is going, mute and leave.

use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, AnyElement, Context, Div, FontWeight, InteractiveElement, IntoElement,
    ParentElement, Role, StatefulInteractiveElement, Styled,
};

use crate::ui::surface::{
    slack_icon, slack_palette, SlackHuddleState, SlackHuddleStatus, SlackShellIcon, SurfaceState,
};

const SLACK_HUDDLE_GREEN: u32 = 0x007a5a;
const SLACK_HUDDLE_RED: u32 = 0xe01e5a;

impl SurfaceState {
    /// The huddle bar, or the last huddle error, when there is one.
    pub(crate) fn render_slack_huddle_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        if let Some(huddle) = self.slack_huddle.as_ref() {
            return Some(self.render_slack_active_huddle_bar(huddle, cx));
        }
        let error = self.slack_huddle_error.clone()?;
        let palette = slack_palette(self.appearance_mode);
        Some(
            div()
                .id("slack-huddle-error")
                .flex_none()
                .m(px(8.0))
                .p(px(10.0))
                .rounded(px(8.0))
                .bg(rgb(palette.reaction_picker_footer_bg))
                .border_1()
                .border_color(rgb(SLACK_HUDDLE_RED))
                .text_size(px(12.0))
                .text_color(rgb(palette.sidebar_text))
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| this.dismiss_slack_huddle_error(cx)))
                .child(error)
                .into_any_element(),
        )
    }

    fn render_slack_active_huddle_bar(
        &self,
        huddle: &SlackHuddleState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let status = match huddle.status {
            SlackHuddleStatus::Joining => "Starting huddle…".to_string(),
            SlackHuddleStatus::Connecting => "Connecting audio…".to_string(),
            SlackHuddleStatus::Leaving => "Leaving…".to_string(),
            SlackHuddleStatus::Connected if huddle.participants > 1 => {
                format!("{} people in the huddle", huddle.participants)
            }
            SlackHuddleStatus::Connected => "Huddle".to_string(),
        };
        let connected = huddle.status == SlackHuddleStatus::Connected;
        div()
            .flex_none()
            .m(px(8.0))
            .p(px(10.0))
            .rounded(px(8.0))
            .bg(rgb(palette.reaction_picker_footer_bg))
            .border_1()
            .border_color(rgb(if connected {
                SLACK_HUDDLE_GREEN
            } else {
                palette.sidebar_border
            }))
            .flex()
            .items_center()
            .gap(px(8.0))
            .child(slack_icon(
                SlackShellIcon::Huddles,
                if connected {
                    SLACK_HUDDLE_GREEN
                } else {
                    palette.sidebar_muted_text
                },
                18.0,
                cx,
            ))
            .child(
                div()
                    .flex_grow(1.0)
                    .min_w(px(0.0))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(palette.sidebar_header_text))
                            .overflow_hidden()
                            .text_ellipsis()
                            .whitespace_nowrap()
                            .child(huddle.label.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(rgb(palette.sidebar_muted_text))
                            .child(status),
                    ),
            )
            .when(connected, |this| {
                this.child(
                    slack_huddle_bar_button(
                        "slack-huddle-mute",
                        if huddle.muted { "Unmute" } else { "Mute" },
                        if huddle.muted {
                            SLACK_HUDDLE_RED
                        } else {
                            palette.reaction_picker_hover_bg
                        },
                        slack_icon(SlackShellIcon::Mic, 0xffffff, 16.0, cx),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_slack_huddle_mute(cx))),
                )
            })
            .child(
                slack_huddle_bar_button(
                    "slack-huddle-leave",
                    "Leave huddle",
                    SLACK_HUDDLE_RED,
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child("Leave")
                        .into_any_element(),
                )
                .px(px(10.0))
                .on_click(cx.listener(|this, _, _, cx| this.leave_slack_huddle(cx))),
            )
            .into_any_element()
    }
}

fn slack_huddle_bar_button(
    id: &'static str,
    label: &'static str,
    background: u32,
    content: AnyElement,
) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .h(px(28.0))
        .min_w(px(28.0))
        .flex_none()
        .rounded(px(6.0))
        .bg(rgb(background))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(content)
}
