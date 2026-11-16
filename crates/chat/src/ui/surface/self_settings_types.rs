use chrono::{Datelike, Duration, Local, NaiveTime, TimeZone};

/// When a status clears itself, as Slack's "Clear after" menu offers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SlackStatusExpirationPreset {
    #[default]
    Never,
    ThirtyMinutes,
    OneHour,
    FourHours,
    Today,
    ThisWeek,
}

impl SlackStatusExpirationPreset {
    pub(crate) const ALL: [Self; 6] = [
        Self::Never,
        Self::ThirtyMinutes,
        Self::OneHour,
        Self::FourHours,
        Self::Today,
        Self::ThisWeek,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Never => "Don't clear",
            Self::ThirtyMinutes => "30 minutes",
            Self::OneHour => "1 hour",
            Self::FourHours => "4 hours",
            Self::Today => "Today",
            Self::ThisWeek => "This week",
        }
    }

    /// The Unix time the status clears at, or 0 for never.
    pub(crate) fn expiration(self) -> Result<i64, String> {
        let now = Local::now();
        let at = match self {
            Self::Never => return Ok(0),
            Self::ThirtyMinutes => now + Duration::minutes(30),
            Self::OneHour => now + Duration::hours(1),
            Self::FourHours => now + Duration::hours(4),
            Self::Today => end_of_day(now, 0)?,
            Self::ThisWeek => {
                let days = 6 - i64::from(now.weekday().num_days_from_monday());
                end_of_day(now, days)?
            }
        };
        Ok(at.timestamp())
    }
}

fn end_of_day(
    now: chrono::DateTime<Local>,
    days_ahead: i64,
) -> Result<chrono::DateTime<Local>, String> {
    let day = now.date_naive() + Duration::days(days_ahead);
    let end = day.and_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap_or_default());
    Local
        .from_local_datetime(&end)
        .earliest()
        .ok_or_else(|| "Slack status expiration falls in a time change".to_string())
}

/// One of Slack's suggested statuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SlackStatusSuggestion {
    pub(crate) emoji: &'static str,
    pub(crate) text: &'static str,
    pub(crate) expiration: SlackStatusExpirationPreset,
}

pub(crate) const SLACK_STATUS_SUGGESTIONS: [SlackStatusSuggestion; 4] = [
    SlackStatusSuggestion {
        emoji: "spiral_calendar_pad",
        text: "In a meeting",
        expiration: SlackStatusExpirationPreset::OneHour,
    },
    SlackStatusSuggestion {
        emoji: "bus",
        text: "Commuting",
        expiration: SlackStatusExpirationPreset::ThirtyMinutes,
    },
    SlackStatusSuggestion {
        emoji: "face_with_thermometer",
        text: "Out sick",
        expiration: SlackStatusExpirationPreset::Today,
    },
    SlackStatusSuggestion {
        emoji: "house_with_garden",
        text: "Working remotely",
        expiration: SlackStatusExpirationPreset::Today,
    },
];

#[derive(Clone, Debug, Default)]
pub(crate) struct SlackSelfStatusDialog {
    pub(crate) text: String,
    /// The emoji's shortcode, with or without colons.
    pub(crate) emoji: String,
    pub(crate) expiration: SlackStatusExpirationPreset,
    /// Whether a status was set when the dialog opened, which offers
    /// "Clear status".
    pub(crate) had_status: bool,
    pub(crate) emoji_picker_open: bool,
    pub(crate) emoji_query: String,
    pub(crate) expiration_menu_open: bool,
    pub(crate) saving: bool,
    pub(crate) error: Option<String>,
}

impl SlackSelfStatusDialog {
    pub(crate) fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.emoji.trim().is_empty()
    }
}

/// The glyph for a status emoji's shortcode, or the shortcode itself for
/// a custom emoji.
pub(crate) fn slack_status_emoji_glyph(shortcode: &str) -> Option<String> {
    let name = shortcode.trim().trim_matches(':');
    if name.is_empty() {
        return None;
    }
    // Slack's names that the emoji table spells differently.
    let name = match name {
        "spiral_calendar_pad" => "spiral_calendar",
        name => name,
    };
    Some(
        emojis::get_by_shortcode(name)
            .map_or_else(|| format!(":{name}:"), |emoji| emoji.as_str().to_string()),
    )
}
