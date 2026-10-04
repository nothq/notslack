use std::collections::HashMap;

use gpui::SharedString;

use crate::ui::surface::PreparedSlackQuickSearchSnapshot;

use super::{
    initials, slack_avatar_fill, slack_quick_search_timestamp, SlackConversationKind,
    SlackMessageActionTarget, SlackQuickSearchConversation, SlackQuickSearchMessage,
    SlackQuickSearchMessageRow, SlackQuickSearchPerson, SlackQuickSearchRow,
    SlackQuickSearchRowKind, SlackQuickSearchSnapshot, SlackQuickSearchTarget, SlackWorkspace,
};

pub(crate) fn prepare_slack_quick_search_snapshot(
    snapshot: SlackQuickSearchSnapshot,
    workspace: Option<&SlackWorkspace>,
) -> PreparedSlackQuickSearchSnapshot {
    let people_by_id = snapshot
        .people
        .iter()
        .map(|person| (person.id.as_str(), person))
        .collect::<HashMap<_, _>>();
    let sidebar_by_conversation = workspace
        .into_iter()
        .flat_map(|workspace| workspace.sections.iter())
        .flat_map(|section| section.items.iter())
        .filter(|item| !item.target_id.is_empty())
        .map(|item| (item.target_id.as_str(), item))
        .collect::<HashMap<_, _>>();

    let mut rows = Vec::with_capacity(
        snapshot.channels.len() + snapshot.people.len() + snapshot.direct_messages.len(),
    );
    rows.extend(snapshot.channels.iter().map(|channel| {
        prepare_conversation_row(
            channel,
            None,
            sidebar_by_conversation.get(channel.id.as_str()).copied(),
        )
    }));
    rows.extend(snapshot.people.iter().map(prepare_person_row));
    rows.extend(snapshot.direct_messages.iter().map(|conversation| {
        let person = conversation
            .user_id
            .as_deref()
            .and_then(|user_id| people_by_id.get(user_id).copied());
        prepare_conversation_row(
            conversation,
            person,
            sidebar_by_conversation
                .get(conversation.id.as_str())
                .copied(),
        )
    }));
    let message_rows = snapshot
        .recent_messages
        .iter()
        .map(prepare_message_row)
        .collect::<Vec<_>>();

    PreparedSlackQuickSearchSnapshot {
        snapshot,
        rows: rows.into(),
        message_rows: message_rows.into(),
    }
}

fn prepare_message_row(message: &SlackQuickSearchMessage) -> SlackQuickSearchMessageRow {
    debug_assert_ne!(message.conversation_kind, SlackConversationKind::Unknown);
    let conversation_label = message.conversation_label.clone();
    let highlights = prepare_message_highlights(message);
    let timestamp_label = slack_quick_search_timestamp(message.timestamp.as_str());
    let action_target = quick_search_message_action_target(message);
    let location_label = if message.thread_timestamp.is_some() {
        format!("thread in {conversation_label}")
    } else {
        conversation_label.clone()
    };
    SlackQuickSearchMessageRow {
        element_id: format!("slack-quick-search-message-{}", message.id).into(),
        accessibility_label: format!(
            "Message from {} in {}, {}: {}",
            message.author_label, location_label, timestamp_label, message.excerpt
        )
        .into(),
        author_label: message.author_label.clone().into(),
        conversation_label: conversation_label.into(),
        conversation_kind: message.conversation_kind,
        is_thread: message.thread_timestamp.is_some(),
        timestamp_label: timestamp_label.into(),
        excerpt: message.excerpt.clone().into(),
        highlights: highlights.into(),
        action_target,
        avatar_initials: initials(&message.author_label).into(),
        avatar_fill: slack_avatar_fill(&message.author_label),
        avatar_image_url: message.avatar_image_url.clone().map(Into::into),
    }
}

