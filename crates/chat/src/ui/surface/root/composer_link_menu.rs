//! The menu a right click on a composer link opens, like Slack's: copy,
//! edit, open, or show the full address.

use gpui::{
    anchored, deferred, div, point, px, rgb, AnyElement, BoxShadow, ClipboardItem, Context,
    InteractiveElement, IntoElement, ParentElement, Role, Stateful, StatefulInteractiveElement,
    Styled,
};
use gpui_components::backdrop::dismissible_click_away;

use crate::ui::{alpha, surface::SurfaceState};

impl SurfaceState {
    pub(super) fn render_slack_composer_link_menu(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let menu = self.slack_composer_link_menu.as_ref()?;
        let url = menu.url.as_str().to_string();
        let copy_url = url.clone();
        let items = div()
            .w(px(260.0))
            .py(px(8.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(alpha(0xffffff, 0.12))
            .bg(rgb(0x222529))
            .shadow(vec![BoxShadow {
                color: alpha(0x000000, 0.4),
                offset: point(px(0.0), px(6.0)),
                blur_radius: px(20.0),
                spread_radius: px(0.0),
                inset: false,
            }])
            .flex()
            .flex_col()
            .child(
                slack_link_menu_item("copy", "🔗  Copy link").on_click(cx.listener(
                    move |this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(copy_url.clone()));
                        this.close_slack_composer_link_menu(cx);
                    },
                )),
            )
            .child(
                slack_link_menu_item("edit", "✏️  Edit link")
                    .on_click(cx.listener(|this, _, _, cx| this.edit_slack_composer_menu_link(cx))),
            )
            .child(div().my(px(6.0)).h(px(1.0)).bg(alpha(0xffffff, 0.10)))
            .child(
                slack_link_menu_item("open", "↗  Open in browser").on_click(cx.listener(
                    move |this, _, _, cx| {
                        cx.open_url(&url);
                        this.close_slack_composer_link_menu(cx);
                    },
                )),
            )
            .child(div().my(px(6.0)).h(px(1.0)).bg(alpha(0xffffff, 0.10)))
            .child(
                div()
                    .px(px(20.0))
                    .py(px(4.0))
                    .text_size(px(13.0))
                    .text_color(rgb(0xababad))
                    .child("Display as"),
            )
            .child(slack_link_menu_item("as-url", "URL").on_click(
                cx.listener(|this, _, _, cx| this.show_slack_composer_menu_link_as_url(cx)),
            ));
        let menu_element = dismissible_click_away(
            items,
            |this: &mut Self, _, _, cx| this.close_slack_composer_link_menu(cx),
            cx,
        );
        Some(
            deferred(
                anchored()
                    .position(menu.position)
                    .snap_to_window()
                    .child(menu_element),
            )
            .with_priority(70)
            .into_any_element(),
        )
    }
}

fn slack_link_menu_item(id: &'static str, label: &'static str) -> Stateful<gpui::Div> {
    div()
        .id(id)
        .role(Role::MenuItem)
        .aria_label(label)
        .h(px(32.0))
        .px(px(20.0))
        .flex()
        .items_center()
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0x1264a3)))
        .text_size(px(15.0))
        .text_color(rgb(0xd1d2d3))
        .child(label)
}
