use std::collections::HashMap;

use crate::ui::{
    SlackConversationKind, SlackDirectMessageUnreadState, SlackDmInboxItem, SlackDmInboxSnapshot,
    SlackSidebarItem, SlackSidebarSnapshot,
};

pub(super) fn merge_slack_dm_sidebar_state(
    snapshot: &mut SlackDmInboxSnapshot,
    sidebar: Option<&SlackSidebarSnapshot>,
) -> bool {
    let Some(sidebar) = sidebar else {
        return false;
    };
    let sidebar_items = sidebar
        .sections
        .iter()
        .flat_map(|section| section.items.iter())
        .filter(|item| {
            matches!(
                item.target_kind,
                SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
            )
        })
        .map(|item| (item.target_id.as_str(), item))
        .collect::<HashMap<_, _>>();
    let unread_states = sidebar
        .direct_message_unread_states
        .iter()
        .map(|state| (state.conversation_id.as_str(), state))
        .collect::<HashMap<_, _>>();
    let mut changed = false;
    for item in &mut snapshot.items {
        let unread_state = unread_states.get(item.conversation_id.as_str()).copied();
        let sidebar_item = sidebar_items.get(item.conversation_id.as_str()).copied();
        changed |= merge_slack_dm_unread_state(item, unread_state);
        changed |= merge_slack_dm_presence(item, sidebar_item);
    }
    changed
}

fn merge_slack_dm_unread_state(
    item: &mut SlackDmInboxItem,
    unread_state: Option<&SlackDirectMessageUnreadState>,
) -> bool {
    let Some(unread_state) = unread_state else {
        return false;
    };
    let mut changed = false;
    if item.unread != unread_state.unread {
        item.unread = unread_state.unread;
        changed = true;
    }
    if item.mention_count != unread_state.display_count {
        item.mention_count = unread_state.display_count;
        changed = true;
    }
    if let Some(latest) = unread_state.latest_message_timestamp.as_ref() {
        let advances_authority = item
            .unread_state_latest_timestamp
            .as_ref()
            .is_none_or(|current| latest.sort_key() > current.sort_key());
        if advances_authority {
            item.unread_state_latest_timestamp = Some(latest.clone());
            changed = true;
        }
    }
    changed
}

fn merge_slack_dm_presence(
    item: &mut SlackDmInboxItem,
    sidebar_item: Option<&SlackSidebarItem>,
) -> bool {
    let Some((sidebar_item, presence)) =
        sidebar_item.and_then(|item| item.presence.map(|presence| (item, presence)))
    else {
        return false;
    };
    let participant_index = sidebar_item
        .user_id
        .as_deref()
        .and_then(|user_id| {
            item.participants
                .iter()
                .position(|participant| participant.user_id == user_id)
        })
        .unwrap_or(0);
    let Some(participant) = item.participants.get_mut(participant_index) else {
        return false;
    };
    if participant.presence.is_some() {
        return false;
    }
    participant.presence = Some(presence);
    true
}
