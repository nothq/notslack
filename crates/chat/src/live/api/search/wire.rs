use crate::model::{SlackConversationKind, SlackReaction, SlackSearchMessage, SlackSearchSnapshot};
use serde::Deserialize;

mod file;
mod highlight;

use file::SlackSearchFileWire;
pub(super) use highlight::{parse_search_highlighted_text_ranges, strip_search_highlight_markers};

use super::{
    require_nonempty, require_nonempty_for, require_slack_timestamp, require_slack_timestamp_for,
    SlackSearchPageRequest, SLACK_SEARCH_MESSAGES_METHOD,
};
use crate::live::api::messages::{
    SlackMessageReference, SlackMessagesListHydration, SLACK_MESSAGES_LIST_METHOD,
};

pub(super) struct SlackSearchSnapshotInput<'a> {
    pub(super) page_request: SlackSearchPageRequest,
    pub(super) team_id: &'a str,
    pub(super) self_user_id: &'a str,
}

#[derive(Deserialize)]
struct SlackHydratedSearchMessageWire {
    #[serde(default)]
    thread_ts: Option<String>,
    #[serde(default)]
    reactions: Vec<SlackSearchReactionWire>,
    #[serde(default)]
    reply_count: Option<u32>,
    #[serde(default)]
    latest_reply: Option<String>,
    #[serde(default)]
    reply_users: Vec<String>,
}

impl SlackHydratedSearchMessageWire {
    fn into_metadata(
        self,
        self_user_id: &str,
    ) -> Result<SlackHydratedSearchMessageMetadata, String> {
        if let Some(thread_timestamp) = self.thread_ts.as_deref() {
            require_slack_timestamp_for(
                SLACK_MESSAGES_LIST_METHOD,
                "messages_data.messages.thread_ts",
                thread_timestamp,
            )?;
        }
        if let Some(latest_reply_timestamp) = self.latest_reply.as_deref() {
            require_slack_timestamp_for(
                SLACK_MESSAGES_LIST_METHOD,
                "messages_data.messages.latest_reply",
                latest_reply_timestamp,
            )?;
        }
        Ok(SlackHydratedSearchMessageMetadata {
            thread_timestamp: self.thread_ts,
            reactions: self
                .reactions
                .into_iter()
                .map(|reaction| reaction.into_reaction(self_user_id))
                .collect::<Result<Vec<_>, _>>()?,
            reply_count: self.reply_count.filter(|count| *count > 0),
            latest_reply_timestamp: self.latest_reply,
            reply_user_ids: self.reply_users,
        })
    }
}

struct SlackHydratedSearchMessageMetadata {
    thread_timestamp: Option<String>,
    reactions: Vec<SlackReaction>,
    reply_count: Option<u32>,
    latest_reply_timestamp: Option<String>,
    reply_user_ids: Vec<String>,
}

#[derive(Deserialize)]
pub(super) struct SlackSearchResponseWire {
    query: String,
    module: String,
    items: Vec<SlackSearchResultWire>,
    pagination: SlackSearchPaginationWire,
}

impl SlackSearchResponseWire {
    pub(super) fn message_references(&self) -> Result<Vec<SlackMessageReference>, String> {
        let mut references = Vec::new();
        for item in &self.items {
            require_nonempty("channel.id", &item.channel.id)?;
            let message = item.primary_message()?;
            require_slack_timestamp("ts", &message.ts)?;
            references.push(SlackMessageReference::parse(&item.channel.id, &message.ts)?);
        }
        Ok(references)
    }

