use gpui::prelude::FluentBuilder;
use gpui::{
    div, point, px, rgb, AnyElement, BoxShadow, Context, Div, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Role, StatefulInteractiveElement,
    Styled,
};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use crate::ui::alpha;
use crate::ui::surface::{slack_icon, SlackNewMessageCandidateKind, SlackShellIcon, SurfaceState};

mod dialog;
mod footer;
mod results;

const SLACK_MESSAGE_FORWARD_DIALOG_WIDTH: f32 = 520.0;
const SLACK_MESSAGE_FORWARD_DIALOG_HEIGHT: f32 = 406.7;
const SLACK_MESSAGE_FORWARD_CONTENT_LEFT: f32 = 28.0;
const SLACK_MESSAGE_FORWARD_CONTENT_WIDTH: f32 = 464.0;
const SLACK_MESSAGE_FORWARD_RESULTS_LEFT: f32 = 16.0;
const SLACK_MESSAGE_FORWARD_RESULTS_WIDTH: f32 = 488.0;
const SLACK_MESSAGE_FORWARD_RESULT_HEIGHT: f32 = 44.0;
const SLACK_MESSAGE_FORWARD_RESULTS_MAX_HEIGHT: f32 = 224.0;

impl SurfaceState {
    pub(super) fn render_slack_message_forward_layer(&self, cx: &mut Context<Self>) -> AnyElement {
        let modal = self
            .slack_message_forward_modal
            .as_ref()
            .expect("Slack message forward layer requires modal state");
        let title = if modal.source.private_message {
            "Forward this private message"
        } else {
            "Forward this message"
        };
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0))
                .bg(alpha(0x000000, 0.56)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_message_forward(cx);
            }),
            cx,
        );
        div()
            .id("slack-message-forward-layer")
            .absolute()
            .top(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .child(backdrop)
            .child(self.slack_message_forward_dialog(title, cx))
            .into_any_element()
    }

    fn slack_message_forward_dialog(
        &self,
        title: &'static str,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let modal = self
            .slack_message_forward_modal
            .as_ref()
            .expect("Slack message forward dialog requires modal state");
        div()
            .id("slack-message-forward-dialog")
            .role(Role::Dialog)
            .aria_label(title)
            .relative()
            .w(px(SLACK_MESSAGE_FORWARD_DIALOG_WIDTH))
            .h(px(SLACK_MESSAGE_FORWARD_DIALOG_HEIGHT))
            .max_w(gpui::relative(0.92))
            .rounded(px(8.0))
            .bg(rgb(0x1a1d21))
            .shadow(slack_message_forward_dialog_shadow())
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(self.render_slack_message_forward_header(title, cx))
            .child(self.render_slack_message_forward_destination_control(cx))
            .child(self.render_slack_message_forward_note(cx))
            .child(self.render_slack_message_forward_preview())
            .when_some(modal.error.as_deref(), |this, error| {
                this.child(self.render_slack_message_forward_error(error))
            })
            .child(self.render_slack_message_forward_footer(cx))
            .when(modal.destination.is_none(), |this| {
                this.child(self.render_slack_message_forward_results(cx))
            })
    }

    fn render_slack_message_forward_destination_icon(
        &self,
        kind: SlackNewMessageCandidateKind,
        color: u32,
        cx: &mut Context<Self>,
    ) -> Div {
        let icon = match kind {
            SlackNewMessageCandidateKind::Channel => SlackShellIcon::HashSmall,
            SlackNewMessageCandidateKind::PrivateChannel => SlackShellIcon::LockSmall,
            SlackNewMessageCandidateKind::GroupMessage
            | SlackNewMessageCandidateKind::DirectMessage
            | SlackNewMessageCandidateKind::Person => SlackShellIcon::People,
        };
        div()
            .size(px(24.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(icon, color, 18.0, cx))
    }
}

fn slack_message_forward_dialog_shadow() -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color: alpha(0xe8e8e8, 0.13),
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(1.0),
            inset: false,
        },
        BoxShadow {
            color: alpha(0x000000, 0.35),
            offset: point(px(0.0), px(18.0)),
            blur_radius: px(48.0),
            spread_radius: px(0.0),
            inset: false,
        },
    ]
}

fn slack_message_forward_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
