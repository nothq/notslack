use std::cell::Cell;

use super::super::{
    SlackDesktopIntegrationStatus, SlackDesktopIntegrationUnavailable, SlackRuntimeRecoveryPolicy,
    SlackWorkspaceRuntimeRegistry,
};
use super::realtime_ownership::in_memory_authenticated_registry;
use super::{
    desktop_app_runtime_registry_from, orchestrate_slack_host_start, SlackDesktopCaptureSupport,
    SlackHostEventEmitter, SlackHostRuntime, SlackHostStartupOutcome,
};

fn empty_available_registry() -> SlackWorkspaceRuntimeRegistry {
    SlackWorkspaceRuntimeRegistry::from_inputs(
        Vec::new(),
        SlackRuntimeRecoveryPolicy::CachedSessionsOnly,
    )
    .expect("empty test Slack registry should be constructible")
}

#[test]
fn absent_desktop_app_is_typed_unavailable_without_loading_credentials() {
    let loader_called = Cell::new(false);
    let registry = desktop_app_runtime_registry_from(
        SlackDesktopCaptureSupport::Supported,
        Err(SlackDesktopIntegrationUnavailable::NativeAppNotInstalled),
        || {
            loader_called.set(true);
            Ok(empty_available_registry())
        },
    );

    assert!(!loader_called.get());
    assert_eq!(
        registry.integration_status(),
        &SlackDesktopIntegrationStatus::Unavailable(
            SlackDesktopIntegrationUnavailable::NativeAppNotInstalled,
        )
    );
}

#[test]
fn supported_capture_platform_without_v3_cache_is_typed_unavailable() {
    let registry =
        desktop_app_runtime_registry_from(SlackDesktopCaptureSupport::Supported, Ok(()), || {
            Ok(empty_available_registry())
        });

    assert_eq!(
        registry.integration_status(),
        &SlackDesktopIntegrationStatus::Unavailable(
            SlackDesktopIntegrationUnavailable::NoCachedSession,
        )
    );
}

#[test]
fn unsupported_capture_platform_without_cache_is_typed_unsupported() {
    let registry =
        desktop_app_runtime_registry_from(SlackDesktopCaptureSupport::Unsupported, Ok(()), || {
            Ok(empty_available_registry())
        });

    assert_eq!(
        registry.integration_status(),
        &SlackDesktopIntegrationStatus::Unavailable(
            SlackDesktopIntegrationUnavailable::UnsupportedPlatform,
        )
    );
}

#[test]
fn unsupported_capture_platform_preserves_a_valid_cached_registry() {
    let registry =
        desktop_app_runtime_registry_from(SlackDesktopCaptureSupport::Unsupported, Ok(()), || {
            in_memory_authenticated_registry()
                .map_err(SlackDesktopIntegrationUnavailable::RuntimeError)
        });

    assert_eq!(
        registry.integration_status(),
        &SlackDesktopIntegrationStatus::Available
    );
    assert_eq!(registry.authenticated_teams().len(), 1);
}

#[test]
fn semantically_invalid_cached_session_is_a_typed_cached_session_error() {
    let raw = r#"{
        "schema_version": 3,
        "ordered_team_ids": [],
        "sessions_by_team_id": {}
    }"#;
    let registry =
        desktop_app_runtime_registry_from(SlackDesktopCaptureSupport::Supported, Ok(()), || {
            crate::live::slack_auth::parse_cached_slack_desktop_sessions_for_test(raw)
                .map_err(SlackDesktopIntegrationUnavailable::CachedSessionError)?;
            Ok(empty_available_registry())
        });

    assert_eq!(
        registry.integration_status(),
        &SlackDesktopIntegrationStatus::Unavailable(
            SlackDesktopIntegrationUnavailable::CachedSessionError(
                "stored Slack Desktop sessions did not include any workspaces".to_string(),
            ),
        )
    );
}

#[test]
fn runtime_registry_initialization_error_keeps_runtime_provenance() {
    let reason = SlackDesktopIntegrationUnavailable::RuntimeError(
        "failed to initialize Slack runtime registry".to_string(),
    );
    let registry =
        desktop_app_runtime_registry_from(SlackDesktopCaptureSupport::Supported, Ok(()), || {
            Err(reason.clone())
        });

    assert_eq!(
        registry.integration_status(),
        &SlackDesktopIntegrationStatus::Unavailable(reason)
    );
}

#[test]
fn authenticated_available_status_selects_host_worker_startup_orchestration() {
    let startup_called = Cell::new(false);
    let outcome =
        orchestrate_slack_host_start((), &SlackDesktopIntegrationStatus::Available, 1, |_| {
            startup_called.set(true);
            Ok("started")
        })
        .expect("authenticated available Slack should run host startup");

    assert!(startup_called.get());
    assert!(matches!(
        outcome,
        SlackHostStartupOutcome::Started("started")
    ));
}

