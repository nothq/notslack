use std::cell::Cell;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
use super::SLACK_DESKTOP_CONNECT_UNSUPPORTED;
use super::{
    authenticated_runtime_input, capture_slack_desktop_credentials_from, confirm_stored_slack_team,
    credentials::{
        load_cached_slack_desktop_sessions_from, parse_cached_slack_desktop_sessions,
        SlackCachedSessionsLoadError,
    },
    join_slack_runtime_input_tasks,
    state::SlackSelectionState,
    with_supported_slack_desktop_capture, SlackDesktopIntegrationUnavailable,
    SlackRuntimeInputLoadError,
};
use crate::live::{api::SlackApiRequestError, SlackDesktopRecovery};

const VALID_V3_CACHE_PAYLOAD: &str = r#"{
  "schema_version": 3,
  "ordered_team_ids": ["T_TEST"],
  "selected_team_id": "T_TEST",
  "sessions_by_team_id": {
    "T_TEST": {
      "workspace_name": "Test Workspace",
      "workspace_logo_url": null,
      "team_id": "T_TEST",
      "team_domain": "test-workspace",
      "user_id": "U_TEST",
      "xoxc_token": "xoxc-test-token",
      "cookie_header": "d=xoxd-test-cookie",
      "web_build_timestamp": "1234567890",
      "draft_count": 0
    }
  }
}"#;

fn unavailable_reason_for_v3_cache(raw: &str) -> SlackDesktopIntegrationUnavailable {
    let error = parse_cached_slack_desktop_sessions(raw)
        .err()
        .expect("invalid v3 Slack cache payload should fail parsing");
    SlackDesktopIntegrationUnavailable::CachedSessionError(error)
}

#[test]
fn malformed_v3_cache_is_a_typed_cached_session_error() {
    let reason = unavailable_reason_for_v3_cache("{");

    let SlackDesktopIntegrationUnavailable::CachedSessionError(message) = reason else {
        panic!("malformed v3 Slack cache must be a cached-session error");
    };
    assert!(
        message.starts_with("failed to decode stored Slack Desktop sessions:"),
        "unexpected malformed-cache error: {message}"
    );
}

#[test]
fn semantically_invalid_v3_cache_is_a_specific_cached_session_error() {
    let raw = r#"{
      "schema_version": 3,
      "ordered_team_ids": [],
      "sessions_by_team_id": {}
    }"#;

    assert_eq!(
        unavailable_reason_for_v3_cache(raw),
        SlackDesktopIntegrationUnavailable::CachedSessionError(
            "stored Slack Desktop sessions did not include any workspaces".to_string(),
        )
    );
}

#[test]
fn valid_v3_cache_parses_into_validated_sessions() {
    let sessions = parse_cached_slack_desktop_sessions(VALID_V3_CACHE_PAYLOAD)
        .expect("valid v3 Slack cache payload should parse");

    assert_eq!(sessions.first_team_id(), "T_TEST");
    assert!(sessions.session("T_TEST").is_some());
}

#[test]
fn credential_capture_only_stores_sessions_without_resolving_a_launch_context() {
    let capture_called = Cell::new(false);
    let result = capture_slack_desktop_credentials_from(|| {
        capture_called.set(true);
        parse_cached_slack_desktop_sessions(VALID_V3_CACHE_PAYLOAD)
    });

    assert_eq!(result, Ok(()));
    assert!(capture_called.get());
}

#[test]
fn runtime_input_error_provenance_maps_to_distinct_unavailable_states() {
    assert_eq!(
        SlackRuntimeInputLoadError::CachedSession("invalid credentials".to_string())
            .into_unavailable(),
        SlackDesktopIntegrationUnavailable::CachedSessionError("invalid credentials".to_string(),)
    );
    assert_eq!(
        SlackRuntimeInputLoadError::Runtime("failed to initialize loader".to_string())
            .into_unavailable(),
        SlackDesktopIntegrationUnavailable::RuntimeError("failed to initialize loader".to_string(),)
    );
}

