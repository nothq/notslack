use super::{
    initials, slack_avatar_fill, slack_dm_relative_timestamp, Arc, OffsetDateTime,
    SlackConversationKind, SlackDmInboxItem, SlackDmInboxSnapshot, SlackDmRow,
    SlackDmRowParticipant,
};

pub(crate) fn build_slack_dm_rows(snapshot: &SlackDmInboxSnapshot) -> Arc<[SlackDmRow]> {
    let now = OffsetDateTime::now_utc();
    snapshot
        .items
        .iter()
        .map(|item| slack_dm_row(item, snapshot.self_user_id.as_str(), now))
        .collect::<Vec<_>>()
        .into()
}

fn slack_dm_row(item: &SlackDmInboxItem, self_user_id: &str, now: OffsetDateTime) -> SlackDmRow {
    let preview = slack_dm_preview(item, self_user_id);
    let finder_avatar_label = item
        .participants
        .first()
        .map(|participant| participant.label.as_str())
        .unwrap_or(item.label.as_str());
    SlackDmRow {
        conversation_id: item.conversation_id.clone().into(),
        latest_message_timestamp: item.latest_timestamp.clone(),
        kind: item.kind,
        title: item.label.clone().into(),
        finder_element_id: format!("slack-dm-finder-row-{}", item.conversation_id).into(),
        finder_accessibility_label: format!("Direct message with {}", item.label).into(),
        finder_search_key: slack_dm_finder_search_key(item).into(),
        finder_group_count_label: (item.kind == SlackConversationKind::GroupMessage)
            .then(|| item.participants.len().to_string().into()),
        finder_avatar_initials: initials(finder_avatar_label).into(),
        finder_avatar_fill: slack_avatar_fill(finder_avatar_label),
        preview: preview.into(),
        timestamp_label: slack_dm_relative_timestamp(&item.latest_timestamp, now).into(),
        participants: item
            .participants
            .iter()
            .map(|participant| SlackDmRowParticipant {
                user_id: participant.user_id.clone().into(),
                label: participant.label.clone().into(),
                avatar_image_url: participant.avatar_image_url.clone().map(Into::into),
                presence: participant.presence,
            })
            .collect::<Vec<_>>()
            .into(),
        unread: item.unread,
        mention_count: item.mention_count,
    }
}

fn slack_dm_finder_search_key(item: &SlackDmInboxItem) -> String {
    let participant_label_bytes = item
        .participants
        .iter()
        .map(|participant| participant.label.len() + 1)
        .sum::<usize>();
    let mut searchable =
        String::with_capacity(item.label.len().saturating_add(participant_label_bytes));
    searchable.push_str(&item.label);
    for participant in &item.participants {
        searchable.push(' ');
        searchable.push_str(&participant.label);
    }
    normalize_slack_dm_finder_text(&searchable)
}

pub(crate) fn normalize_slack_dm_finder_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    for token in text.split_whitespace() {
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.extend(token.chars().flat_map(char::to_lowercase));
    }
    normalized
}

fn slack_dm_preview(item: &SlackDmInboxItem, self_user_id: &str) -> String {
    let body = item.latest_message_text.trim();
    if body.is_empty() {
        return String::new();
    }
    if item.latest_sender_user_id.as_deref() == Some(self_user_id) {
        return format!("You: {body}");
    }
    if item.kind == SlackConversationKind::GroupMessage {
        if let Some(sender) = item
            .latest_sender_label
            .as_deref()
            .map(str::trim)
            .filter(|sender| !sender.is_empty())
        {
            return format!("{sender}: {body}");
        }
    }
    body.to_string()
}
