use crate::model::SlackConversationKind;

pub(crate) fn conversation_kind_for_label(label: &str) -> SlackConversationKind {
    if label.trim_start().starts_with("mpdm-") {
        SlackConversationKind::GroupMessage
    } else {
        SlackConversationKind::Channel
    }
}

pub(crate) fn normalize_conversation_label(label: &str, kind: SlackConversationKind) -> String {
    match kind {
        SlackConversationKind::GroupMessage => humanize_group_message_label(label),
        _ => label.trim().to_string(),
    }
}

fn humanize_group_message_label(label: &str) -> String {
    let trimmed = label.trim();
    let participants = trimmed
        .strip_prefix("mpdm-")
        .unwrap_or(trimmed)
        .rsplit_once('-')
        .filter(|(_, suffix)| suffix.chars().all(|character| character.is_ascii_digit()))
        .map_or(
            trimmed.strip_prefix("mpdm-").unwrap_or(trimmed),
            |(body, _)| body,
        )
        .split("--")
        .map(humanize_group_message_part)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if participants.is_empty() {
        trimmed.to_string()
    } else {
        participants.join(", ")
    }
}

fn humanize_group_message_part(part: &str) -> String {
    part.trim_matches('-')
        .split('-')
        .filter(|segment| !segment.is_empty())
        .map(titlecase_segment)
        .collect::<Vec<_>>()
        .join(" ")
}

fn titlecase_segment(segment: &str) -> String {
    let mut characters = segment.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };
    let mut titlecased = first.to_uppercase().collect::<String>();
    titlecased.push_str(characters.as_str());
    titlecased
}

#[cfg(test)]
mod tests {
    use crate::live::conversation::*;

    #[gpui::test]
    fn normalize_conversation_label_humanizes_group_messages() {
        assert_eq!(
            normalize_conversation_label(
                "mpdm-ada--grace--charlesjavelona-1",
                SlackConversationKind::GroupMessage
            ),
            "Ada, Grace, Charlesjavelona"
        );
    }

    #[gpui::test]
    fn conversation_kind_for_label_detects_group_messages() {
        assert_eq!(
            conversation_kind_for_label("mpdm-ada--grace-1"),
            SlackConversationKind::GroupMessage
        );
        assert_eq!(
            conversation_kind_for_label("design"),
            SlackConversationKind::Channel
        );
    }
}
