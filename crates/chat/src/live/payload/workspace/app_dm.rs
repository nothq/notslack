use serde_json::Value;

use super::SlackLiveWorkspaceLoader;
use crate::live::payload::message::slack_view_rich_text_body;
use crate::live::payload::util::string_at;
use crate::model::{SlackAppDmDetails, SlackAppHome, SlackAppHomeView, SlackAppProfile};

const SLACK_CONVERSATIONS_INFO_METHOD: &str = "conversations.info";
const SLACK_APPS_PROFILE_GET_METHOD: &str = "apps.profile.get";
const SLACK_VIEWS_GET_METHOD: &str = "views.get";

impl SlackLiveWorkspaceLoader {
    /// Loads the App Home metadata and app profile for an app DM. Only
    /// read-only methods are called; nothing is dispatched to the app.
    pub fn load_app_dm_details(
        &self,
        conversation_id: &str,
        app_id: Option<&str>,
        bot_id: Option<&str>,
    ) -> Result<SlackAppDmDetails, String> {
        let conversation_id = conversation_id.trim();
        if conversation_id.is_empty() {
            return Err("Slack app DM conversation_id must not be empty".to_string());
        }
        let mut errors = Vec::new();
        let app_home = match self.api.post(
            SLACK_CONVERSATIONS_INFO_METHOD,
            &[
                ("channel", conversation_id.to_string()),
                ("return_app_home", "true".to_string()),
            ],
        ) {
            Ok(response) => slack_app_home_from_conversation_info(&response),
            Err(error) => {
                errors.push(error);
                None
            }
        };
        let app_id = app_home
            .as_ref()
            .and_then(|home| home.app_id.as_deref())
            .or(app_id)
            .map(str::trim)
            .filter(|id| !id.is_empty());
        let bot_id = bot_id.map(str::trim).filter(|id| !id.is_empty());
        let profile_params = match (app_id, bot_id) {
            (Some(app_id), _) => Some(("app", app_id.to_string())),
            (None, Some(bot_id)) => Some(("bot", bot_id.to_string())),
            (None, None) => None,
        };
        let profile = match profile_params {
            Some(param) => match self.api.post(SLACK_APPS_PROFILE_GET_METHOD, &[param]) {
                Ok(response) => {
                    let profile = slack_app_profile_from_response(&response);
                    if profile.is_none() {
                        errors.push(format!(
                            "Slack {SLACK_APPS_PROFILE_GET_METHOD} response omitted app_profile"
                        ));
                    }
                    profile
                }
                Err(error) => {
                    errors.push(error);
                    None
                }
            },
            None => {
                errors.push("Slack app DM has no app or bot id".to_string());
                None
            }
        };
        if app_home.is_none() && profile.is_none() {
            return Err(errors.join("; "));
        }
        Ok(SlackAppDmDetails {
            app_home,
            profile,
            error: (!errors.is_empty()).then(|| errors.join("; ")),
        })
    }

    pub fn load_app_home_view(&self, view_id: &str) -> Result<SlackAppHomeView, String> {
        let view_id = view_id.trim();
        if view_id.is_empty() {
            return Err("Slack App Home view_id must not be empty".to_string());
        }
        let response = self
            .api
            .post(SLACK_VIEWS_GET_METHOD, &[("view_id", view_id.to_string())])?;
        slack_app_home_view_from_response(view_id, &response)
    }
}

