//! Keeping the direct message list current: a message sent or received in a
//! direct message reloads the first page of the inbox, so its preview and
//! order follow the conversation.

use std::collections::HashSet;

use super::super::{build_slack_dm_rows, Context, PreparedSlackDmInboxSnapshot, SurfaceState};
use crate::ui::SlackDmInboxSnapshot;

impl SurfaceState {
    /// Marks the inbox stale when realtime reports a change in a direct
    /// message: a conversation already listed, or any one-to-one conversation.
    pub(in crate::ui::surface::state) fn note_slack_realtime_dm_changes(
        &mut self,
        full_resync: bool,
        conversation_ids: &[String],
    ) {
        let touches_inbox = |conversation_id: &String| {
            conversation_id.starts_with('D')
                || self.slack_dm_inbox_snapshot.as_ref().is_some_and(|inbox| {
                    inbox
                        .items
                        .iter()
                        .any(|item| &item.conversation_id == conversation_id)
                })
        };
        if full_resync || conversation_ids.iter().any(touches_inbox) {
            self.slack_realtime_dm_inbox_stale = true;
        }
    }

    pub(in crate::ui::surface::state) fn refresh_slack_dm_inbox_from_realtime(
        &mut self,
        cx: &mut Context<Self>,
    ) {
        if !self.slack_realtime_dm_inbox_stale
            || self.slack_dm_inbox_refresh_in_flight
            || self.slack_dm_inbox_snapshot.is_none()
            || !self.slack_workspace_api_capabilities.load_dm_inbox
        {
            return;
        }
        let Some(team_id) = self
            .slack_workspace()
            .map(|workspace| workspace.team_id.clone())
        else {
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        self.slack_realtime_dm_inbox_stale = false;
        self.slack_dm_inbox_refresh_in_flight = true;
        self.spawn_background_task(
            (workspace_api, team_id),
            cx,
            |(workspace_api, team_id)| {
                let inbox = workspace_api.load_slack_dm_inbox(None);
                (team_id, inbox)
            },
            |this, (team_id, inbox), cx| {
                this.finish_slack_dm_inbox_realtime_refresh(&team_id, inbox, cx);
            },
        );
    }

    fn finish_slack_dm_inbox_realtime_refresh(
        &mut self,
        team_id: &str,
        inbox: Result<SlackDmInboxSnapshot, String>,
        cx: &mut Context<Self>,
    ) {
        self.slack_dm_inbox_refresh_in_flight = false;
        match inbox {
            Ok(inbox) if inbox.team_id == team_id => {
                let inbox = match self.slack_dm_inbox_snapshot.as_ref() {
                    Some(current) => merge_refreshed_slack_dm_inbox(current, inbox),
                    None => inbox,
                };
                let prepared = PreparedSlackDmInboxSnapshot {
                    rows: build_slack_dm_rows(&inbox),
                    snapshot: inbox,
                };
                self.apply_prepared_slack_dm_inbox_snapshot(prepared, false, cx);
            }
            Ok(_) => {}
            Err(error) => eprintln!("Slack DMs inbox realtime refresh failed: {error}"),
        }
        self.refresh_slack_dm_inbox_from_realtime(cx);
    }
}

/// The refreshed first page, followed by the conversations already loaded
/// that it does not list. When any remain, paging continues from where the
/// loaded list had reached.
fn merge_refreshed_slack_dm_inbox(
    current: &SlackDmInboxSnapshot,
    mut refreshed: SlackDmInboxSnapshot,
) -> SlackDmInboxSnapshot {
    let listed = refreshed
        .items
        .iter()
        .map(|item| item.conversation_id.clone())
        .collect::<HashSet<_>>();
    let older = current
        .items
        .iter()
        .filter(|item| !listed.contains(&item.conversation_id))
        .cloned()
        .collect::<Vec<_>>();
    if !older.is_empty() {
        refreshed.items.extend(older);
        refreshed.next_cursor = current.next_cursor.clone();
    }
    if refreshed.slackbot_conversation_id.is_none() {
        refreshed.slackbot_conversation_id = current.slackbot_conversation_id.clone();
    }
    refreshed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inbox(conversation_ids: &[&str], next_cursor: Option<&str>) -> SlackDmInboxSnapshot {
        SlackDmInboxSnapshot {
            team_id: "T1".to_string(),
            items: conversation_ids
                .iter()
                .map(|conversation_id| crate::ui::SlackDmInboxItem {
                    conversation_id: conversation_id.to_string(),
                    kind: crate::ui::SlackConversationKind::DirectMessage,
                    label: "Ada".to_string(),
                    participants: Vec::new(),
                    latest_sender_user_id: None,
                    latest_sender_label: None,
                    latest_message_text: String::new(),
                    latest_timestamp: crate::ui::SlackMessageTimestamp::parse("1700000000.000100")
                        .expect("test timestamp should parse"),
                    unread_state_latest_timestamp: None,
                    unread: false,
                    mention_count: None,
                })
                .collect(),
            next_cursor: next_cursor.map(str::to_string),
            ..SlackDmInboxSnapshot::default()
        }
    }

    fn ids(inbox: &SlackDmInboxSnapshot) -> Vec<&str> {
        inbox
            .items
            .iter()
            .map(|item| item.conversation_id.as_str())
            .collect()
    }

    #[test]
    fn the_refreshed_page_leads_and_older_conversations_follow() {
        let current = inbox(&["D1", "D2", "D3", "D4"], Some("page-3"));
        let refreshed = inbox(&["D3", "D9", "D1"], Some("page-2"));
        let merged = merge_refreshed_slack_dm_inbox(&current, refreshed);
        assert_eq!(ids(&merged), ["D3", "D9", "D1", "D2", "D4"]);
        assert_eq!(merged.next_cursor.as_deref(), Some("page-3"));
    }

    #[test]
    fn a_refresh_that_lists_everything_keeps_its_own_cursor() {
        let current = inbox(&["D1", "D2"], Some("page-2"));
        let refreshed = inbox(&["D2", "D1"], Some("page-2b"));
        let merged = merge_refreshed_slack_dm_inbox(&current, refreshed);
        assert_eq!(ids(&merged), ["D2", "D1"]);
        assert_eq!(merged.next_cursor.as_deref(), Some("page-2b"));
    }
}
