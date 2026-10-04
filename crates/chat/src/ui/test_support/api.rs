use std::collections::HashMap;

use super::{
    slack_test_workspace, MockSlackLoadMetrics, MockSlackSendOutcome, MockSlackWorkspaceApi,
    SlackTestApi, SlowSlackTestApi,
};
use crate::model::{SlackProfile, SlackWorkspace};
use remote_image_model::RemoteImageData;
use std::sync::{Arc, Mutex};

pub fn slack_test_api() -> SlackTestApi {
    slack_test_api_with_remote_images(HashMap::new())
}

pub fn slack_test_api_with_send_delay(send_delay: std::time::Duration) -> SlackTestApi {
    slack_test_api_with_send_outcome(send_delay, MockSlackSendOutcome::Success)
}

pub fn slack_test_api_with_send_error(
    send_delay: std::time::Duration,
    error: impl Into<String>,
) -> SlackTestApi {
    slack_test_api_with_send_outcome(send_delay, MockSlackSendOutcome::Rejected(error.into()))
}

pub fn slack_test_api_with_accepted_send_error(
    send_delay: std::time::Duration,
    error: impl Into<String>,
) -> SlackTestApi {
    slack_test_api_with_send_outcome(
        send_delay,
        MockSlackSendOutcome::AcceptedWithError(error.into()),
    )
}

pub fn slack_test_api_with_accepted_invalid_receipt() -> SlackTestApi {
    slack_test_api_with_send_outcome(
        std::time::Duration::ZERO,
        MockSlackSendOutcome::AcceptedWithInvalidReceipt,
    )
}

fn slack_test_api_with_send_outcome(
    send_delay: std::time::Duration,
    send_outcome: MockSlackSendOutcome,
) -> SlackTestApi {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let api = MockSlackWorkspaceApi {
        conversations: Arc::new(Mutex::new(slack_test_conversations())),
        profiles: slack_test_profiles(),
        remote_images: HashMap::new(),
        sent_messages: sent_messages.clone(),
        load_delay: std::time::Duration::ZERO,
        send_delay,
        send_outcome,
        load_metrics: None,
    };
    (Arc::new(api), sent_messages)
}

pub fn slack_test_api_with_remote_images(
    remote_images: HashMap<String, RemoteImageData>,
) -> SlackTestApi {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let api = MockSlackWorkspaceApi {
        conversations: Arc::new(Mutex::new(slack_test_conversations())),
        profiles: slack_test_profiles(),
        remote_images,
        sent_messages: sent_messages.clone(),
        load_delay: std::time::Duration::ZERO,
        send_delay: std::time::Duration::ZERO,
        send_outcome: MockSlackSendOutcome::Success,
        load_metrics: None,
    };
    (Arc::new(api), sent_messages)
}

pub fn slack_test_api_with_conversations(
    conversations: HashMap<String, SlackWorkspace>,
) -> SlackTestApi {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let api = MockSlackWorkspaceApi {
        conversations: Arc::new(Mutex::new(conversations)),
        profiles: slack_test_profiles(),
        remote_images: HashMap::new(),
        sent_messages: sent_messages.clone(),
        load_delay: std::time::Duration::ZERO,
        send_delay: std::time::Duration::ZERO,
        send_outcome: MockSlackSendOutcome::Success,
        load_metrics: None,
    };
    (Arc::new(api), sent_messages)
}

fn slack_test_conversations() -> HashMap<String, SlackWorkspace> {
    HashMap::from([
        (
            "C_DESIGN".to_string(),
            slack_test_workspace("C_DESIGN", "design", false),
        ),
        (
            "C_DEPLOYS".to_string(),
            slack_test_workspace("C_DEPLOYS", "deploys", true),
        ),
    ])
}

fn slack_test_profiles() -> HashMap<String, SlackProfile> {
    HashMap::from([
        (
            "U_ADA".to_string(),
            SlackProfile {
                user_id: "U_ADA".to_string(),
                display_name: "Ada Lovelace".to_string(),
                real_name: "Ada Lovelace".to_string(),
                title: Some("Founder".to_string()),
                status_text: Some("Reviewing builds".to_string()),
                email: Some("ada@example.com".to_string()),
                phone: None,
                timezone_label: Some("America/Toronto".to_string()),
                avatar_label: Some("IT".to_string()),
                avatar_image_url: None,
                avatar_image_base64: None,
                avatar_image_mimetype: None,
            },
        ),
        (
            "U_ACME".to_string(),
            SlackProfile {
                user_id: "U_ACME".to_string(),
                display_name: "Acme".to_string(),
                real_name: "Acme".to_string(),
                title: Some("Automation".to_string()),
                status_text: Some("Watching builds".to_string()),
                email: None,
                phone: None,
                timezone_label: Some("America/Toronto".to_string()),
                avatar_label: Some("PB".to_string()),
                avatar_image_url: None,
                avatar_image_base64: None,
                avatar_image_mimetype: None,
            },
        ),
    ])
}

pub fn slow_slack_test_api(load_delay: std::time::Duration) -> SlowSlackTestApi {
    let sent_messages = Arc::new(Mutex::new(Vec::new()));
    let load_metrics = Arc::new(Mutex::new(MockSlackLoadMetrics::default()));
    let api = MockSlackWorkspaceApi {
        conversations: Arc::new(Mutex::new(HashMap::from([
            (
                "C_DESIGN".to_string(),
                slack_test_workspace("C_DESIGN", "design", false),
            ),
            (
                "C_DEPLOYS".to_string(),
                slack_test_workspace("C_DEPLOYS", "deploys", false),
            ),
            (
                "C_RANDOM".to_string(),
                slack_test_workspace("C_RANDOM", "random", false),
            ),
        ]))),
        profiles: HashMap::new(),
        remote_images: HashMap::new(),
        sent_messages: sent_messages.clone(),
        load_delay,
        send_delay: std::time::Duration::ZERO,
        send_outcome: MockSlackSendOutcome::Success,
        load_metrics: Some(load_metrics.clone()),
    };
    (Arc::new(api), sent_messages, load_metrics)
}
