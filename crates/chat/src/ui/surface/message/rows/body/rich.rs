use crate::model::{
    SlackBlockKitActionKind, SlackBlockKitActionStyle, SlackBlockKitBlock,
    SlackBlockKitContextElement, SlackBlockKitText, SlackBlockKitTextKind, SlackRichTextBlock,
    SlackRichTextBody, SlackRichTextInline, SlackRichTextListStyle, SlackRichTextStyle,
};

use crate::ui::surface::{
    SlackMessageBody, SlackMessageBodyAction, SlackMessageBodyActionKind,
    SlackMessageBodyActionStyle, SlackMessageBodyBlockKind, SlackMessageBodyImage,
    SlackMessageBodyTarget, SlackMessageTextStyle,
};

use super::super::reactions::slack_reaction_display;
use super::builder::{
    slack_decode_text_entities, slack_supported_link_url, SlackMessageBodyBlockBuilder,
    SlackMessageBodyBuilder,
};
use super::legacy::push_slack_legacy_message_text;

pub(super) fn prepare_slack_rich_message_body(
    body: &SlackRichTextBody,
    id_stem: &str,
) -> SlackMessageBody {
    let mut prepared = SlackMessageBodyBuilder::new(id_stem);
    for block in &body.blocks {
        push_slack_rich_text_block(&mut prepared, block);
    }
    for block in &body.block_kit {
        push_slack_block_kit_block(&mut prepared, block);
    }
    prepared.finish()
}

fn push_slack_rich_text_block(
    prepared: &mut SlackMessageBodyBuilder<'_>,
    block: &SlackRichTextBlock,
) {
    match block {
        SlackRichTextBlock::Section { elements } => {
            let mut paragraph =
                SlackMessageBodyBlockBuilder::new(SlackMessageBodyBlockKind::Paragraph);
            push_slack_rich_text_elements(&mut paragraph, elements, false);
            prepared.push_block(paragraph);
        }
        SlackRichTextBlock::List {
            style,
            indent,
            offset,
            items,
        } => {
            for (item_index, item) in items.iter().enumerate() {
                let mut list_item =
                    SlackMessageBodyBlockBuilder::new(SlackMessageBodyBlockKind::ListItem {
                        indent: *indent,
                    });
                match style {
                    SlackRichTextListStyle::Bullet => {
                        list_item.push_text("• ", SlackMessageTextStyle::default());
                    }
                    SlackRichTextListStyle::Ordered => {
                        let number = offset
                            .unwrap_or(0)
                            .saturating_add(u32::try_from(item_index).unwrap_or(u32::MAX))
                            .saturating_add(1);
                        list_item
                            .push_text(&format!("{number}. "), SlackMessageTextStyle::default());
                    }
                }
                push_slack_rich_text_elements(&mut list_item, &item.elements, false);
                prepared.push_block(list_item);
            }
        }
        SlackRichTextBlock::Quote { elements } => {
            let mut quote = SlackMessageBodyBlockBuilder::new(SlackMessageBodyBlockKind::Quote);
            push_slack_rich_text_elements(&mut quote, elements, false);
            prepared.push_block(quote);
        }
        SlackRichTextBlock::Preformatted { elements } => {
            let mut preformatted =
                SlackMessageBodyBlockBuilder::new(SlackMessageBodyBlockKind::Preformatted);
            push_slack_rich_text_elements(&mut preformatted, elements, true);
            prepared.push_block(preformatted);
        }
    }
}

fn push_slack_block_kit_block(
    prepared: &mut SlackMessageBodyBuilder<'_>,
    block: &SlackBlockKitBlock,
) {
    match block {
        SlackBlockKitBlock::Header { text } => push_slack_block_kit_header(prepared, text),
        SlackBlockKitBlock::Section {
            text,
            fields,
            accessory,
        } => push_slack_block_kit_section(prepared, text.as_ref(), fields, accessory.as_ref()),
        SlackBlockKitBlock::Context { elements } => {
            push_slack_block_kit_context(prepared, elements)
        }
        SlackBlockKitBlock::Divider => prepared.push_block(SlackMessageBodyBlockBuilder::new(
            SlackMessageBodyBlockKind::BlockKitDivider,
        )),
        SlackBlockKitBlock::Actions { elements } => {
            push_slack_block_kit_actions(prepared, elements)
        }
    }
}