pub(super) fn slack_app_home_from_conversation_info(response: &Value) -> Option<SlackAppHome> {
    let home = response
        .get("app_home")
        .or_else(|| response.get("channel")?.get("app_home"))?;
    if !home.is_object() {
        return None;
    }
    Some(SlackAppHome {
        app_id: string_at(home, &["app_id"]),
        home_tab_enabled: home
            .get("home_tab_enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        home_view_id: string_at(home, &["home_view_id"]),
        messages_tab_enabled: home.get("messages_tab_enabled").and_then(Value::as_bool),
    })
}

pub(super) fn slack_app_profile_from_response(response: &Value) -> Option<SlackAppProfile> {
    let profile = response.get("app_profile")?;
    if !profile.is_object() {
        return None;
    }
    let icon_url = [
        "image_512",
        "image_192",
        "image_96",
        "image_72",
        "image_64",
        "image_48",
        "image_36",
        "image_32",
    ]
    .into_iter()
    .find_map(|size| string_at(profile, &["icons", size]))
    .filter(|url| url.starts_with("https://") || url.starts_with("http://"));
    let url = string_at(profile, &["url"])
        .filter(|url| url.starts_with("https://") || url.starts_with("http://"));
    let long_desc = profile
        .get("long_desc")
        .and_then(Value::as_str)
        .map(|text| text.replace("\r\n", "\n").trim().to_string())
        .filter(|text| !text.is_empty());
    Some(SlackAppProfile {
        id: string_at(profile, &["id"]),
        name: string_at(profile, &["name"]),
        desc: string_at(profile, &["desc"]),
        long_desc,
        icon_url,
        developer_name: string_at(profile, &["developer_name"]),
        url,
        home_tab_enabled: profile
            .get("app_home_metadata")
            .and_then(|metadata| metadata.get("home_tab_enabled"))
            .and_then(Value::as_bool),
    })
}

pub(super) fn slack_app_home_view_from_response(
    view_id: &str,
    response: &Value,
) -> Result<SlackAppHomeView, String> {
    let view = response
        .get("view")
        .filter(|view| view.is_object())
        .ok_or_else(|| format!("Slack {SLACK_VIEWS_GET_METHOD} response omitted view"))?;
    let body = view
        .get("blocks")
        .and_then(Value::as_array)
        .and_then(|blocks| slack_view_rich_text_body(blocks));
    Ok(SlackAppHomeView {
        view_id: string_at(view, &["id"]).unwrap_or_else(|| view_id.to_string()),
        body,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::model::SlackBlockKitBlock;

    #[test]
    fn dm_peer_bot_user_is_detected_as_app() {
        use crate::live::payload::workspace::build::slack_dm_app_from_user;

        let bot = json!({
            "id": "U0BOT",
            "is_bot": true,
            "profile": {
                "real_name": "Google Calendar",
                "bot_id": "B123",
                "api_app_id": "A123",
                "image_192": "https://example.com/bot.png"
            }
        });
        let app = slack_dm_app_from_user("U0BOT", &bot, "Google Calendar").unwrap();
        assert_eq!(app.user_id, "U0BOT");
        assert_eq!(app.app_id.as_deref(), Some("A123"));
        assert_eq!(app.bot_id.as_deref(), Some("B123"));
        assert_eq!(app.name, "Google Calendar");

        let person = json!({"id": "U1", "profile": {"real_name": "Ada"}});
        assert_eq!(slack_dm_app_from_user("U1", &person, "Ada"), None);
    }

    #[test]
    fn app_home_decodes_from_conversation_info() {
        let response = json!({
            "ok": true,
            "channel": {"id": "D1", "is_im": true},
            "app_home": {
                "app_id": "A123",
                "home_tab_enabled": true,
                "home_view_id": "V999",
                "messages_tab_enabled": true,
                "app_installed_team_id": "T1"
            }
        });
        assert_eq!(
            slack_app_home_from_conversation_info(&response),
            Some(SlackAppHome {
                app_id: Some("A123".to_string()),
                home_tab_enabled: true,
                home_view_id: Some("V999".to_string()),
                messages_tab_enabled: Some(true),
            })
        );
    }

    #[test]
    fn app_home_tolerates_missing_fields() {
        let response = json!({"ok": true, "app_home": {"app_id": "A1"}});
        let home = slack_app_home_from_conversation_info(&response).unwrap();
        assert_eq!(home.app_id.as_deref(), Some("A1"));
        assert!(!home.home_tab_enabled);
        assert_eq!(home.home_view_id, None);
        assert_eq!(home.messages_tab_enabled, None);
        assert_eq!(
            slack_app_home_from_conversation_info(&json!({"ok": true, "channel": {}})),
            None
        );
    }

    #[test]
    fn app_profile_decodes_icon_text_and_links() {
        let response = json!({
            "ok": true,
            "app_profile": {
                "id": "A123",
                "name": "Google Calendar",
                "desc": "Stay on schedule",
                "long_desc": "Line one\r\n\r\nLine two\n",
                "icons": {
                    "image_32": "https://example.com/32.png",
                    "image_192": "https://example.com/192.png",
                    "image_512": ""
                },
                "developer_name": "Slack Technologies",
                "url": "https://example.com/apps/A123",
                "app_home_metadata": {"home_tab_enabled": true, "messages_tab_enabled": true},
                "bot_user": {"id": "U1"}
            }
        });
        assert_eq!(
            slack_app_profile_from_response(&response),
            Some(SlackAppProfile {
                id: Some("A123".to_string()),
                name: Some("Google Calendar".to_string()),
                desc: Some("Stay on schedule".to_string()),
                long_desc: Some("Line one\n\nLine two".to_string()),
                icon_url: Some("https://example.com/192.png".to_string()),
                developer_name: Some("Slack Technologies".to_string()),
                url: Some("https://example.com/apps/A123".to_string()),
                home_tab_enabled: Some(true),
            })
        );
    }

    #[test]
    fn app_profile_rejects_non_http_url_and_missing_profile() {
        let response = json!({
            "ok": true,
            "app_profile": {"name": "Bot", "url": "javascript:alert(1)"}
        });
        let profile = slack_app_profile_from_response(&response).unwrap();
        assert_eq!(profile.url, None);
        assert_eq!(profile.icon_url, None);
        assert_eq!(slack_app_profile_from_response(&json!({"ok": true})), None);
    }

    #[test]
    fn app_home_view_decodes_block_kit_blocks() {
        let response = json!({
            "ok": true,
            "view": {
                "id": "V999",
                "type": "home",
                "blocks": [
                    {"type": "header", "text": {"type": "plain_text", "text": "Today"}},
                    {"type": "divider"},
                    {"type": "section", "text": {"type": "mrkdwn", "text": "*Standup* at 10:00"}},
                    {"type": "input", "label": {"type": "plain_text", "text": "Unsupported"}}
                ]
            }
        });
        let view = slack_app_home_view_from_response("V999", &response).unwrap();
        assert_eq!(view.view_id, "V999");
        let body = view.body.expect("home view should have a body");
        assert_eq!(body.block_kit.len(), 3);
        assert!(matches!(
            body.block_kit[0],
            SlackBlockKitBlock::Header { .. }
        ));
        assert!(matches!(body.block_kit[1], SlackBlockKitBlock::Divider));
        assert!(matches!(
            body.block_kit[2],
            SlackBlockKitBlock::Section { .. }
        ));
    }

    #[test]
    fn app_home_view_without_blocks_has_no_body() {
        let view =
            slack_app_home_view_from_response("V1", &json!({"ok": true, "view": {"blocks": []}}))
                .unwrap();
        assert_eq!(view.view_id, "V1");
        assert_eq!(view.body, None);
        assert!(slack_app_home_view_from_response("V1", &json!({"ok": true})).is_err());
    }
}
