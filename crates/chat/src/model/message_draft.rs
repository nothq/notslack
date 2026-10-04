use crate::model::{
    SlackRichTextBlock, SlackRichTextBody, SlackRichTextBroadcastRange, SlackRichTextInline,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackMessageDraft {
    fallback_text: String,
    rich_text: Option<SlackRichTextBody>,
}

impl SlackMessageDraft {
    pub fn new(
        fallback_text: impl Into<String>,
        rich_text: Option<SlackRichTextBody>,
    ) -> Result<Self, String> {
        let fallback_text = fallback_text.into().trim().to_string();
        if fallback_text.is_empty() {
            return Err("Slack message fallback text must not be empty".to_string());
        }
        if let Some(rich_text) = rich_text.as_ref() {
            validate_rich_text_body(rich_text)?;
        }
        Ok(Self {
            fallback_text,
            rich_text,
        })
    }

    pub fn plain_text(text: impl Into<String>) -> Result<Self, String> {
        Self::new(text, None)
    }

    pub fn fallback_text(&self) -> &str {
        &self.fallback_text
    }

    pub fn rich_text(&self) -> Option<&SlackRichTextBody> {
        self.rich_text.as_ref()
    }
}

fn validate_rich_text_body(body: &SlackRichTextBody) -> Result<(), String> {
    if !body.block_kit.is_empty() {
        return Err(
            "Slack Block Kit content cannot be sent as rich-text message content".to_string(),
        );
    }
    if body.blocks.is_empty() {
        return Err("Slack rich-text message body must contain at least one block".to_string());
    }
    for block in &body.blocks {
        match block {
            SlackRichTextBlock::Section { elements }
            | SlackRichTextBlock::Quote { elements }
            | SlackRichTextBlock::Preformatted { elements } => {
                validate_rich_text_elements(elements)?;
            }
            SlackRichTextBlock::List { items, .. } => {
                if items.is_empty() {
                    return Err("Slack rich-text list must contain at least one item".to_string());
                }
                for item in items {
                    validate_rich_text_elements(&item.elements)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_rich_text_elements(elements: &[SlackRichTextInline]) -> Result<(), String> {
    if elements.is_empty() {
        return Err("Slack rich-text block must contain at least one element".to_string());
    }
    for element in elements {
        match element {
            SlackRichTextInline::Text { text, .. } if text.is_empty() => {
                return Err("Slack rich-text text element must not be empty".to_string());
            }
            SlackRichTextInline::Link { url, .. } if url.trim().is_empty() => {
                return Err("Slack rich-text link URL must not be empty".to_string());
            }
            SlackRichTextInline::User { user_id, .. } if user_id.trim().is_empty() => {
                return Err("Slack rich-text user ID must not be empty".to_string());
            }
            SlackRichTextInline::Channel { channel_id, .. } if channel_id.trim().is_empty() => {
                return Err("Slack rich-text channel ID must not be empty".to_string());
            }
            SlackRichTextInline::Broadcast {
                range: SlackRichTextBroadcastRange::Other(range),
                ..
            } if range.trim().is_empty() => {
                return Err("Slack rich-text broadcast range must not be empty".to_string());
            }
            SlackRichTextInline::Emoji {
                name,
                unicode,
                skin_tone,
                ..
            } => {
                if name.trim().is_empty() {
                    return Err("Slack rich-text emoji name must not be empty".to_string());
                }
                if unicode.as_ref().is_some_and(String::is_empty) {
                    return Err("Slack rich-text emoji Unicode value must not be empty".to_string());
                }
                if skin_tone.is_some_and(|skin_tone| !(2..=6).contains(&skin_tone)) {
                    return Err(
                        "Slack rich-text emoji skin tone must be between 2 and 6".to_string()
                    );
                }
            }
            SlackRichTextInline::Text { .. }
            | SlackRichTextInline::Link { .. }
            | SlackRichTextInline::User { .. }
            | SlackRichTextInline::Channel { .. }
            | SlackRichTextInline::Broadcast { .. } => {}
        }
    }
    Ok(())
}
