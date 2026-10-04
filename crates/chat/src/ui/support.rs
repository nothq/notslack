use crate::ui::{
    div, px, AnyElement, IntoElement, KeyDownEvent, ParentElement, SlackWorkspace, Styled,
};

pub fn slack_sidebar_icon(icon: Option<&str>) -> &'static str {
    match icon {
        Some("hash") => "#",
        Some("lock") => "L",
        Some("person") => "@",
        _ => ">",
    }
}

pub fn slack_channel_meta(workspace: &SlackWorkspace) -> String {
    let mut parts = Vec::new();
    if let Some(member_count) = workspace.member_count {
        parts.push(format!("{member_count} members"));
    }
    if !workspace.channel_topic.is_empty() {
        parts.push(workspace.channel_topic.clone());
    }
    if parts.is_empty() {
        "Live workspace".to_string()
    } else {
        parts.join("  ")
    }
}

pub fn slack_workspace_title(workspace: &SlackWorkspace) -> String {
    if workspace.channel_kind.is_channel() {
        format!("# {}", workspace.channel_name)
    } else {
        workspace.channel_name.clone()
    }
}

pub fn slack_avatar_fill(label: &str) -> u32 {
    const PALETTE: [u32; 6] = [0xe01e5a, 0x36c5f0, 0x2eb67d, 0xecb22e, 0x611f69, 0x1d9bd1];
    let hash = label.bytes().fold(0usize, |accumulator, byte| {
        accumulator.wrapping_add(byte as usize)
    });
    PALETTE[hash % PALETTE.len()]
}

pub fn initials(value: &str) -> String {
    let initials = value
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase();
    if initials.is_empty() {
        "?".to_string()
    } else {
        initials
    }
}

pub fn render_chat_empty_state(message: impl Into<String>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child("Chat")
        .child(message.into())
        .into_any_element()
}

pub fn render_slack_empty_state(message: impl Into<String>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.0))
        .child("Slack")
        .child(message.into())
        .into_any_element()
}

pub(crate) fn keystroke_input_text(event: &KeyDownEvent) -> Option<&str> {
    let modifiers = &event.keystroke.modifiers;
    if modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
        return None;
    }
    if let Some(key_char) = event.keystroke.key_char.as_deref() {
        return (!key_char.is_empty()).then_some(key_char);
    }
    match event.keystroke.key.as_str() {
        "space" => Some(" "),
        key if key.chars().count() == 1 => Some(key),
        _ => None,
    }
}
