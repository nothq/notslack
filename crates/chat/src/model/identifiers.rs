#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SlackReactionName(String);

impl SlackReactionName {
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.is_empty() || value.bytes().any(|byte| byte.is_ascii_whitespace()) {
            return Err(
                "Slack reaction name must be non-empty and contain no whitespace".to_string(),
            );
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
