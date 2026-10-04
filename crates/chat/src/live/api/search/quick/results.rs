use serde::Deserialize;

use crate::live::payload::{
    select_slack_avatar_image_url, SlackAvatarImageUrls, SlackAvatarPurpose,
};
use crate::model::{SlackConversationKind, SlackQuickSearchConversation, SlackQuickSearchPerson};

use super::{
    require_nonempty_for, require_slack_timestamp_for, strip_search_highlight_markers,
    SLACK_SEARCH_DMS_METHOD, SLACK_SEARCH_PEOPLE_METHOD,
};

#[derive(Deserialize)]
pub(super) struct SlackQuickSearchPersonWire {
    id: String,
    iid: String,
    team_id: String,
    username: String,
    profile: SlackQuickSearchProfileWire,
}

#[derive(Deserialize)]
struct SlackQuickSearchProfileWire {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    real_name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    status_text: Option<String>,
    #[serde(default)]
    image_24: Option<String>,
    #[serde(default)]
    image_32: Option<String>,
    #[serde(default)]
    image_48: Option<String>,
    #[serde(default)]
    image_72: Option<String>,
    #[serde(default)]
    image_192: Option<String>,
    #[serde(default)]
    image_512: Option<String>,
}

impl SlackQuickSearchPersonWire {
    pub(super) fn into_person(self) -> Result<SlackQuickSearchPerson, String> {
        require_nonempty_for(SLACK_SEARCH_PEOPLE_METHOD, "item.id", &self.id)?;
        require_nonempty_for(SLACK_SEARCH_PEOPLE_METHOD, "item.iid", &self.iid)?;
        require_nonempty_for(SLACK_SEARCH_PEOPLE_METHOD, "item.team_id", &self.team_id)?;
        require_nonempty_for(SLACK_SEARCH_PEOPLE_METHOD, "item.username", &self.username)?;
        let profile = self.profile;
        let display_name = parse_optional_highlighted(
            SLACK_SEARCH_PEOPLE_METHOD,
            "item.profile.display_name",
            profile.display_name,
        )?;
        let real_name = parse_optional_highlighted(
            SLACK_SEARCH_PEOPLE_METHOD,
            "item.profile.real_name",
            profile.real_name,
        )?;
        let label = display_name
            .as_deref()
            .or(real_name.as_deref())
            .unwrap_or(&self.username)
            .to_string();
        let title = parse_optional_highlighted(
            SLACK_SEARCH_PEOPLE_METHOD,
            "item.profile.title",
            profile.title,
        )?
        .unwrap_or_default();
        let status_text = parse_optional_highlighted(
            SLACK_SEARCH_PEOPLE_METHOD,
            "item.profile.status_text",
            profile.status_text,
        )?
        .unwrap_or_default();
        let avatar_image_url = select_slack_avatar_image_url(
            SlackAvatarPurpose::Message,
            SlackAvatarImageUrls {
                image_24: profile.image_24.as_deref(),
                image_32: profile.image_32.as_deref(),
                image_48: profile.image_48.as_deref(),
                image_72: profile.image_72.as_deref(),
                image_192: profile.image_192.as_deref(),
                image_512: profile.image_512.as_deref(),
            },
        );
        Ok(SlackQuickSearchPerson {
            id: self.id,
            team_id: self.team_id,
            username: self.username,
            label,
            real_name: real_name.unwrap_or_default(),
            title,
            status_text,
            avatar_image_url,
        })
    }
}

#[derive(Deserialize)]
pub(super) struct SlackQuickSearchDmWire {
    channel: SlackQuickSearchDmChannelWire,
    message: SlackQuickSearchDmMessageWire,
}

#[derive(Deserialize)]
struct SlackQuickSearchDmChannelWire {
    id: String,
    context_team_id: String,
    is_im: bool,
    is_mpim: bool,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    members: Vec<String>,
    channel_name_highlighted: String,
}

#[derive(Deserialize)]
struct SlackQuickSearchDmMessageWire {
    ts: String,
}

impl SlackQuickSearchDmWire {
    pub(super) fn into_direct_message(self) -> Result<SlackQuickSearchConversation, String> {
        let channel = self.channel;
        require_nonempty_for(SLACK_SEARCH_DMS_METHOD, "item.channel.id", &channel.id)?;
        require_nonempty_for(
            SLACK_SEARCH_DMS_METHOD,
            "item.channel.context_team_id",
            &channel.context_team_id,
        )?;
        require_slack_timestamp_for(SLACK_SEARCH_DMS_METHOD, "item.message.ts", &self.message.ts)?;
        let kind = match (channel.is_im, channel.is_mpim) {
            (true, false) => SlackConversationKind::DirectMessage,
            (false, true) => SlackConversationKind::GroupMessage,
            _ => {
                return Err(format!(
                    "Slack {SLACK_SEARCH_DMS_METHOD} returned an invalid direct-message kind"
                ));
            }
        };
        if kind == SlackConversationKind::DirectMessage {
            require_nonempty_for(
                SLACK_SEARCH_DMS_METHOD,
                "item.channel.user",
                channel.user.as_deref().unwrap_or_default(),
            )?;
        }
        let label = strip_search_highlight_markers(
            SLACK_SEARCH_DMS_METHOD,
            "item.channel.channel_name_highlighted",
            channel.channel_name_highlighted,
        )?;
        require_nonempty_for(
            SLACK_SEARCH_DMS_METHOD,
            "item.channel.channel_name_highlighted",
            &label,
        )?;
        let member_count = u32::try_from(channel.members.len())
            .map_err(|_| format!("Slack {SLACK_SEARCH_DMS_METHOD} returned too many members"))?;
        Ok(SlackQuickSearchConversation {
            id: channel.id,
            team_id: channel.context_team_id,
            kind,
            label,
            is_member: true,
            user_id: channel.user,
            member_user_ids: channel.members,
            member_count: (member_count > 0).then_some(member_count),
        })
    }
}

fn parse_optional_highlighted(
    method: &str,
    field: &str,
    value: Option<String>,
) -> Result<Option<String>, String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| strip_search_highlight_markers(method, field, value))
        .transpose()
}
