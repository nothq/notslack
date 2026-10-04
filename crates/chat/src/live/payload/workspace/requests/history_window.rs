use std::{collections::HashSet, thread};

use serde_json::Value;

use super::{
    merge_slack_conversation_history_pages, ops, slack_conversation_history_page_state,
    SlackApiClient, SlackConversationHistoryCursor, SlackLastReadTimestamp,
    SlackLiveWorkspaceLoader, SlackMessageTimestamp, SLACK_CONVERSATION_HISTORY_PAGE_SIZE,
};

const SLACK_INITIAL_UNREAD_HISTORY_PAGE_LIMIT: usize = 4;

pub(in crate::live::payload::workspace) struct SlackInitialHistoryWindow {
    pub(in crate::live::payload::workspace) history: Value,
    pub(in crate::live::payload::workspace) source_pages: Vec<Value>,
    pub(in crate::live::payload::workspace) next_cursor: Option<SlackConversationHistoryCursor>,
    pub(in crate::live::payload::workspace) last_read_boundary_loaded: bool,
}

impl SlackInitialHistoryWindow {
    pub(in crate::live::payload::workspace) fn newest(history: Value) -> Result<Self, String> {
        let next_cursor = slack_conversation_history_page_state(&history, None)?.next_cursor;
        Ok(Self {
            history: history.clone(),
            source_pages: vec![history],
            next_cursor,
            last_read_boundary_loaded: true,
        })
    }
}

#[derive(Default)]
struct UnreadHistoryPages {
    source_pages: Vec<Value>,
    exhausted: bool,
    overlaps_newest: bool,
}

struct UnreadHistoryLoad<'a> {
    conversation_id: &'a str,
    last_read: &'a SlackLastReadTimestamp,
    last_read_key: (u64, u32),
    newest_timestamps: &'a HashSet<(u64, u32)>,
}

impl SlackLiveWorkspaceLoader {
    pub(in crate::live::payload::workspace) fn load_initial_history_window(
        &self,
        conversation_id: &str,
        newest_history: Value,
        last_read: &SlackLastReadTimestamp,
    ) -> Result<SlackInitialHistoryWindow, String> {
        let newest_state = slack_conversation_history_page_state(&newest_history, None)?;
        let newest_next_cursor = newest_state.next_cursor.clone();
        let last_read_key = last_read.sort_key();
        if newest_state
            .timestamps
            .last()
            .is_none_or(|timestamp| timestamp.sort_key() <= last_read_key)
        {
            return SlackInitialHistoryWindow::newest(newest_history);
        }
        let (before_history, after_history) =
            self.load_history_boundary_pages(conversation_id, last_read)?;
        let before_state = slack_conversation_history_page_state(&before_history, None)?;
        validate_before_history(&before_state.timestamps, last_read_key)?;
        let newest_timestamps = newest_state
            .timestamps
            .iter()
            .map(SlackMessageTimestamp::sort_key)
            .collect::<HashSet<_>>();
        let mut unread = self.load_unread_history_pages(
            UnreadHistoryLoad {
                conversation_id,
                last_read,
                last_read_key,
                newest_timestamps: &newest_timestamps,
            },
            after_history,
        )?;
        if !unread.exhausted || !unread.overlaps_newest {
            return Ok(SlackInitialHistoryWindow {
                history: newest_history.clone(),
                source_pages: vec![newest_history],
                next_cursor: newest_next_cursor,
                last_read_boundary_loaded: false,
            });
        }
        unread.source_pages.insert(0, before_history);
        unread.source_pages.push(newest_history);
        let history = merge_slack_conversation_history_pages(&unread.source_pages)?;
        Ok(SlackInitialHistoryWindow {
            history,
            source_pages: unread.source_pages,
            next_cursor: before_state.next_cursor,
            last_read_boundary_loaded: true,
        })
    }

    fn load_history_boundary_pages(
        &self,
        conversation_id: &str,
        last_read: &SlackLastReadTimestamp,
    ) -> Result<(Value, Value), String> {
        let before_request =
            self.spawn_conversation_history_before_request(conversation_id, last_read);
        let after_request =
            self.spawn_conversation_history_after_request(conversation_id, last_read, None);
        let before =
            ops::join_slack_request("conversations.history latest=last_read", before_request)?;
        let after =
            ops::join_slack_request("conversations.history oldest=last_read", after_request)?;
        Ok((before, after))
    }

