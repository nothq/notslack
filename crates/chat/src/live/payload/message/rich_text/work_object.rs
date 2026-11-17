//! Slack's Work Object cards: apps such as Jira Cloud attach a layout for
//! the record they unfurl, which Slack draws as a card in place of the
//! attachment's blocks.

use serde_json::Value;

use crate::model::{
    SlackBlockKitBlock, SlackWorkObject, SlackWorkObjectField, SlackWorkObjectValue,
};

use super::block_kit::{slack_block_kit_action, slack_block_kit_web_url};

pub(super) fn slack_work_object_block(
    attachment: &Value,
    as_of: Option<&str>,
) -> Option<SlackBlockKitBlock> {
    let entity = attachment.get("work_object_entity")?;
    let layout = entity.pointer("/layouts/expanded")?;
    let title = text_at(layout, "/title/text")?;
    let app_name = text_at(layout, "/header_title/text")
        .or_else(|| text_at(entity, "/app_name"))
        .or_else(|| text_at(entity, "/product_name"))
        .unwrap_or_default();
    let card = SlackWorkObject {
        app_name,
        app_icon_url: url_at(entity, "/app_icons/image_36/image_url"),
        title,
        url: url_at(entity, "/external_url").or_else(|| url_at(attachment, "/from_url")),
        subtitle: text_at(layout, "/subtitle/text"),
        icon_url: url_at(layout, "/product_icon/image_url"),
        fields: layout
            .pointer("/fields/elements")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(slack_work_object_field)
            .collect(),
        actions: layout
            .pointer("/actions/primary_actions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(slack_block_kit_action)
            .collect(),
        as_of: as_of.filter(|as_of| !as_of.is_empty()).map(str::to_string),
    };
    Some(SlackBlockKitBlock::WorkObject {
        card: Box::new(card),
    })
}

fn slack_work_object_field(field: &Value) -> Option<SlackWorkObjectField> {
    let label = text_at(field, "/label")?;
    let values: Vec<SlackWorkObjectValue> = match field.get("tag").and_then(Value::as_array) {
        Some(tags) => tags
            .iter()
            .filter_map(|tag| {
                Some(SlackWorkObjectValue {
                    text: text_at(tag, "/text")?,
                    icon_url: url_at(tag, "/icon_url"),
                    color: text_at(tag, "/color"),
                })
            })
            .collect(),
        // Other field types carry their value as text.
        None => text_at(field, "/text")
            .or_else(|| text_at(field, "/text/text"))
            .or_else(|| text_at(field, "/value"))
            .map(|text| SlackWorkObjectValue {
                text,
                ..SlackWorkObjectValue::default()
            })
            .into_iter()
            .collect(),
    };
    (!values.is_empty()).then_some(SlackWorkObjectField { label, values })
}

fn text_at(value: &Value, pointer: &str) -> Option<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn url_at(value: &Value, pointer: &str) -> Option<String> {
    text_at(value, pointer).filter(|url| slack_block_kit_web_url(url))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::slack_work_object_block;
    use crate::model::{SlackBlockKitActionStyle, SlackBlockKitBlock};

    #[test]
    fn jira_issues_become_work_object_cards() {
        let attachment = json!({
            "blocks": [{"type": "section", "text": {"type": "mrkdwn", "text": "old"}}],
            "work_object_entity": {
                "app_name": "Jira Cloud",
                "external_url": "https://example.atlassian.net/browse/GE-1",
                "app_icons": {"image_36": {"image_url": "https://example.com/jira.png"}},
                "layouts": {"expanded": {
                    "title": {"text": "Fix the form"},
                    "subtitle": {"text": "Task GE-1 in Jira Cloud"},
                    "header_title": {"text": "Jira Cloud"},
                    "product_icon": {"image_url": "https://example.com/task.png"},
                    "fields": {"elements": [
                        {"label": "Status", "field_type": "tag",
                         "tag": [{"text": "In Progress", "color": "lagoon"}]},
                        {"label": "Assignee", "field_type": "tag",
                         "tag": [{"text": "Dalbir", "icon_url": "https://example.com/a.png"}]},
                        {"label": "Empty", "field_type": "tag", "tag": []},
                    ]},
                    "actions": {"primary_actions": [
                        {"type": "button", "text": {"type": "plain_text", "text": "Open in Jira"},
                         "style": "primary", "url": "https://example.atlassian.net/browse/GE-1"},
                    ]},
                }},
            },
        });
        let Some(SlackBlockKitBlock::WorkObject { card }) =
            slack_work_object_block(&attachment, Some("11:34 PM"))
        else {
            panic!("expected a work object card");
        };
        assert_eq!(card.app_name, "Jira Cloud");
        assert_eq!(card.title, "Fix the form");
        assert_eq!(card.subtitle.as_deref(), Some("Task GE-1 in Jira Cloud"));
        assert_eq!(
            card.url.as_deref(),
            Some("https://example.atlassian.net/browse/GE-1")
        );
        assert_eq!(card.fields.len(), 2);
        assert_eq!(card.fields[0].values[0].color.as_deref(), Some("lagoon"));
        assert_eq!(card.actions[0].style, SlackBlockKitActionStyle::Primary);
        assert_eq!(card.as_of.as_deref(), Some("11:34 PM"));
    }

    #[test]
    fn attachments_without_a_layout_keep_their_blocks() {
        assert!(slack_work_object_block(&json!({"blocks": []}), None).is_none());
    }
}