#[test]
fn cache_storage_and_decode_failures_keep_distinct_provenance() {
    let storage_error = match load_cached_slack_desktop_sessions_from(|| {
        Err("keychain is unavailable".to_string())
    }) {
        Err(error) => error,
        Ok(_) => panic!("keychain failure should not load cached sessions"),
    };
    assert_eq!(
        storage_error,
        SlackCachedSessionsLoadError::Storage("keychain is unavailable".to_string())
    );
    assert_eq!(
        SlackRuntimeInputLoadError::from(storage_error).into_unavailable(),
        SlackDesktopIntegrationUnavailable::RuntimeError("keychain is unavailable".to_string())
    );

    let decode_error = match load_cached_slack_desktop_sessions_from(|| Ok(Some("{".to_string()))) {
        Err(error) => error,
        Ok(_) => panic!("malformed cache should not load cached sessions"),
    };
    let SlackCachedSessionsLoadError::CachedSession(message) = decode_error else {
        panic!("malformed cache must remain a cached-session failure");
    };
    assert!(message.starts_with("failed to decode stored Slack Desktop sessions:"));
}

#[test]
fn cached_domain_and_cookie_shape_failures_are_reconnectable() {
    let sessions = parse_cached_slack_desktop_sessions(VALID_V3_CACHE_PAYLOAD)
        .expect("valid v3 Slack cache payload should parse");
    let record = sessions
        .ordered_records()
        .next()
        .expect("valid cache should contain a workspace")
        .clone();

    let mut invalid_domain = record.clone();
    invalid_domain.session.team_domain = "INVALID DOMAIN".to_string();
    let domain_error = match authenticated_runtime_input(
        &invalid_domain,
        &SlackSelectionState::default(),
        false,
        true,
    ) {
        Err(error) => error,
        Ok(_) => panic!("invalid cached team domain should fail before runtime construction"),
    };
    assert!(matches!(
        domain_error,
        SlackRuntimeInputLoadError::CachedSession(message)
            if message.contains("invalid Slack team domain")
    ));

    let mut invalid_cookie = record;
    invalid_cookie.session.cookie_header = "d=value\ninvalid=value".to_string();
    let cookie_error = match authenticated_runtime_input(
        &invalid_cookie,
        &SlackSelectionState::default(),
        false,
        true,
    ) {
        Err(error) => error,
        Ok(_) => panic!("invalid cached cookie should fail before runtime construction"),
    };
    assert_eq!(
        cookie_error,
        SlackRuntimeInputLoadError::CachedSession(
            "Slack Desktop cookie header is not a valid HTTP header value".to_string()
        )
    );
}

#[test]
fn auth_response_schema_and_team_mismatch_have_distinct_provenance() {
    assert_eq!(
        confirm_stored_slack_team(&serde_json::json!({ "ok": true }), "T_STORED"),
        Err(SlackRuntimeInputLoadError::Runtime(
            "Slack auth.test response did not include a team id".to_string()
        ))
    );
    assert_eq!(
        confirm_stored_slack_team(
            &serde_json::json!({ "ok": true, "team_id": "T_OTHER" }),
            "T_STORED",
        ),
        Err(SlackRuntimeInputLoadError::CachedSession(
            "Slack auth.test confirmed team T_OTHER, not stored team T_STORED".to_string()
        ))
    );
}

#[test]
fn typed_auth_rejection_is_reconnectable_but_api_runtime_failure_is_passive() {
    assert_eq!(
        SlackRuntimeInputLoadError::from(SlackApiRequestError::AuthenticationRejected(
            "Slack API auth.test failed: invalid_auth".to_string(),
        ))
        .into_unavailable(),
        SlackDesktopIntegrationUnavailable::CachedSessionError(
            "Slack API auth.test failed: invalid_auth".to_string(),
        )
    );
    assert_eq!(
        SlackRuntimeInputLoadError::from(SlackApiRequestError::Runtime(
            "Slack API auth.test rate limited".to_string(),
        ))
        .into_unavailable(),
        SlackDesktopIntegrationUnavailable::RuntimeError(
            "Slack API auth.test rate limited".to_string(),
        )
    );
}