    fn load_unread_history_pages(
        &self,
        load: UnreadHistoryLoad<'_>,
        mut after_history: Value,
    ) -> Result<UnreadHistoryPages, String> {
        let UnreadHistoryLoad {
            conversation_id,
            last_read,
            last_read_key,
            newest_timestamps,
        } = load;
        let mut result = UnreadHistoryPages::default();
        let mut seen_cursors = HashSet::new();
        let mut request_cursor = None;
        let mut previous_unread_newest = None;
        for page_index in 0..SLACK_INITIAL_UNREAD_HISTORY_PAGE_LIMIT {
            let after_state =
                slack_conversation_history_page_state(&after_history, request_cursor.as_ref())?;
            let has_more = after_history.get("has_more").and_then(Value::as_bool);
            let next_cursor = after_state.next_cursor;
            if after_state.timestamps.is_empty() {
                result.exhausted = has_more == Some(false) && next_cursor.is_none();
                break;
            }
            let Some(current_newest) = validate_unread_page(
                &after_state.timestamps,
                last_read_key,
                previous_unread_newest.as_ref(),
            )?
            else {
                break;
            };
            result.overlaps_newest |=
                unread_page_overlaps(&after_state.timestamps, newest_timestamps);
            result.source_pages.push(after_history);
            match (has_more, next_cursor) {
                (Some(false), None) => {
                    result.exhausted = true;
                    break;
                }
                (Some(true), Some(next_cursor)) => {
                    record_unread_history_cursor(&mut seen_cursors, &next_cursor)?;
                    if page_index + 1 == SLACK_INITIAL_UNREAD_HISTORY_PAGE_LIMIT {
                        break;
                    }
                    previous_unread_newest = Some(current_newest);
                    request_cursor = Some(next_cursor);
                    after_history = self.load_conversation_history_after(
                        conversation_id,
                        last_read,
                        request_cursor.as_ref(),
                    )?;
                }
                _ => break,
            }
        }
        Ok(result)
    }

    fn spawn_conversation_history_before_request(
        &self,
        conversation_id: &str,
        last_read: &SlackLastReadTimestamp,
    ) -> thread::JoinHandle<Result<Value, String>> {
        let conversation_id = conversation_id.to_string();
        let last_read = last_read.as_str().to_string();
        self.spawn_slack_request(move |api| {
            api.post(
                "conversations.history",
                &[
                    ("channel", conversation_id),
                    ("limit", SLACK_CONVERSATION_HISTORY_PAGE_SIZE.to_string()),
                    ("include_all_metadata", "true".to_string()),
                    ("latest", last_read),
                    ("inclusive", "true".to_string()),
                ],
            )
        })
    }

    fn spawn_conversation_history_after_request(
        &self,
        conversation_id: &str,
        last_read: &SlackLastReadTimestamp,
        cursor: Option<&SlackConversationHistoryCursor>,
    ) -> thread::JoinHandle<Result<Value, String>> {
        let api = self.api.clone();
        let conversation_id = conversation_id.to_string();
        let last_read = last_read.clone();
        let cursor = cursor.cloned();
        thread::spawn(move || {
            load_conversation_history_after(&api, &conversation_id, &last_read, cursor.as_ref())
        })
    }

    fn load_conversation_history_after(
        &self,
        conversation_id: &str,
        last_read: &SlackLastReadTimestamp,
        cursor: Option<&SlackConversationHistoryCursor>,
    ) -> Result<Value, String> {
        load_conversation_history_after(&self.api, conversation_id, last_read, cursor)
    }
}

fn validate_before_history(
    timestamps: &[SlackMessageTimestamp],
    last_read_key: (u64, u32),
) -> Result<(), String> {
    if timestamps
        .iter()
        .any(|timestamp| timestamp.sort_key() > last_read_key)
    {
        return Err(
            "Slack conversations.history latest=last_read returned a newer message".to_string(),
        );
    }
    Ok(())
}

fn record_unread_history_cursor(
    seen_cursors: &mut HashSet<SlackConversationHistoryCursor>,
    cursor: &SlackConversationHistoryCursor,
) -> Result<(), String> {
    if !seen_cursors.insert(cursor.clone()) {
        return Err("Slack conversations.history oldest=last_read repeated a cursor".to_string());
    }
    Ok(())
}

fn unread_page_overlaps(
    timestamps: &[SlackMessageTimestamp],
    newest_timestamps: &HashSet<(u64, u32)>,
) -> bool {
    timestamps
        .iter()
        .any(|timestamp| newest_timestamps.contains(&timestamp.sort_key()))
}

fn validate_unread_page(
    timestamps: &[SlackMessageTimestamp],
    last_read_key: (u64, u32),
    previous_unread_newest: Option<&SlackMessageTimestamp>,
) -> Result<Option<SlackMessageTimestamp>, String> {
    if timestamps
        .iter()
        .any(|timestamp| timestamp.sort_key() <= last_read_key)
    {
        return Err(
            "Slack conversations.history oldest=last_read did not return a strict unread page"
                .to_string(),
        );
    }
    let current_newest = timestamps
        .first()
        .expect("validated Slack unread history page must not be empty")
        .clone();
    let current_oldest = timestamps
        .last()
        .expect("validated Slack unread history page must not be empty");
    if previous_unread_newest
        .is_some_and(|previous| current_oldest.sort_key() <= previous.sort_key())
    {
        return Ok(None);
    }
    Ok(Some(current_newest))
}

fn load_conversation_history_after(
    api: &SlackApiClient,
    conversation_id: &str,
    last_read: &SlackLastReadTimestamp,
    cursor: Option<&SlackConversationHistoryCursor>,
) -> Result<Value, String> {
    let mut params = vec![
        ("channel", conversation_id.to_string()),
        ("limit", SLACK_CONVERSATION_HISTORY_PAGE_SIZE.to_string()),
        ("include_all_metadata", "true".to_string()),
        ("oldest", last_read.as_str().to_string()),
        ("inclusive", "false".to_string()),
    ];
    if let Some(cursor) = cursor {
        params.push(("cursor", cursor.as_str().to_string()));
    }
    api.post("conversations.history", &params)
}