fn push_slack_block_kit_header(
    prepared: &mut SlackMessageBodyBuilder<'_>,
    text: &SlackBlockKitText,
) {
    let mut header = SlackMessageBodyBlockBuilder::new(SlackMessageBodyBlockKind::BlockKitHeader);
    header.push_text(
        &slack_decode_text_entities(&text.text),
        SlackMessageTextStyle {
            bold: true,
            ..SlackMessageTextStyle::default()
        },
    );
    prepared.push_block(header);
}

fn push_slack_block_kit_section(
    prepared: &mut SlackMessageBodyBuilder<'_>,
    text: Option<&SlackBlockKitText>,
    fields: &[SlackBlockKitText],
    accessory: Option<&crate::model::SlackBlockKitImage>,
) {
    let mut section = SlackMessageBodyBlockBuilder::new(SlackMessageBodyBlockKind::BlockKitSection);
    if let Some(text) = text {
        push_slack_block_kit_text(&mut section, text);
    }
    for field in fields {
        let mut prepared_field =
            SlackMessageBodyBlockBuilder::new(SlackMessageBodyBlockKind::Paragraph);
        push_slack_block_kit_text(&mut prepared_field, field);
        section.push_field(prepared_field);
    }
    if let Some(accessory) = accessory {
        section.set_accessory_image(SlackMessageBodyImage {
            image_url: accessory.image_url.clone().into(),
            alt_text: accessory.alt_text.clone().into(),
        });
    }
    prepared.push_block(section);
}

fn push_slack_block_kit_context(
    prepared: &mut SlackMessageBodyBuilder<'_>,
    elements: &[SlackBlockKitContextElement],
) {
    let mut context = SlackMessageBodyBlockBuilder::new(SlackMessageBodyBlockKind::BlockKitContext);
    for element in elements {
        match element {
            SlackBlockKitContextElement::Text { text } => {
                let mut prepared_text =
                    SlackMessageBodyBlockBuilder::new(SlackMessageBodyBlockKind::Paragraph);
                push_slack_block_kit_text(&mut prepared_text, text);
                context.push_context_text(prepared_text);
            }
            SlackBlockKitContextElement::Image { image } => {
                context.push_context_image(SlackMessageBodyImage {
                    image_url: image.image_url.clone().into(),
                    alt_text: image.alt_text.clone().into(),
                });
            }
        }
    }
    prepared.push_block(context);
}

fn push_slack_block_kit_actions(
    prepared: &mut SlackMessageBodyBuilder<'_>,
    elements: &[crate::model::SlackBlockKitAction],
) {
    let mut actions = SlackMessageBodyBlockBuilder::new(SlackMessageBodyBlockKind::BlockKitActions);
    for action in elements {
        actions.push_action(SlackMessageBodyAction {
            label: action.label.clone().into(),
            kind: match action.kind {
                SlackBlockKitActionKind::Button => SlackMessageBodyActionKind::Button,
                SlackBlockKitActionKind::Select => SlackMessageBodyActionKind::Select,
                SlackBlockKitActionKind::Overflow => SlackMessageBodyActionKind::Overflow,
                SlackBlockKitActionKind::DatePicker => SlackMessageBodyActionKind::DatePicker,
                SlackBlockKitActionKind::TimePicker => SlackMessageBodyActionKind::TimePicker,
            },
            style: match action.style {
                SlackBlockKitActionStyle::Default => SlackMessageBodyActionStyle::Default,
                SlackBlockKitActionStyle::Primary => SlackMessageBodyActionStyle::Primary,
                SlackBlockKitActionStyle::Danger => SlackMessageBodyActionStyle::Danger,
            },
            url: action.url.clone().map(Into::into),
        });
    }
    prepared.push_block(actions);
}