fn prepare_message_highlights(message: &SlackQuickSearchMessage) -> Vec<std::ops::Range<usize>> {
    message
        .highlights
        .iter()
        .map(|highlight| {
            assert!(
                highlight.start < highlight.end
                    && highlight.end <= message.excerpt.len()
                    && message.excerpt.is_char_boundary(highlight.start)
                    && message.excerpt.is_char_boundary(highlight.end),
                "validated Slack quick-search highlight must remain in bounds"
            );
            highlight.start..highlight.end
        })
        .collect()
}

fn quick_search_message_action_target(
    message: &SlackQuickSearchMessage,
) -> std::sync::Arc<SlackMessageActionTarget> {
    message
        .thread_timestamp
        .as_ref()
        .filter(|thread_timestamp| *thread_timestamp != &message.timestamp)
        .and_then(|thread_timestamp| {
            SlackMessageActionTarget::thread_reply(
                &message.team_id,
                &message.conversation_id,
                thread_timestamp.as_str(),
                message.timestamp.as_str(),
            )
        })
        .or_else(|| {
            SlackMessageActionTarget::conversation_message(
                &message.team_id,
                &message.conversation_id,
                message.timestamp.as_str(),
            )
        })
        .expect("validated Slack quick-search message must produce a navigation target")
}

fn prepare_conversation_row(
    conversation: &SlackQuickSearchConversation,
    person: Option<&SlackQuickSearchPerson>,
    sidebar_item: Option<&crate::ui::SlackSidebarItem>,
) -> SlackQuickSearchRow {
    let (kind, detail) = match conversation.kind {
        SlackConversationKind::Channel => (
            SlackQuickSearchRowKind::Channel,
            conversation_detail(conversation, "Channel"),
        ),
        SlackConversationKind::PrivateChannel => (
            SlackQuickSearchRowKind::PrivateChannel,
            conversation_detail(conversation, "Private channel"),
        ),
        SlackConversationKind::DirectMessage => (
            SlackQuickSearchRowKind::DirectMessage,
            "Direct message".to_string(),
        ),
        SlackConversationKind::GroupMessage => (
            SlackQuickSearchRowKind::GroupMessage,
            conversation_detail(conversation, "Group message"),
        ),
        SlackConversationKind::Unknown => {
            unreachable!("quick-search conversations are validated at the API boundary")
        }
    };
    let avatar_image_url = person
        .and_then(|person| person.avatar_image_url.as_deref())
        .or_else(|| sidebar_item.and_then(|item| item.avatar_image_url.as_deref()))
        .map(SharedString::from);
    SlackQuickSearchRow {
        element_id: format!("slack-quick-search-conversation-{}", conversation.id).into(),
        accessibility_label: format!("Open {} {}", detail, conversation.label).into(),
        label: conversation.label.clone().into(),
        detail: detail.into(),
        kind,
        target: SlackQuickSearchTarget::Conversation(conversation.id.clone().into()),
        avatar_initials: initials(&conversation.label).into(),
        avatar_fill: slack_avatar_fill(&conversation.label),
        avatar_image_url,
    }
}

fn prepare_person_row(person: &SlackQuickSearchPerson) -> SlackQuickSearchRow {
    let detail = if !person.title.is_empty() {
        person.title.clone()
    } else if !person.real_name.is_empty() && person.real_name != person.label {
        person.real_name.clone()
    } else {
        format!("@{}", person.username)
    };
    SlackQuickSearchRow {
        element_id: format!("slack-quick-search-person-{}", person.id).into(),
        accessibility_label: format!("Open profile for {}", person.label).into(),
        label: person.label.clone().into(),
        detail: detail.into(),
        kind: SlackQuickSearchRowKind::Person,
        target: SlackQuickSearchTarget::Profile(person.id.clone().into()),
        avatar_initials: initials(&person.label).into(),
        avatar_fill: slack_avatar_fill(&person.label),
        avatar_image_url: person.avatar_image_url.clone().map(Into::into),
    }
}

fn conversation_detail(conversation: &SlackQuickSearchConversation, kind: &str) -> String {
    if !conversation.is_member {
        return "· Not in channel".to_string();
    }
    match conversation.member_count {
        Some(count) => format!("· {count} member{}", if count == 1 { "" } else { "s" }),
        None => kind.to_string(),
    }
}
