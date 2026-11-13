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

/// A pack of custom emoji Slack offers to add to a workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackEmojiPack {
    pub id: String,
    pub name: String,
    pub author: String,
    pub installed: bool,
    /// The pack's emoji in Slack's order.
    pub emoji: Vec<SlackEmojiPackEmoji>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlackEmojiPackEmoji {
    pub name: String,
    pub url: String,
}

/// An image to add to the workspace as a custom emoji.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackCustomEmojiUpload {
    pub name: String,
    pub file_name: String,
    pub mimetype: String,
    pub image: Vec<u8>,
}
