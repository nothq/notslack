use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackReactionCatalogSnapshot {
    pub team_id: String,
    pub revision: [u8; 32],
    pub frequent: Vec<String>,
    pub custom: Vec<SlackCustomEmoji>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackCustomEmoji {
    pub name: String,
    pub asset: SlackCustomEmojiAsset,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SlackCustomEmojiAsset {
    ImageUrl(String),
    Alias(String),
}
