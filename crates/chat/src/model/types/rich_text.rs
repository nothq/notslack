use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackRichTextBody {
    #[serde(default)]
    pub blocks: Vec<SlackRichTextBlock>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub block_kit: Vec<SlackBlockKitBlock>,
}

impl SlackRichTextBody {
    pub fn plain_text(&self) -> String {
        let mut text = String::new();
        push_slack_rich_text_blocks(&mut text, &self.blocks);
        push_slack_block_kit_blocks(&mut text, &self.block_kit);
        text
    }
}

fn push_slack_rich_text_blocks(text: &mut String, blocks: &[SlackRichTextBlock]) {
    for block in blocks {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        match block {
            SlackRichTextBlock::Section { elements }
            | SlackRichTextBlock::Quote { elements }
            | SlackRichTextBlock::Preformatted { elements } => {
                push_slack_rich_text_elements(text, elements);
            }
            SlackRichTextBlock::List {
                style,
                indent,
                offset,
                items,
            } => push_slack_rich_text_list(text, *style, *indent, *offset, items),
        }
    }
}

fn push_slack_rich_text_list(
    text: &mut String,
    style: SlackRichTextListStyle,
    indent: u8,
    offset: Option<u32>,
    items: &[SlackRichTextSection],
) {
    for (item_index, item) in items.iter().enumerate() {
        if item_index > 0 {
            text.push('\n');
        }
        for _ in 0..usize::from(indent) {
            text.push_str("    ");
        }
        match style {
            SlackRichTextListStyle::Bullet => text.push_str("• "),
            SlackRichTextListStyle::Ordered => {
                let number = offset
                    .unwrap_or(0)
                    .saturating_add(u32::try_from(item_index).unwrap_or(u32::MAX))
                    .saturating_add(1);
                text.push_str(&number.to_string());
                text.push_str(". ");
            }
        }
        push_slack_rich_text_elements(text, &item.elements);
    }
}

fn push_slack_block_kit_blocks(text: &mut String, blocks: &[SlackBlockKitBlock]) {
    for block in blocks {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        push_slack_block_kit_block(text, block);
        if let SlackBlockKitBlock::Section { fields, .. } = block {
            for field in fields {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                text.push_str(&field.text);
            }
        }
    }
}

fn push_slack_block_kit_block(text: &mut String, block: &SlackBlockKitBlock) {
    match block {
        SlackBlockKitBlock::Header { text: header }
        | SlackBlockKitBlock::Section {
            text: Some(header),
            fields: _,
            accessory: _,
        } => text.push_str(&header.text),
        SlackBlockKitBlock::Section { text: None, .. } | SlackBlockKitBlock::Divider => {}
        SlackBlockKitBlock::Context { elements } => push_slack_block_kit_context(text, elements),
        SlackBlockKitBlock::Actions { elements } => {
            for (index, element) in elements.iter().enumerate() {
                if index > 0 {
                    text.push(' ');
                }
                text.push_str(&element.label);
            }
        }
    }
}

