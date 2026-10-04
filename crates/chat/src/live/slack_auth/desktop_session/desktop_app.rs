//! Finding, quitting and relaunching the installed Slack Desktop app.

use std::{
    io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const SLACK_QUIT_WAIT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(super) enum SlackDesktopAppError {
    NotInstalled,
    HomeUnavailable,
    InspectionFailed { path: PathBuf, source: io::Error },
    InvalidBundle(PathBuf),
}

impl std::fmt::Display for SlackDesktopAppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled => formatter.write_str(
                "Slack Desktop is required to import Slack web session credentials automatically",
            ),
            Self::HomeUnavailable => {
                formatter.write_str("failed to resolve the home directory for Slack Desktop")
            }
            Self::InspectionFailed { path, source } => write!(
                formatter,
                "failed to inspect Slack Desktop application {}: {source}",
                path.display()
            ),
            Self::InvalidBundle(path) => write!(
                formatter,
                "Slack Desktop application {} is not a valid application",
                path.display()
            ),
        }
    }
}

pub(super) fn relaunch_slack_with_debugging(port: u16) -> Result<(), String> {
    let app_path = installed_slack_app_path().map_err(|error| error.to_string())?;
    quit_slack_desktop();
    thread::sleep(Duration::from_millis(1200));
    launch_slack_with_debugging(&app_path, port)
}

fn debugging_args(port: u16) -> [String; 2] {
    [
        format!("--remote-debugging-port={port}"),
        "--remote-debugging-address=127.0.0.1".to_string(),
    ]
}

fn quit_slack_desktop() {
    force_quit_slack_desktop();
    let _ = wait_for_slack_exit(SLACK_QUIT_WAIT);
}

fn wait_for_slack_exit(timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if !slack_process_running() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn quiet_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// Returns the first candidate that exists, so the search order decides which
/// install wins when several are present.
fn first_existing(
    candidates: impl IntoIterator<Item = PathBuf>,
    is_valid: impl Fn(&Path) -> Result<bool, SlackDesktopAppError>,
) -> Result<PathBuf, SlackDesktopAppError> {
    for path in candidates {
        match std::fs::metadata(&path) {
            Ok(_) if is_valid(&path)? => return Ok(path),
            Ok(_) => return Err(SlackDesktopAppError::InvalidBundle(path)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(source) => return Err(SlackDesktopAppError::InspectionFailed { path, source }),
        }
    }
    Err(SlackDesktopAppError::NotInstalled)
}

fn home_dir() -> Result<PathBuf, SlackDesktopAppError> {
    dirs::home_dir().ok_or(SlackDesktopAppError::HomeUnavailable)
}

#[cfg(target_os = "macos")]
pub(super) fn installed_slack_app_path() -> Result<PathBuf, SlackDesktopAppError> {
    let candidates = [
        PathBuf::from("/Applications/Slack.app"),
        home_dir()?.join("Applications/Slack.app"),
    ];
    first_existing(candidates, |path| {
        if !path.is_dir() {
            return Ok(false);
        }
        let info_path = path.join("Contents/Info.plist");
        match std::fs::metadata(&info_path) {
            Ok(metadata) => Ok(metadata.is_file()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(source) => Err(SlackDesktopAppError::InspectionFailed {
                path: info_path,
                source,
            }),
        }
    })
}

#[cfg(target_os = "macos")]
fn launch_slack_with_debugging(app_path: &Path, port: u16) -> Result<(), String> {
    let status = Command::new("open")
        .arg("-na")
        .arg(app_path)
        .arg("--args")
        .args(debugging_args(port))
        .status()
        .map_err(|error| format!("failed to launch Slack Desktop: {error}"))?;
    if !status.success() {
        return Err(format!(
            "failed to launch Slack Desktop with status {status}"
        ));
    }
    let _ = quiet_command("open").arg("slack://").status();
    Ok(())
}

#[cfg(target_os = "macos")]
fn slack_process_running() -> bool {
    quiet_command("pgrep")
        .args(["-x", "Slack"])
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(target_os = "macos")]
fn force_quit_slack_desktop() {
    let _ = quiet_command("killall").arg("Slack").status();
}

/// Slack's per-user installer, then the machine-wide MSI install. The Microsoft
/// Store build cannot be launched with extra arguments, so it is not listed.
#[cfg(target_os = "windows")]
pub(super) fn installed_slack_app_path() -> Result<PathBuf, SlackDesktopAppError> {
    let mut candidates = Vec::new();
    if let Some(local) = dirs::data_local_dir() {
        candidates.push(local.join("slack").join("slack.exe"));
    }
    if let Some(program_files) = std::env::var_os("ProgramFiles") {
        candidates.push(PathBuf::from(program_files).join("Slack").join("slack.exe"));
    }
    first_existing(candidates, |path| Ok(path.is_file()))
}

#[cfg(target_os = "windows")]
fn slack_process_running() -> bool {
    quiet_command("tasklist")
        .args(["/FI", "IMAGENAME eq slack.exe", "/NH"])
        .stdout(Stdio::piped())
        .output()
        .is_ok_and(|output| {
            String::from_utf8_lossy(&output.stdout)
                .to_ascii_lowercase()
                .contains("slack.exe")
        })
}

#[cfg(target_os = "windows")]
fn force_quit_slack_desktop() {
    let _ = quiet_command("taskkill")
        .args(["/IM", "slack.exe", "/T", "/F"])
        .status();
}

/// The .deb and .rpm packages, then Snap, then Flatpak. Snap and Flatpak expose
/// launchers that forward their arguments to Slack.
#[cfg(target_os = "linux")]
pub(super) fn installed_slack_app_path() -> Result<PathBuf, SlackDesktopAppError> {
    let home = home_dir()?;
    let candidates = [
        PathBuf::from("/usr/bin/slack"),
        PathBuf::from("/usr/lib/slack/slack"),
        PathBuf::from("/snap/bin/slack"),
        PathBuf::from("/var/lib/flatpak/exports/bin/com.slack.Slack"),
        home.join(".local/share/flatpak/exports/bin/com.slack.Slack"),
    ];
    first_existing(candidates, |path| Ok(!path.is_dir()))
}

#[cfg(target_os = "linux")]
fn slack_process_running() -> bool {
    quiet_command("pgrep")
        .args(["-x", "slack"])
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(target_os = "linux")]
fn force_quit_slack_desktop() {
    let _ = quiet_command("pkill").args(["-x", "slack"]).status();
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn launch_slack_with_debugging(app_path: &Path, port: u16) -> Result<(), String> {
    quiet_command(app_path)
        .args(debugging_args(port))
        .spawn()
        .map(drop)
        .map_err(|error| format!("failed to launch Slack Desktop: {error}"))
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub(super) fn installed_slack_app_path() -> Result<PathBuf, SlackDesktopAppError> {
    Err(SlackDesktopAppError::NotInstalled)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn launch_slack_with_debugging(_: &Path, _: u16) -> Result<(), String> {
    Err(SlackDesktopAppError::NotInstalled.to_string())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn slack_process_running() -> bool {
    false
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn force_quit_slack_desktop() {}
