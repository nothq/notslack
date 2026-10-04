use crate::ui::surface::{
    alpha, slack_activity_palette, slack_icon, SlackActivityRow, SlackShellIcon, SurfaceState,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, Context, Div, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Role,
    Stateful, StatefulInteractiveElement, Styled,
};

impl SurfaceState {
    pub(super) fn slack_activity_row_actions_supported(&self, row: &SlackActivityRow) -> bool {
        (row.read_target.is_some()
            && if row.unread {
                self.slack_workspace_api_capabilities
                    .mark_activity_item_read
            } else {
                self.slack_workspace_api_capabilities
                    .mark_activity_item_unread
            })
            || (row.archive_target.is_some()
                && if row.archived {
                    self.slack_workspace_api_capabilities
                        .unarchive_activity_item
                } else {
                    self.slack_workspace_api_capabilities.archive_activity_item
                })
    }

    pub(super) fn render_slack_activity_row_actions(
        &self,
        row: &SlackActivityRow,
        selected: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let pending = self.slack_activity_item_mutation_is_pending(row.key.as_ref());
        let persistent = selected || pending;
        let read_supported = row.read_target.is_some()
            && if row.unread {
                self.slack_workspace_api_capabilities
                    .mark_activity_item_read
            } else {
                self.slack_workspace_api_capabilities
                    .mark_activity_item_unread
            };
        let archive_supported = row.archive_target.is_some()
            && if row.archived {
                self.slack_workspace_api_capabilities
                    .unarchive_activity_item
            } else {
                self.slack_workspace_api_capabilities.archive_activity_item
            };
        div()
            .id(row.actions_id.clone())
            .role(Role::Group)
            .aria_label("Activity item actions")
            .absolute()
            .block_mouse_except_scroll()
            .right(px(8.0))
            .top(px(8.0))
            .h(px(36.0))
            .p(px(2.0))
            .flex()
            .items_center()
            .when(read_supported, |this| {
                this.child(self.render_slack_activity_read_action(row, persistent, pending, cx))
            })
            .when(archive_supported, |this| {
                this.child(self.render_slack_activity_archive_action(row, persistent, pending, cx))
            })
    }

    fn render_slack_activity_read_action(
        &self,
        row: &SlackActivityRow,
        persistent: bool,
        pending: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = slack_activity_palette(self.appearance_mode);
        let click_key = row.key.clone();
        let keyboard_key = row.key.clone();
        let label = if row.unread {
            "Mark as read"
        } else {
            "Mark as unread"
        };
        let icon = if row.unread {
            SlackShellIcon::MenuCheck
        } else {
            SlackShellIcon::MessageMenuMarkUnread
        };
        div()
            .id(row.read_action_id.clone())
            .role(Role::Button)
            .aria_label(label)
            .focusable()
            .tab_stop(!pending)
            .relative()
            .size(px(32.0))
            .rounded(px(7.0))
            .opacity(if persistent { 1.0 } else { 0.0 })
            .group_hover(row.actions_hover_group.clone(), |style| style.opacity(1.0))
            .focus_visible(move |style| {
                style
                    .opacity(1.0)
                    .bg(alpha(palette.action_hover, palette.action_hover_alpha))
            })
            .when(!pending, |this| {
                this.cursor_pointer()
                    .hover(move |style| {
                        style.bg(alpha(palette.action_hover, palette.action_hover_alpha))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_slack_activity_item_read(click_key.as_ref(), cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if activity_row_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.toggle_slack_activity_item_read(keyboard_key.as_ref(), cx);
                        }
                    }))
            })
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(icon, palette.primary_text, 18.0, cx))
    }

    fn render_slack_activity_archive_action(
        &self,
        row: &SlackActivityRow,
        persistent: bool,
        pending: bool,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let palette = slack_activity_palette(self.appearance_mode);
        let click_key = row.key.clone();
        let keyboard_key = row.key.clone();
        let (label, icon) = slack_activity_archive_action_content(row);
        div()
            .id(row.archive_action_id.clone())
            .role(Role::Button)
            .aria_label(label)
            .focusable()
            .tab_stop(!pending)
            .relative()
            .size(px(32.0))
            .rounded(px(7.0))
            .opacity(if persistent { 1.0 } else { 0.0 })
            .group_hover(row.actions_hover_group.clone(), |style| style.opacity(1.0))
            .focus_visible(move |style| {
                style
                    .opacity(1.0)
                    .bg(alpha(palette.action_hover, palette.action_hover_alpha))
            })
            .when(!pending, |this| {
                this.cursor_pointer()
                    .hover(move |style| {
                        style.bg(alpha(palette.action_hover, palette.action_hover_alpha))
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.toggle_slack_activity_archive_from_control(click_key.as_ref(), cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if activity_row_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.toggle_slack_activity_archive_from_control(
                                keyboard_key.as_ref(),
                                cx,
                            );
                        }
                    }))
            })
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(icon, palette.primary_text, 18.0, cx))
    }

    fn toggle_slack_activity_archive_from_control(&mut self, key: &str, cx: &mut Context<Self>) {
        self.toggle_slack_activity_item_archive(
            key,
            "clear_notification_button",
            "restore_notification_button",
            cx,
        );
    }
}

fn activity_row_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}

fn slack_activity_archive_action_content(row: &SlackActivityRow) -> (&'static str, SlackShellIcon) {
    if row.archived {
        ("Restore notification", SlackShellIcon::Back)
    } else {
        ("Clear notification", SlackShellIcon::ActivityClear)
    }
}
