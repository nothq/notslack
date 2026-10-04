use crate::model::{SlackRealtimeBatch, SlackRealtimeNotification, SlackRealtimeRecvError};

use super::{SlackTeamNotificationPolicySource, SlackTeamRealtimeSource, SlackWorkspaceRuntime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SlackNotificationPolicyState {
    Unchanged,
    RefreshRequested,
}

pub(super) enum SlackNotificationPolicyUpdateOutcome {
    Applied(crate::model::SlackNotificationTeamBadge),
    Superseded,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SlackRealtimePresentationEvent {
    Published {
        notifications: Vec<SlackRealtimeNotification>,
        notification_policy: SlackNotificationPolicyState,
    },
    Closed,
}

impl SlackTeamRealtimeSource {
    pub(super) fn team_id(&self) -> &str {
        &self.team_id
    }

    pub(super) async fn recv_presentation_event(&mut self) -> SlackRealtimePresentationEvent {
        let mut batch = match self.recv().await {
            Ok(batch) => batch,
            Err(SlackRealtimeRecvError::Lagged) => SlackRealtimeBatch::resync(),
            Err(SlackRealtimeRecvError::Closed) => {
                self.publish_presentation_batch(SlackRealtimeBatch::closed());
                return SlackRealtimePresentationEvent::Closed;
            }
        };
        let notification_policy = if batch.full_resync || batch.sidebar_changed {
            SlackNotificationPolicyState::RefreshRequested
        } else {
            SlackNotificationPolicyState::Unchanged
        };
        let notifications = std::mem::take(&mut batch.notifications);
        self.publish_presentation_batch(batch);
        SlackRealtimePresentationEvent::Published {
            notifications,
            notification_policy,
        }
    }

    async fn recv(&mut self) -> Result<SlackRealtimeBatch, SlackRealtimeRecvError> {
        loop {
            if self.adopt_replacement().await {
                return Ok(SlackRealtimeBatch::resync());
            }
            enum RealtimeReceiveEvent {
                Batch(Result<SlackRealtimeBatch, crate::model::SlackRealtimeRecvError>),
                Replacement(Result<(), tokio::sync::watch::error::RecvError>),
            }
            let event = {
                let subscription = &mut self.subscription;
                let replacement_generation = &mut self.replacement_generation;
                tokio::select! {
                    result = subscription.recv() => RealtimeReceiveEvent::Batch(result),
                    result = replacement_generation.changed() => {
                        RealtimeReceiveEvent::Replacement(result)
                    }
                }
            };
            match event {
                RealtimeReceiveEvent::Replacement(Ok(())) => continue,
                RealtimeReceiveEvent::Replacement(Err(_)) => {
                    return Err(crate::model::SlackRealtimeRecvError::Closed);
                }
                RealtimeReceiveEvent::Batch(result) => match self.slot.snapshot() {
                    Ok(snapshot) if snapshot.generation != self.generation => continue,
                    Ok(_) => return result,
                    Err(error) => {
                        eprintln!(
                            "failed to inspect replacement Slack runtime for team {}: {error}",
                            self.team_id
                        );
                        return Err(crate::model::SlackRealtimeRecvError::Closed);
                    }
                },
            }
        }
    }

    async fn adopt_replacement(&mut self) -> bool {
        loop {
            match self.slot.claim_realtime(Some(self.generation)) {
                Ok(None) => return false,
                Ok(Some(claim)) => {
                    self.generation = claim.generation;
                    self.runtime = claim.runtime;
                    self.subscription = claim.subscription;
                    drop(self.replacement_generation.borrow_and_update());
                    return true;
                }
                Err(error) => {
                    eprintln!(
                        "failed to claim replacement Slack realtime runtime for team {}: {error}",
                        self.team_id
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
        }
    }

    fn publish_presentation_batch(&self, batch: SlackRealtimeBatch) {
        match self.slot.snapshot() {
            Ok(snapshot) if snapshot.generation == self.generation => {
                self.runtime.publish_presentation_batch(batch);
            }
            Ok(_) => {}
            Err(error) => eprintln!(
                "failed to verify Slack presentation runtime for team {}: {error}",
                self.team_id
            ),
        }
    }

    pub(super) fn notification_policy_source(&self) -> SlackTeamNotificationPolicySource {
        SlackTeamNotificationPolicySource {
            team_id: self.team_id.clone(),
            slot: self.slot.clone(),
        }
    }

    pub(super) fn should_deliver_notification(
        &self,
        notification: &crate::model::SlackRealtimeNotification,
    ) -> Result<bool, String> {
        let snapshot = self.slot.snapshot()?;
        if snapshot.generation != self.generation {
            return Ok(false);
        }
        self.runtime.should_deliver_notification(notification)
    }
}

impl SlackTeamNotificationPolicySource {
    pub(super) fn team_id(&self) -> &str {
        &self.team_id
    }

    pub(super) fn initialize_notification_policy(
        &self,
    ) -> Result<SlackNotificationPolicyUpdateOutcome, String> {
        self.update_notification_policy(SlackWorkspaceRuntime::initialize_notification_policy)
    }

    pub(super) fn refresh_notification_policy(
        &self,
    ) -> Result<SlackNotificationPolicyUpdateOutcome, String> {
        self.update_notification_policy(SlackWorkspaceRuntime::refresh_notification_policy)
    }

    fn update_notification_policy(
        &self,
        update: impl FnOnce(&SlackWorkspaceRuntime) -> Result<(), String>,
    ) -> Result<SlackNotificationPolicyUpdateOutcome, String> {
        let snapshot = self.slot.snapshot()?;
        let update_result =
            update(&snapshot.runtime).and_then(|()| snapshot.runtime.notification_badge_count());
        let current_generation = self.slot.snapshot()?.generation;
        if current_generation != snapshot.generation {
            return Ok(SlackNotificationPolicyUpdateOutcome::Superseded);
        }
        Ok(SlackNotificationPolicyUpdateOutcome::Applied(
            crate::model::SlackNotificationTeamBadge {
                team_id: self.team_id.clone(),
                count: update_result?,
            },
        ))
    }
}
