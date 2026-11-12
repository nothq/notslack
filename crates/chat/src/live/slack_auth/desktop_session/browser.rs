//! The Linux fallback for machines without Slack Desktop (Slack publishes no
//! ARM64 build): a Chromium-family browser with a profile of its own, where the
//! user signs in to Slack on the web.

use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::desktop_app::debugging_args;

const SLACK_WEB_CLIENT_URL: &str = "https://app.slack.com/client";
const BROWSER_EXIT_WAIT: Duration = Duration::from_secs(3);

/// The browsers that speak the Chrome DevTools Protocol, as a word found in
/// their desktop entry and the names of their launchers.
const BROWSER_FAMILIES: [(&str, &[&str]); 5] = [
    ("chromium", &["chromium-browser", "chromium"]),
    ("chrome", &["google-chrome-stable", "google-chrome"]),
    ("brave", &["brave-browser", "brave"]),
    (
        "microsoft-edge",
        &["microsoft-edge-stable", "microsoft-edge"],
    ),
    ("vivaldi", &["vivaldi-stable", "vivaldi"]),
];

/// The default browser when it is one of the supported families, otherwise the
/// first supported browser on the PATH.
pub(super) fn installed_browser() -> Option<PathBuf> {
    let search_path = std::env::var_os("PATH")?;
    let directories = std::env::split_paths(&search_path).collect::<Vec<_>>();
    browser_from(default_browser_desktop_id().as_deref(), |name| {
        directories
            .iter()
            .map(|directory| directory.join(name))
            .find(|path| path.is_file())
    })
}

fn browser_from(
    default_desktop_id: Option<&str>,
    find: impl Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let default_desktop_id = default_desktop_id.map(str::to_ascii_lowercase);
    let (preferred, others): (Vec<_>, Vec<_>) = BROWSER_FAMILIES.iter().partition(|(family, _)| {
        default_desktop_id
            .as_deref()
            .is_some_and(|id| id.contains(family))
    });
    preferred
        .into_iter()
        .chain(others)
        .flat_map(|(_, launchers)| launchers.iter())
        .find_map(|launcher| find(launcher))
}

fn default_browser_desktop_id() -> Option<String> {
    let output = Command::new("xdg-settings")
        .args(["get", "default-web-browser"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let desktop_id = String::from_utf8(output.stdout).ok()?;
    let desktop_id = desktop_id.trim();
    (!desktop_id.is_empty()).then(|| desktop_id.to_string())
}

/// The browser notslack launched, closed again when the capture ends.
pub(super) struct BrowserProcess(Child);

impl Drop for BrowserProcess {
    fn drop(&mut self) {
        // SIGTERM rather than `Child::kill`, so the browser saves its cookies
        // and the next capture finds the profile still signed in.
        let _ = Command::new("kill")
            .arg(self.0.id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        let deadline = Instant::now() + BROWSER_EXIT_WAIT;
        while matches!(self.0.try_wait(), Ok(None)) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(100));
        }
    }
}

pub(super) fn launch_browser_with_debugging(
    browser: &Path,
    port: u16,
) -> Result<BrowserProcess, String> {
    let profile = browser_profile_dir()?;
    Command::new(browser)
        .args(debugging_args(port))
        .arg(format!("--user-data-dir={}", profile.display()))
        .args(["--no-first-run", "--no-default-browser-check"])
        .arg(SLACK_WEB_CLIENT_URL)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(BrowserProcess)
        .map_err(|error| format!("failed to launch {}: {error}", browser.display()))
}

/// Browsers refuse remote debugging on their default profile, so the Slack
/// sign-in lives in ~/.notslack/browser-profile, readable only by the user.
fn browser_profile_dir() -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;

    let profile = dirs::home_dir()
        .ok_or_else(|| "failed to resolve the home directory".to_string())?
        .join(".notslack/browser-profile");
    std::fs::create_dir_all(&profile)
        .and_then(|()| std::fs::set_permissions(&profile, std::fs::Permissions::from_mode(0o700)))
        .map_err(|error| {
            format!(
                "failed to prepare the browser profile {}: {error}",
                profile.display()
            )
        })?;
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn installed(launchers: &'static [&'static str]) -> impl Fn(&str) -> Option<PathBuf> {
        move |name| {
            launchers
                .contains(&name)
                .then(|| PathBuf::from("/usr/bin").join(name))
        }
    }

    #[test]
    fn a_default_browser_without_devtools_falls_back_to_an_installed_one() {
        assert_eq!(
            browser_from(
                Some("zen-browser.desktop"),
                installed(&["chromium-browser"])
            ),
            Some(PathBuf::from("/usr/bin/chromium-browser"))
        );
    }

    #[test]
    fn a_supported_default_browser_wins_over_the_search_order() {
        assert_eq!(
            browser_from(
                Some("brave-browser.desktop"),
                installed(&["chromium", "brave-browser"])
            ),
            Some(PathBuf::from("/usr/bin/brave-browser"))
        );
    }

    #[test]
    fn chrome_as_the_default_is_not_mistaken_for_chromium() {
        assert_eq!(
            browser_from(
                Some("google-chrome.desktop"),
                installed(&["chromium", "google-chrome"])
            ),
            Some(PathBuf::from("/usr/bin/google-chrome"))
        );
    }

    #[test]
    fn no_supported_browser_means_no_fallback() {
        assert_eq!(browser_from(Some("firefox.desktop"), installed(&[])), None);
    }
}
