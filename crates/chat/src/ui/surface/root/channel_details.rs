use super::{
    alpha, div, px, relative, rgb, AnyElement, Context, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, ParentElement, StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::{AppearanceMode, SlackWorkspace};
use gpui::{point, BoxShadow, Div, KeyDownEvent, Role};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

mod content;
mod header;

const SLACK_CHANNEL_DETAILS_DIALOG_WIDTH: f32 = 580.0;
const SLACK_CHANNEL_DETAILS_DIALOG_HEIGHT: f32 = 820.0;

impl SurfaceState {
    pub(super) fn render_slack_channel_details_layer(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let dialog_height =
            (self.viewport_height - 24.0).clamp(320.0, SLACK_CHANNEL_DETAILS_DIALOG_HEIGHT);
        let dialog_top = ((self.viewport_height - dialog_height) / 2.0 + 6.5).max(12.0);
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .left(px(0.0))
                .right(px(0.0))
                .top(px(0.0))
                .bottom(px(0.0))
                .bg(alpha(0x000000, 0.28)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_channel_details(cx);
            }),
            cx,
        );
        div()
            .id("slack-channel-details-layer")
            .role(Role::Dialog)
            .aria_label(format!("Details for channel #{}", workspace.channel_name))
            .absolute()
            .left(px(0.0))
            .right(px(0.0))
            .top(px(0.0))
            .bottom(px(0.0))
            .occlude()
            .flex()
            .items_start()
            .justify_center()
            .pt(px(dialog_top))
            .child(backdrop)
            .child(self.render_slack_channel_details_dialog(workspace, dialog_height, cx))
            .into_any_element()
    }

    fn render_slack_channel_details_dialog(
        &self,
        workspace: &SlackWorkspace,
        dialog_height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        div()
            .id("slack-channel-details-dialog")
            .w(px(SLACK_CHANNEL_DETAILS_DIALOG_WIDTH))
            .h(px(dialog_height))
            .max_w(relative(0.92))
            .rounded(px(8.0))
            .overflow_hidden()
            .occlude()
            .bg(rgb(slack_channel_details_background(self.appearance_mode)))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.28),
                offset: point(px(0.0), px(8.0)),
                blur_radius: px(28.0),
                spread_radius: px(0.0),
                inset: false,
            }])
            .flex()
            .flex_col()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_: &mut Self, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(self.render_slack_channel_details_header(workspace, cx))
            .child(self.render_slack_channel_details_body())
    }
}

pub(super) fn slack_channel_details_background(appearance_mode: AppearanceMode) -> u32 {
    match appearance_mode {
        AppearanceMode::Dark => 0x1a1d21,
        AppearanceMode::Light => 0xf8f8f8,
    }
}

pub(super) fn slack_channel_details_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
