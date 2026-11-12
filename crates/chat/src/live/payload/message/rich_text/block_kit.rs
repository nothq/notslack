use crate::model::{
    SlackBlockKitAction, SlackBlockKitActionKind, SlackBlockKitActionStyle, SlackBlockKitBlock,
    SlackBlockKitContextElement, SlackBlockKitImage, SlackBlockKitText, SlackBlockKitTextKind,
    SlackRichTextBody,
};
use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::live::payload::util::slack_user_display_name;

#[derive(Deserialize)]
#[serde(tag = "type")]
enum SlackWireBlockKitBlock {
    #[serde(rename = "header")]
    Header { text: SlackWireBlockKitText },
    #[serde(rename = "section")]
    Section {
        #[serde(default)]
        text: Option<SlackWireBlockKitText>,
        #[serde(default)]
        fields: Vec<SlackWireBlockKitText>,
        #[serde(default)]
        accessory: Option<SlackWireBlockKitAccessory>,
    },
    #[serde(rename = "context")]
    Context {
        #[serde(default)]
        elements: Vec<SlackWireBlockKitContextElement>,
    },
    #[serde(rename = "divider")]
    Divider,
    #[serde(rename = "actions")]
    Actions {
        #[serde(default)]
        elements: Vec<SlackWireBlockKitAction>,
    },
    #[serde(other)]
    Unsupported,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum SlackWireBlockKitText {
    #[serde(rename = "plain_text")]
    Plain { text: String },
    #[serde(rename = "mrkdwn")]
    Markdown { text: String },
    #[serde(other)]
    Unsupported,
}

impl SlackWireBlockKitText {
    fn into_model(self) -> Option<SlackBlockKitText> {
        let (text, kind) = match self {
            Self::Plain { text } => (text, SlackBlockKitTextKind::Plain),
            Self::Markdown { text } => (text, SlackBlockKitTextKind::Markdown),
            Self::Unsupported => return None,
        };
        let text = text.trim().to_string();
        (!text.is_empty()).then_some(SlackBlockKitText { text, kind })
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum SlackWireBlockKitAccessory {
    #[serde(rename = "image")]
    Image {
        image_url: String,
        #[serde(default)]
        alt_text: String,
    },
    #[serde(other)]
    Unsupported,
}

impl SlackWireBlockKitAccessory {
    fn into_image(self) -> Option<SlackBlockKitImage> {
        match self {
            Self::Image {
                image_url,
                alt_text,
            } => slack_block_kit_image(image_url, alt_text),
            Self::Unsupported => None,
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum SlackWireBlockKitContextElement {
    #[serde(rename = "plain_text")]
    Plain { text: String },
    #[serde(rename = "mrkdwn")]
    Markdown { text: String },
    #[serde(rename = "image")]
    Image {
        image_url: String,
        #[serde(default)]
        alt_text: String,
    },
    #[serde(other)]
    Unsupported,
}

impl SlackWireBlockKitContextElement {
    fn into_model(self) -> Option<SlackBlockKitContextElement> {
        match self {
            Self::Plain { text } => SlackWireBlockKitText::Plain { text }
                .into_model()
                .map(|text| SlackBlockKitContextElement::Text { text }),
            Self::Markdown { text } => SlackWireBlockKitText::Markdown { text }
                .into_model()
                .map(|text| SlackBlockKitContextElement::Text { text }),
            Self::Image {
                image_url,
                alt_text,
            } => slack_block_kit_image(image_url, alt_text).map(|context_image| {
                SlackBlockKitContextElement::Image {
                    image: context_image,
                }
            }),
            Self::Unsupported => None,
        }
    }
}

#[derive(Deserialize)]
struct SlackWireBlockKitOption {
    text: SlackWireBlockKitText,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum SlackWireBlockKitAction {
    #[serde(rename = "button")]
    Button {
        text: SlackWireBlockKitText,
        #[serde(default)]
        url: Option<String>,
        #[serde(default)]
        style: Option<String>,
    },
    #[serde(
        rename = "static_select",
        alias = "external_select",
        alias = "users_select",
        alias = "conversations_select",
        alias = "channels_select",
        alias = "multi_static_select",
        alias = "multi_external_select",
        alias = "multi_users_select",
        alias = "multi_conversations_select",
        alias = "multi_channels_select"
    )]
    Select {
        #[serde(default)]
        placeholder: Option<SlackWireBlockKitText>,
        #[serde(default)]
        initial_option: Option<SlackWireBlockKitOption>,
    },
    #[serde(rename = "overflow")]
    Overflow,
    #[serde(rename = "datepicker")]
    DatePicker {
        #[serde(default)]
        placeholder: Option<SlackWireBlockKitText>,
        #[serde(default)]
        initial_date: Option<String>,
    },
    #[serde(rename = "timepicker")]
    TimePicker {
        #[serde(default)]
        placeholder: Option<SlackWireBlockKitText>,
        #[serde(default)]
        initial_time: Option<String>,
    },
    #[serde(other)]
    Unsupported,
}

impl SlackWireBlockKitAction {
    fn into_model(self) -> Option<SlackBlockKitAction> {
        match self {
            Self::Button { text, url, style } => Some(SlackBlockKitAction {
                label: text.into_model()?.text,
                kind: SlackBlockKitActionKind::Button,
                style: match style.as_deref() {
                    Some("primary") => SlackBlockKitActionStyle::Primary,
                    Some("danger") => SlackBlockKitActionStyle::Danger,
                    _ => SlackBlockKitActionStyle::Default,
                },
                url: url.filter(|url| slack_block_kit_supported_url(url)),
            }),
            Self::Select {
                placeholder,
                initial_option,
            } => Some(SlackBlockKitAction {
                label: initial_option
                    .and_then(|option| option.text.into_model())
                    .or_else(|| placeholder.and_then(SlackWireBlockKitText::into_model))?
                    .text,
                kind: SlackBlockKitActionKind::Select,
                style: SlackBlockKitActionStyle::Default,
                url: None,
            }),
            Self::Overflow => Some(SlackBlockKitAction {
                label: "More actions".to_string(),
                kind: SlackBlockKitActionKind::Overflow,
                style: SlackBlockKitActionStyle::Default,
                url: None,
            }),
            Self::DatePicker {
                placeholder,
                initial_date,
            } => slack_block_kit_picker_action(
                placeholder,
                initial_date,
                "Select a date",
                SlackBlockKitActionKind::DatePicker,
            ),
            Self::TimePicker {
                placeholder,
                initial_time,
            } => slack_block_kit_picker_action(
                placeholder,
                initial_time,
                "Select a time",
                SlackBlockKitActionKind::TimePicker,
            ),
            Self::Unsupported => None,
        }
    }
}

fn slack_block_kit_picker_action(
    placeholder: Option<SlackWireBlockKitText>,
    initial_value: Option<String>,
    fallback: &str,
    kind: SlackBlockKitActionKind,
) -> Option<SlackBlockKitAction> {
    let label = initial_value
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            placeholder
                .and_then(SlackWireBlockKitText::into_model)
                .map(|text| text.text)
        })
        .unwrap_or_else(|| fallback.to_string());
    Some(SlackBlockKitAction {
        label,
        kind,
        style: SlackBlockKitActionStyle::Default,
        url: None,
    })
}

fn slack_block_kit_image(image_url: String, alt_text: String) -> Option<SlackBlockKitImage> {
    slack_block_kit_supported_image_url(&image_url).then(|| SlackBlockKitImage {
        image_url,
        alt_text: alt_text.trim().to_string(),
    })
}

fn slack_block_kit_supported_image_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

fn slack_block_kit_supported_url(url: &str) -> bool {
    slack_block_kit_supported_image_url(url)
        || url.starts_with("mailto:")
        || url.starts_with("tel:")
}

pub(super) fn slack_block_kit_body(message: &Value, has_attachments: bool) -> String {
    let Some(blocks) = message.get("blocks").and_then(Value::as_array) else {
        return String::new();
    };
    SlackRichTextBody {
        blocks: Vec::new(),
        block_kit: slack_block_kit_blocks(blocks, has_attachments),
    }
    .plain_text()
}

pub(super) fn slack_block_kit_blocks(
    raw_blocks: &[Value],
    has_attachments: bool,
) -> Vec<SlackBlockKitBlock> {
    raw_blocks
        .iter()
        .enumerate()
        .filter_map(|(index, raw_block)| {
            let block = SlackWireBlockKitBlock::deserialize(raw_block).ok()?;
            match block {
                SlackWireBlockKitBlock::Header { text } => text
                    .into_model()
                    .map(|text| SlackBlockKitBlock::Header { text }),
                SlackWireBlockKitBlock::Section {
                    text,
                    fields,
                    accessory,
                } => {
                    let text = text.and_then(|text| {
                        let serves_next_image = has_attachments
                            && slack_block_kit_text_serves_image_link(
                                &text,
                                raw_blocks.get(index + 1),
                            );
                        (!serves_next_image).then(|| text.into_model()).flatten()
                    });
                    let fields = fields
                        .into_iter()
                        .filter_map(SlackWireBlockKitText::into_model)
                        .collect::<Vec<_>>();
                    let accessory = accessory.and_then(SlackWireBlockKitAccessory::into_image);
                    (text.is_some() || !fields.is_empty() || accessory.is_some()).then_some(
                        SlackBlockKitBlock::Section {
                            text,
                            fields,
                            accessory,
                        },
                    )
                }
                SlackWireBlockKitBlock::Context { elements } => {
                    let elements = elements
                        .into_iter()
                        .filter_map(SlackWireBlockKitContextElement::into_model)
                        .collect::<Vec<_>>();
                    (!elements.is_empty()).then_some(SlackBlockKitBlock::Context { elements })
                }
                SlackWireBlockKitBlock::Divider => Some(SlackBlockKitBlock::Divider),
                SlackWireBlockKitBlock::Actions { elements } => {
                    let elements = elements
                        .into_iter()
                        .filter_map(SlackWireBlockKitAction::into_model)
                        .collect::<Vec<_>>();
                    (!elements.is_empty()).then_some(SlackBlockKitBlock::Actions { elements })
                }
                SlackWireBlockKitBlock::Unsupported => None,
            }
        })
        .collect()
}

fn slack_block_kit_text_serves_image_link(
    text: &SlackWireBlockKitText,
    next_block: Option<&Value>,
) -> bool {
    let SlackWireBlockKitText::Markdown { text } = text else {
        return false;
    };
    let next_block_is_image = next_block.is_some_and(|block| {
        block.get("type").and_then(Value::as_str) == Some("image")
            && block
                .get("image_url")
                .and_then(Value::as_str)
                .is_some_and(|url| !url.trim().is_empty())
    });
    if !next_block_is_image {
        return false;
    }
    let Some(link) = text
        .trim()
        .strip_prefix('<')
        .and_then(|text| text.strip_suffix('>'))
        .filter(|text| !text.contains('<') && !text.contains('>'))
    else {
        return false;
    };
    let target = link
        .split_once('|')
        .map_or(link, |(target, _label)| target)
        .trim();
    target.starts_with("https://") || target.starts_with("http://")
}

/// Names the people mentioned in mrkdwn text: `<@U123>` becomes
/// `<@U123|Their Name>`, the form that carries its own label. A mention of
/// someone not in `users` is left as it came.
pub(super) fn label_slack_block_kit_mentions(
    blocks: &mut [SlackBlockKitBlock],
    users: &HashMap<String, Value>,
) {
    let label = |text: &mut SlackBlockKitText| {
        if text.kind == SlackBlockKitTextKind::Markdown {
            text.text = label_slack_mrkdwn_mentions(&text.text, users);
        }
    };
    for block in blocks {
        match block {
            SlackBlockKitBlock::Header { text } => label(text),
            SlackBlockKitBlock::Section { text, fields, .. } => {
                text.iter_mut().chain(fields.iter_mut()).for_each(label);
            }
            SlackBlockKitBlock::Context { elements } => {
                for element in elements {
                    if let SlackBlockKitContextElement::Text { text } = element {
                        label(text);
                    }
                }
            }
            SlackBlockKitBlock::Divider | SlackBlockKitBlock::Actions { .. } => {}
        }
    }
}

fn label_slack_mrkdwn_mentions(text: &str, users: &HashMap<String, Value>) -> String {
    let mut labeled = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<@") {
        let (before, mention) = rest.split_at(start);
        labeled.push_str(before);
        let Some(end) = mention.find('>') else {
            rest = mention;
            break;
        };
        let user_id = &mention[2..end];
        let name = users
            .get(user_id)
            .and_then(slack_user_display_name)
            .map(|name| name.replace(['<', '>', '|'], ""))
            .filter(|name| !name.trim().is_empty());
        match name {
            Some(name) => labeled.push_str(&format!("<@{user_id}|{name}>")),
            None => labeled.push_str(&mention[..=end]),
        }
        rest = &mention[end + 1..];
    }
    labeled.push_str(rest);
    labeled
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn users() -> HashMap<String, Value> {
        HashMap::from([(
            "U1".to_string(),
            json!({"id": "U1", "profile": {"real_name": "Grace Hopper"}}),
        )])
    }

    #[test]
    fn a_bare_mention_gets_the_persons_name() {
        assert_eq!(
            label_slack_mrkdwn_mentions("by <@U1>, see <https://example.com|run>", &users()),
            "by <@U1|Grace Hopper>, see <https://example.com|run>"
        );
    }

    #[test]
    fn labeled_and_unknown_mentions_are_left_alone() {
        let text = "<@U1|grace> and <@U404> and an open <@U1";
        assert_eq!(label_slack_mrkdwn_mentions(text, &users()), text);
    }

    #[test]
    fn only_mrkdwn_text_is_labeled() {
        let text = |kind| SlackBlockKitText {
            text: "<@U1>".to_string(),
            kind,
        };
        let mut blocks = vec![
            SlackBlockKitBlock::Header {
                text: text(SlackBlockKitTextKind::Plain),
            },
            SlackBlockKitBlock::Section {
                text: Some(text(SlackBlockKitTextKind::Markdown)),
                fields: vec![text(SlackBlockKitTextKind::Markdown)],
                accessory: None,
            },
        ];
        label_slack_block_kit_mentions(&mut blocks, &users());
        assert_eq!(
            blocks,
            vec![
                SlackBlockKitBlock::Header {
                    text: text(SlackBlockKitTextKind::Plain),
                },
                SlackBlockKitBlock::Section {
                    text: Some(SlackBlockKitText {
                        text: "<@U1|Grace Hopper>".to_string(),
                        kind: SlackBlockKitTextKind::Markdown,
                    }),
                    fields: vec![SlackBlockKitText {
                        text: "<@U1|Grace Hopper>".to_string(),
                        kind: SlackBlockKitTextKind::Markdown,
                    }],
                    accessory: None,
                },
            ]
        );
    }
}
