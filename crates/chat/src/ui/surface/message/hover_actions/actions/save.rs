use crate::ui::surface::{
    div, px, rgb, slack_icon, slack_palette, Context, FluentBuilder, InteractiveElement,
    IntoElement, KeyDownEvent, ParentElement, SlackMessageRow, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::SlackSavedMessageMutation;
use gpui::Role;

impl SurfaceState {
    pub(super) fn render_slack_save_action(
        &self,
        row: &SlackMessageRow,
        hover_group: &str,
        persistent: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let label = self.slack_message_save_label(row);
        let any_pending = self.slack_pending_message_saved.is_some();
        let target = row
            .action_target
            .clone()
            .expect("supported Slack save action must have a typed target");
        let keyboard_target = target.clone();
        let saved_state = row.saved_state;
        div()
            .id(format!("slack-message-save-{}", row.id))
            .role(Role::Button)
            .aria_label(label)
            .focusable()
            .tab_stop(true)
            .relative()
            .size(px(32.0))
            .rounded(px(8.0))
            .opacity(if persistent { 1.0 } else { 0.0 })
            .group_hover(hover_group.to_string(), |style| style.opacity(1.0))
            .focus_visible(|style| style.opacity(1.0).bg(rgb(0xe8f5fa)))
            .when(!any_pending, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(rgb(palette.composer_chip_bg)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_slack_message_saved_state_for_target(
                            target.clone(),
                            saved_state,
                            cx,
                        );
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if slack_save_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.toggle_slack_message_saved_state_for_target(
                                keyboard_target.clone(),
                                saved_state,
                                cx,
                            );
                        }
                    }))
            })
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::Later,
                palette.main_secondary_text,
                18.0,
                cx,
            ))
    }

    fn slack_message_save_label(&self, row: &SlackMessageRow) -> &'static str {
        let target = row.action_target.as_deref();
        let pending = self.slack_pending_message_saved.as_ref().filter(|request| {
            target.is_some_and(|target| {
                request.team_id == target.team_id()
                    && request.conversation_id == target.conversation_id()
                    && request.message_timestamp == *target.message_timestamp()
            })
        });
        pending.map_or_else(
            || {
                if row.saved_state.is_some() {
                    "Remove from later"
                } else {
                    "Save for later"
                }
            },
            |request| match request.mutation {
                SlackSavedMessageMutation::Save => "Saving for later…",
                SlackSavedMessageMutation::Remove => "Removing from later…",
            },
        )
    }
}

fn slack_save_action_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}
