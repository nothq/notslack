use super::{
    alpha, div, px, rgb, Context, Div, FontWeight, InteractiveElement, MouseButton, MouseDownEvent,
    ParentElement, SlackRailView, StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::SlackWorkspace;
use gpui::Role;

const SLACK_RAIL_SURFACE_HEADER_HEIGHT: f32 = 49.0;

impl SurfaceState {
    pub(super) fn render_slack_more_surface(&self, cx: &mut Context<Self>) -> Div {
        div()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .bg(rgb(0x1b1d21))
            .child(self.render_slack_rail_surface_header("Customize navigation bar"))
            .child(self.render_slack_navigation_customization_body(cx))
    }

    pub(super) fn render_slack_admin_surface(
        &self,
        _workspace: &SlackWorkspace,
        _cx: &mut Context<Self>,
    ) -> Div {
        div()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .bg(rgb(0x1b1d21))
            .child(self.render_slack_rail_surface_header("Admin"))
    }

    fn render_slack_rail_surface_header(&self, title: &'static str) -> Div {
        div()
            .h(px(SLACK_RAIL_SURFACE_HEADER_HEIGHT))
            .flex_none()
            .px(px(16.0))
            .border_b_1()
            .border_color(rgb(0x34363a))
            .flex()
            .items_center()
            .text_size(px(18.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xf8f8f8))
            .child(title)
    }

    fn render_slack_navigation_customization_body(&self, cx: &mut Context<Self>) -> Div {
        div()
            .max_w(px(680.0))
            .p(px(24.0))
            .flex()
            .flex_col()
            .gap(px(8.0))
            .child(
                div()
                    .pb(px(8.0))
                    .text_size(px(13.0))
                    .line_height(px(20.0))
                    .text_color(rgb(0xb9babd))
                    .child("Choose which optional destinations appear in the navigation bar."),
            )
            .children(
                [
                    SlackRailView::Dms,
                    SlackRailView::Activity,
                    SlackRailView::Files,
                    SlackRailView::Later,
                ]
                .into_iter()
                .filter(|view| self.slack_rail_view_available(*view))
                .map(|view| self.render_slack_navigation_visibility_row(view, cx)),
            )
    }

    fn render_slack_navigation_visibility_row(
        &self,
        view: SlackRailView,
        cx: &mut Context<Self>,
    ) -> impl gpui::IntoElement {
        let hidden = self.slack_hidden_rail_views.contains(&view);
        let label = view.title();
        div()
            .id(slack_navigation_visibility_element_id(label))
            .role(Role::Button)
            .aria_label(slack_navigation_visibility_label(label, hidden))
            .focusable()
            .tab_stop(true)
            .h(px(40.0))
            .px(px(12.0))
            .rounded(px(6.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x25282c)))
            .focus_visible(|style| style.bg(rgb(0x25282c)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    this.toggle_slack_rail_view_visibility(view, cx);
                }),
            )
            .on_key_down(
                cx.listener(move |this, event: &gpui::KeyDownEvent, window, cx| {
                    if matches!(event.keystroke.key.as_str(), "enter" | "space")
                        && !event.keystroke.modifiers.modified()
                    {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.toggle_slack_rail_view_visibility(view, cx);
                    }
                }),
            )
            .flex()
            .items_center()
            .justify_between()
            .text_size(px(14.0))
            .text_color(rgb(0xf8f8f8))
            .child(label)
            .child(
                div()
                    .rounded_full()
                    .px(px(8.0))
                    .py(px(2.0))
                    .bg(if hidden {
                        alpha(0x34363a, 1.0)
                    } else {
                        alpha(0x2eb67d, 0.22)
                    })
                    .text_size(px(11.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(if hidden { 0xb9babd } else { 0x7de2b8 }))
                    .child(if hidden { "Hidden" } else { "Shown" }),
            )
    }
}

fn slack_navigation_visibility_element_id(label: &str) -> String {
    format!(
        "slack-customize-navigation-{}",
        label.to_ascii_lowercase().replace(' ', "-")
    )
}

fn slack_navigation_visibility_label(label: &str, hidden: bool) -> String {
    if hidden {
        format!("Show {label} in navigation")
    } else {
        format!("Hide {label} from navigation")
    }
}
