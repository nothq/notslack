use crate::model::{
    SlackActivityAtUser, SlackActivityBotDmBundle, SlackActivityContent, SlackActivityDm,
    SlackActivityMessageReaction, SlackActivityThreadV2, SlackMessageTimestamp,
};
use serde_json::Value;

use super::{
    HydratedSlackActivityMessage, SlackActivityFeedContent, SlackActivityHydrationContext,
};
use crate::live::{
    api::messages::{SlackMessageReference, SLACK_MESSAGES_LIST_METHOD},
    payload::{
        slack_activity_actor_from_user_payload, slack_message_from_value_with_context_in_timezone,
    },
};

impl SlackActivityFeedContent {
    pub(super) fn message_reference(&self) -> Option<&SlackMessageReference> {
        match self {
            Self::MessageReaction { reference, .. }
            | Self::ThreadV2 { reference, .. }
            | Self::AtUser { reference, .. }
            | Self::Dm { reference }
            | Self::BotDmBundle { reference, .. } => Some(reference),
            Self::Unsupported { .. } => None,
        }
    }

    pub(super) fn into_activity_content(
        self,
        context: &mut SlackActivityHydrationContext<'_>,
    ) -> Result<SlackActivityContent, String> {
        match self {
            Self::MessageReaction {
                reference,
                reaction_name,
                reaction_user_id,
            } => activity_message_reaction(reference, reaction_name, reaction_user_id, context),
            Self::ThreadV2 {
                reference,
                thread_timestamp,
                unread_message_count,
            } => activity_thread(reference, thread_timestamp, unread_message_count, context),
            Self::AtUser {
                reference,
                author_user_id,
                broadcast,
                thread_timestamp,
            } => activity_at_user(
                reference,
                author_user_id,
                broadcast,
                thread_timestamp,
                context,
            ),
            Self::Dm { reference } => activity_dm(reference, context),
            Self::BotDmBundle {
                reference,
                unread_count,
            } => activity_bot_dm_bundle(reference, unread_count, context),
            Self::Unsupported { kind } => Ok(SlackActivityContent::Unsupported { kind }),
        }
    }
}

fn activity_message_reaction(
    reference: SlackMessageReference,
    reaction_name: String,
    reaction_user_id: String,
    context: &mut SlackActivityHydrationContext<'_>,
) -> Result<SlackActivityContent, String> {
    let HydratedSlackActivityMessage {
        message,
        thread_timestamp,
    } = hydrate_activity_message(
        &reference,
        ActivityThreadRootExpectation::Unspecified,
        context,
    )?;
    let actor = context
        .users
        .get(&reaction_user_id)
        .ok_or_else(|| format!("Slack Activity reaction actor {reaction_user_id} was not hydrated"))
        .and_then(|user| slack_activity_actor_from_user_payload(&reaction_user_id, user))?;
    Ok(SlackActivityContent::MessageReaction(
        SlackActivityMessageReaction {
            channel_id: reference.conversation_id().to_string(),
            message_timestamp: reference.timestamp().clone(),
            thread_timestamp,
            message,
            reaction_name,
            actor,
        },
    ))
}

fn activity_thread(
    reference: SlackMessageReference,
    expected_thread_timestamp: SlackMessageTimestamp,
    unread_message_count: u32,
    context: &mut SlackActivityHydrationContext<'_>,
) -> Result<SlackActivityContent, String> {
    let HydratedSlackActivityMessage {
        message,
        thread_timestamp,
    } = hydrate_activity_message(
        &reference,
        ActivityThreadRootExpectation::Exact(Some(&expected_thread_timestamp)),
        context,
    )?;
    Ok(SlackActivityContent::ThreadV2(SlackActivityThreadV2 {
        channel_id: reference.conversation_id().to_string(),
        latest_timestamp: reference.timestamp().clone(),
        thread_timestamp,
        message,
        unread_message_count,
    }))
}

