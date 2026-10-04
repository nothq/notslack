use super::super::{
    SlackConversationKind, SlackDirectMessageUnreadState, SlackLastReadTimestamp,
    SlackMessageTimestamp, SlackRailBadges, SlackSidebarSection,
};
use super::{
    SlackConversationSnapshot, SlackDmInboxItem, SlackDmInboxSnapshot, SlackSidebarSnapshot,
    SlackWorkspace,
};

struct SidebarUnreadState<'a> {
    sections: &'a mut [SlackSidebarSection],
    direct_message_unread_states: &'a mut [SlackDirectMessageUnreadState],
    rail_badges: &'a mut SlackRailBadges,
}

#[derive(Default)]
struct ClearedDirectMessageState {
    changed: bool,
    unread: bool,
    display_count: u32,
}

impl SlackWorkspace {
    pub(crate) fn apply_conversation_read_receipt(
        &mut self,
        conversation_id: &str,
        last_read: &SlackLastReadTimestamp,
        authoritative_latest: Option<&SlackMessageTimestamp>,
    ) -> bool {
        let mut changed = apply_sidebar_read_receipt(
            SidebarUnreadState {
                sections: &mut self.sections,
                direct_message_unread_states: &mut self.direct_message_unread_states,
                rail_badges: &mut self.rail_badges,
            },
            conversation_id,
            last_read,
            authoritative_latest,
        );
        if self.conversation_id == conversation_id
            && (self.last_read.as_ref() != Some(last_read) || !self.last_read_boundary_loaded)
        {
            self.last_read = Some(last_read.clone());
            self.last_read_boundary_loaded = true;
            changed = true;
        }
        changed
    }
}

impl SlackSidebarSnapshot {
    pub(crate) fn apply_conversation_read_receipt(
        &mut self,
        conversation_id: &str,
        last_read: &SlackLastReadTimestamp,
        authoritative_latest: Option<&SlackMessageTimestamp>,
    ) -> bool {
        apply_sidebar_read_receipt(
            SidebarUnreadState {
                sections: &mut self.sections,
                direct_message_unread_states: &mut self.direct_message_unread_states,
                rail_badges: &mut self.rail_badges,
            },
            conversation_id,
            last_read,
            authoritative_latest,
        )
    }
}

impl SlackDmInboxItem {
    pub(crate) fn effective_unread_latest_timestamp(&self) -> &SlackMessageTimestamp {
        self.unread_state_latest_timestamp
            .as_ref()
            .filter(|latest| latest.sort_key() > self.latest_timestamp.sort_key())
            .unwrap_or(&self.latest_timestamp)
    }
}

impl SlackDmInboxSnapshot {
    pub(crate) fn apply_conversation_read_receipt(
        &mut self,
        conversation_id: &str,
        last_read: &SlackLastReadTimestamp,
    ) -> bool {
        let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.conversation_id == conversation_id)
        else {
            return false;
        };
        if item.effective_unread_latest_timestamp().sort_key() > last_read.sort_key() {
            return false;
        }
        let changed = item.unread || item.mention_count.is_some();
        item.unread = false;
        item.mention_count = None;
        changed
    }
}

impl SlackConversationSnapshot {
    pub(crate) fn apply_conversation_read_receipt(
        &mut self,
        conversation_id: &str,
        last_read: &SlackLastReadTimestamp,
    ) -> bool {
        if self.conversation_id != conversation_id
            || (self.last_read.as_ref() == Some(last_read) && self.last_read_boundary_loaded)
        {
            return false;
        }
        self.last_read = Some(last_read.clone());
        self.last_read_boundary_loaded = true;
        true
    }
}

