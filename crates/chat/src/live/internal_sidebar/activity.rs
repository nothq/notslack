use crate::model::{
    SlackActivityArchiveKind, SlackActivityArchiveTarget, SlackActivityCursor, SlackActivityItem,
    SlackActivityReadKind, SlackActivityReadTarget, SlackActivitySnapshot, SlackMessage,
    SlackMessageTimestamp,
};
use chrono_tz::Tz;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

mod hydration;
mod wire;

use wire::{activity_request_metadata, decode_activity_feed, decode_activity_mutation};

use super::SlackInternalSidebarClient;
use crate::live::{
    api::messages::{SlackMessageReference, SlackMessagesListHydration},
    payload::sidebar_dom::SlackSidebarSnapshot,
};

const ACTIVITY_FEED: &str = "activity.feed";
const ACTIVITY_ARCHIVE: &str = "activity.archive";
const ACTIVITY_MARK_READ: &str = "activity.markRead";
const ACTIVITY_MARK_UNREAD: &str = "activity.markUnread";
const ACTIVITY_UNARCHIVE: &str = "activity.unarchive";
const ACTIVITY_PAGE_SIZE: usize = 20;
const ACTIVITY_TYPES: &str = "at_user,at_user_group,at_channel,at_everyone,keyword,list_record_assigned,list_user_mentioned,list_todo_notification,list_approval_request,list_approval_reviewed,unjoined_channel_mention,at_user,unjoined_channel_mention,at_channel,at_everyone,at_user_group,keyword,thread_v2,message_reaction,bot_dm_bundle,dm,prejoin_dm_welcome_party_alert,internal_channel_invite,external_channel_invite,external_dm_invite,quietly_added_to_channel,channel,saved_reminder,list_record_edited";

pub(crate) struct SlackActivityFeedPage {
    activity_views_date_updated: String,
    items: Vec<SlackActivityFeedItem>,
    next_cursor: Option<SlackActivityCursor>,
}

pub(crate) struct SlackActivitySnapshotHydrationInput<'a> {
    messages: SlackMessagesListHydration,
    users: &'a HashMap<String, Value>,
    sidebar_snapshot: Option<&'a SlackSidebarSnapshot>,
    self_user_id: &'a str,
    timezone: Tz,
}

impl<'a> SlackActivitySnapshotHydrationInput<'a> {
    pub(crate) fn new(
        messages: SlackMessagesListHydration,
        users: &'a HashMap<String, Value>,
        sidebar_snapshot: Option<&'a SlackSidebarSnapshot>,
        self_user_id: &'a str,
        timezone: Tz,
    ) -> Self {
        Self {
            messages,
            users,
            sidebar_snapshot,
            self_user_id,
            timezone,
        }
    }
}

struct SlackActivityHydrationContext<'a> {
    messages: &'a mut SlackMessagesListHydration,
    users: &'a HashMap<String, Value>,
    sidebar_snapshot: Option<&'a SlackSidebarSnapshot>,
    self_user_id: &'a str,
    timezone: Tz,
}

struct HydratedSlackActivityMessage {
    message: SlackMessage,
    thread_timestamp: Option<SlackMessageTimestamp>,
}

struct SlackActivityFeedItem {
    key: String,
    feed_timestamp: String,
    version: String,
    archived: bool,
    unread: bool,
    bot: Option<bool>,
    content: SlackActivityFeedContent,
}

enum SlackActivityFeedContent {
    MessageReaction {
        reference: SlackMessageReference,
        reaction_name: String,
        reaction_user_id: String,
    },
    ThreadV2 {
        reference: SlackMessageReference,
        thread_timestamp: SlackMessageTimestamp,
        unread_message_count: u32,
    },
    AtUser {
        reference: SlackMessageReference,
        author_user_id: String,
        broadcast: bool,
        thread_timestamp: Option<SlackMessageTimestamp>,
    },
    Dm {
        reference: SlackMessageReference,
    },
    BotDmBundle {
        reference: SlackMessageReference,
        unread_count: u32,
    },
    Unsupported {
        kind: String,
    },
}

impl SlackInternalSidebarClient {
    pub(crate) fn load_activity_feed(
        &self,
        cursor: Option<&SlackActivityCursor>,
    ) -> Result<SlackActivityFeedPage, String> {
        let mut params = vec![
            ("limit", ACTIVITY_PAGE_SIZE.to_string()),
            ("types", ACTIVITY_TYPES.to_string()),
            ("mode", "chrono_v1".to_string()),
            ("archive_only", "false".to_string()),
            ("unread_only", "false".to_string()),
            ("priority_only", "false".to_string()),
            ("only_salesforce_channels", "false".to_string()),
            ("exclude_automations", "false".to_string()),
            ("automations_only", "false".to_string()),
            ("is_activity_inbox", "true".to_string()),
        ];
        params.extend(activity_request_metadata("fetchActivityFeed"));
        if let Some(cursor) = cursor {
            params.push(("cursor", cursor.as_str().to_string()));
        }
        let body = self.post_internal_method(ACTIVITY_FEED, params)?;
        decode_activity_feed(&body)
    }