#[test]
fn unavailable_or_empty_registry_invokes_zero_host_worker_starts() {
    let cases = [
        (
            SlackDesktopIntegrationStatus::Unavailable(
                SlackDesktopIntegrationUnavailable::NativeAppNotInstalled,
            ),
            1,
        ),
        (
            SlackDesktopIntegrationStatus::Unavailable(
                SlackDesktopIntegrationUnavailable::NoCachedSession,
            ),
            1,
        ),
        (
            SlackDesktopIntegrationStatus::Unavailable(
                SlackDesktopIntegrationUnavailable::CachedSessionError(
                    "invalid cached session".to_string(),
                ),
            ),
            1,
        ),
        (
            SlackDesktopIntegrationStatus::Unavailable(
                SlackDesktopIntegrationUnavailable::RuntimeError(
                    "failed to initialize Slack runtime".to_string(),
                ),
            ),
            1,
        ),
        (SlackDesktopIntegrationStatus::Available, 0),
    ];

    for (status, authenticated_team_count) in cases {
        let startup_called = Cell::new(false);
        let outcome = orchestrate_slack_host_start((), &status, authenticated_team_count, |_| {
            startup_called.set(true);
            Ok(())
        })
        .expect("unavailable Slack should select inactive host startup");

        assert!(!startup_called.get());
        assert!(matches!(outcome, SlackHostStartupOutcome::Inactive(())));
    }
}

#[test]
fn unavailable_registries_start_zero_slack_workers() {
    let reasons = [
        SlackDesktopIntegrationUnavailable::UnsupportedPlatform,
        SlackDesktopIntegrationUnavailable::NativeAppNotInstalled,
        SlackDesktopIntegrationUnavailable::NoCachedSession,
        SlackDesktopIntegrationUnavailable::NoWorkspace,
        SlackDesktopIntegrationUnavailable::CachedSessionError(
            "invalid cached session".to_string(),
        ),
        SlackDesktopIntegrationUnavailable::RuntimeError(
            "failed to initialize Slack runtime".to_string(),
        ),
    ];

    for reason in reasons {
        let host = SlackHostRuntime::start(
            SlackWorkspaceRuntimeRegistry::unavailable(reason.clone()),
            SlackHostEventEmitter::discarding(),
        )
        .expect("unavailable desktop Slack host startup should be accepted");
        assert_eq!(
            host.integration_status(),
            SlackDesktopIntegrationStatus::Unavailable(reason)
        );
        assert_eq!(host.worker_count_for_test(), 0);
        assert!(!host.is_available());
    }
}

#[test]
fn desktop_host_start_failure_becomes_nonfatal_runtime_error() {
    let host = SlackHostRuntime::accept_desktop_start_result(Err(
        "failed to claim Slack realtime source".to_string(),
    ));

    assert_eq!(
        host.integration_status(),
        SlackDesktopIntegrationStatus::Unavailable(
            SlackDesktopIntegrationUnavailable::RuntimeError(
                "failed to claim Slack realtime source".to_string(),
            ),
        )
    );
    assert_eq!(host.worker_count_for_test(), 0);
}

#[test]
fn runtime_component_failure_transitions_host_to_typed_unavailable() {
    let host = SlackHostRuntime::from_inactive_registry(empty_available_registry());

    let reason =
        host.mark_integration_unavailable("failed to initialize Slack notification runtime");

    assert_eq!(
        reason,
        SlackDesktopIntegrationUnavailable::RuntimeError(
            "failed to initialize Slack notification runtime".to_string(),
        )
    );
    assert_eq!(
        host.integration_status(),
        SlackDesktopIntegrationStatus::Unavailable(reason)
    );
    assert_eq!(host.worker_count_for_test(), 0);
    assert!(!host.is_available());
}

#[test]
fn authenticated_in_memory_host_owns_workers_and_orders_idempotent_shutdown() {
    let (host, _events, probe) = SlackHostRuntime::in_memory_authenticated_with_probe()
        .expect("in-memory authenticated Slack host should start");
    let mut worker_starts = probe.worker_starts();
    worker_starts.sort_unstable();

    assert!(host.is_available());
    host.require_available()
        .expect("authenticated in-memory Slack host should be healthy");
    assert_eq!(host.worker_count_for_test(), 3);
    assert_eq!(worker_starts, ["notification-policy", "realtime"]);

    host.stop_and_join()
        .expect("authenticated Slack host should stop cleanly");
    assert_eq!(host.worker_count_for_test(), 0);
    assert_eq!(
        probe.shutdown_steps(),
        ["request-stop", "realtime-source-stop", "worker-join"]
    );

    host.stop_and_join()
        .expect("Slack host shutdown should be idempotent");
    assert_eq!(
        probe.shutdown_steps(),
        ["request-stop", "realtime-source-stop", "worker-join"]
    );
    assert!(host.require_available().is_err());
}
