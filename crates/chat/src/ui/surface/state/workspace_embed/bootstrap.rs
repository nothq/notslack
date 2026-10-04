use super::{
    div, font, px, rgb, slack_bootstrap_message_rows, slack_bootstrap_sidebar_rows, slack_palette,
    AnyElement, FluentBuilder, IntoElement, ParentElement, Styled, SurfaceState,
    SLACK_HISTORY_HEIGHT, SLACK_MAIN_HEADER_HEIGHT, SLACK_SIDEBAR_HEADER_HEIGHT,
};
use crate::ui::{SLACK_NAV_RAIL_WIDTH, SLACK_TOP_NAV_RAIL_WIDTH, SLACK_WORKSPACE_RAIL_WIDTH};

impl SurfaceState {
    pub(in crate::ui::surface::state) fn render_slack_bootstrap_shell(&self) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let shimmer = rgb(palette.main_border);
        div()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .bg(rgb(palette.surface_bg))
            .text_color(rgb(palette.surface_text))
            .font(font("Lato"))
            .child(self.render_slack_bootstrap_history())
            .child(self.render_slack_bootstrap_body(shimmer))
            .into_any_element()
    }

    fn render_slack_bootstrap_history(&self) -> AnyElement {
        div()
            .h(px(SLACK_HISTORY_HEIGHT))
            .w_full()
            .border_b_1()
            .border_color(rgb(0x242629))
            .bg(rgb(0x0d0d0d))
            .flex()
            .items_center()
            .pl(px(SLACK_TOP_NAV_RAIL_WIDTH
                + self.slack_sidebar_width()
                + 52.0))
            .child(
                div()
                    .h(px(28.0))
                    .flex_grow(1.0)
                    .max_w(px(795.0))
                    .rounded(px(6.0))
                    .bg(rgb(0x3b3b3b)),
            )
            .into_any_element()
    }

    fn render_slack_bootstrap_body(&self, shimmer: gpui::Rgba) -> AnyElement {
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .when(self.slack_workspace_switcher_expanded, |this| {
                this.child(
                    div()
                        .w(px(SLACK_WORKSPACE_RAIL_WIDTH))
                        .h_full()
                        .bg(rgb(0x0e0e0e)),
                )
            })
            .child(
                div()
                    .w(px(SLACK_NAV_RAIL_WIDTH))
                    .h_full()
                    .border_l_1()
                    .border_color(rgb(0x2f2f31))
                    .bg(rgb(0x101112)),
            )
            .child(self.render_slack_bootstrap_sidebar(shimmer))
            .child(self.render_slack_bootstrap_main(shimmer))
            .into_any_element()
    }

    fn render_slack_bootstrap_sidebar(&self, shimmer: gpui::Rgba) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .w(px(self.slack_sidebar_width()))
            .h_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(rgb(palette.sidebar_border))
            .bg(rgb(palette.sidebar_bg))
            .child(
                div()
                    .h(px(SLACK_SIDEBAR_HEADER_HEIGHT))
                    .px(px(16.0))
                    .flex()
                    .items_center()
                    .child(div().w(px(118.0)).h(px(16.0)).rounded(px(4.0)).bg(shimmer)),
            )
            .child(slack_bootstrap_sidebar_rows(shimmer))
            .into_any_element()
    }

    fn render_slack_bootstrap_main(&self, shimmer: gpui::Rgba) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.main_bg))
            .child(
                div()
                    .h(px(SLACK_MAIN_HEADER_HEIGHT))
                    .px(px(16.0))
                    .border_b_1()
                    .border_color(rgb(palette.main_border))
                    .flex()
                    .items_center()
                    .child(div().w(px(154.0)).h(px(16.0)).rounded(px(4.0)).bg(shimmer)),
            )
            .child(slack_bootstrap_message_rows(shimmer))
            .child(
                div()
                    .mx(px(20.0))
                    .mb(px(20.0))
                    .h(px(80.0))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(rgb(palette.composer_border))
                    .bg(rgb(palette.composer_bg))
                    .when_some(self.slack_error.clone(), |this, error| {
                        this.flex()
                            .items_center()
                            .px(px(12.0))
                            .text_size(px(12.0))
                            .text_color(rgb(0xf2d8d6))
                            .child(error)
                    }),
            )
            .into_any_element()
    }
}