fn apply_sidebar_read_receipt(
    sidebar: SidebarUnreadState<'_>,
    conversation_id: &str,
    last_read: &SlackLastReadTimestamp,
    authoritative_latest: Option<&SlackMessageTimestamp>,
) -> bool {
    let SidebarUnreadState {
        sections,
        direct_message_unread_states,
        rail_badges,
    } = sidebar;
    let state_index = direct_message_unread_states
        .iter()
        .position(|state| state.conversation_id == conversation_id);
    let authority_covers_receipt = state_index.is_some()
        && direct_message_latest_sort_key(
            sections,
            direct_message_unread_states,
            state_index,
            conversation_id,
            authoritative_latest,
        )
        .is_some_and(|latest| latest <= last_read.sort_key());
    let mut cleared = clear_direct_message_state(
        direct_message_unread_states,
        state_index,
        authority_covers_receipt,
    );
    let sidebar_cleared = clear_sidebar_items(
        sections,
        conversation_id,
        last_read,
        state_index.is_some(),
        authority_covers_receipt,
    );
    cleared.changed |= sidebar_cleared.changed;
    cleared.unread |= sidebar_cleared.unread;
    cleared.display_count = cleared.display_count.max(sidebar_cleared.display_count);
    apply_direct_message_badge_change(rail_badges, cleared)
}

fn direct_message_latest_sort_key(
    sections: &[SlackSidebarSection],
    states: &[SlackDirectMessageUnreadState],
    state_index: Option<usize>,
    conversation_id: &str,
    authoritative_latest: Option<&SlackMessageTimestamp>,
) -> Option<(u64, u32)> {
    state_index
        .and_then(|index| states[index].latest_message_timestamp.as_ref())
        .into_iter()
        .chain(authoritative_latest)
        .chain(
            sections
                .iter()
                .flat_map(|section| section.items.iter())
                .filter(|item| {
                    item.target_id == conversation_id
                        && matches!(
                            item.target_kind,
                            SlackConversationKind::DirectMessage
                                | SlackConversationKind::GroupMessage
                        )
                })
                .filter_map(|item| item.latest_message_timestamp.as_ref()),
        )
        .map(SlackMessageTimestamp::sort_key)
        .max()
}

fn clear_direct_message_state(
    states: &mut [SlackDirectMessageUnreadState],
    state_index: Option<usize>,
    authority_covers_receipt: bool,
) -> ClearedDirectMessageState {
    if !authority_covers_receipt {
        return ClearedDirectMessageState::default();
    }
    let state = &mut states[state_index.expect("covered DM read receipt requires state")];
    let cleared = ClearedDirectMessageState {
        changed: state.unread || state.display_count.is_some(),
        unread: state.unread,
        display_count: state.display_count.unwrap_or_default(),
    };
    state.unread = false;
    state.display_count = None;
    cleared
}

fn clear_sidebar_items(
    sections: &mut [SlackSidebarSection],
    conversation_id: &str,
    last_read: &SlackLastReadTimestamp,
    has_direct_message_state: bool,
    authority_covers_receipt: bool,
) -> ClearedDirectMessageState {
    let mut cleared = ClearedDirectMessageState::default();
    for item in sections
        .iter_mut()
        .flat_map(|section| section.items.iter_mut())
        .filter(|item| item.target_id == conversation_id)
    {
        let direct_message = matches!(
            item.target_kind,
            SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage
        );
        let covered = if direct_message && has_direct_message_state {
            authority_covers_receipt
        } else {
            item.latest_message_timestamp
                .as_ref()
                .is_some_and(|latest| latest.sort_key() <= last_read.sort_key())
        };
        if !covered {
            continue;
        }
        if direct_message && !has_direct_message_state {
            cleared.unread |= item.unread;
            cleared.display_count = cleared.display_count.max(item.count.unwrap_or_default());
        }
        cleared.changed |= item.unread || item.count.is_some();
        item.unread = false;
        item.count = None;
    }
    cleared
}

fn apply_direct_message_badge_change(
    rail_badges: &mut SlackRailBadges,
    cleared: ClearedDirectMessageState,
) -> bool {
    let mut changed = cleared.changed;
    if cleared.unread {
        changed |= subtract_badge(&mut rail_badges.dms, 1);
    }
    if cleared.display_count > 0 {
        changed |= subtract_badge(&mut rail_badges.dms_unread_messages, cleared.display_count);
    }
    changed
}

fn subtract_badge(value: &mut Option<u32>, amount: u32) -> bool {
    let next = value
        .map(|value| value.saturating_sub(amount))
        .filter(|value| *value > 0);
    if *value == next {
        return false;
    }
    *value = next;
    true
}