fn activity_at_user(
    reference: SlackMessageReference,
    author_user_id: String,
    broadcast: bool,
    expected_thread_timestamp: Option<SlackMessageTimestamp>,
    context: &mut SlackActivityHydrationContext<'_>,
) -> Result<SlackActivityContent, String> {
    let HydratedSlackActivityMessage {
        message,
        thread_timestamp,
    } = hydrate_activity_message(
        &reference,
        ActivityThreadRootExpectation::Exact(expected_thread_timestamp.as_ref()),
        context,
    )?;
    if message.user_id.as_deref() != Some(author_user_id.as_str()) {
        return Err(format!(
            "Slack Activity mention author {author_user_id} did not match hydrated message author {:?}",
            message.user_id.as_deref(),
        ));
    }
    Ok(SlackActivityContent::AtUser(SlackActivityAtUser {
        channel_id: reference.conversation_id().to_string(),
        broadcast,
        message_timestamp: reference.timestamp().clone(),
        thread_timestamp,
        message,
    }))
}

fn activity_dm(
    reference: SlackMessageReference,
    context: &mut SlackActivityHydrationContext<'_>,
) -> Result<SlackActivityContent, String> {
    let HydratedSlackActivityMessage {
        message,
        thread_timestamp,
    } = hydrate_activity_message(
        &reference,
        ActivityThreadRootExpectation::Unspecified,
        context,
    )?;
    Ok(SlackActivityContent::Dm(SlackActivityDm {
        channel_id: reference.conversation_id().to_string(),
        latest_message_timestamp: reference.timestamp().clone(),
        thread_timestamp,
        message,
    }))
}

fn activity_bot_dm_bundle(
    reference: SlackMessageReference,
    unread_count: u32,
    context: &mut SlackActivityHydrationContext<'_>,
) -> Result<SlackActivityContent, String> {
    let HydratedSlackActivityMessage {
        message,
        thread_timestamp,
    } = hydrate_activity_message(
        &reference,
        ActivityThreadRootExpectation::Unspecified,
        context,
    )?;
    Ok(SlackActivityContent::BotDmBundle(
        SlackActivityBotDmBundle {
            channel_id: reference.conversation_id().to_string(),
            message_timestamp: reference.timestamp().clone(),
            thread_timestamp,
            message,
            unread_count,
        },
    ))
}

enum ActivityThreadRootExpectation<'a> {
    Unspecified,
    Exact(Option<&'a SlackMessageTimestamp>),
}

fn hydrate_activity_message(
    reference: &SlackMessageReference,
    thread_root_expectation: ActivityThreadRootExpectation<'_>,
    context: &mut SlackActivityHydrationContext<'_>,
) -> Result<HydratedSlackActivityMessage, String> {
    let raw_message = context.messages.take(reference)?;
    let thread_timestamp = raw_activity_thread_timestamp(&raw_message, reference.timestamp())?;
    if let ActivityThreadRootExpectation::Exact(expected) = thread_root_expectation {
        let expected = expected.filter(|timestamp| *timestamp != reference.timestamp());
        if thread_timestamp.as_ref() != expected {
            return Err(format!(
                "Slack {SLACK_MESSAGES_LIST_METHOD} returned thread root {:?} for Activity message {}:{} expecting {:?}",
                thread_timestamp.as_ref().map(SlackMessageTimestamp::as_str),
                reference.conversation_id(),
                reference.timestamp().as_str(),
                expected.map(SlackMessageTimestamp::as_str),
            ));
        }
    }
    let message = slack_message_from_value_with_context_in_timezone(
        raw_message,
        context.users,
        context.sidebar_snapshot,
        Some(context.self_user_id),
        context.timezone,
    );
    if message.id != reference.timestamp().as_str() {
        return Err(format!(
            "Slack {SLACK_MESSAGES_LIST_METHOD} message timestamp changed while decoding Activity message {}:{}",
            reference.conversation_id(),
            reference.timestamp().as_str(),
        ));
    }
    Ok(HydratedSlackActivityMessage {
        message,
        thread_timestamp,
    })
}

fn raw_activity_thread_timestamp(
    message: &Value,
    message_timestamp: &SlackMessageTimestamp,
) -> Result<Option<SlackMessageTimestamp>, String> {
    let Some(value) = message.get("thread_ts") else {
        return Ok(None);
    };
    let timestamp = value.as_str().ok_or_else(|| {
        format!("Slack {SLACK_MESSAGES_LIST_METHOD} returned a non-string thread_ts")
    })?;
    let timestamp = SlackMessageTimestamp::parse(timestamp).map_err(|error| {
        format!("Slack {SLACK_MESSAGES_LIST_METHOD} returned an invalid thread_ts: {error}")
    })?;
    Ok((timestamp != *message_timestamp).then_some(timestamp))
}
