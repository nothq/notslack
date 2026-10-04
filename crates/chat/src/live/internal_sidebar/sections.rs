use serde::Deserialize;

use crate::model::SlackSidebarSectionCreateRequest;

use super::SlackInternalSidebarClient;

const USERS_CHANNEL_SECTIONS_LIST: &str = "users.channelSections.list";
const USERS_CHANNEL_SECTIONS_CREATE: &str = "users.channelSections.create";

#[derive(Debug, Deserialize)]
pub(super) struct SlackChannelSectionsResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    pub(super) channel_sections: Vec<SlackChannelSection>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SlackChannelSection {
    pub(super) channel_section_id: String,
    #[serde(default)]
    pub(super) name: Option<String>,
    #[serde(rename = "type")]
    pub(super) kind: String,
    #[serde(default)]
    pub(super) channel_ids_page: SlackChannelIdsPage,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct SlackChannelIdsPage {
    #[serde(default)]
    pub(super) channel_ids: Vec<String>,
}

pub(super) struct SlackChannelSectionsClient<'a> {
    inner: &'a SlackInternalSidebarClient,
}

impl<'a> SlackChannelSectionsClient<'a> {
    pub(super) fn new(inner: &'a SlackInternalSidebarClient) -> Self {
        Self { inner }
    }

    pub(super) fn load(&self) -> Result<SlackChannelSectionsResponse, String> {
        let body = self.inner.post_internal_method(
            USERS_CHANNEL_SECTIONS_LIST,
            vec![
                ("_x_reason", "conditional-fetch-manager".to_string()),
                ("_x_mode", "online".to_string()),
                ("_x_sonic", "true".to_string()),
                ("_x_app_name", "client".to_string()),
            ],
        )?;
        decode_channel_sections(&body)
    }

    pub(super) fn create(&self, request: &SlackSidebarSectionCreateRequest) -> Result<(), String> {
        let channels = serde_json::to_string(request.conversation_ids()).map_err(|error| {
            format!("failed to encode Slack sidebar section conversations: {error}")
        })?;
        let body = self.inner.post_internal_method(
            USERS_CHANNEL_SECTIONS_CREATE,
            vec![
                ("name", request.name().to_string()),
                ("emoji", ":bookmark_tabs:".to_string()),
                ("channels", channels),
                ("_x_reason", "create-channel-section".to_string()),
                ("_x_mode", "online".to_string()),
                ("_x_sonic", "true".to_string()),
                ("_x_app_name", "client".to_string()),
            ],
        )?;
        let response =
            serde_json::from_str::<SlackCreateChannelSectionResponse>(&body).map_err(|error| {
                format!("failed to decode Slack {USERS_CHANNEL_SECTIONS_CREATE} response: {error}")
            })?;
        if !response.ok {
            return Err(format!(
                "Slack {USERS_CHANNEL_SECTIONS_CREATE} failed: {}",
                response
                    .error
                    .unwrap_or_else(|| "unknown_error".to_string())
            ));
        }
        Ok(())
    }
}

#[derive(Deserialize)]
struct SlackCreateChannelSectionResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
}

fn decode_channel_sections(body: &str) -> Result<SlackChannelSectionsResponse, String> {
    let response = serde_json::from_str::<SlackChannelSectionsResponse>(body).map_err(|error| {
        format!("failed to decode Slack internal sidebar API response: {error}")
    })?;
    if !response.ok {
        return Err(format!(
            "Slack internal sidebar API failed: {}",
            response
                .error
                .unwrap_or_else(|| "unknown_error".to_string())
        ));
    }
    if response.channel_sections.is_empty() {
        return Err("Slack internal sidebar API returned no channel sections".to_string());
    }
    Ok(response)
}