fn push_slack_block_kit_context(text: &mut String, elements: &[SlackBlockKitContextElement]) {
    for element in elements {
        if let SlackBlockKitContextElement::Text { text: context } = element {
            if !text.is_empty() && !text.ends_with('\n') && !text.ends_with(' ') {
                text.push(' ');
            }
            text.push_str(&context.text);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlackBlockKitBlock {
    Header {
        text: SlackBlockKitText,
    },
    Section {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<SlackBlockKitText>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fields: Vec<SlackBlockKitText>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        accessory: Option<SlackBlockKitImage>,
    },
    Context {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        elements: Vec<SlackBlockKitContextElement>,
    },
    Divider,
    Actions {
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        elements: Vec<SlackBlockKitAction>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackBlockKitText {
    pub text: String,
    pub kind: SlackBlockKitTextKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackBlockKitTextKind {
    Plain,
    Markdown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlackBlockKitContextElement {
    Text { text: SlackBlockKitText },
    Image { image: SlackBlockKitImage },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackBlockKitImage {
    pub image_url: String,
    pub alt_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackBlockKitAction {
    pub label: String,
    pub kind: SlackBlockKitActionKind,
    pub style: SlackBlockKitActionStyle,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackBlockKitActionKind {
    Button,
    Select,
    Overflow,
    DatePicker,
    TimePicker,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackBlockKitActionStyle {
    #[default]
    Default,
    Primary,
    Danger,
}

fn push_slack_rich_text_elements(output: &mut String, elements: &[SlackRichTextInline]) {
    for element in elements {
        match element {
            SlackRichTextInline::Text { text, .. } => output.push_str(text),
            SlackRichTextInline::Link { url, label, .. } => {
                output.push_str(
                    label
                        .as_deref()
                        .filter(|label| !label.is_empty())
                        .unwrap_or(url),
                );
            }
            SlackRichTextInline::User { label, .. } => {
                output.push('@');
                output.push_str(label.trim_start_matches('@'));
            }
            SlackRichTextInline::Channel { label, .. } => {
                output.push('#');
                output.push_str(label.trim_start_matches('#'));
            }
            SlackRichTextInline::Broadcast { range, .. } => {
                output.push('@');
                output.push_str(range.label());
            }
            SlackRichTextInline::Emoji {
                name,
                unicode,
                skin_tone,
                ..
            } => {
                if let Some(unicode) = unicode {
                    output.push_str(unicode);
                    if let Some(modifier) = slack_rich_text_skin_tone_modifier(*skin_tone) {
                        let already_modified = unicode
                            .chars()
                            .any(|character| ('\u{1f3fb}'..='\u{1f3ff}').contains(&character));
                        if !already_modified {
                            output.push(modifier);
                        }
                    }
                } else {
                    output.push(':');
                    output.push_str(name);
                    output.push(':');
                }
            }
        }
    }
}

fn slack_rich_text_skin_tone_modifier(skin_tone: Option<u8>) -> Option<char> {
    skin_tone
        .and_then(|skin_tone| skin_tone.checked_sub(2))
        .filter(|skin_tone| *skin_tone <= 4)
        .and_then(|skin_tone| char::from_u32(0x1f3fb + u32::from(skin_tone)))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlackRichTextBlock {
    Section {
        #[serde(default)]
        elements: Vec<SlackRichTextInline>,
    },
    List {
        style: SlackRichTextListStyle,
        #[serde(default)]
        indent: u8,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<u32>,
        #[serde(default)]
        items: Vec<SlackRichTextSection>,
    },
    Quote {
        #[serde(default)]
        elements: Vec<SlackRichTextInline>,
    },
    Preformatted {
        #[serde(default)]
        elements: Vec<SlackRichTextInline>,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackRichTextSection {
    #[serde(default)]
    pub elements: Vec<SlackRichTextInline>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackRichTextListStyle {
    Bullet,
    Ordered,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlackRichTextInline {
    Text {
        text: String,
        #[serde(default)]
        style: SlackRichTextStyle,
    },
    Link {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        label: Option<String>,
        #[serde(default, rename = "unsafe", skip_serializing_if = "is_false")]
        unsafe_url: bool,
        #[serde(default)]
        style: SlackRichTextStyle,
    },
    User {
        user_id: String,
        label: String,
        #[serde(default)]
        style: SlackRichTextStyle,
    },
    Channel {
        channel_id: String,
        label: String,
        #[serde(default)]
        style: SlackRichTextStyle,
    },
    Broadcast {
        range: SlackRichTextBroadcastRange,
        #[serde(default)]
        style: SlackRichTextStyle,
    },
    Emoji {
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unicode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        skin_tone: Option<u8>,
        #[serde(default)]
        style: SlackRichTextStyle,
    },
}

impl SlackRichTextInline {
    pub fn style(&self) -> SlackRichTextStyle {
        match self {
            Self::Text { style, .. }
            | Self::Link { style, .. }
            | Self::User { style, .. }
            | Self::Channel { style, .. }
            | Self::Broadcast { style, .. }
            | Self::Emoji { style, .. } => *style,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackRichTextStyle {
    #[serde(default, skip_serializing_if = "is_false")]
    pub bold: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub italic: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub underline: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub strike: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub code: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlackRichTextBroadcastRange {
    Here,
    Channel,
    Everyone,
    Other(String),
}

impl SlackRichTextBroadcastRange {
    pub fn label(&self) -> &str {
        match self {
            Self::Here => "here",
            Self::Channel => "channel",
            Self::Everyone => "everyone",
            Self::Other(value) => value,
        }
    }
}

fn is_false(value: &bool) -> bool {
    !value
}