#[test]
fn scoped_loader_join_contains_later_panic_and_prefers_runtime_failure() {
    let outcome = std::panic::catch_unwind(|| {
        std::thread::scope(|scope| {
            let tasks = vec![
                scope.spawn(|| {
                    Err::<(), _>(SlackRuntimeInputLoadError::CachedSession(
                        "first cached-session error".to_string(),
                    ))
                }),
                scope.spawn(|| -> Result<(), SlackRuntimeInputLoadError> {
                    panic!("later loader panic");
                }),
            ];
            join_slack_runtime_input_tasks(tasks)
        })
    });
    let result = outcome.expect("joining a later panicked loader must not unwind the scope");

    assert_eq!(
        result,
        Err(SlackRuntimeInputLoadError::Runtime(
            "authenticated Slack team loader panicked; earlier Slack cached-session failure: first cached-session error"
                .to_string()
        ))
    );
}

#[test]
fn scoped_loader_panic_becomes_runtime_error() {
    let result = std::thread::scope(|scope| {
        join_slack_runtime_input_tasks(vec![scope.spawn(
            || -> Result<(), SlackRuntimeInputLoadError> {
                panic!("loader panic");
            },
        )])
    });

    assert_eq!(
        result,
        Err(SlackRuntimeInputLoadError::Runtime(
            "authenticated Slack team loader panicked".to_string()
        ))
    );
}

#[test]
fn recovery_labels_are_typed() {
    assert_eq!(SlackDesktopRecovery::Connect.label(), "Connect Slack");
    assert_eq!(SlackDesktopRecovery::Reconnect.label(), "Reconnect Slack");
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
#[test]
fn macos_unavailable_recovery_matrix_is_explicit() {
    let cases = [
        (
            SlackDesktopIntegrationUnavailable::UnsupportedPlatform,
            None,
        ),
        (
            SlackDesktopIntegrationUnavailable::NativeAppNotInstalled,
            None,
        ),
        (
            SlackDesktopIntegrationUnavailable::NoCachedSession,
            Some(SlackDesktopRecovery::Connect),
        ),
        (
            SlackDesktopIntegrationUnavailable::NoWorkspace,
            Some(SlackDesktopRecovery::Connect),
        ),
        (
            SlackDesktopIntegrationUnavailable::CachedSessionError(
                "invalid cached session".to_string(),
            ),
            Some(SlackDesktopRecovery::Reconnect),
        ),
        (
            SlackDesktopIntegrationUnavailable::RuntimeError(
                "failed to initialize runtime".to_string(),
            ),
            None,
        ),
    ];

    for (reason, expected) in cases {
        assert_eq!(
            reason.recovery(),
            expected,
            "unexpected recovery for {reason}"
        );
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
#[test]
fn unsupported_platform_unavailable_states_never_offer_desktop_capture() {
    let reasons = [
        SlackDesktopIntegrationUnavailable::UnsupportedPlatform,
        SlackDesktopIntegrationUnavailable::NativeAppNotInstalled,
        SlackDesktopIntegrationUnavailable::NoCachedSession,
        SlackDesktopIntegrationUnavailable::NoWorkspace,
        SlackDesktopIntegrationUnavailable::CachedSessionError(
            "invalid cached session".to_string(),
        ),
        SlackDesktopIntegrationUnavailable::RuntimeError(
            "failed to initialize runtime".to_string(),
        ),
    ];

    for reason in reasons {
        assert_eq!(reason.recovery(), None, "unexpected recovery for {reason}");
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
#[test]
fn explicit_desktop_capture_invokes_capture_boundary_on_supported_platforms() {
    let capture_called = Cell::new(false);
    let result = with_supported_slack_desktop_capture(|| {
        capture_called.set(true);
        Ok("captured")
    });

    assert_eq!(result.as_deref(), Ok("captured"));
    assert!(capture_called.get());
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
#[test]
fn explicit_desktop_capture_rejects_platform_without_invoking_capture() {
    let capture_called = Cell::new(false);
    let error = with_supported_slack_desktop_capture(|| {
        capture_called.set(true);
        Ok("captured")
    })
    .expect_err("unsupported-platform Slack Desktop capture must be unavailable");

    assert_eq!(error, SLACK_DESKTOP_CONNECT_UNSUPPORTED);
    assert!(!capture_called.get());
}
