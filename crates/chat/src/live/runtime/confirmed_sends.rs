use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
};

use crate::model::{SlackConversationSnapshot, SlackMessage, SlackMessageSendReceipt};

use crate::live::cache::SLACK_CONFIRMED_SEND_CACHE_LIMIT;

use super::SlackWorkspaceRuntime;

pub(super) type SlackConversationMessageBounds = ((u64, u32), (u64, u32));
pub(super) type SlackMessageTimestampKey = (u64, u32);
pub(super) type SlackConfirmedConversationSends = BTreeMap<SlackMessageTimestampKey, SlackMessage>;
pub(super) type SlackConfirmedConversationLedger = Arc<Mutex<SlackConfirmedConversationSends>>;
pub(super) type SlackConfirmedSendMap = HashMap<String, SlackConfirmedConversationLedger>;
pub(super) type SlackConfirmedSendReconciliation =
    (SlackConversationSnapshot, SlackConfirmedConversationSends);

impl SlackWorkspaceRuntime {
    pub(super) fn confirmed_send_ledger(
        &self,
        conversation_id: &str,
    ) -> Result<SlackConfirmedConversationLedger, String> {
        if let Some(ledger) = self
            .confirmed_sends
            .lock()
            .map_err(|_| "Slack confirmed-send map mutex poisoned".to_string())?
            .get(conversation_id)
            .cloned()
        {
            return Ok(ledger);
        }
        let cached_messages = self
            .cache()
            .map(|cache| cache.load_confirmed_sends(conversation_id))
            .transpose()?
            .unwrap_or_default()
            .into_iter()
            .map(|message| slack_message_timestamp_key(&message.id).map(|key| (key, message)))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        let loaded = Arc::new(Mutex::new(cached_messages));
        let mut confirmed_sends = self
            .confirmed_sends
            .lock()
            .map_err(|_| "Slack confirmed-send map mutex poisoned".to_string())?;
        Ok(confirmed_sends
            .entry(conversation_id.to_string())
            .or_insert(loaded)
            .clone())
    }

    pub(super) fn merge_cached_confirmed_sends(
        &self,
        mut conversation: SlackConversationSnapshot,
    ) -> Result<SlackConversationSnapshot, String> {
        let ledger = self.confirmed_send_ledger(&conversation.conversation_id)?;
        let confirmed = ledger
            .lock()
            .map_err(|_| "Slack confirmed-send mutex poisoned".to_string())?;
        if confirmed.is_empty() {
            return Ok(conversation);
        }
        let mut messages = std::mem::take(&mut conversation.messages)
            .into_iter()
            .map(|message| slack_message_timestamp_key(&message.id).map(|key| (key, message)))
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        for (timestamp, message) in confirmed.iter() {
            messages
                .entry(*timestamp)
                .or_insert_with(|| message.clone());
        }
        conversation.messages = messages.into_values().collect();
        Ok(conversation)
    }

    pub(super) fn record_confirmed_send(
        &self,
        ledger: &SlackConfirmedConversationLedger,
        receipt: &SlackMessageSendReceipt,
    ) {
        let timestamp = slack_message_timestamp_key(&receipt.timestamp)
            .expect("validated Slack send receipt must contain a valid timestamp");
        let Ok(mut confirmed) = ledger.lock() else {
            eprintln!(
                "Slack confirmed-send bookkeeping failed after delivery: mutex poisoned"
            );
            return;
        };
        match insert_slack_confirmed_send(&mut confirmed, timestamp, receipt.message.clone()) {
            Ok(true) => self.persist_confirmed_sends(&receipt.conversation_id, &confirmed),
            Ok(false) => {}
            Err(error) => {
                eprintln!("Slack confirmed-send bookkeeping failed after delivery: {error}");
            }
        }
    }

    pub(super) fn reconcile_remote_conversation(
        &self,
        conversation: SlackConversationSnapshot,
    ) -> Result<SlackConversationSnapshot, String> {
        if conversation.team_id != self.loader.team_id() {
            return Err(
                "Slack conversation refresh returned a mismatched runtime team".to_string(),
            );
        }
        let ledger = self.confirmed_send_ledger(&conversation.conversation_id)?;
        let mut confirmed = ledger
            .lock()
            .map_err(|_| "Slack confirmed-send mutex poisoned".to_string())?;
        let (mut conversation, unresolved) =
            reconcile_slack_confirmed_sends(conversation, &confirmed)?;
        if *confirmed != unresolved {
            *confirmed = unresolved;
            self.persist_confirmed_sends(&conversation.conversation_id, &confirmed);
        }
        drop(confirmed);
        if let Some(cache) = self.cache() {
            cache.reconcile_conversation_read_receipts(&mut conversation)?;
        }
        Ok(conversation)
    }

