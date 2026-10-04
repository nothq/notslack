use std::{ops::Range, sync::Arc};

use crate::model::SlackRichTextBroadcastRange;
use gpui::Hsla;
use gpui_components::text_input::TextInputHighlight;
use url::Url;

use super::SlackComposerFormatAction;

mod edit;
mod entities;
mod export;
mod highlights;
mod lines;
mod normalize;
mod parse;
mod query;
mod remap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SlackComposerInlineStyle {
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
    code: bool,
}

impl SlackComposerInlineStyle {
    fn contains(self, format: SlackComposerInlineFormat) -> bool {
        match format {
            SlackComposerInlineFormat::Bold => self.bold,
            SlackComposerInlineFormat::Italic => self.italic,
            SlackComposerInlineFormat::Underline => self.underline,
            SlackComposerInlineFormat::Strikethrough => self.strikethrough,
            SlackComposerInlineFormat::Code => self.code,
        }
    }

    fn set(&mut self, format: SlackComposerInlineFormat, enabled: bool) {
        match format {
            SlackComposerInlineFormat::Bold => self.bold = enabled,
            SlackComposerInlineFormat::Italic => self.italic = enabled,
            SlackComposerInlineFormat::Underline => self.underline = enabled,
            SlackComposerInlineFormat::Strikethrough => self.strikethrough = enabled,
            SlackComposerInlineFormat::Code => self.code = enabled,
        }
    }

