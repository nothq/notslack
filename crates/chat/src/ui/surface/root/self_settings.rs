use gpui::{div, px, AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled};
use gpui_components::backdrop::{dismissible_backdrop, BackdropDismissal};

use crate::ui::surface::SurfaceState;
use crate::ui::SlackWorkspace;

impl SurfaceState {
    pub(super) fn render_slack_self_menu_layer(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let presence = self
            .slack_presence_authority
            .resolve_self_presence(workspace);
        let backdrop = dismissible_backdrop(
            div()
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .left(px(0.0)),
            BackdropDismissal::new(|this: &mut Self, _, _, cx| {
                this.close_slack_self_menu(cx);
            }),
            cx,
        );
        div()
            .id("slack-self-menu-layer")
            .absolute()
            .top(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .occlude()
            .child(backdrop)
            .child(self.render_slack_self_menu(workspace, presence, cx))
            .into_any_element()
    }
}
