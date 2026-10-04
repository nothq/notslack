use super::super::super::{
    alpha, div, px, rgb, slack_icon, AnyElement, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, SlackRailItemSpec,
    SlackRailView, StatefulInteractiveElement, Styled, SurfaceState, SLACK_RAIL_ITEMS,
};
use super::slack_rail_action_key;
use crate::ui::surface::SlackRailMenu;
use crate::ui::SlackWorkspace;
use gpui::{Orientation, Role};

impl SurfaceState {
    fn render_slack_rail_item(
        &self,
        workspace: &SlackWorkspace,
        item: &SlackRailItemSpec,
        active: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let view = SlackRailView::from_label(item.label);
        let icon_highlighted = self.slack_rail_item_icon_highlighted(item.label, active);
        div()
            .id(format!("slack-rail-{}", item.label.to_ascii_lowercase()))
            .role(Role::Tab)
            .aria_label(self.slack_rail_item_accessibility_label(workspace, item.label))
            .aria_selected(active)
            .focusable()
            .tab_stop(true)
            .w(px(52.0))
            .h(px(68.0))
            .when(item.label == "Admin", |this| this.mt(px(21.0)))
            .flex()
            .flex_col()
            .items_center()
            .pt(px(8.0))
            .gap(px(4.0))
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|style| style.bg(alpha(0xf6f6f6, 0.08)))
            .focus_visible(|style| style.bg(alpha(0xf6f6f6, 0.12)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                    if let Some(view) = view {
                        this.activate_slack_rail_control(view, cx);
                    }
                }),
            )
            .on_key_down(cx.listener(move |this, event, window, cx| {
                if slack_rail_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    if let Some(view) = view {
                        this.activate_slack_rail_control(view, cx);
                    }
                }
            }))
            .child(self.render_slack_rail_item_icon(workspace, item, icon_highlighted, cx))
            .child(slack_rail_item_label(item.label))
    }

    fn slack_rail_item_icon_highlighted(&self, label: &str, active: bool) -> bool {
        let menu_open = matches!(
            (label, self.slack_rail_menu),
            ("More", Some(SlackRailMenu::More)) | ("Admin", Some(SlackRailMenu::Admin))
        );
        active || menu_open || (label == "DMs" && self.slack_dms_peek_visible)
    }

    fn render_slack_rail_item_icon(
        &self,
        workspace: &SlackWorkspace,
        item: &SlackRailItemSpec,
        highlighted: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        div()
            .size(px(36.0))
            .rounded(px(8.0))
            .relative()
            .flex()
            .items_center()
            .justify_center()
            .when(highlighted, |this| this.bg(alpha(0xf6f6f6, 0.25)))
            .child(slack_icon(item.icon, 0xffffff, 20.0, cx))
            .when_some(
                self.slack_rail_item_badge_value(workspace, item.label),
                |this, value| this.child(self.render_slack_rail_item_badge(item.label, &value)),
            )
    }

    fn slack_rail_item_accessibility_label(
        &self,
        workspace: &SlackWorkspace,
        label: &str,
    ) -> String {
        match label {
            "Home" if workspace.rail_badges.home.is_some_and(|count| count > 0) => {
                "Home, with unreads".to_string()
            }
            "DMs" => self
                .slack_conversation_read_overlay_dm_badges(workspace)
                .1
                .filter(|count| *count > 0)
                .map_or_else(
                    || "DMs".to_string(),
                    |count| {
                        let message = if count == 1 { "message" } else { "messages" };
                        format!("DMs, {count} unread {message}")
                    },
                ),
            "Activity" => workspace
                .rail_badges
                .activity
                .filter(|count| *count > 0)
                .map_or_else(
                    || "Activity".to_string(),
                    |count| format!("Activity, {count} notifications"),
                ),
            "More" => "More…".to_string(),
            _ => label.to_string(),
        }
    }

    pub(super) fn render_slack_rail_primary_section(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .px(px(9.0))
            .pt(px(5.0))
            .flex()
            .flex_col()
            .items_center()
            .child(self.render_slack_rail_tabs(workspace, cx))
            .into_any_element()
    }

    fn render_slack_rail_tabs(
        &self,
        workspace: &SlackWorkspace,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .id("slack-rail-tabs")
            .role(Role::TabList)
            .aria_label(workspace.workspace_name.clone())
            .aria_orientation(Orientation::Vertical)
            .children(
                SLACK_RAIL_ITEMS
                    .iter()
                    .filter(|item| self.slack_rail_item_supported(workspace, item))
                    .map(|item| {
                        let active = SlackRailView::from_label(item.label)
                            == Some(self.slack_active_rail_view);
                        self.render_slack_rail_item(workspace, item, active, cx)
                    }),
            )
    }

    fn slack_rail_item_supported(
        &self,
        workspace: &SlackWorkspace,
        item: &SlackRailItemSpec,
    ) -> bool {
        let Some(view) = SlackRailView::from_label(item.label) else {
            return false;
        };
        if self.slack_hidden_rail_views.contains(&view) {
            return false;
        }
        match view {
            SlackRailView::More => true,
            SlackRailView::Admin => workspace.rail_badges.admin_visible,
            _ => self.slack_rail_view_available(view),
        }
    }

    fn activate_slack_rail_control(&mut self, view: SlackRailView, cx: &mut Context<Self>) {
        match view {
            SlackRailView::More => self.toggle_slack_rail_menu(SlackRailMenu::More, cx),
            SlackRailView::Admin => self.toggle_slack_rail_menu(SlackRailMenu::Admin, cx),
            _ => self.activate_slack_rail_view(view, cx),
        }
    }
}

fn slack_rail_item_label(label: &'static str) -> Div {
    div()
        .h(px(12.0))
        .text_size(px(11.0))
        .line_height(px(12.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .child(label)
}
