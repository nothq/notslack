use std::collections::BTreeMap;
use std::time::Instant;

use crate::model::{SlackConversationKind, SlackConversationSnapshot, SlackMessage};

use super::{
    insert_slack_confirmed_send, merge_slack_conversation_windows, reconcile_slack_confirmed_sends,
    slack_message_timestamp_key,
};

#[test]
fn stale_history_retains_server_confirmed_send() {
    let confirmed = confirmed_sends([message("200.000001", "sent")]);
    let (snapshot, unresolved) =
        reconcile_slack_confirmed_sends(snapshot([message("100.000001", "older")]), &confirmed)
            .expect("stale history should reconcile");

    assert_eq!(
        snapshot
            .messages
            .iter()
            .map(|message| message.body.as_str())
            .collect::<Vec<_>>(),
        ["older", "sent"]
    );
    assert_eq!(unresolved, confirmed);
}

#[test]
fn observed_remote_message_releases_confirmed_send() {
    let confirmed = confirmed_sends([message("200.000001", "receipt")]);
    let remote = message("200.000001", "remote enrichment");
    let (snapshot, unresolved) = reconcile_slack_confirmed_sends(
        snapshot([message("100.000001", "older"), remote.clone()]),
        &confirmed,
    )
    .expect("observed send should reconcile");

    assert_eq!(snapshot.messages.last(), Some(&remote));
    assert!(unresolved.is_empty());
}

#[test]
fn newer_remote_window_retains_absent_confirmed_send_until_exact_observation() {
    let confirmed = confirmed_sends([message("200.000001", "deleted")]);
    let (snapshot, unresolved) = reconcile_slack_confirmed_sends(
        snapshot([
            message("100.000001", "older"),
            message("300.000001", "newer"),
        ]),
        &confirmed,
    )
    .expect("newer history should reconcile");

    assert_eq!(snapshot.messages.len(), 3);
    assert_eq!(unresolved, confirmed);
}

#[test]
fn empty_remote_window_retains_server_confirmed_send() {
    let confirmed = confirmed_sends([message("200.000001", "sent")]);
    let (snapshot, unresolved) = reconcile_slack_confirmed_sends(snapshot([]), &confirmed)
        .expect("empty history should reconcile");

    assert_eq!(snapshot.messages, vec![message("200.000001", "sent")]);
    assert_eq!(unresolved, confirmed);
}

#[test]
fn anchored_remote_observation_wins_over_cached_receipt() {
    let confirmed = confirmed_sends([message("200.000001", "receipt")]);
    let remote = message("200.000001", "remote enrichment");
    let merged = merge_slack_conversation_windows(
        snapshot([message("300.000001", "latest")]),
        snapshot([message("100.000001", "older"), remote.clone()]),
    )
    .expect("remote windows should merge");
    let (snapshot, unresolved) = reconcile_slack_confirmed_sends(merged, &confirmed)
        .expect("merged history should reconcile");

    assert_eq!(
        snapshot
            .messages
            .iter()
            .find(|message| message.id == remote.id),
        Some(&remote)
    );
    assert!(unresolved.is_empty());
}

#[test]
fn duplicate_receipt_is_idempotent_and_conflict_fails() {
    let receipt = message("200.000001", "sent");
    let timestamp = slack_message_timestamp_key(&receipt.id).expect("timestamp should parse");
    let mut confirmed = BTreeMap::new();

    assert!(
        insert_slack_confirmed_send(&mut confirmed, timestamp, receipt.clone())
            .expect("first receipt should record")
    );
    assert!(
        !insert_slack_confirmed_send(&mut confirmed, timestamp, receipt)
            .expect("duplicate receipt should be idempotent")
    );
    assert!(insert_slack_confirmed_send(
        &mut confirmed,
        timestamp,
        message("200.000001", "conflict"),
    )
    .is_err());
}

#[test]
fn confirmed_send_cap_evicts_oldest_without_failing_delivery() {
    let mut confirmed = BTreeMap::new();
    for second in 0..=100 {
        let id = format!("{}.000001", 1_700_000_000 + second);
        let message = message(&id, &format!("sent {second}"));
        let timestamp = slack_message_timestamp_key(&id).expect("timestamp should parse");
        insert_slack_confirmed_send(&mut confirmed, timestamp, message)
            .expect("ledger cap must not fail a confirmed send");
    }

    assert_eq!(confirmed.len(), 100);
    assert!(!confirmed.contains_key(&(1_700_000_000, 1_000)));
    assert!(confirmed.contains_key(&(1_700_000_100, 1_000)));
}

