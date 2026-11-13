//! Custom emoji and emoji packs, through the methods Slack's own web client
//! uses for its Add emoji dialog.

use reqwest::blocking::multipart::{Form, Part};
use serde_json::Value;

use crate::model::{SlackCustomEmojiUpload, SlackEmojiPack, SlackEmojiPackEmoji};

use super::{http::slack_api_payload_from_response, SlackApiAuth, SlackApiClient};

const SLACK_EMOJI_PACKS_METHOD: &str = "emoji.collections.list";
const SLACK_ADD_EMOJI_PACK_METHOD: &str = "emoji.addCollection";
const SLACK_REMOVE_EMOJI_PACK_METHOD: &str = "emoji.removeCollection";
const SLACK_ADD_EMOJI_METHOD: &str = "emoji.add";

impl SlackApiClient {
    pub(crate) fn load_emoji_packs(&self) -> Result<Vec<SlackEmojiPack>, String> {
        let payload = self.post(
            SLACK_EMOJI_PACKS_METHOD,
            &[("installed_only", "0".to_string())],
        )?;
        parse_emoji_packs(&payload)
    }

    pub(crate) fn set_emoji_pack_installed(
        &self,
        pack_id: &str,
        installed: bool,
    ) -> Result<(), String> {
        let method = if installed {
            SLACK_ADD_EMOJI_PACK_METHOD
        } else {
            SLACK_REMOVE_EMOJI_PACK_METHOD
        };
        match self.post(method, &[("collection_id", pack_id.to_string())]) {
            Ok(_) => Ok(()),
            // Slack's client treats adding a pack that is already in as done.
            Err(error) if installed && error.ends_with(": already_installed") => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub(crate) fn add_custom_emoji(&self, upload: SlackCustomEmojiUpload) -> Result<(), String> {
        let SlackApiAuth::Desktop {
            xoxc_token,
            cookie_header,
            web_build_timestamp,
            ..
        } = &self.auth
        else {
            return Err(format!(
                "Slack API request {SLACK_ADD_EMOJI_METHOD} requires authenticated Slack credentials"
            ));
        };
        let image = Part::bytes(upload.image)
            .file_name(upload.file_name)
            .mime_str(&upload.mimetype)
            .map_err(|error| format!("Slack emoji image has an invalid type: {error}"))?;
        let form = Form::new()
            .text("token", xoxc_token.clone())
            .text("_x_version_ts", web_build_timestamp.as_str().to_string())
            .text("name", upload.name)
            .text("mode", "data")
            .part("image", image);
        let response = self
            .http
            .post(format!("{}/{SLACK_ADD_EMOJI_METHOD}", self.base_url))
            .header(reqwest::header::COOKIE, cookie_header.clone())
            .multipart(form)
            .send()
            .map_err(|error| {
                format!("Slack API request {SLACK_ADD_EMOJI_METHOD} failed: {error}")
            })?;
        let status = response.status().as_u16();
        let body = response.text().map_err(|error| {
            format!("failed to read Slack API response for {SLACK_ADD_EMOJI_METHOD}: {error}")
        })?;
        slack_api_payload_from_response(SLACK_ADD_EMOJI_METHOD, status, None, &body)
            .map(|_| ())
            .map_err(|error| error.into_message())
    }
}

/// Slack lists every pack under `available` and the workspace's own under
/// `installed`, matched by id.
fn parse_emoji_packs(payload: &Value) -> Result<Vec<SlackEmojiPack>, String> {
    let available = payload
        .get("available")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("Slack {SLACK_EMOJI_PACKS_METHOD} returned no available packs"))?;
    let installed = payload
        .get("installed")
        .and_then(Value::as_array)
        .map(|packs| {
            packs
                .iter()
                .filter_map(|pack| pack.get("id").and_then(Value::as_str))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(available
        .iter()
        .filter_map(|pack| parse_emoji_pack(pack, &installed))
        .collect())
}

/// Packs Slack describes incompletely are left out rather than failing the
/// whole list.
fn parse_emoji_pack(pack: &Value, installed: &[&str]) -> Option<SlackEmojiPack> {
    let text = |key: &str| pack.get(key).and_then(Value::as_str).map(str::to_string);
    let emoji = pack
        .get("emoji")
        .and_then(Value::as_object)?
        .iter()
        .filter_map(|(name, url)| {
            Some(SlackEmojiPackEmoji {
                name: name.clone(),
                url: url.as_str()?.to_string(),
            })
        })
        .collect();
    let id = text("id")?;
    Some(SlackEmojiPack {
        installed: installed.contains(&id.as_str()),
        id,
        name: text("name")?,
        author: text("author").unwrap_or_default(),
        emoji,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::parse_emoji_packs;

    #[test]
    fn packs_keep_their_emoji_in_slack_order_and_skip_incomplete_ones() {
        let payload = json!({
            "ok": true,
            "available": [
                {
                    "id": "EC1",
                    "name": "Alphabet",
                    "author": "Rebeca Anaya",
                    "emoji": {"alphabet-a": "https://e/a.png", "alphabet-at": "https://e/at.png"}
                },
                {"id": "EC2", "name": "Gratitude", "author": "Vaughn Fender", "emoji": {}},
                {"name": "No id", "emoji": {}},
            ],
            "installed": [{"id": "EC2"}]
        });
        let packs = parse_emoji_packs(&payload).unwrap();
        assert_eq!(packs.len(), 2);
        assert_eq!(packs[0].id, "EC1");
        assert!(!packs[0].installed);
        assert!(packs[1].installed);
        assert_eq!(
            packs[0]
                .emoji
                .iter()
                .map(|emoji| emoji.name.as_str())
                .collect::<Vec<_>>(),
            ["alphabet-a", "alphabet-at"]
        );
    }
}
