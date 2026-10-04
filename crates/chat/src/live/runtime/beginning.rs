use std::collections::HashSet;

use crate::model::{
    SlackConversationHistoryCursor, SlackConversationHistoryPage, SlackConversationSnapshot,
    SlackMessage,
};

use crate::live::SlackLiveWorkspaceLoader;

use super::validate_slack_conversation_message_order;

type MessageBounds = ((u64, u32), (u64, u32));

struct BeginningHistory {
    cursor: Option<SlackConversationHistoryCursor>,
    messages: Vec<SlackMessage>,
    bounds: Option<MessageBounds>,
    seen_cursors: HashSet<String>,
}

impl BeginningHistory {
    fn from_conversation(conversation: &mut SlackConversationSnapshot) -> Result<Self, String> {
        let cursor = conversation.history_next_cursor.take();
        let messages = std::mem::take(&mut conversation.messages);
        let bounds = validate_slack_conversation_message_order(&messages)?;
        let seen_cursors = cursor
            .iter()
            .map(|cursor| cursor.as_str().to_string())
            .collect();
        Ok(Self {
            cursor,
            messages,
            bounds,
            seen_cursors,
        })
    }

    fn apply_page(
        &mut self,
        page: SlackConversationHistoryPage,
        team_id: &str,
        conversation_id: &str,
    ) -> Result<(), String> {
        if page.team_id != team_id || page.conversation_id != conversation_id {
            return Err(
                "Slack beginning navigation returned a mismatched history page".to_string(),
            );
        }
        let page_bounds = validate_slack_conversation_message_order(&page.messages)?;
        if page.messages.is_empty() && page.next_cursor.is_some() {
            return Err(
                "Slack beginning navigation received an empty non-terminal history page"
                    .to_string(),
            );
        }
        if let (Some((_, page_newest)), Some((current_oldest, _))) = (page_bounds, self.bounds) {
            if page_newest >= current_oldest {
                return Err(
                    "Slack beginning navigation history pages overlap or are out of order"
                        .to_string(),
                );
            }
        }
        if !page.messages.is_empty() {
            self.messages = page.messages;
            self.bounds = page_bounds;
        }
        if let Some(next_cursor) = page.next_cursor.as_ref() {
            if !self.seen_cursors.insert(next_cursor.as_str().to_string()) {
                return Err(
                    "Slack beginning navigation repeated a history pagination cursor".to_string(),
                );
            }
        }
        self.cursor = page.next_cursor;
        Ok(())
    }
}

pub(super) fn load(
    loader: &SlackLiveWorkspaceLoader,
    conversation_id: &str,
) -> Result<SlackConversationSnapshot, String> {
    let mut conversation = loader.load_conversation(conversation_id)?;
    if conversation.team_id != loader.team_id() || conversation.conversation_id != conversation_id {
        return Err(
            "Slack beginning navigation returned a mismatched conversation snapshot".to_string(),
        );
    }
    let mut history = BeginningHistory::from_conversation(&mut conversation)?;
    while let Some(cursor) = history.cursor.take() {
        let page = loader.load_conversation_history_page(conversation_id, &cursor)?;
        history.apply_page(page, &conversation.team_id, conversation_id)?;
    }
    conversation.messages = history.messages;
    conversation.history_next_cursor = None;
    conversation.last_read_boundary_loaded = true;
    Ok(conversation)
}
