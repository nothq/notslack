use super::super::slack_message_render_context_id;
use super::SlackMessageHoverActionsContext;
use crate::ui::surface::{
    alpha, div, point, px, rgb, slack_icon, slack_palette, BoxShadow, Context, Div, FluentBuilder,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, SlackMessageMenuOpenContext,
    SlackMessageRow, SlackReactionPickerAnchor, SlackReactionPickerState, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use gpui::{ClickEvent, Role};

mod reaction;
mod save;

use reaction::SlackAddReactionActionContext;

impl SurfaceState {
    pub(in crate::ui::surface::message) fn render_slack_message_hover_actions(
        &self,
        row: &SlackMessageRow,
        hover_group: String,
        context: SlackMessageHoverActionsContext,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let picker = self.slack_hover_action_reaction_picker(row, context);
        let persistent = picker.is_some() || self.slack_message_actions_persistent(row);
        div()
            .id(format!(
                "slack-message-actions-{}-{}",
                slack_message_render_context_id(context.render_context),
                row.id
            ))
            .role(Role::Group)
            .aria_label("Message actions")
            .absolute()
            .block_mouse_except_scroll()
            .right(px(16.0))
            .top(px(-16.0))
            .h(px(40.0))
            .p(px(4.0))
            .flex()
            .items_center()
            .child(slack_message_actions_background(
                palette.main_bg,
                persistent,
                &hover_group,
            ))
            .when(context.reaction_supported, |this| {
                this.child(self.render_slack_add_reaction_action(
                    row,
                    SlackAddReactionActionContext::new(
                        context.render_context,
                        &hover_group,
                        persistent,
                    ),
                    cx,
                ))
            })
            .when(context.reply_supported, |this| {
                this.child(self.render_slack_reply_action(row, &hover_group, persistent, cx))
            })
            .when(context.forward_supported, |this| {
                this.child(self.render_slack_forward_action(row, &hover_group, persistent, cx))
            })
            .when(context.save_supported, |this| {
                this.child(self.render_slack_save_action(row, &hover_group, persistent, cx))
            })
            .when(context.more_supported, |this| {
                this.child(self.render_slack_more_action(row, &hover_group, persistent, cx))
            })
            .when_some(picker, |this, picker| {
                this.child(self.render_slack_reaction_picker(picker, cx))
            })
    }

    fn slack_hover_action_reaction_picker<'a>(
        &'a self,
        row: &SlackMessageRow,
        context: SlackMessageHoverActionsContext,
    ) -> Option<&'a SlackReactionPickerState> {
        self.slack_reaction_picker.as_ref().filter(|picker| {
            row.action_target.as_ref().is_some_and(|target| {
                picker.identity.target() == target
                    && picker.identity.render_context() == context.render_context
                    && picker.identity.anchor() == SlackReactionPickerAnchor::HoverAction
            })
        })
    }

    fn slack_message_actions_persistent(&self, row: &SlackMessageRow) -> bool {
        let Some(target) = row.action_target.as_deref() else {
            return false;
        };
        self.slack_pending_message_saved
            .as_ref()
            .is_some_and(|request| {
                request.team_id == target.team_id()
                    && request.conversation_id == target.conversation_id()
                    && request.message_timestamp == *target.message_timestamp()
            })
            || self.slack_message_menu.as_ref().is_some_and(|menu| {
                menu.team_id == target.team_id()
                    && menu.conversation_id == target.conversation_id()
                    && menu.message_timestamp == *target.message_timestamp()
            })
            || self
                .slack_message_forward_modal
                .as_ref()
                .is_some_and(|modal| {
                    modal.source.team_id == target.team_id()
                        && modal.source.conversation_id == target.conversation_id()
                        && modal.source.message_timestamp == *target.message_timestamp()
                })
    }

    fn render_slack_reply_action(
        &self,
        row: &SlackMessageRow,
        hover_group: &str,
        persistent: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let target = row
            .action_target
            .clone()
            .expect("supported Slack reply action must have a typed target");
        let keyboard_target = target.clone();
        div()
            .id(format!("slack-message-reply-{}", row.id))
            .role(Role::Button)
            .aria_label("Reply in thread")
            .focusable()
            .tab_stop(true)
            .relative()
            .size(px(32.0))
            .rounded(px(8.0))
            .opacity(if persistent { 1.0 } else { 0.0 })
            .group_hover(hover_group.to_string(), |style| style.opacity(1.0))
            .focus_visible(|style| style.opacity(1.0).bg(rgb(0xe8f5fa)))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(palette.composer_chip_bg)))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.slack_reaction_picker = None;
                this.open_slack_message_thread_target(&target, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.slack_reaction_picker = None;
                this.open_slack_message_thread_target(&keyboard_target, cx);
            }))
            .child(slack_icon(
                SlackShellIcon::ReplyThread,
                palette.main_secondary_text,
                18.0,
                cx,
            ))
    }

    fn render_slack_forward_action(
        &self,
        row: &SlackMessageRow,
        hover_group: &str,
        persistent: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let target = row
            .action_target
            .clone()
            .expect("supported Slack forward action must have a typed target");
        let source = self
            .slack_message_forward_source_for_target(&target, row)
            .expect("supported Slack forward action must have a valid source");
        let keyboard_source = source.clone();
        div()
            .id(format!("slack-message-forward-{}", row.id))
            .role(Role::Button)
            .aria_label("Forward message…")
            .focusable()
            .tab_stop(true)
            .relative()
            .size(px(32.0))
            .rounded(px(8.0))
            .opacity(if persistent { 1.0 } else { 0.0 })
            .group_hover(hover_group.to_string(), |style| style.opacity(1.0))
            .focus_visible(|style| style.opacity(1.0).bg(rgb(0xe8f5fa)))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(palette.composer_chip_bg)))
            .flex()
            .items_center()
            .justify_center()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_slack_message_forward_source(source.clone(), cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.open_slack_message_forward_source(keyboard_source.clone(), cx);
            }))
            .child(slack_icon(
                SlackShellIcon::MessageForward,
                palette.main_secondary_text,
                18.0,
                cx,
            ))
    }

    fn render_slack_more_action(
        &self,
        row: &SlackMessageRow,
        hover_group: &str,
        persistent: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let palette = slack_palette(self.appearance_mode);
        let target = row
            .action_target
            .clone()
            .expect("supported Slack message menu action must have a typed target");
        let menu_context =
            SlackMessageMenuOpenContext::new(target, row.body.text.clone(), row.user_id.clone());
        let keyboard_menu_context = menu_context.clone();
        div()
            .id(format!("slack-message-more-{}", row.id))
            .role(Role::Button)
            .aria_label("More actions")
            .focusable()
            .tab_stop(true)
            .relative()
            .size(px(32.0))
            .rounded(px(8.0))
            .opacity(if persistent { 1.0 } else { 0.0 })
            .group_hover(hover_group.to_string(), |style| style.opacity(1.0))
            .focus_visible(|style| style.opacity(1.0).bg(rgb(0xe8f5fa)))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(palette.composer_chip_bg)))
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                let position = event.position();
                this.open_slack_message_menu_for_target(menu_context.clone(), position, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !matches!(event.keystroke.key.as_str(), "enter" | "space")
                    || event.keystroke.modifiers.modified()
                {
                    return;
                }
                window.prevent_default();
                cx.stop_propagation();
                this.open_slack_message_menu_for_target(
                    keyboard_menu_context.clone(),
                    point(
                        px((this.preview_width - 32.0).max(16.0)),
                        px(this.viewport_height / 2.0),
                    ),
                    cx,
                );
            }))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                SlackShellIcon::More,
                palette.main_secondary_text,
                18.0,
                cx,
            ))
    }
}

fn slack_message_actions_background(background: u32, persistent: bool, hover_group: &str) -> Div {
    div()
        .absolute()
        .left(px(0.0))
        .right(px(0.0))
        .top(px(0.0))
        .bottom(px(0.0))
        .rounded(px(12.0))
        .bg(rgb(background))
        .shadow(vec![
            BoxShadow {
                color: alpha(0x1d1c1d, 0.13),
                offset: point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(1.0),
                inset: false,
            },
            BoxShadow {
                color: alpha(0x000000, 0.08),
                offset: point(px(0.0), px(1.0)),
                blur_radius: px(3.0),
                spread_radius: px(0.0),
                inset: false,
            },
        ])
        .opacity(if persistent { 1.0 } else { 0.0 })
        .group_hover(hover_group.to_string(), |style| style.opacity(1.0))
}