    pub(super) fn into_snapshot(
        self,
        input: SlackSearchSnapshotInput<'_>,
        mut hydration: SlackMessagesListHydration,
    ) -> Result<SlackSearchSnapshot, String> {
        let Self {
            query: response_query,
            module,
            items,
            pagination,
        } = self;
        require_nonempty("query", &response_query)?;
        if module != "messages" {
            return Err(format!(
                "Slack {SLACK_SEARCH_MESSAGES_METHOD} returned unexpected module {module}"
            ));
        }
        let next_page = pagination.next_page(input.page_request.page(), items.is_empty())?;
        let messages = items
            .into_iter()
            .map(|item| item.into_message(input.team_id, input.self_user_id, &mut hydration))
            .collect::<Result<Vec<_>, _>>()?;
        hydration.ensure_empty()?;
        Ok(SlackSearchSnapshot {
            query: response_query,
            total: pagination.total_count,
            messages,
            next_cursor: next_page.map(|page| input.page_request.next_cursor(page)),
        })
    }
}

#[derive(Deserialize)]
struct SlackSearchResultWire {
    iid: String,
    #[serde(rename = "team")]
    source_team_id: String,
    channel: SlackSearchChannelWire,
    messages: Vec<SlackSearchMessageWire>,
}

impl SlackSearchResultWire {
    fn primary_message(&self) -> Result<&SlackSearchMessageWire, String> {
        self.messages.first().ok_or_else(|| {
            format!("Slack {SLACK_SEARCH_MESSAGES_METHOD} response returned an empty item.messages")
        })
    }

    fn into_message(
        self,
        team_id: &str,
        self_user_id: &str,
        hydration: &mut SlackMessagesListHydration,
    ) -> Result<SlackSearchMessage, String> {
        let Self {
            iid,
            source_team_id,
            channel,
            messages,
        } = self;
        require_nonempty("item.iid", &iid)?;
        require_nonempty("item.team", &source_team_id)?;
        channel.validate()?;
        let conversation_kind = channel.kind()?;
        let message = messages.into_iter().next().ok_or_else(|| {
            format!("Slack {SLACK_SEARCH_MESSAGES_METHOD} response returned an empty item.messages")
        })?;
        message.into_message(
            SlackSearchMessageInput {
                result_id: iid,
                team_id,
                channel: &channel,
                conversation_kind,
                self_user_id,
            },
            hydration,
        )
    }
}

struct SlackSearchMessageInput<'a> {
    result_id: String,
    team_id: &'a str,
    channel: &'a SlackSearchChannelWire,
    conversation_kind: SlackConversationKind,
    self_user_id: &'a str,
}

#[derive(Deserialize)]
struct SlackSearchMessageWire {
    iid: String,
    text: String,
    ts: String,
    user: String,
    username: String,
    permalink: String,
    #[serde(default)]
    files: Vec<SlackSearchFileWire>,
}

impl SlackSearchMessageWire {
    fn into_message(
        self,
        input: SlackSearchMessageInput<'_>,
        hydration: &mut SlackMessagesListHydration,
    ) -> Result<SlackSearchMessage, String> {
        require_nonempty("iid", &self.iid)?;
        require_slack_timestamp("ts", &self.ts)?;
        require_nonempty("permalink", &self.permalink)?;
        let body = parse_search_highlighted_text("message text", self.text)?;
        let reference = SlackMessageReference::parse(&input.channel.id, &self.ts)?;
        let hydrated_message = hydration.take(&reference)?;
        let metadata = serde_json::from_value::<SlackHydratedSearchMessageWire>(hydrated_message)
            .map_err(|error| {
                format!("failed to decode Slack {SLACK_MESSAGES_LIST_METHOD} response: {error}")
            })?
            .into_metadata(input.self_user_id)?;
        Ok(SlackSearchMessage {
            id: input.result_id,
            team_id: input.team_id.to_string(),
            conversation_id: input.channel.id.clone(),
            conversation_name: input.channel.name.clone(),
            conversation_kind: input.conversation_kind,
            user_id: self.user,
            username: self.username,
            avatar_image_url: None,
            timestamp: self.ts,
            thread_timestamp: metadata.thread_timestamp,
            body,
            permalink: self.permalink,
            attachments: self
                .files
                .into_iter()
                .map(SlackSearchFileWire::into_attachment)
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .collect(),
            reactions: metadata.reactions,
            reply_count: metadata.reply_count,
            latest_reply_timestamp: metadata.latest_reply_timestamp,
            reply_user_ids: metadata.reply_user_ids,
            reply_user_avatar_image_urls: Vec::new(),
        })
    }
}

