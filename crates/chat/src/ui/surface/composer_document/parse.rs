use std::sync::Arc;

use crate::model::{
    SlackMessage, SlackRichTextBlock, SlackRichTextBody, SlackRichTextInline,
    SlackRichTextListStyle, SlackRichTextStyle,
};

use super::lines::{logical_line_count, plain_line_blocks};
use super::normalize::{normalize_entity_runs, normalize_link_runs, normalize_runs};
use super::{
    checked_emoji, SlackComposerDocument, SlackComposerEntity, SlackComposerEntityRun,
    SlackComposerInlineStyle, SlackComposerLineBlock, SlackComposerLinkRun, SlackComposerLinkUrl,
    SlackComposerStyleRun,
};

type SlackRichElementContent = (Option<SlackComposerEntity>, Option<SlackComposerLinkUrl>);

impl SlackComposerDocument {
    pub(crate) fn plain_text(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            line_blocks: plain_line_blocks(&text),
            text,
            ..Self::default()
        }
    }

    pub(crate) fn from_rich_body(body: &SlackRichTextBody) -> Result<Self, String> {
        if !body.block_kit.is_empty() {
            return Err(
                "Slack Block Kit message bodies cannot be preserved by the editor".to_string(),
            );
        }
        if body.blocks.is_empty() {
            return Err("Slack rich-text message body contains no blocks".to_string());
        }
        let mut document = Self::empty_rich_document();
        for block in &body.blocks {
            if !document.text.is_empty() {
                document.text.push('\n');
            }
            document.push_rich_body_block(block)?;
        }
        document.finish_rich_body()
    }

    fn empty_rich_document() -> Self {
        Self {
            text: String::new(),
            runs: Vec::new(),
            links: Vec::new(),
            entities: Vec::new(),
            line_blocks: Vec::new(),
            cursor_style: None,
            revision: 0,
            cached_text_color: None,
            cached_code_background: None,
            cached_code_block_background: None,
            cached_link_color: None,
            cached_highlights: Arc::default(),
            highlights_dirty: true,
        }
    }

    fn push_rich_body_block(&mut self, block: &SlackRichTextBlock) -> Result<(), String> {
        match block {
            SlackRichTextBlock::Section { elements } => {
                self.push_rich_block(elements, SlackComposerLineBlock::Section)
            }
            SlackRichTextBlock::Quote { elements } => {
                self.push_rich_block(elements, SlackComposerLineBlock::Quote)
            }
            SlackRichTextBlock::Preformatted { elements } => {
                self.push_rich_block(elements, SlackComposerLineBlock::Preformatted)
            }
            SlackRichTextBlock::List { .. } => self.push_rich_list(block),
        }
    }

    fn push_rich_list(&mut self, block: &SlackRichTextBlock) -> Result<(), String> {
        let SlackRichTextBlock::List {
            style,
            indent,
            offset,
            items,
        } = block
        else {
            unreachable!("Slack composer list loader requires a list block");
        };
        if *indent != 0 || offset.is_some() {
            return Err(
                "Slack rich-text list metadata cannot be preserved by the editor".to_string(),
            );
        }
        if items.is_empty() {
            return Err("Slack rich-text list contains no items".to_string());
        }
        let line_block = match style {
            SlackRichTextListStyle::Bullet => SlackComposerLineBlock::BulletedList,
            SlackRichTextListStyle::Ordered => SlackComposerLineBlock::OrderedList,
        };
        for (item_index, item) in items.iter().enumerate() {
            if item_index > 0 {
                self.text.push('\n');
            }
            let item_start = self.text.len();
            self.push_rich_elements(&item.elements)?;
            if self.text.len() == item_start {
                return Err("Slack rich-text list contains an empty item".to_string());
            }
            if self.text[item_start..].contains('\n') {
                return Err(
                    "Slack rich-text list item contains an unsupported line break".to_string(),
                );
            }
            self.line_blocks.push(line_block);
        }
        Ok(())
    }

    fn finish_rich_body(mut self) -> Result<Self, String> {
        if self.text.trim().is_empty() {
            return Err("Slack rich-text message body is empty".to_string());
        }
        if self.line_blocks.len() != logical_line_count(&self.text) {
            return Err("Slack rich-text block boundaries cannot be preserved".to_string());
        }
        self.runs = normalize_runs(self.runs, &self.text);
        self.links = normalize_link_runs(self.links, &self.text);
        self.entities = normalize_entity_runs(self.entities, &self.text);
        self.export_message_draft()?;
        Ok(self)
    }

    pub(crate) fn from_message(message: &SlackMessage) -> Option<Self> {
        match message.rich_body.as_deref() {
            Some(body) => Self::from_rich_body(body).ok(),
            None if !message.body.trim().is_empty() => {
                let document = Self::plain_text(message.body.clone());
                document.export_message_draft().ok()?;
                Some(document)
            }
            None => None,
        }
    }

    fn push_rich_block(
        &mut self,
        elements: &[SlackRichTextInline],
        line_block: SlackComposerLineBlock,
    ) -> Result<(), String> {
        let block_start = self.text.len();
        self.push_rich_elements(elements)?;
        if self.text.len() == block_start {
            return Err("Slack rich-text block is empty".to_string());
        }
        self.line_blocks.extend(std::iter::repeat_n(
            line_block,
            logical_line_count(&self.text[block_start..]),
        ));
        Ok(())
    }

    fn push_rich_elements(&mut self, elements: &[SlackRichTextInline]) -> Result<(), String> {
        if elements.is_empty() {
            return Err("Slack rich-text block contains no elements".to_string());
        }
        for element in elements {
            self.push_rich_element(element)?;
        }
        Ok(())
    }

    fn push_rich_element(&mut self, element: &SlackRichTextInline) -> Result<(), String> {
        let start = self.text.len();
        let (entity, link) = self.push_rich_element_content(element)?;
        let end = self.text.len();
        if start == end {
            return Err("Slack rich-text contains an empty element".to_string());
        }
        if element.style() != SlackRichTextStyle::default() {
            self.runs.push(SlackComposerStyleRun {
                range: start..end,
                style: slack_composer_inline_style(element.style()),
            });
        }
        if let Some(url) = link {
            self.links.push(SlackComposerLinkRun {
                range: start..end,
                url,
            });
        }
        if let Some(entity) = entity {
            self.entities.push(SlackComposerEntityRun {
                range: start..end,
                entity,
            });
        }
        Ok(())
    }

    fn push_rich_element_content(
        &mut self,
        element: &SlackRichTextInline,
    ) -> Result<SlackRichElementContent, String> {
        match element {
            SlackRichTextInline::Text { text, .. } => {
                if text.is_empty() {
                    return Err("Slack rich-text contains an empty text element".to_string());
                }
                self.text.push_str(text);
                Ok((None, None))
            }
            SlackRichTextInline::Link {
                url,
                label,
                unsafe_url,
                ..
            } => self.push_rich_link(url, label.as_deref(), *unsafe_url),
            SlackRichTextInline::User { .. }
            | SlackRichTextInline::Channel { .. }
            | SlackRichTextInline::Broadcast { .. }
            | SlackRichTextInline::Emoji { .. } => {
                let entity = rich_entity(element)?;
                entity.push_display_text(&mut self.text);
                Ok((Some(entity), None))
            }
        }
    }

    fn push_rich_link(
        &mut self,
        raw_url: &str,
        label: Option<&str>,
        unsafe_url: bool,
    ) -> Result<SlackRichElementContent, String> {
        if unsafe_url {
            return Err("Slack rich-text contains an unsafe link".to_string());
        }
        let Some(url) = SlackComposerLinkUrl::parse(raw_url) else {
            return Err("Slack rich-text contains a link the editor cannot preserve".to_string());
        };
        let display = label
            .filter(|label| !label.is_empty())
            .unwrap_or(url.as_str());
        if display.is_empty() {
            return Err("Slack rich-text contains an empty link".to_string());
        }
        self.text.push_str(display);
        Ok((None, Some(url)))
    }
}

