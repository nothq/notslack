use gpui::{Div, FontWeight, ParentElement, Styled};

use super::super::{div, px, rgb};
use super::SlackWorkspaceAttention;
use crate::ui::SlackWorkspace;

pub(super) fn slack_workspace_attention_badge(attention: SlackWorkspaceAttention) -> Div {
    match attention {
        SlackWorkspaceAttention::None => div(),
        SlackWorkspaceAttention::Unread => div()
            .absolute()
            .top(px(1.0))
            .right(px(-1.0))
            .size(px(7.0))
            .rounded_full()
            .bg(rgb(0xef3e75)),
        SlackWorkspaceAttention::Mentions(count) => div()
            .absolute()
            .top(px(-4.0))
            .right(px(-5.0))
            .min_w(px(16.0))
            .h(px(16.0))
            .px(px(4.0))
            .rounded_full()
            .bg(rgb(0xef3e75))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(10.0))
            .line_height(px(10.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child(if count > 99 {
                "99+".to_string()
            } else {
                count.to_string()
            }),
    }
}

pub(super) fn slack_workspace_attention(workspace: &SlackWorkspace) -> SlackWorkspaceAttention {
    let sidebar_mentions = workspace
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
        .filter_map(|item| item.count)
        .fold(0_u32, u32::saturating_add);
    let mentions = workspace
        .rail_badges
        .activity
        .unwrap_or_default()
        .max(sidebar_mentions);
    if mentions > 0 {
        return SlackWorkspaceAttention::Mentions(mentions);
    }
    let unread = workspace.rail_badges.home.unwrap_or_default() > 0
        || workspace.rail_badges.dms.unwrap_or_default() > 0
        || workspace
            .rail_badges
            .dms_unread_messages
            .unwrap_or_default()
            > 0
        || workspace
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .any(|item| item.unread);
    if unread {
        SlackWorkspaceAttention::Unread
    } else {
        SlackWorkspaceAttention::None
    }
}

pub(super) fn slack_workspace_accessibility_label(
    workspace_name: &str,
    index: usize,
    count: usize,
    attention: SlackWorkspaceAttention,
    shortcut: Option<&str>,
) -> String {
    let mut label = format!("{workspace_name}, workspace {} of {count}", index + 1);
    match attention {
        SlackWorkspaceAttention::None => {}
        SlackWorkspaceAttention::Unread => label.push_str(", with unread messages"),
        SlackWorkspaceAttention::Mentions(1) => label.push_str(", 1 mention"),
        SlackWorkspaceAttention::Mentions(mentions) => {
            label.push_str(&format!(", {mentions} mentions"));
        }
    }
    if let Some(shortcut) = shortcut {
        label.push_str(&format!(", {shortcut}"));
    }
    label
}