fn parse_search_highlighted_text(field: &str, text: String) -> Result<String, String> {
    strip_search_highlight_markers(SLACK_SEARCH_MESSAGES_METHOD, field, text)
}

#[derive(Deserialize)]
struct SlackSearchReactionWire {
    name: String,
    count: u32,
    #[serde(default)]
    users: Vec<String>,
}

impl SlackSearchReactionWire {
    fn into_reaction(self, self_user_id: &str) -> Result<SlackReaction, String> {
        require_nonempty_for(
            SLACK_MESSAGES_LIST_METHOD,
            "messages_data.messages.reaction.name",
            &self.name,
        )?;
        if self.count == 0 {
            return Err(format!(
                "Slack {SLACK_MESSAGES_LIST_METHOD} returned a zero-count reaction"
            ));
        }
        Ok(SlackReaction {
            emoji: self.name,
            count: self.count,
            active: self.users.iter().any(|user_id| user_id == self_user_id),
        })
    }
}

#[derive(Clone, Deserialize)]
struct SlackSearchChannelWire {
    id: String,
    name: String,
    is_channel: bool,
    is_group: bool,
    is_im: bool,
    is_mpim: bool,
    is_private: bool,
}

impl SlackSearchChannelWire {
    fn validate(&self) -> Result<(), String> {
        require_nonempty("channel.id", &self.id)?;
        require_nonempty("channel.name", &self.name)?;
        self.kind()?;
        Ok(())
    }

    fn kind(&self) -> Result<SlackConversationKind, String> {
        if self.is_im {
            return (self.is_private
                && !self.is_channel
                && !self.is_group
                && !self.is_mpim)
                .then_some(SlackConversationKind::DirectMessage)
                .ok_or_else(|| {
                    format!(
                        "Slack {SLACK_SEARCH_MESSAGES_METHOD} returned inconsistent direct-message channel flags"
                    )
                });
        }
        if self.is_mpim {
            return (self.is_private && self.is_channel && !self.is_group && !self.is_im)
                .then_some(SlackConversationKind::GroupMessage)
                .ok_or_else(|| {
                    format!(
                        "Slack {SLACK_SEARCH_MESSAGES_METHOD} returned inconsistent group-message channel flags"
                    )
                });
        }
        if self.is_channel == self.is_group {
            return Err(format!(
                "Slack {SLACK_SEARCH_MESSAGES_METHOD} returned inconsistent channel kind flags"
            ));
        }
        if self.is_private {
            Ok(SlackConversationKind::PrivateChannel)
        } else {
            Ok(SlackConversationKind::Channel)
        }
    }
}

#[derive(Deserialize)]
struct SlackSearchPaginationWire {
    total_count: u32,
    page: u32,
    page_count: u32,
}

impl SlackSearchPaginationWire {
    fn next_page(&self, requested_page: u32, items_empty: bool) -> Result<Option<u32>, String> {
        if self.page != requested_page {
            return Err(format!(
                "Slack {SLACK_SEARCH_MESSAGES_METHOD} returned page {} for requested page {requested_page}",
                self.page
            ));
        }
        if self.total_count == 0 {
            if !items_empty || self.page != 1 || self.page_count > 1 {
                return Err(format!(
                    "Slack {SLACK_SEARCH_MESSAGES_METHOD} returned inconsistent empty pagination"
                ));
            }
            return Ok(None);
        }
        if self.page == 0 || self.page_count == 0 || self.page > self.page_count {
            return Err(format!(
                "Slack {SLACK_SEARCH_MESSAGES_METHOD} returned malformed pagination"
            ));
        }
        Ok((self.page < self.page_count).then(|| self.page + 1))
    }
}