    pub(super) fn forget_confirmed_send(
        &self,
        conversation_id: &str,
        timestamp: &crate::model::SlackMessageTimestamp,
    ) -> Result<(), String> {
        let ledger = self.confirmed_send_ledger(conversation_id)?;
        let mut confirmed = ledger
            .lock()
            .map_err(|_| "Slack confirmed-send mutex poisoned".to_string())?;
        if confirmed.remove(&timestamp.sort_key()).is_some() {
            self.persist_confirmed_sends(conversation_id, &confirmed);
        }
        Ok(())
    }

    fn persist_confirmed_sends(
        &self,
        conversation_id: &str,
        confirmed: &SlackConfirmedConversationSends,
    ) {
        if let Some(cache) = self.cache() {
            let messages = confirmed.values().cloned().collect::<Vec<_>>();
            cache.persist_confirmed_sends(conversation_id, &messages);
        }
    }
}

pub(super) fn insert_slack_confirmed_send(
    confirmed: &mut SlackConfirmedConversationSends,
    timestamp: SlackMessageTimestampKey,
    message: SlackMessage,
) -> Result<bool, String> {
    match confirmed.get(&timestamp) {
        Some(existing) if existing != &message => Err(format!(
            "Slack returned conflicting send receipts for timestamp {}",
            message.id
        )),
        Some(_) => Ok(false),
        None => {
            confirmed.insert(timestamp, message);
            while confirmed.len() > SLACK_CONFIRMED_SEND_CACHE_LIMIT {
                confirmed.pop_first();
            }
            Ok(true)
        }
    }
}

pub(super) fn reconcile_slack_confirmed_sends(
    mut conversation: SlackConversationSnapshot,
    confirmed: &SlackConfirmedConversationSends,
) -> Result<SlackConfirmedSendReconciliation, String> {
    validate_slack_conversation_message_order(&conversation.messages)?;
    if confirmed.is_empty() {
        return Ok((conversation, BTreeMap::new()));
    }
    let mut remote_messages = std::mem::take(&mut conversation.messages)
        .into_iter()
        .map(|message| slack_message_timestamp_key(&message.id).map(|key| (key, message)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut unresolved = BTreeMap::new();
    for (timestamp, message) in confirmed {
        if remote_messages.contains_key(timestamp) {
            continue;
        }
        remote_messages.insert(*timestamp, message.clone());
        unresolved.insert(*timestamp, message.clone());
    }
    conversation.messages = remote_messages.into_values().collect();
    Ok((conversation, unresolved))
}

pub(super) fn merge_slack_conversation_windows(
    mut latest: SlackConversationSnapshot,
    anchored: SlackConversationSnapshot,
) -> Result<SlackConversationSnapshot, String> {
    if latest.team_id != anchored.team_id || latest.conversation_id != anchored.conversation_id {
        return Err(
            "Slack permalink navigation returned mismatched conversation windows".to_string(),
        );
    }
    let mut messages = anchored
        .messages
        .into_iter()
        .map(|message| slack_message_timestamp_key(&message.id).map(|key| (key, message)))
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    for message in std::mem::take(&mut latest.messages) {
        messages.insert(slack_message_timestamp_key(&message.id)?, message);
    }
    latest.messages = messages.into_values().collect();
    latest.history_next_cursor = anchored.history_next_cursor;
    Ok(latest)
}

pub(super) fn slack_message_timestamp_key(timestamp: &str) -> Result<(u64, u32), String> {
    let (seconds, fractional) = timestamp.split_once('.').ok_or_else(|| {
        format!("Slack conversation navigation received invalid timestamp {timestamp:?}")
    })?;
    if fractional.is_empty() || fractional.len() > 9 {
        return Err(format!(
            "Slack conversation navigation received invalid timestamp {timestamp:?}"
        ));
    }
    let fractional_digits = fractional.len();
    let seconds = seconds.parse::<u64>().map_err(|_| {
        format!("Slack conversation navigation received invalid timestamp {timestamp:?}")
    })?;
    let fractional = fractional.parse::<u32>().map_err(|_| {
        format!("Slack conversation navigation received invalid timestamp {timestamp:?}")
    })?;
    let nanoseconds = fractional
        .checked_mul(
            10_u32.pow(u32::try_from(9 - fractional_digits).map_err(|_| {
                format!("Slack conversation navigation received invalid timestamp {timestamp:?}")
            })?),
        )
        .ok_or_else(|| {
            format!("Slack conversation navigation received invalid timestamp {timestamp:?}")
        })?;
    Ok((seconds, nanoseconds))
}

pub(super) fn validate_slack_conversation_message_order(
    messages: &[SlackMessage],
) -> Result<Option<SlackConversationMessageBounds>, String> {
    let mut keys = messages
        .iter()
        .map(|message| slack_message_timestamp_key(&message.id));
    let Some(oldest) = keys.next().transpose()? else {
        return Ok(None);
    };
    let mut previous = oldest;
    let mut newest = oldest;
    for key in keys {
        let key = key?;
        if key <= previous {
            return Err(
                "Slack conversation history must use strict oldest-first timestamp order"
                    .to_string(),
            );
        }
        previous = key;
        newest = key;
    }
    Ok(Some((oldest, newest)))
}
