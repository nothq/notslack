use std::fmt;

const SLACK_MEDIA_CAPABILITY_TOKEN_LENGTH: usize = 43;

#[derive(Clone, PartialEq, Eq)]
pub struct SlackPreparedMediaSource {
    url: String,
}

impl SlackPreparedMediaSource {
    pub fn from_loopback_url(url: String) -> Result<Self, String> {
        let parsed = url::Url::parse(&url)
            .map_err(|error| format!("prepared Slack media URL is invalid: {error}"))?;
        let capability = parsed.path().strip_prefix("/media/");
        if parsed.scheme() != "http"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.host_str() != Some("127.0.0.1")
            || parsed.port().is_none()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !capability.is_some_and(|capability| {
                capability.len() == SLACK_MEDIA_CAPABILITY_TOKEN_LENGTH
                    && capability
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            })
        {
            return Err(
                "prepared Slack media URL must be an unparameterized loopback HTTP capability"
                    .to_string(),
            );
        }
        Ok(Self {
            url: parsed.to_string(),
        })
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}

impl fmt::Debug for SlackPreparedMediaSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SlackPreparedMediaSource")
            .field("url", &"<redacted loopback capability>")
            .finish()
    }
}
