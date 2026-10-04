use std::collections::HashSet;

use serde::Deserialize;

use crate::model::{SlackMessageTimestamp, SlackQuickSearchHighlight};

use super::super::{SlackQuickSearchMessageReference, SlackQuickSearchPaginationWire};
use super::{
    permalink::validate_slack_quick_search_permalink, SLACK_QUICK_SEARCH_EXTRACT_MAX_BYTES,
    SLACK_QUICK_SEARCH_MESSAGE_LIMIT, SLACK_SEARCH_INLINE_METHOD,
};
use crate::live::api::search::{
    require_nonempty_for, require_slack_timestamp_for, wire::parse_search_highlighted_text_ranges,
};

#[derive(Deserialize)]
pub(super) struct SlackQuickSearchInlineResponseWire {
    ok: bool,
    query: String,
    items: Vec<SlackQuickSearchInlineItemWire>,
    pagination: SlackQuickSearchPaginationWire,
}

#[derive(Deserialize)]
struct SlackQuickSearchInlineItemWire {
    ts: String,
    extracts: Vec<SlackQuickSearchInlineExtractWire>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    bot_id: Option<String>,
    #[serde(default)]
    thread_ts: Option<String>,
    channel_id: String,
    permalink: String,
    iid: String,
}

#[derive(Deserialize)]
struct SlackQuickSearchInlineExtractWire {
    text: String,
    truncated_head: bool,
    truncated_tail: bool,
}

struct SlackQuickSearchAuthorIds {
    user_id: Option<String>,
    bot_id: Option<String>,
}

struct SlackQuickSearchTimestamps {
    message: SlackMessageTimestamp,
    thread: Option<SlackMessageTimestamp>,
}

struct SlackQuickSearchExcerpt {
    text: String,
    highlights: Vec<SlackQuickSearchHighlight>,
}

impl SlackQuickSearchInlineResponseWire {
    pub(super) fn validate(&self, expected_query: &str) -> Result<(), String> {
        if !self.ok {
            return Err(format!(
                "Slack {SLACK_SEARCH_INLINE_METHOD} returned ok=false"
            ));
        }
        require_nonempty_for(SLACK_SEARCH_INLINE_METHOD, "query", &self.query)?;
        if self.query != expected_query {
            return Err(format!(
                "Slack {SLACK_SEARCH_INLINE_METHOD} returned a mismatched query"
            ));
        }
        if self.items.len() > SLACK_QUICK_SEARCH_MESSAGE_LIMIT {
            return Err(format!(
                "Slack {SLACK_SEARCH_INLINE_METHOD} returned more than {SLACK_QUICK_SEARCH_MESSAGE_LIMIT} items"
            ));
        }
        let mut item_ids = HashSet::with_capacity(self.items.len());
        for item in &self.items {
            require_nonempty_for(SLACK_SEARCH_INLINE_METHOD, "item.iid", &item.iid)?;
            if !item_ids.insert(item.iid.as_str()) {
                return Err(format!(
                    "Slack {SLACK_SEARCH_INLINE_METHOD} returned duplicate item.iid"
                ));
            }
        }
        self.pagination
            .validate(SLACK_SEARCH_INLINE_METHOD, self.items.len())
    }

    pub(super) fn into_references(
        self,
        expected_conversation_id: Option<&str>,
    ) -> Result<Vec<SlackQuickSearchMessageReference>, String> {
        self.items
            .into_iter()
            .map(SlackQuickSearchInlineItemWire::into_reference)
            .collect::<Result<Vec<_>, _>>()
            .map(|references| {
                references
                    .into_iter()
                    .filter(|reference| {
                        expected_conversation_id.is_none_or(|conversation_id| {
                            reference.conversation_id == conversation_id
                        })
                    })
                    .collect()
            })
    }
}