#[test]
fn explicit_delete_removal_prevents_receipt_resurrection() {
    let mut confirmed = confirmed_sends([message("200.000001", "sent")]);
    confirmed.remove(
        &slack_message_timestamp_key("200.000001").expect("deleted timestamp should parse"),
    );
    let (snapshot, unresolved) =
        reconcile_slack_confirmed_sends(snapshot([message("100.000001", "older")]), &confirmed)
            .expect("post-delete history should reconcile");

    assert_eq!(snapshot.messages, vec![message("100.000001", "older")]);
    assert!(unresolved.is_empty());
}

#[test]
#[ignore = "performance profile"]
fn profile_confirmed_send_reconciliation() {
    let mut remote = snapshot([]);
    remote.messages = (0..2_000)
        .map(|index| message(&format!("{}.{:06}", 1_700_000_000 + index, index), "remote"))
        .collect();
    let confirmed = confirmed_sends((0..100).map(|index| {
        message(
            &format!("{}.{:06}", 1_800_000_000 + index, index),
            "confirmed",
        )
    }));
    let frames = std::env::var("NOTSLACK_SLACK_PROFILE_FRAMES")
        .ok()
        .map(|value| value.parse::<usize>().expect("profile frame count"))
        .unwrap_or(120);
    let mut samples_ms = Vec::with_capacity(frames);

    for _ in 0..frames {
        let started = Instant::now();
        let reconciled = reconcile_slack_confirmed_sends(remote.clone(), &confirmed)
            .expect("profile history should reconcile");
        std::hint::black_box(reconciled);
        samples_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
    }

    samples_ms.sort_by(f64::total_cmp);
    let percentile = |percent: usize| {
        let index = ((samples_ms.len() - 1) * percent).div_ceil(100);
        samples_ms[index]
    };
    let mean = samples_ms.iter().sum::<f64>() / samples_ms.len() as f64;
    let spikes_16 = samples_ms.iter().filter(|sample| **sample > 16.67).count();
    let spikes_33 = samples_ms.iter().filter(|sample| **sample > 33.33).count();

    eprintln!(
        "confirmed-send reconciliation: frames={frames} mean={mean:.4}ms p50={:.4}ms \
         p95={:.4}ms p99={:.4}ms max={:.4}ms spikes>16.67ms={spikes_16} \
         spikes>33.33ms={spikes_33}",
        percentile(50),
        percentile(95),
        percentile(99),
        samples_ms[samples_ms.len() - 1],
    );
}

fn confirmed_sends(
    messages: impl IntoIterator<Item = SlackMessage>,
) -> BTreeMap<(u64, u32), SlackMessage> {
    messages
        .into_iter()
        .map(|message| {
            (
                slack_message_timestamp_key(&message.id)
                    .expect("test message timestamp should be valid"),
                message,
            )
        })
        .collect()
}

fn snapshot<const N: usize>(messages: [SlackMessage; N]) -> SlackConversationSnapshot {
    SlackConversationSnapshot {
        team_id: "T_TEST".to_string(),
        conversation_id: "C_TEST".to_string(),
        self_timezone_id: None,
        channel_kind: SlackConversationKind::Channel,
        channel_name: "test".to_string(),
        channel_topic: String::new(),
        member_count: None,
        tabs: Vec::new(),
        messages: messages.into(),
        last_read: None,
        last_read_boundary_loaded: true,
        history_next_cursor: None,
        mention_suggestions: Vec::new(),
        emoji_picker_sections: Vec::new(),
        composer_notice: None,
        peer_notifications_paused: false,
        dm_peer_local_time_context: None,
        composer_draft_text: None,
        composer_placeholder: "Message #test".to_string(),
    }
}

fn message(id: &str, body: &str) -> SlackMessage {
    SlackMessage {
        id: id.to_string(),
        client_message_id: None,
        author: "You".to_string(),
        timestamp: "Now".to_string(),
        user_id: Some("U_SELF".to_string()),
        avatar_label: None,
        avatar_image_url: None,
        avatar_image_base64: None,
        avatar_image_mimetype: None,
        body: body.to_string(),
        rich_body: None,
        table_rows: Vec::new(),
        date_divider_label: None,
        edited_label: None,
        attachments: Vec::new(),
        reactions: Vec::new(),
        saved_state: None,
        reply_count: None,
        latest_reply_timestamp: None,
        reply_participants: Vec::new(),
        replies: Vec::new(),
    }
}
