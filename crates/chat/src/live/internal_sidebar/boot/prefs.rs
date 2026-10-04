use super::types::SlackClientPrefs;

impl SlackClientPrefs {
    pub(super) fn ensure_supported(&self) -> Result<(), String> {
        require_string_pref("channel_sort", self.channel_sort.as_deref(), "default")?;
        require_string_pref(
            "sidebar_behavior",
            self.sidebar_behavior.as_deref(),
            "hide_inactive_channels",
        )?;
        require_bool_pref(
            "separate_shared_channels",
            self.separate_shared_channels,
            true,
        )?;
        require_bool_pref(
            "separate_private_channels",
            self.separate_private_channels,
            false,
        )?;
        require_bool_pref(
            "hide_muted_channels_from_sidebar",
            self.hide_muted_channels_from_sidebar,
            false,
        )?;
        require_bool_pref(
            "remove_sidebar_customizations",
            self.remove_sidebar_customizations,
            false,
        )?;
        require_bool_pref("undo_channel_intermix", self.undo_channel_intermix, false)
    }
}

fn require_string_pref(name: &str, actual: Option<&str>, expected: &str) -> Result<(), String> {
    let Some(actual) = actual else {
        return Err(format!("Slack client.userBoot prefs missing {name}"));
    };
    if actual == expected {
        return Ok(());
    }
    Err(format!(
        "unsupported Slack sidebar preference {name}={actual}; only {expected} is supported"
    ))
}

fn require_bool_pref(name: &str, actual: Option<bool>, expected: bool) -> Result<(), String> {
    let Some(actual) = actual else {
        return Err(format!("Slack client.userBoot prefs missing {name}"));
    };
    if actual == expected {
        return Ok(());
    }
    Err(format!(
        "unsupported Slack sidebar preference {name}={actual}; only {expected} is supported"
    ))
}
