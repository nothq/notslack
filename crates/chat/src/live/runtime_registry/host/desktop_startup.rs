use super::super::{
    SlackDesktopIntegrationStatus, SlackDesktopIntegrationUnavailable,
    SlackWorkspaceRuntimeRegistry,
};

pub(super) enum SlackHostStartupOutcome<I, O> {
    Inactive(I),
    Started(O),
}

pub(super) fn orchestrate_slack_host_start<I, O>(
    input: I,
    status: &SlackDesktopIntegrationStatus,
    authenticated_team_count: usize,
    start: impl FnOnce(I) -> Result<O, String>,
) -> Result<SlackHostStartupOutcome<I, O>, String> {
    if should_start_slack_host_workers(status, authenticated_team_count) {
        start(input).map(SlackHostStartupOutcome::Started)
    } else {
        Ok(SlackHostStartupOutcome::Inactive(input))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SlackDesktopCaptureSupport {
    Supported,
    Unsupported,
}

impl SlackDesktopCaptureSupport {
    const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::Supported
        } else {
            Self::Unsupported
        }
    }

    const fn empty_cache_reason(self) -> SlackDesktopIntegrationUnavailable {
        match self {
            Self::Supported => SlackDesktopIntegrationUnavailable::NoCachedSession,
            Self::Unsupported => SlackDesktopIntegrationUnavailable::UnsupportedPlatform,
        }
    }
}

pub(super) fn desktop_app_runtime_registry() -> SlackWorkspaceRuntimeRegistry {
    #[cfg(target_os = "macos")]
    let desktop_app_status = crate::live::check_slack_desktop_app();
    #[cfg(not(target_os = "macos"))]
    let desktop_app_status = Ok(());

    desktop_app_runtime_registry_from(
        SlackDesktopCaptureSupport::current(),
        desktop_app_status,
        SlackWorkspaceRuntimeRegistry::load_available,
    )
}

pub(super) fn desktop_app_runtime_registry_from(
    capture_support: SlackDesktopCaptureSupport,
    desktop_app_status: Result<(), SlackDesktopIntegrationUnavailable>,
    load_available: impl FnOnce() -> Result<
        SlackWorkspaceRuntimeRegistry,
        SlackDesktopIntegrationUnavailable,
    >,
) -> SlackWorkspaceRuntimeRegistry {
    if let Err(reason) = desktop_app_status {
        return SlackWorkspaceRuntimeRegistry::unavailable(reason);
    }

    match load_available() {
        Ok(registry) if registry.authenticated_teams().is_empty() => {
            SlackWorkspaceRuntimeRegistry::unavailable(capture_support.empty_cache_reason())
        }
        Ok(registry) => registry,
        Err(reason) => SlackWorkspaceRuntimeRegistry::unavailable(reason),
    }
}

fn should_start_slack_host_workers(
    status: &SlackDesktopIntegrationStatus,
    authenticated_team_count: usize,
) -> bool {
    matches!(status, SlackDesktopIntegrationStatus::Available) && authenticated_team_count > 0
}
