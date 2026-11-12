use super::SlackDesktopIntegrationUnavailable;

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
const SLACK_DESKTOP_CONNECTION_DISCLOSURE: &str =
    "Connecting will quit and relaunch Slack Desktop. notslack will not restart automatically.";
#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
const SLACK_BROWSER_CONNECTION_DISCLOSURE: &str =
    "Connecting opens a browser window where you sign in to Slack. notslack will not restart automatically.";
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
const SLACK_DESKTOP_CONNECTION_DISCLOSURE: &str =
    "Connecting Slack Desktop is supported only on macOS, Windows and Linux.";

/// What clicking Connect Slack is about to do on this machine.
pub fn slack_desktop_connection_disclosure() -> &'static str {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    if super::slack_auth::connects_through_browser() {
        return SLACK_BROWSER_CONNECTION_DISCLOSURE;
    }
    SLACK_DESKTOP_CONNECTION_DISCLOSURE
}
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
        #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
        {
            match self {
                Self::NoCachedSession | Self::NoWorkspace => Some(SlackDesktopRecovery::Connect),
                Self::CachedSessionError(_) => Some(SlackDesktopRecovery::Reconnect),
                Self::UnsupportedPlatform | Self::NativeAppNotInstalled | Self::RuntimeError(_) => {
                    None
                }
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
        {
            let _ = self;
            None
        }
    }
}
