use gpui::SharedString;
use time::{macros::format_description, OffsetDateTime, UtcOffset};

use crate::ui::{SlackChannelDetailsSnapshot, SlackProfile};

pub(crate) type SlackChannelCreatorLoadKey = (String, String, String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedSlackChannelDetails {
    pub(crate) team_id: SharedString,
    pub(crate) conversation_id: SharedString,
    pub(crate) name: SharedString,
    pub(crate) topic: Option<SharedString>,
    pub(crate) description: Option<SharedString>,
    pub(crate) creation: Option<PreparedSlackChannelCreation>,
    pub(crate) creator_user_id: Option<SharedString>,
    pub(crate) creator_needs_hydration: bool,
    pub(crate) created_date_label: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedSlackChannelCreation {
    pub(crate) row_label: &'static str,
    pub(crate) value: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackChannelDetailsLoadRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackChannelCreatorLoadRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) user_id: String,
    pub(crate) created_date_label: Option<SharedString>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreparedSlackChannelCreatorHydration {
    pub(crate) user_id: SharedString,
    pub(crate) creation: PreparedSlackChannelCreation,
}

pub(crate) fn prepare_slack_channel_details(
    snapshot: SlackChannelDetailsSnapshot,
) -> Result<PreparedSlackChannelDetails, String> {
    let creator_user_id = snapshot
        .creator
        .as_ref()
        .map(|creator| SharedString::from(creator.user_id.clone()));
    let creator_needs_hydration = snapshot.creator.as_ref().is_some_and(|creator| {
        creator.display_name.is_none() && creator.is_deactivated != Some(true)
    });
    let creator_label = snapshot.creator.as_ref().map(|creator| {
        if creator.is_deactivated == Some(true) {
            SharedString::from("Deactivated User")
        } else {
            creator.display_name.as_ref().map_or_else(
                || SharedString::from(creator.user_id.clone()),
                SharedString::from,
            )
        }
    });
    let created_date_label = snapshot
        .created_at_unix_seconds
        .map(format_slack_channel_created_date)
        .transpose()?
        .map(SharedString::from);
    let creation = prepare_slack_channel_creation(creator_label, created_date_label.clone());
    Ok(PreparedSlackChannelDetails {
        team_id: snapshot.team_id.into(),
        conversation_id: snapshot.conversation_id.into(),
        name: format!("# {}", snapshot.name).into(),
        topic: snapshot.text.topic.map(SharedString::from),
        description: snapshot.text.purpose.map(SharedString::from),
        creation,
        creator_user_id,
        creator_needs_hydration,
        created_date_label,
    })
}

pub(crate) fn prepare_slack_channel_creator_hydration(
    expected_user_id: &str,
    profile: SlackProfile,
    created_date_label: Option<SharedString>,
) -> Result<PreparedSlackChannelCreatorHydration, String> {
    if profile.user_id != expected_user_id {
        return Err(format!(
            "Slack returned profile {} for channel creator {expected_user_id}",
            profile.user_id
        ));
    }
    let display_name = profile.display_name.trim();
    if display_name.is_empty() {
        return Err(format!(
            "Slack returned an empty display name for channel creator {expected_user_id}"
        ));
    }
    let creator_label = SharedString::from(display_name.to_string());
    let creation = PreparedSlackChannelCreation {
        row_label: "Created by",
        value: created_date_label.as_ref().map_or_else(
            || creator_label.clone(),
            |date| format!("{} on {}", creator_label.as_ref(), date.as_ref()).into(),
        ),
    };
    Ok(PreparedSlackChannelCreatorHydration {
        user_id: profile.user_id.into(),
        creation,
    })
}

fn prepare_slack_channel_creation(
    creator_label: Option<SharedString>,
    created_date_label: Option<SharedString>,
) -> Option<PreparedSlackChannelCreation> {
    match (creator_label, created_date_label) {
        (Some(creator), Some(date)) => Some(PreparedSlackChannelCreation {
            row_label: "Created by",
            value: format!("{} on {}", creator.as_ref(), date.as_ref()).into(),
        }),
        (Some(creator), None) => Some(PreparedSlackChannelCreation {
            row_label: "Created by",
            value: creator,
        }),
        (None, Some(date)) => Some(PreparedSlackChannelCreation {
            row_label: "Created",
            value: date,
        }),
        (None, None) => None,
    }
}

fn format_slack_channel_created_date(unix_seconds: i64) -> Result<String, String> {
    let created_at = OffsetDateTime::from_unix_timestamp(unix_seconds)
        .map_err(|error| format!("Slack channel created timestamp was invalid: {error}"))?;
    created_at
        .to_offset(UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC))
        .format(format_description!(
            "[month repr:long] [day padding:none], [year]"
        ))
        .map_err(|error| format!("failed to format Slack channel created date: {error}"))
}
