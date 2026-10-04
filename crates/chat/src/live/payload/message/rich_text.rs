mod block_kit;
mod reactions;

use std::collections::HashMap;

use crate::model::{
    SlackRichTextBlock, SlackRichTextBody, SlackRichTextBroadcastRange, SlackRichTextInline,
    SlackRichTextListStyle, SlackRichTextSection, SlackRichTextStyle,
};
use serde_json::Value;

use crate::live::payload::{sidebar_dom::SlackSidebarSnapshot, util::slack_user_display_name};

use block_kit::{slack_block_kit_blocks, slack_block_kit_body};
pub(super) use reactions::slack_message_reactions;

pub(crate) fn slack_message_body(message: &Value, has_attachments: bool) -> String {
    let block_body = slack_block_kit_body(message, has_attachments);
    if !block_body.is_empty() {
        return block_body;
    }

    let body = message
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if has_attachments && slack_attachment_only_fallback(body) {
        String::new()
    } else {
        body.to_string()
    }
}

fn slack_attachment_only_fallback(body: &str) -> bool {
    let body = body.trim();
    body.starts_with('<') && body.ends_with('>') && !body.contains(char::is_whitespace)
}

pub(super) fn slack_rich_text_body(
    message: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> Option<SlackRichTextBody> {
    slack_rich_text_body_from_blocks(
        message.get("blocks").and_then(Value::as_array)?,
        users,
        sidebar_snapshot,
    )
}

pub(in crate::live) fn slack_rich_text_body_from_blocks(
    raw_blocks: &[Value],
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> Option<SlackRichTextBody> {
    let blocks = raw_blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("rich_text"))
        .flat_map(|block| {
            block
                .get("elements")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|block| slack_rich_text_block(block, users, sidebar_snapshot))
        .collect::<Vec<_>>();
    let block_kit = slack_block_kit_blocks(raw_blocks, true);
    (!blocks.is_empty() || !block_kit.is_empty()).then_some(SlackRichTextBody { blocks, block_kit })
}

fn slack_rich_text_block(
    block: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> Option<SlackRichTextBlock> {
    match block.get("type").and_then(Value::as_str)? {
        "rich_text_section" => Some(SlackRichTextBlock::Section {
            elements: slack_rich_text_elements(block, users, sidebar_snapshot),
        }),
        "rich_text_quote" => Some(SlackRichTextBlock::Quote {
            elements: slack_rich_text_elements(block, users, sidebar_snapshot),
        }),
        "rich_text_preformatted" => Some(SlackRichTextBlock::Preformatted {
            elements: slack_rich_text_elements(block, users, sidebar_snapshot),
        }),
        "rich_text_list" => slack_rich_text_list(block, users, sidebar_snapshot),
        _ => None,
    }
}

fn slack_rich_text_list(
    block: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> Option<SlackRichTextBlock> {
    let style = match block.get("style").and_then(Value::as_str)? {
        "bullet" => SlackRichTextListStyle::Bullet,
        "ordered" => SlackRichTextListStyle::Ordered,
        _ => return None,
    };
    let items = block
        .get("elements")
        .and_then(Value::as_array)?
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("rich_text_section"))
        .map(|item| SlackRichTextSection {
            elements: slack_rich_text_elements(item, users, sidebar_snapshot),
        })
        .collect();
    Some(SlackRichTextBlock::List {
        style,
        indent: block
            .get("indent")
            .and_then(Value::as_u64)
            .and_then(|indent| u8::try_from(indent).ok())
            .unwrap_or(0),
        offset: block
            .get("offset")
            .and_then(Value::as_u64)
            .and_then(|offset| u32::try_from(offset).ok()),
        items,
    })
}

fn slack_rich_text_elements(
    container: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> Vec<SlackRichTextInline> {
    container
        .get("elements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|element| slack_rich_text_inline(element, users, sidebar_snapshot))
        .collect()
}

fn slack_rich_text_inline(
    element: &Value,
    users: &HashMap<String, Value>,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
) -> Option<SlackRichTextInline> {
    let style = slack_rich_text_style(element);
    match element.get("type").and_then(Value::as_str)? {
        "text" => Some(SlackRichTextInline::Text {
            text: element.get("text").and_then(Value::as_str)?.to_string(),
            style,
        }),
        "link" => Some(SlackRichTextInline::Link {
            url: element.get("url").and_then(Value::as_str)?.to_string(),
            label: element
                .get("text")
                .and_then(Value::as_str)
                .filter(|label| !label.is_empty())
                .map(str::to_string),
            unsafe_url: element
                .get("unsafe")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            style,
        }),
        "user" => slack_rich_text_user(element, users, style),
        "channel" => slack_rich_text_channel(element, sidebar_snapshot, style),
        "broadcast" => slack_rich_text_broadcast(element, style),
        "emoji" => slack_rich_text_emoji(element, style),
        _ => None,
    }
}

fn slack_rich_text_user(
    element: &Value,
    users: &HashMap<String, Value>,
    style: SlackRichTextStyle,
) -> Option<SlackRichTextInline> {
    let user_id = element.get("user_id").and_then(Value::as_str)?;
    let label = users
        .get(user_id)
        .and_then(slack_user_display_name)
        .unwrap_or_else(|| user_id.to_string());
    Some(SlackRichTextInline::User {
        user_id: user_id.to_string(),
        label,
        style,
    })
}

fn slack_rich_text_channel(
    element: &Value,
    sidebar_snapshot: Option<&SlackSidebarSnapshot>,
    style: SlackRichTextStyle,
) -> Option<SlackRichTextInline> {
    let channel_id = element.get("channel_id").and_then(Value::as_str)?;
    let label = sidebar_snapshot
        .and_then(|sidebar| sidebar.item(channel_id))
        .map(|item| item.label.clone())
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| channel_id.to_string());
    Some(SlackRichTextInline::Channel {
        channel_id: channel_id.to_string(),
        label,
        style,
    })
}

fn slack_rich_text_broadcast(
    element: &Value,
    style: SlackRichTextStyle,
) -> Option<SlackRichTextInline> {
    let range = match element.get("range").and_then(Value::as_str)? {
        "here" => SlackRichTextBroadcastRange::Here,
        "channel" => SlackRichTextBroadcastRange::Channel,
        "everyone" => SlackRichTextBroadcastRange::Everyone,
        range => SlackRichTextBroadcastRange::Other(range.to_string()),
    };
    Some(SlackRichTextInline::Broadcast { range, style })
}

fn slack_rich_text_emoji(
    element: &Value,
    style: SlackRichTextStyle,
) -> Option<SlackRichTextInline> {
    let name = element.get("name").and_then(Value::as_str)?;
    Some(SlackRichTextInline::Emoji {
        name: name.to_string(),
        unicode: element
            .get("unicode")
            .and_then(Value::as_str)
            .and_then(slack_rich_text_emoji_unicode),
        skin_tone: element
            .get("skin_tone")
            .and_then(Value::as_u64)
            .and_then(|skin_tone| u8::try_from(skin_tone).ok()),
        style,
    })
}

fn slack_rich_text_style(element: &Value) -> SlackRichTextStyle {
    let style = element.get("style");
    SlackRichTextStyle {
        bold: style
            .and_then(|style| style.get("bold"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        italic: style
            .and_then(|style| style.get("italic"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        underline: style
            .and_then(|style| style.get("underline"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        strike: style
            .and_then(|style| style.get("strike"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        code: style
            .and_then(|style| style.get("code"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

fn slack_rich_text_emoji_unicode(value: &str) -> Option<String> {
    if value
        .chars()
        .any(|character| !character.is_ascii_hexdigit() && character != '-' && character != '_')
    {
        return Some(value.to_string());
    }
    value
        .split(['-', '_'])
        .map(|component| {
            u32::from_str_radix(component, 16)
                .ok()
                .and_then(char::from_u32)
        })
        .collect()
}
