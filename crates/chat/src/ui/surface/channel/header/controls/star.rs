use crate::ui::surface::{slack_icon, slack_palette, SlackShellIcon, SurfaceState};
use crate::ui::{
    div, px, rgb, AppearanceMode, Context, Div, InteractiveElement, ParentElement,
    StatefulInteractiveElement, Styled,
};
use gpui::{AppContext, Role, Stateful};

use super::{slack_header_action_key, slack_header_button_hover, SlackHeaderTooltip};

impl SurfaceState {
    pub(in crate::ui::surface::channel::header) fn render_slack_header_star_button(
        &self,
        id: &'static str,
        add_label: &'static str,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = slack_palette(self.appearance_mode);
        let starred = self.slack_active_channel_is_starred();
        let action_label = if starred {
            "Remove from Starred"
        } else {
            add_label
        };
        let border = match self.appearance_mode {
            AppearanceMode::Dark => 0x34363b,
            AppearanceMode::Light => palette.main_border,
        };
        let hover = slack_header_button_hover(self.appearance_mode);
        div()
            .id(id)
            .role(Role::Button)
            .aria_label(action_label)
            .focusable()
            .tab_stop(true)
            .size(px(28.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(border))
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(hover)))
            .focus_visible(|style| style.border_2().border_color(rgb(0x1264a3)))
            .tooltip(move |_, cx| {
                cx.new(|_| SlackHeaderTooltip {
                    label: action_label,
                })
                .into()
            })
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::HeaderStar,
                if starred { 0xe3a300 } else { palette.main_text },
                20.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, _, cx| {
                this.toggle_slack_active_channel_star(cx);
            }))
            .on_key_down(cx.listener(|this, event, window, cx| {
                if slack_header_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_slack_active_channel_star(cx);
                }
            }))
    }
}
