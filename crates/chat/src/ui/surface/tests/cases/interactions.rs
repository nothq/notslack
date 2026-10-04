use crate::ui::surface::tests::cases::*;

mod control_wiring;
mod conversation_navigation;
mod message_delivery;
mod message_reconciliation;
mod send;

use send::{assert_slack_hi_loaded, send_slack_hi_with_enter};

fn slack_send_payloads(attempts: &[SlackSendAttempt]) -> Vec<(&str, &str)> {
    attempts
        .iter()
        .map(|attempt| (attempt.conversation_id.as_str(), attempt.text.as_str()))
        .collect()
}
