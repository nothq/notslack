#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlackStatusExpirationPreset {
    #[default]
    Never,
    OneHour,
    FourHours,
    Tomorrow,
}

impl SlackStatusExpirationPreset {
    pub(crate) const ALL: [Self; 4] = [Self::Never, Self::OneHour, Self::FourHours, Self::Tomorrow];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Never => "Don't clear",
            Self::OneHour => "Clear in 1 hour",
            Self::FourHours => "Clear in 4 hours",
            Self::Tomorrow => "Clear tomorrow",
        }
    }

    pub(crate) fn expiration(self) -> Result<i64, String> {
        let seconds = match self {
            Self::Never => return Ok(0),
            Self::OneHour => 60 * 60,
            Self::FourHours => 4 * 60 * 60,
            Self::Tomorrow => 24 * 60 * 60,
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "system clock is before the Unix epoch".to_string())?
            .as_secs();
        let expiration = now
            .checked_add(seconds)
            .ok_or_else(|| "Slack status expiration overflowed".to_string())?;
        i64::try_from(expiration).map_err(|_| "Slack status expiration is out of range".to_string())
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct SlackSelfStatusDialog {
    pub(crate) text: String,
    pub(crate) emoji: String,
    pub(crate) expiration: SlackStatusExpirationPreset,
    pub(crate) saving: bool,
    pub(crate) error: Option<String>,
}
