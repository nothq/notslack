use super::{
    div, px, rgb, AnyElement, Context, FluentBuilder, IntoElement, ParentElement, Styled,
    SurfaceState,
};
use crate::model::ChatSurfaceEvent;
use crate::ui::{SlackWorkspace, SLACK_NAV_RAIL_WIDTH, SLACK_WORKSPACE_RAIL_WIDTH};

mod actions;
mod badges;
mod footer;
mod items;
mod menu;

impl SurfaceState {
    pub(crate) fn slack_rail_width(&self) -> f32 {
        SLACK_NAV_RAIL_WIDTH
            + if self.slack_workspace_switcher_expanded {
                SLACK_WORKSPACE_RAIL_WIDTH
            } else {
                0.0
            }
    }

    pub(crate) fn set_slack_workspace_switcher_expanded(
        &mut self,
        expanded: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slack_workspace_switcher_expanded == expanded {
            return false;
        }
        self.slack_workspace_switcher_expanded = expanded;
        cx.notify();
        true
    }

    pub(crate) fn toggle_slack_workspace_switcher(&mut self, cx: &mut Context<Self>) {
        let expanded = !self.slack_workspace_switcher_expanded;
        self.set_slack_workspace_switcher_expanded(expanded, cx);
        cx.emit(ChatSurfaceEvent::SlackWorkspaceSwitcherExpandedCommitted { expanded });
    }

    pub(super) fn render_slack_rail(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .w(px(self.slack_rail_width()))
            .h_full()
            .relative()
            .flex()
            .bg(rgb(0x0e0e0e))
            .when(self.slack_workspace_switcher_expanded, |this| {
                this.child(
                    div()
                        .w(px(SLACK_WORKSPACE_RAIL_WIDTH))
                        .h_full()
                        .flex_none()
                        .pt(px(8.0))
                        .flex()
                        .flex_col()
                        .items_center(),
                )
            })
            .child(
                div()
                    .w(px(SLACK_NAV_RAIL_WIDTH))
                    .h_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .bg(rgb(0x101112))
                    .child(self.render_slack_rail_primary_section(workspace, cx))
                    .child(self.render_slack_rail_footer(workspace, cx)),
            )
            .when(self.slack_workspace_switcher_expanded, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(SLACK_WORKSPACE_RAIL_WIDTH))
                        .top(px(0.0))
                        .bottom(px(0.0))
                        .w(px(1.0))
                        .bg(rgb(0x2f2f31)),
                )
            })
            .into_any_element()
    }
}

fn slack_rail_action_key(event: &gpui::KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