fn push_slack_block_kit_text(
    prepared: &mut SlackMessageBodyBlockBuilder,
    text: &SlackBlockKitText,
) {
    match text.kind {
        SlackBlockKitTextKind::Plain => prepared.push_decoded_text(&text.text),
        SlackBlockKitTextKind::Markdown => push_slack_legacy_message_text(prepared, &text.text),
    }
}

fn push_slack_rich_text_elements(
    prepared: &mut SlackMessageBodyBlockBuilder,
    elements: &[SlackRichTextInline],
    force_code: bool,
) {
    for element in elements {
        push_slack_rich_text_element(prepared, element, force_code);
    }
}

fn push_slack_rich_text_element(
    prepared: &mut SlackMessageBodyBlockBuilder,
    element: &SlackRichTextInline,
    force_code: bool,
) {
    let mut style = slack_message_text_style(element.style());
    style.code |= force_code;
    match element {
        SlackRichTextInline::Text { text, .. } => prepared.push_text(text, style),
        SlackRichTextInline::Link {
            url,
            label,
            unsafe_url,
            ..
        } => {
            style.link = true;
            let display = label
                .as_deref()
                .filter(|label| !label.is_empty())
                .unwrap_or(url);
            let target = (!unsafe_url && slack_supported_link_url(url))
                .then(|| SlackMessageBodyTarget::Url(url.clone().into()));
            prepared.push_target(display, style, target);
        }
        SlackRichTextInline::User { user_id, label, .. } => {
            style.mention = true;
            prepared.push_target(
                &format!("@{}", label.trim_start_matches('@')),
                style,
                Some(SlackMessageBodyTarget::User(user_id.clone().into())),
            );
        }
        SlackRichTextInline::Channel {
            channel_id, label, ..
        } => {
            style.mention = true;
            prepared.push_target(
                &format!("#{}", label.trim_start_matches('#')),
                style,
                Some(SlackMessageBodyTarget::Channel(channel_id.clone().into())),
            );
        }
        SlackRichTextInline::Broadcast { range, .. } => {
            style.mention = true;
            prepared.push_text(&format!("@{}", range.label()), style);
        }
        SlackRichTextInline::Emoji {
            name,
            unicode,
            skin_tone,
            ..
        } => prepared.push_text(
            &slack_rich_text_emoji_display(name, unicode.as_deref(), *skin_tone),
            style,
        ),
    }
}

fn slack_rich_text_emoji_display(
    name: &str,
    unicode: Option<&str>,
    skin_tone: Option<u8>,
) -> String {
    unicode
        .map(|unicode| slack_rich_text_unicode_with_skin_tone(unicode, skin_tone))
        .unwrap_or_else(|| {
            let shortcode = skin_tone.map_or_else(
                || name.to_string(),
                |skin_tone| format!("{name}::skin-tone-{skin_tone}"),
            );
            slack_reaction_display(&shortcode)
        })
}

fn slack_rich_text_unicode_with_skin_tone(unicode: &str, skin_tone: Option<u8>) -> String {
    let mut display = unicode.to_string();
    let modifier = skin_tone
        .and_then(|skin_tone| skin_tone.checked_sub(2))
        .filter(|skin_tone| *skin_tone <= 4)
        .and_then(|skin_tone| char::from_u32(0x1f3fb + u32::from(skin_tone)));
    if let Some(modifier) = modifier {
        let already_modified = display
            .chars()
            .any(|character| ('\u{1f3fb}'..='\u{1f3ff}').contains(&character));
        if !already_modified {
            display.push(modifier);
        }
    }
    display
}

fn slack_message_text_style(style: SlackRichTextStyle) -> SlackMessageTextStyle {
    SlackMessageTextStyle {
        bold: style.bold,
        italic: style.italic,
        underline: style.underline,
        strike: style.strike,
        code: style.code,
        link: false,
        mention: false,
    }
}
