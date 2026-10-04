use super::SlackDesktopIntegrationUnavailable;

#[cfg(target_os = "macos")]
pub const SLACK_DESKTOP_CONNECTION_DISCLOSURE: &str =
    "Connecting will quit and relaunch Slack Desktop. notslack will not restart automatically.";
#[cfg(not(target_os = "macos"))]
pub const SLACK_DESKTOP_CONNECTION_DISCLOSURE: &str =
    "Connecting Slack Desktop is supported only on macOS.";
pub const SLACK_DESKTOP_CONNECTION_SUCCESS: &str =
    "Slack credentials were saved. Restart notslack to use Chat.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlackDesktopRecovery {
    Connect,
    Reconnect,
}

impl SlackDesktopRecovery {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Connect => "Connect Slack",
            Self::Reconnect => "Reconnect Slack",
        }
    }
}

impl SlackDesktopIntegrationUnavailable {
    pub const fn recovery(&self) -> Option<SlackDesktopRecovery> {
        #[cfg(target_os = "macos")]
        {
            match self {
                Self::NoCachedSession | Self::NoWorkspace => Some(SlackDesktopRecovery::Connect),
                Self::CachedSessionError(_) => Some(SlackDesktopRecovery::Reconnect),
                Self::UnsupportedPlatform | Self::NativeAppNotInstalled | Self::RuntimeError(_) => {
                    None
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = self;
            None
        }
    }
}
