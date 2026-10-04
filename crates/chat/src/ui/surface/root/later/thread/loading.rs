use crate::ui::surface::{
    div, px, rgb, slack_palette, AnyElement, Context, Div, IntoElement, ParentElement,
    SlackPalette, Styled, SurfaceState,
};
use gpui::SharedString;

impl SurfaceState {
    pub(in crate::ui::surface::root::later) fn render_slack_later_thread_loading(
        &self,
        conversation_name: Option<SharedString>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .size_full()
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .child(self.render_slack_later_detail_header("Thread", conversation_name, cx))
            .child(self.render_slack_later_thread_loading_skeleton(&palette))
            .into_any_element()
    }

    pub(in crate::ui::surface::root::later) fn render_slack_later_thread_loading_skeleton(
        &self,
        palette: &SlackPalette,
    ) -> Div {
        div()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .px(px(16.0))
            .pt(px(18.0))
            .flex()
            .flex_col()
            .gap(px(20.0))
            .children((0..4).map(|index| {
                div()
                    .h(px(52.0))
                    .flex_none()
                    .flex()
                    .items_start()
                    .gap(px(10.0))
                    .child(
                        div()
                            .size(px(36.0))
                            .rounded(px(6.0))
                            .bg(rgb(palette.composer_chip_bg)),
                    )
                    .child(
                        div()
                            .flex_grow(1.0)
                            .pt(px(3.0))
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .h(px(10.0))
                                    .w(px(if index == 0 { 104.0 } else { 82.0 }))
                                    .rounded(px(5.0))
                                    .bg(rgb(palette.composer_chip_bg)),
                            )
                            .child(
                                div()
                                    .h(px(10.0))
                                    .w_full()
                                    .rounded(px(5.0))
                                    .bg(rgb(palette.composer_chip_bg)),
                            ),
                    )
            }))
    }
}
