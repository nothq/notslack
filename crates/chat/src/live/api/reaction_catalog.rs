mod wire;

use std::{collections::HashSet, thread};

use crate::model::SlackReactionName;
use crate::model::{SlackCustomEmoji, SlackCustomEmojiAsset, SlackReactionCatalogSnapshot};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::SlackApiClient;

const SLACK_TOP_EMOJIS_METHOD: &str = "search.autocomplete.topEmojis";
const SLACK_REACTION_CATALOG_FREQUENT_COUNT: u16 = 100;

#[derive(Deserialize)]
struct SlackTopEmojisResponseWire {
    user: Vec<String>,
    org: Vec<String>,
}

impl SlackApiClient {
    pub(crate) fn load_reaction_catalog(
        &self,
        team_id: &str,
    ) -> Result<SlackReactionCatalogSnapshot, String> {
        let (frequent, custom) = thread::scope(|scope| {
            let frequent = scope.spawn(|| self.load_frequent_reaction_names());
            let custom = scope.spawn(|| wire::load_custom_emoji(self));
            (
                frequent
                    .join()
                    .map_err(|_| "Slack frequent emoji request thread panicked".to_string()),
                custom
                    .join()
                    .map_err(|_| "Slack custom emoji request thread panicked".to_string()),
            )
        });
        let frequent = frequent??;
        let custom = custom??;
        let revision = reaction_catalog_revision(team_id, &frequent, &custom);
        Ok(SlackReactionCatalogSnapshot {
            team_id: team_id.to_string(),
            revision,
            frequent,
            custom,
        })
    }

    fn load_frequent_reaction_names(&self) -> Result<Vec<String>, String> {
        let payload = self.post(
            SLACK_TOP_EMOJIS_METHOD,
            &[("count", SLACK_REACTION_CATALOG_FREQUENT_COUNT.to_string())],
        )?;
        let response =
            serde_json::from_value::<SlackTopEmojisResponseWire>(payload).map_err(|error| {
                format!("failed to decode Slack {SLACK_TOP_EMOJIS_METHOD} response: {error}")
            })?;
        let names = if response.user.is_empty() {
            response.org
        } else {
            response.user
        };
        parse_unique_reaction_names(SLACK_TOP_EMOJIS_METHOD, names)
    }
}

fn reaction_catalog_revision(
    team_id: &str,
    frequent: &[String],
    custom: &[SlackCustomEmoji],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    hash_string(&mut digest, "notslack-slack-reaction-catalog-v1");
    hash_string(&mut digest, team_id);
    hash_len(&mut digest, frequent.len());
    for name in frequent {
        hash_string(&mut digest, name);
    }
    hash_len(&mut digest, custom.len());
    for emoji in custom {
        hash_string(&mut digest, &emoji.name);
        match &emoji.asset {
            SlackCustomEmojiAsset::ImageUrl(url) => {
                digest.update([0]);
                hash_string(&mut digest, url);
            }
            SlackCustomEmojiAsset::Alias(target) => {
                digest.update([1]);
                hash_string(&mut digest, target);
            }
        }
    }
    digest.finalize().into()
}

fn hash_len(digest: &mut Sha256, value: usize) {
    digest.update((value as u64).to_le_bytes());
}

fn hash_string(digest: &mut Sha256, value: &str) {
    hash_len(digest, value.len());
    digest.update(value.as_bytes());
}

fn parse_unique_reaction_names(method: &str, names: Vec<String>) -> Result<Vec<String>, String> {
    let mut seen = HashSet::new();
    names
        .into_iter()
        .map(|name| {
            let name = parse_reaction_name(method, name)?;
            if !seen.insert(name.clone()) {
                return Err(format!("Slack {method} returned duplicate emoji {name}"));
            }
            Ok(name)
        })
        .collect()
}

fn parse_reaction_name(method: &str, name: String) -> Result<String, String> {
    SlackReactionName::parse(&name)
        .map_err(|error| format!("Slack {method} returned an invalid emoji name: {error}"))?;
    Ok(name)
}
