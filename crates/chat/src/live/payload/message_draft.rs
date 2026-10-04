use crate::model::{
    SlackMessageDraft, SlackRichTextBlock, SlackRichTextInline, SlackRichTextListStyle,
    SlackRichTextStyle,
};
use serde::Serialize;

pub(super) fn slack_message_blocks_json(
    draft: &SlackMessageDraft,
) -> Result<Option<String>, String> {
    let Some(rich_text) = draft.rich_text() else {
        return Ok(None);
    };
    let block = SlackWireRichText {
        kind: "rich_text",
        elements: rich_text.blocks.iter().map(wire_block).collect(),
    };
    serde_json::to_string(&[block])
        .map(Some)
        .map_err(|error| format!("failed to encode Slack rich-text message blocks: {error}"))
}

pub(in crate::live) fn slack_draft_blocks_json(
    draft: &SlackMessageDraft,
) -> Result<String, String> {
    if let Some(blocks) = slack_message_blocks_json(draft)? {
        return Ok(blocks);
    }
    let block = SlackWireRichText {
        kind: "rich_text",
        elements: vec![SlackWireRichTextBlock::Section {
            elements: vec![SlackWireRichTextInline::Text {
                text: draft.fallback_text(),
                style: None,
            }],
        }],
    };
    serde_json::to_string(&[block])
        .map_err(|error| format!("failed to encode Slack draft blocks: {error}"))
}

#[derive(Serialize)]
struct SlackWireRichText<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    elements: Vec<SlackWireRichTextBlock<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum SlackWireRichTextBlock<'a> {
    #[serde(rename = "rich_text_section")]
    Section {
        elements: Vec<SlackWireRichTextInline<'a>>,
    },
    #[serde(rename = "rich_text_list")]
    List {
        style: &'static str,
        indent: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        offset: Option<u32>,
        elements: Vec<SlackWireRichTextSection<'a>>,
    },
    #[serde(rename = "rich_text_quote")]
    Quote {
        elements: Vec<SlackWireRichTextInline<'a>>,
    },
    #[serde(rename = "rich_text_preformatted")]
    Preformatted {
        elements: Vec<SlackWireRichTextInline<'a>>,
    },
}

#[derive(Serialize)]
struct SlackWireRichTextSection<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    elements: Vec<SlackWireRichTextInline<'a>>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum SlackWireRichTextInline<'a> {
    #[serde(rename = "text")]
    Text {
        text: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<SlackWireRichTextStyle>,
    },
    #[serde(rename = "link")]
    Link {
        url: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<SlackWireRichTextStyle>,
    },
    #[serde(rename = "user")]
    User {
        user_id: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<SlackWireRichTextStyle>,
    },
    #[serde(rename = "channel")]
    Channel {
        channel_id: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<SlackWireRichTextStyle>,
    },
    #[serde(rename = "broadcast")]
    Broadcast {
        range: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<SlackWireRichTextStyle>,
    },
    #[serde(rename = "emoji")]
    Emoji {
        name: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        unicode: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        skin_tone: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<SlackWireRichTextStyle>,
    },
}

#[derive(Serialize)]
struct SlackWireRichTextStyle {
    #[serde(skip_serializing_if = "is_false")]
    bold: bool,
    #[serde(skip_serializing_if = "is_false")]
    italic: bool,
    #[serde(skip_serializing_if = "is_false")]
    underline: bool,
    #[serde(skip_serializing_if = "is_false")]
    strike: bool,
    #[serde(skip_serializing_if = "is_false")]
    code: bool,
}

fn wire_block(block: &SlackRichTextBlock) -> SlackWireRichTextBlock<'_> {
    match block {
        SlackRichTextBlock::Section { elements } => SlackWireRichTextBlock::Section {
            elements: wire_elements(elements),
        },
        SlackRichTextBlock::List {
            style,
            indent,
            offset,
            items,
        } => SlackWireRichTextBlock::List {
            style: match style {
                SlackRichTextListStyle::Bullet => "bullet",
                SlackRichTextListStyle::Ordered => "ordered",
            },
            indent: *indent,
            offset: *offset,
            elements: items
                .iter()
                .map(|item| SlackWireRichTextSection {
                    kind: "rich_text_section",
                    elements: wire_elements(&item.elements),
                })
                .collect(),
        },
        SlackRichTextBlock::Quote { elements } => SlackWireRichTextBlock::Quote {
            elements: wire_elements(elements),
        },
        SlackRichTextBlock::Preformatted { elements } => SlackWireRichTextBlock::Preformatted {
            elements: wire_elements(elements),
        },
    }
}

fn wire_elements(elements: &[SlackRichTextInline]) -> Vec<SlackWireRichTextInline<'_>> {
    elements.iter().map(wire_inline).collect()
}

fn wire_inline(inline: &SlackRichTextInline) -> SlackWireRichTextInline<'_> {
    match inline {
        SlackRichTextInline::Text { text, style } => SlackWireRichTextInline::Text {
            text,
            style: wire_style(*style),
        },
        SlackRichTextInline::Link {
            url,
            label,
            unsafe_url: _,
            style,
        } => SlackWireRichTextInline::Link {
            url,
            text: label.as_deref(),
            style: wire_style(*style),
        },
        SlackRichTextInline::User {
            user_id,
            label: _,
            style,
        } => SlackWireRichTextInline::User {
            user_id,
            style: wire_style(*style),
        },
        SlackRichTextInline::Channel {
            channel_id,
            label: _,
            style,
        } => SlackWireRichTextInline::Channel {
            channel_id,
            style: wire_style(*style),
        },
        SlackRichTextInline::Broadcast { range, style } => SlackWireRichTextInline::Broadcast {
            range: range.label(),
            style: wire_style(*style),
        },
        SlackRichTextInline::Emoji {
            name,
            unicode,
            skin_tone,
            style,
        } => SlackWireRichTextInline::Emoji {
            name,
            unicode: unicode.as_deref().map(wire_emoji_unicode),
            skin_tone: *skin_tone,
            style: wire_style(*style),
        },
    }
}

fn wire_emoji_unicode(unicode: &str) -> String {
    unicode
        .chars()
        .map(|character| format!("{:x}", u32::from(character)))
        .collect::<Vec<_>>()
        .join("-")
}

fn wire_style(style: SlackRichTextStyle) -> Option<SlackWireRichTextStyle> {
    let style = SlackWireRichTextStyle {
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
        strike: style.strike,
        code: style.code,
    };
    (!style.is_empty()).then_some(style)
}

impl SlackWireRichTextStyle {
    fn is_empty(&self) -> bool {
        !self.bold && !self.italic && !self.underline && !self.strike && !self.code
    }
}

fn is_false(value: &bool) -> bool {
    !value
}