    fn is_empty(self) -> bool {
        self == Self::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlackComposerInlineFormat {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Code,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum SlackComposerLineBlock {
    #[default]
    Section,
    OrderedList,
    BulletedList,
    Quote,
    Preformatted,
}

impl SlackComposerLineBlock {
    fn from_action(action: SlackComposerFormatAction) -> Option<Self> {
        match action {
            SlackComposerFormatAction::OrderedList => Some(Self::OrderedList),
            SlackComposerFormatAction::BulletedList => Some(Self::BulletedList),
            SlackComposerFormatAction::Quote => Some(Self::Quote),
            SlackComposerFormatAction::CodeBlock => Some(Self::Preformatted),
            SlackComposerFormatAction::Bold
            | SlackComposerFormatAction::Italic
            | SlackComposerFormatAction::Underline
            | SlackComposerFormatAction::Strikethrough
            | SlackComposerFormatAction::Link
            | SlackComposerFormatAction::Code => None,
        }
    }
}

impl SlackComposerInlineFormat {
    fn from_action(action: SlackComposerFormatAction) -> Option<Self> {
        match action {
            SlackComposerFormatAction::Bold => Some(Self::Bold),
            SlackComposerFormatAction::Italic => Some(Self::Italic),
            SlackComposerFormatAction::Underline => Some(Self::Underline),
            SlackComposerFormatAction::Strikethrough => Some(Self::Strikethrough),
            SlackComposerFormatAction::Code => Some(Self::Code),
            SlackComposerFormatAction::Link
            | SlackComposerFormatAction::OrderedList
            | SlackComposerFormatAction::BulletedList
            | SlackComposerFormatAction::Quote
            | SlackComposerFormatAction::CodeBlock => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SlackComposerStyleRun {
    range: Range<usize>,
    style: SlackComposerInlineStyle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackComposerLinkUrl(Arc<str>);

impl SlackComposerLinkUrl {
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        let value = raw.trim();
        let parsed = Url::parse(value).ok()?;
        let valid = match parsed.scheme() {
            "http" | "https" => parsed.host_str().is_some(),
            "mailto" => !parsed.path().is_empty(),
            _ => false,
        };
        valid.then(|| Self(value.into()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SlackComposerLinkRun {
    range: Range<usize>,
    url: SlackComposerLinkUrl,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SlackComposerEntity {
    User {
        user_id: String,
        label: String,
    },
    Channel {
        channel_id: String,
        label: String,
    },
    Broadcast(SlackRichTextBroadcastRange),
    Emoji {
        name: String,
        unicode: Option<String>,
        skin_tone: Option<u8>,
    },
}

impl SlackComposerEntity {
    fn display_len(&self) -> usize {
        match self {
            Self::User { label, .. } => 1 + label.len(),
            Self::Channel { label, .. } => 1 + label.len(),
            Self::Broadcast(range) => 1 + range.label().len(),
            Self::Emoji {
                name,
                unicode,
                skin_tone,
            } => slack_composer_emoji_display(name, unicode.as_deref(), *skin_tone).len(),
        }
    }

    fn push_display_text(&self, text: &mut String) {
        match self {
            Self::User { label, .. } => {
                text.push('@');
                text.push_str(label);
            }
            Self::Channel { label, .. } => {
                text.push('#');
                text.push_str(label);
            }
            Self::Broadcast(range) => {
                text.push('@');
                text.push_str(range.label());
            }
            Self::Emoji {
                name,
                unicode,
                skin_tone,
            } => text.push_str(&slack_composer_emoji_display(
                name,
                unicode.as_deref(),
                *skin_tone,
            )),
        }
    }

    fn matches_display_text(&self, text: &str) -> bool {
        match self {
            Self::User {
                label: expected, ..
            } => text.strip_prefix('@') == Some(expected),
            Self::Channel {
                label: expected, ..
            } => text.strip_prefix('#') == Some(expected),
            Self::Broadcast(range) => text.strip_prefix('@') == Some(range.label()),
            Self::Emoji {
                name,
                unicode,
                skin_tone,
            } => {
                text == slack_composer_emoji_display(name, unicode.as_deref(), *skin_tone).as_str()
            }
        }
    }

    fn assert_canonical(&self) {
        match self {
            Self::User { user_id, label } => {
                assert!(
                    !user_id.is_empty() && user_id.trim() == user_id.as_str(),
                    "Slack composer user mentions require a canonical user ID"
                );
                assert_canonical_entity_label(label);
            }
            Self::Channel { channel_id, label } => {
                assert!(
                    !channel_id.is_empty() && channel_id.trim() == channel_id.as_str(),
                    "Slack composer channel mentions require a canonical channel ID"
                );
                assert_canonical_entity_label(label);
            }
            Self::Broadcast(range) => assert_canonical_entity_label(range.label()),
            Self::Emoji {
                name,
                unicode,
                skin_tone,
            } => assert_canonical_emoji(name, unicode.as_deref(), *skin_tone),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SlackComposerEntityRun {
    range: Range<usize>,
    entity: SlackComposerEntity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackComposerLinkEdit {
    pub(crate) range: Range<usize>,
    pub(crate) text: String,
    pub(crate) url: Option<SlackComposerLinkUrl>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SlackComposerCursorStyle {
    offset: usize,
    style: SlackComposerInlineStyle,
}

struct ContiguousTextEdit {
    old_range: Range<usize>,
    replacement_len: usize,
}

#[derive(Clone)]
pub(crate) struct SlackComposerDocument {
    text: String,
    runs: Vec<SlackComposerStyleRun>,
    links: Vec<SlackComposerLinkRun>,
    entities: Vec<SlackComposerEntityRun>,
    line_blocks: Vec<SlackComposerLineBlock>,
    cursor_style: Option<SlackComposerCursorStyle>,
    revision: u64,
    cached_text_color: Option<Hsla>,
    cached_code_background: Option<Hsla>,
    cached_code_block_background: Option<Hsla>,
    cached_link_color: Option<Hsla>,
    cached_highlights: Arc<[TextInputHighlight]>,
    highlights_dirty: bool,
}

impl Default for SlackComposerDocument {
    fn default() -> Self {
        Self {
            text: String::new(),
            runs: Vec::new(),
            links: Vec::new(),
            entities: Vec::new(),
            line_blocks: vec![SlackComposerLineBlock::Section],
            cursor_style: None,
            revision: 0,
            cached_text_color: None,
            cached_code_background: None,
            cached_code_block_background: None,
            cached_link_color: None,
            cached_highlights: Arc::default(),
            highlights_dirty: false,
        }
    }
}

fn checked_emoji(name: &str, unicode: Option<&str>, skin_tone: Option<u8>) -> Result<(), String> {
    if name.is_empty() || name.trim() != name || name.contains([':', '\r', '\n']) {
        return Err("Slack rich-text contains a non-canonical emoji name".to_string());
    }
    if unicode.is_some_and(str::is_empty) {
        return Err("Slack rich-text contains an empty emoji Unicode value".to_string());
    }
    if skin_tone.is_some_and(|skin_tone| !(2..=6).contains(&skin_tone)) {
        return Err("Slack rich-text contains an invalid emoji skin tone".to_string());
    }
    Ok(())
}

fn assert_canonical_emoji(name: &str, unicode: Option<&str>, skin_tone: Option<u8>) {
    assert!(
        checked_emoji(name, unicode, skin_tone).is_ok(),
        "Slack composer emoji entities must be canonical"
    );
}

fn slack_composer_emoji_display(
    name: &str,
    unicode: Option<&str>,
    skin_tone: Option<u8>,
) -> String {
    if let Some(unicode) = unicode {
        let mut display = unicode.to_string();
        if let Some(modifier) = slack_composer_skin_tone_modifier(skin_tone) {
            let already_modified = unicode
                .chars()
                .any(|character| ('\u{1f3fb}'..='\u{1f3ff}').contains(&character));
            if !already_modified {
                display.push(modifier);
            }
        }
        display
    } else {
        format!(":{name}:")
    }
}

fn slack_composer_skin_tone_modifier(skin_tone: Option<u8>) -> Option<char> {
    skin_tone
        .and_then(|skin_tone| skin_tone.checked_sub(2))
        .filter(|skin_tone| *skin_tone <= 4)
        .and_then(|skin_tone| char::from_u32(0x1f3fb + u32::from(skin_tone)))
}

fn assert_canonical_entity_label(label: &str) {
    assert!(
        !label.is_empty()
            && label.trim() == label
            && !label.starts_with('@')
            && !label.contains(['\r', '\n']),
        "Slack composer entity labels must be canonical"
    );
}
