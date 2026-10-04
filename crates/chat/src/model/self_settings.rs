const SLACK_STATUS_TEXT_MAX_BYTES: usize = 100;
const SLACK_STATUS_EMOJI_MAX_BYTES: usize = 64;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlackSelfStatus {
    text: String,
    emoji: String,
    expiration: i64,
}

impl SlackSelfStatus {
    pub fn new(text: String, emoji: String, expiration: i64) -> Result<Self, String> {
        let text = text.trim().to_string();
        let emoji = emoji.trim().to_string();
        if text.len() > SLACK_STATUS_TEXT_MAX_BYTES {
            return Err(format!(
                "Slack status text exceeds {SLACK_STATUS_TEXT_MAX_BYTES} bytes"
            ));
        }
        if emoji.len() > SLACK_STATUS_EMOJI_MAX_BYTES {
            return Err(format!(
                "Slack status emoji exceeds {SLACK_STATUS_EMOJI_MAX_BYTES} bytes"
            ));
        }
        if expiration < 0 {
            return Err("Slack status expiration cannot be negative".to_string());
        }
        Ok(Self {
            text,
            emoji,
            expiration,
        })
    }

    pub fn cleared() -> Self {
        Self::default()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn emoji(&self) -> &str {
        &self.emoji
    }

    pub fn expiration(&self) -> i64 {
        self.expiration
    }

    pub fn is_clear(&self) -> bool {
        self.text.is_empty() && self.emoji.is_empty()
    }
}