impl SlackQuickSearchInlineItemWire {
    fn into_reference(self) -> Result<SlackQuickSearchMessageReference, String> {
        require_nonempty_for(SLACK_SEARCH_INLINE_METHOD, "item.iid", &self.iid)?;
        require_nonempty_for(
            SLACK_SEARCH_INLINE_METHOD,
            "item.channel_id",
            &self.channel_id,
        )?;
        let author = SlackQuickSearchAuthorIds::parse(self.user, self.bot_id)?;
        let timestamps = SlackQuickSearchTimestamps::parse(self.ts, self.thread_ts)?;
        validate_slack_quick_search_permalink(
            &self.permalink,
            &self.channel_id,
            &timestamps.message,
            timestamps.thread.as_ref(),
        )?;
        let excerpt = SlackQuickSearchExcerpt::parse(self.extracts)?;
        Ok(SlackQuickSearchMessageReference {
            id: self.iid,
            conversation_id: self.channel_id,
            user_id: author.user_id,
            bot_id: author.bot_id,
            timestamp: timestamps.message,
            thread_timestamp: timestamps.thread,
            excerpt: excerpt.text,
            highlights: excerpt.highlights,
        })
    }
}

impl SlackQuickSearchAuthorIds {
    fn parse(user_id: Option<String>, bot_id: Option<String>) -> Result<Self, String> {
        let user_id = validated_optional_id("item.user", user_id)?;
        let bot_id = validated_optional_id("item.bot_id", bot_id)?;
        if user_id.is_none() && bot_id.is_none() {
            return Err(format!(
                "Slack {SLACK_SEARCH_INLINE_METHOD} returned an item without a user or bot"
            ));
        }
        Ok(Self { user_id, bot_id })
    }
}

impl SlackQuickSearchTimestamps {
    fn parse(message: String, thread: Option<String>) -> Result<Self, String> {
        require_slack_timestamp_for(SLACK_SEARCH_INLINE_METHOD, "item.ts", &message)?;
        let message = SlackMessageTimestamp::parse(&message)?;
        let thread = thread
            .map(|timestamp| {
                require_slack_timestamp_for(
                    SLACK_SEARCH_INLINE_METHOD,
                    "item.thread_ts",
                    &timestamp,
                )?;
                SlackMessageTimestamp::parse(&timestamp)
            })
            .transpose()?;
        Ok(Self { message, thread })
    }
}

impl SlackQuickSearchExcerpt {
    fn parse(extracts: Vec<SlackQuickSearchInlineExtractWire>) -> Result<Self, String> {
        if extracts.len() != 1 {
            return Err(format!(
                "Slack {SLACK_SEARCH_INLINE_METHOD} returned an item without exactly one extract"
            ));
        }
        let extract = extracts
            .into_iter()
            .next()
            .expect("validated Slack quick-search extract must exist");
        let mut highlighted = parse_search_highlighted_text_ranges(
            SLACK_SEARCH_INLINE_METHOD,
            "item.extracts.text",
            extract.text,
        )?;
        require_nonempty_for(
            SLACK_SEARCH_INLINE_METHOD,
            "item.extracts.text",
            &highlighted.text,
        )?;
        if highlighted.text.len() > SLACK_QUICK_SEARCH_EXTRACT_MAX_BYTES {
            return Err(format!(
                "Slack {SLACK_SEARCH_INLINE_METHOD} returned an oversized item.extracts.text"
            ));
        }
        if extract.truncated_head {
            highlighted.text.insert(0, '…');
            let prefix_bytes = '…'.len_utf8();
            for range in &mut highlighted.ranges {
                range.start += prefix_bytes;
                range.end += prefix_bytes;
            }
        }
        if extract.truncated_tail {
            highlighted.text.push('…');
        }
        Ok(Self {
            text: highlighted.text,
            highlights: highlighted
                .ranges
                .into_iter()
                .map(|range| SlackQuickSearchHighlight {
                    start: range.start,
                    end: range.end,
                })
                .collect(),
        })
    }
}

fn validated_optional_id(field: &str, value: Option<String>) -> Result<Option<String>, String> {
    value
        .map(|value| {
            require_nonempty_for(SLACK_SEARCH_INLINE_METHOD, field, &value)?;
            Ok(value)
        })
        .transpose()
}
