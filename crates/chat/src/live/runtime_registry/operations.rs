use std::sync::mpsc;

use crate::model::{SlackMessageTimestamp, SlackWorkspaceApi};

use super::{
    SlackNotificationReadReceipt, SlackNotificationReplyReceipt, SlackNotificationReplyRequest,
    SlackTeamRealtimeSource, SlackWorkspaceRuntime, SlackWorkspaceRuntimeRegistry,
};

impl SlackWorkspaceRuntimeRegistry {
    pub(super) fn claim_realtime_sources(&self) -> Result<Vec<SlackTeamRealtimeSource>, String> {
        self.ordered_teams
            .iter()
            .map(|team| {
                let slot = self.runtime_slot(&team.team_id)?;
                let replacement_generation = slot.replacement_generation.subscribe();
                let claim = slot
                    .claim_realtime(None)?
                    .expect("an unclaimed Slack realtime source must have a current runtime");
                Ok(SlackTeamRealtimeSource {
                    team_id: team.team_id.clone(),
                    slot,
                    generation: claim.generation,
                    runtime: claim.runtime,
                    subscription: claim.subscription,
                    replacement_generation,
                })
            })
            .collect()
    }

    pub(super) fn claim_notification_read_receipts(
        &self,
    ) -> Result<mpsc::Receiver<SlackNotificationReadReceipt>, String> {
        self.notification_read_receipts
            .lock()
            .map_err(|_| "Slack notification read receipt mutex poisoned".to_string())?
            .take()
            .ok_or_else(|| "Slack notification read receipts were already claimed".to_string())
    }

    pub(super) fn require_realtime_sources_running(&self) -> Result<(), String> {
        let mut failures = Vec::new();
        for team in self.ordered_teams.iter() {
            let result = self
                .runtime_slot(&team.team_id)
                .and_then(|slot| slot.snapshot())
                .and_then(|snapshot| snapshot.runtime.require_realtime_running());
            if let Err(error) = result {
                failures.push(format!("team {}: {error}", team.team_id));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Slack realtime source health check failed: {}",
                failures.join("; ")
            ))
        }
    }

    pub(super) fn stop_realtime_sources(&self) -> Result<(), String> {
        let mut failures = Vec::new();
        for team in self.ordered_teams.iter() {
            let result = self
                .runtime_slot(&team.team_id)
                .and_then(|slot| slot.snapshot())
                .and_then(|snapshot| snapshot.runtime.retire_for_replacement());
            if let Err(error) = result {
                failures.push(format!("team {}: {error}", team.team_id));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "failed to stop Slack realtime sources: {}",
                failures.join("; ")
            ))
        }
    }

    pub(super) fn send_notification_reply(
        &self,
        request: &SlackNotificationReplyRequest,
    ) -> Result<SlackMessageTimestamp, String> {
        if request.conversation_id.trim().is_empty() {
            return Err("Slack notification reply is missing its conversation".to_string());
        }
        if request.team_id.trim().is_empty() {
            return Err("Slack notification reply is missing its team".to_string());
        }
        let runtime = self.runtime(&request.team_id)?;
        let result = send_notification_reply(runtime.as_ref(), request)?;
        notification_reply_timestamp(request, result)
            .ok_or_else(|| "Slack notification reply returned a mismatched receipt".to_string())
    }
}

fn send_notification_reply(
    runtime: &SlackWorkspaceRuntime,
    request: &SlackNotificationReplyRequest,
) -> Result<SlackNotificationReplyReceipt, String> {
    match request.thread_timestamp.as_ref() {
        Some(thread_timestamp) => runtime
            .send_slack_thread_reply(
                crate::model::SlackThreadReplyTarget {
                    conversation_id: &request.conversation_id,
                    thread_timestamp,
                    broadcast: false,
                },
                &request.client_message_id,
                &request.draft,
            )
            .map(SlackNotificationReplyReceipt::Thread),
        None => runtime
            .send_slack_message(
                &request.conversation_id,
                &request.client_message_id,
                &request.draft,
            )
            .map(SlackNotificationReplyReceipt::Conversation),
    }
}

fn notification_reply_timestamp(
    request: &SlackNotificationReplyRequest,
    result: SlackNotificationReplyReceipt,
) -> Option<SlackMessageTimestamp> {
    match result {
        SlackNotificationReplyReceipt::Conversation(receipt) => {
            if request.thread_timestamp.is_none()
                && receipt.team_id == request.team_id
                && receipt.conversation_id == request.conversation_id
                && receipt.message.client_message_id.as_ref() == Some(&request.client_message_id)
            {
                SlackMessageTimestamp::parse(&receipt.timestamp).ok()
            } else {
                None
            }
        }
        SlackNotificationReplyReceipt::Thread(receipt) => {
            if request
                .thread_timestamp
                .as_ref()
                .is_some_and(|thread_timestamp| {
                    receipt.team_id == request.team_id
                        && receipt.conversation_id == request.conversation_id
                        && receipt.thread_timestamp == thread_timestamp.as_str()
                        && receipt.reply.client_message_id.as_ref()
                            == Some(&request.client_message_id)
                })
            {
                SlackMessageTimestamp::parse(&receipt.reply.timestamp).ok()
            } else {
                None
            }
        }
    }
}
