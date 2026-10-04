use crate::ui::surface::{
    div, px, rgb, Context, Div, FluentBuilder, ParentElement, Styled, SurfaceState,
};

mod detail;
mod feed;
mod helpers;
mod reminder;
mod states;
mod thread;

impl SurfaceState {
    pub(super) fn render_slack_later_surface(&self, cx: &mut Context<Self>) -> Div {
        div()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .relative()
            .flex()
            .bg(rgb(0x1b1d21))
            .child(self.render_slack_later_list_pane(cx))
            .child(self.render_slack_later_detail_pane(cx))
            .when(self.slack_later_reminder_dialog.is_some(), |this| {
                this.child(self.render_slack_later_reminder_layer(cx))
            })
    }
}