    pub(crate) fn mark_activity_item_read(
        &self,
        target: &SlackActivityReadTarget,
    ) -> Result<(), String> {
        let mut params = vec![
            ("type", activity_read_kind_name(target.kind()).to_string()),
            ("feed_ts", target.feed_timestamp().to_string()),
            ("key", target.key().to_string()),
        ];
        if target.kind() == SlackActivityReadKind::Dm {
            params.push(("ts", target.timestamp().as_str().to_string()));
        }
        params.extend(activity_request_metadata("mark-as-read-v2"));
        let body = self.post_internal_method(ACTIVITY_MARK_READ, params)?;
        decode_activity_mutation(ACTIVITY_MARK_READ, &body)
    }

    pub(crate) fn mark_activity_item_unread(
        &self,
        target: &SlackActivityReadTarget,
    ) -> Result<(), String> {
        let mut params = vec![
            ("type", activity_read_kind_name(target.kind()).to_string()),
            ("key", target.key().to_string()),
            ("ts", target.timestamp().as_str().to_string()),
            ("feed_ts", target.feed_timestamp().to_string()),
        ];
        params.extend(activity_request_metadata("mark-as-unread"));
        let body = self.post_internal_method(ACTIVITY_MARK_UNREAD, params)?;
        decode_activity_mutation(ACTIVITY_MARK_UNREAD, &body)
    }

    pub(crate) fn archive_activity_item(
        &self,
        target: &SlackActivityArchiveTarget,
        reason: &str,
    ) -> Result<(), String> {
        self.mutate_activity_item_archive(ACTIVITY_ARCHIVE, target, reason)
    }

    pub(crate) fn unarchive_activity_item(
        &self,
        target: &SlackActivityArchiveTarget,
        reason: &str,
    ) -> Result<(), String> {
        self.mutate_activity_item_archive(ACTIVITY_UNARCHIVE, target, reason)
    }

    fn mutate_activity_item_archive(
        &self,
        method: &str,
        target: &SlackActivityArchiveTarget,
        reason: &str,
    ) -> Result<(), String> {
        let mut params = vec![
            (
                "type",
                activity_archive_kind_name(target.kind()).to_string(),
            ),
            ("key", target.key().to_string()),
            ("ts", target.timestamp().as_str().to_string()),
        ];
        params.extend(activity_request_metadata(reason));
        let body = self.post_internal_method(method, params)?;
        decode_activity_mutation(method, &body)
    }
}

fn activity_read_kind_name(kind: SlackActivityReadKind) -> &'static str {
    match kind {
        SlackActivityReadKind::ThreadV2 => "thread_v2",
        SlackActivityReadKind::AtUser => "at_user",
        SlackActivityReadKind::Dm => "dm",
        SlackActivityReadKind::BotDmBundle => "bot_dm_bundle",
    }
}

fn activity_archive_kind_name(kind: SlackActivityArchiveKind) -> &'static str {
    match kind {
        SlackActivityArchiveKind::MessageReaction => "message_reaction",
        SlackActivityArchiveKind::ThreadV2 => "thread_v2",
        SlackActivityArchiveKind::AtUser => "at_user",
        SlackActivityArchiveKind::Dm => "dm",
        SlackActivityArchiveKind::BotDmBundle => "bot_dm_bundle",
    }
}

impl SlackActivityFeedPage {
    pub(crate) fn message_references(&self) -> Vec<SlackMessageReference> {
        self.items
            .iter()
            .filter_map(|item| item.content.message_reference().cloned())
            .collect()
    }

    pub(crate) fn reaction_actor_user_ids(&self) -> BTreeSet<String> {
        self.items
            .iter()
            .filter_map(|item| match &item.content {
                SlackActivityFeedContent::MessageReaction {
                    reaction_user_id, ..
                } => Some(reaction_user_id.clone()),
                _ => None,
            })
            .collect()
    }

    pub(crate) fn into_snapshot(
        self,
        input: SlackActivitySnapshotHydrationInput<'_>,
    ) -> Result<SlackActivitySnapshot, String> {
        let SlackActivitySnapshotHydrationInput {
            mut messages,
            users,
            sidebar_snapshot,
            self_user_id,
            timezone,
        } = input;
        let items = {
            let mut context = SlackActivityHydrationContext {
                messages: &mut messages,
                users,
                sidebar_snapshot,
                self_user_id,
                timezone,
            };
            self.items
                .into_iter()
                .map(|item| item.into_activity_item(&mut context))
                .collect::<Result<Vec<_>, _>>()?
        };
        messages.ensure_empty()?;
        Ok(SlackActivitySnapshot {
            activity_views_date_updated: self.activity_views_date_updated,
            items,
            next_cursor: self.next_cursor,
        })
    }
}

impl SlackActivityFeedItem {
    fn into_activity_item(
        self,
        context: &mut SlackActivityHydrationContext<'_>,
    ) -> Result<SlackActivityItem, String> {
        Ok(SlackActivityItem {
            key: self.key,
            feed_timestamp: self.feed_timestamp,
            version: self.version,
            archived: self.archived,
            unread: self.unread,
            bot: self.bot,
            content: self.content.into_activity_content(context)?,
        })
    }
}