fn rich_entity(element: &SlackRichTextInline) -> Result<SlackComposerEntity, String> {
    match element {
        SlackRichTextInline::User { user_id, label, .. } => Ok(SlackComposerEntity::User {
            user_id: checked_entity_id(user_id, "user")?.to_string(),
            label: checked_entity_label(label, '@')?.to_string(),
        }),
        SlackRichTextInline::Channel {
            channel_id, label, ..
        } => Ok(SlackComposerEntity::Channel {
            channel_id: checked_entity_id(channel_id, "channel")?.to_string(),
            label: checked_entity_label(label, '#')?.to_string(),
        }),
        SlackRichTextInline::Broadcast { range, .. } => {
            checked_entity_label(range.label(), '@')?;
            Ok(SlackComposerEntity::Broadcast(range.clone()))
        }
        SlackRichTextInline::Emoji {
            name,
            unicode,
            skin_tone,
            ..
        } => {
            checked_emoji(name, unicode.as_deref(), *skin_tone)?;
            Ok(SlackComposerEntity::Emoji {
                name: name.to_string(),
                unicode: unicode.clone(),
                skin_tone: *skin_tone,
            })
        }
        SlackRichTextInline::Text { .. } | SlackRichTextInline::Link { .. } => {
            unreachable!("Slack composer entity loader requires an entity inline")
        }
    }
}

fn slack_composer_inline_style(style: SlackRichTextStyle) -> SlackComposerInlineStyle {
    SlackComposerInlineStyle {
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
        strikethrough: style.strike,
        code: style.code,
    }
}

fn checked_entity_id<'a>(value: &'a str, kind: &str) -> Result<&'a str, String> {
    if value.is_empty() || value.trim() != value {
        return Err(format!(
            "Slack rich-text contains a non-canonical {kind} ID"
        ));
    }
    Ok(value)
}

fn checked_entity_label(value: &str, prefix: char) -> Result<&str, String> {
    let value = value.trim_start_matches(prefix);
    if value.is_empty() || value.trim() != value || value.contains(['@', '#', '\r', '\n']) {
        return Err("Slack rich-text contains a non-canonical entity label".to_string());
    }
    Ok(value)
}
