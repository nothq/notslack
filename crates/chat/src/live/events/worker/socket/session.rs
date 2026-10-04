use futures_util::{SinkExt, StreamExt};
use std::{sync as std_sync, time};
use tokio::{sync as tokio_sync, time as tokio_time};
use tokio_tungstenite::tungstenite::{self, Message};

use super::{SlackSocketOutcome, SlackSocketReconnectReason, SlackSocketRunContext};
use crate::{
    live::{
        api,
        events::{
            wire::{self, SlackRealtimeWireAction},
            worker::{accumulator, presence},
            SlackPresenceSubscriptionIds,
        },
    },
    model,
};

enum SlackSocketEvent {
    Stop,
    Flush,
    InitialPresence,
    PresenceChanged(bool),
    Heartbeat,
    Message(Option<Result<Message, tungstenite::Error>>),
}

pub(super) struct SlackSocketSession<'a> {
    write: super::super::SlackRealtimeSocketWriter,
    read: super::super::SlackRealtimeSocketReader,
    sender: &'a tokio_sync::broadcast::Sender<model::SlackRealtimeBatch>,
    presence_subscription_ids: &'a mut tokio_sync::watch::Receiver<SlackPresenceSubscriptionIds>,
    presence_snapshot: &'a tokio_sync::watch::Sender<model::SlackRealtimePresenceSnapshot>,
    presence_commit: &'a std_sync::Mutex<()>,
    stop: &'a mut tokio_sync::watch::Receiver<bool>,
    context: SlackSocketRunContext<'a>,
    accumulator: accumulator::SlackRealtimeAccumulator,
    flush: tokio_time::Interval,
    heartbeat: tokio_time::Interval,
    presence_send: std::pin::Pin<Box<tokio_time::Sleep>>,
    outbound_id: u64,
    last_pong: time::Instant,
    reconnect_url: Option<api::SlackRealtimeSocketUrl>,
    connected: bool,
    hello_seen: bool,
    hello_at: Option<time::Instant>,
    malformed_resync_sent: bool,
    desired_presence_ids: SlackPresenceSubscriptionIds,
    sent_presence_ids: SlackPresenceSubscriptionIds,
    presence_send_pending: bool,
    initial_presence_sent: bool,
}

impl<'a> SlackSocketSession<'a> {
    pub(super) fn new(
        socket: super::super::SlackRealtimeSocket,
        sender: &'a tokio_sync::broadcast::Sender<model::SlackRealtimeBatch>,
        presence: presence::SlackSocketPresence<'a>,
        stop: &'a mut tokio_sync::watch::Receiver<bool>,
        context: SlackSocketRunContext<'a>,
    ) -> Self {
        let presence::SlackSocketPresence {
            subscription_ids: presence_subscription_ids,
            snapshot: presence_snapshot,
            commit: presence_commit,
        } = presence;
        let (write, read) = socket.split();
        let mut flush = tokio_time::interval_at(
            tokio_time::Instant::now() + super::super::SLACK_REALTIME_COALESCE_INTERVAL,
            super::super::SLACK_REALTIME_COALESCE_INTERVAL,
        );
        flush.set_missed_tick_behavior(tokio_time::MissedTickBehavior::Skip);
        let mut heartbeat = tokio_time::interval_at(
            tokio_time::Instant::now() + super::super::SLACK_REALTIME_HEARTBEAT_INTERVAL,
            super::super::SLACK_REALTIME_HEARTBEAT_INTERVAL,
        );
        heartbeat.set_missed_tick_behavior(tokio_time::MissedTickBehavior::Delay);
        let (desired_presence_ids, initial_snapshot) = presence::refresh_presence_roster(
            presence_commit,
            presence_subscription_ids,
            presence_snapshot,
        );
        let mut accumulator = accumulator::SlackRealtimeAccumulator::default();
        if let Some(snapshot) = initial_snapshot {
            accumulator.merge(presence::presence_snapshot_batch(snapshot));
        }
        Self {
            write,
            read,
            sender,
            presence_subscription_ids,
            presence_snapshot,
            presence_commit,
            stop,
            context,
            accumulator,
            flush,
            heartbeat,
            presence_send: Box::pin(tokio_time::sleep(time::Duration::from_secs(24 * 60 * 60))),
            outbound_id: super::super::SLACK_REALTIME_INITIAL_PING_ID,
            last_pong: time::Instant::now(),
            reconnect_url: None,
            connected: false,
            hello_seen: false,
            hello_at: None,
            malformed_resync_sent: false,
            desired_presence_ids,
            sent_presence_ids: None,
            presence_send_pending: false,
            initial_presence_sent: false,
        }
    }

    pub(super) async fn run(mut self) -> SlackSocketOutcome {
        loop {
            let event = tokio::select! {
                changed = self.stop.changed() => {
                    let _ = changed;
                    SlackSocketEvent::Stop
                }
                _ = self.flush.tick() => SlackSocketEvent::Flush,
                _ = self.presence_send.as_mut(), if self.presence_send_pending => {
                    SlackSocketEvent::InitialPresence
                }
                changed = self.presence_subscription_ids.changed() => {
                    SlackSocketEvent::PresenceChanged(changed.is_ok())
                }
                _ = self.heartbeat.tick() => SlackSocketEvent::Heartbeat,
                message = self.read.next() => SlackSocketEvent::Message(message),
            };
            if let Some(outcome) = self.handle_event(event).await {
                return outcome;
            }
        }
    }

    async fn handle_event(&mut self, event: SlackSocketEvent) -> Option<SlackSocketOutcome> {
        match event {
            SlackSocketEvent::Stop => Some(self.stop().await),
            SlackSocketEvent::Flush => {
                accumulator::flush_accumulator(&mut self.accumulator, self.sender);
                None
            }
            SlackSocketEvent::InitialPresence => self.send_initial_presence().await,
            SlackSocketEvent::PresenceChanged(receiver_open) => {
                self.update_presence(receiver_open).await
            }
            SlackSocketEvent::Heartbeat => self.send_heartbeat().await,
            SlackSocketEvent::Message(message) => self.handle_message(message).await,
        }
    }

    async fn stop(&mut self) -> SlackSocketOutcome {
        let _ = self.write.close().await;
        accumulator::flush_accumulator(&mut self.accumulator, self.sender);
        SlackSocketOutcome::Stopped
    }

    async fn send_initial_presence(&mut self) -> Option<SlackSocketOutcome> {
        self.presence_send_pending = false;
        self.refresh_presence();
        let ids = self.desired_presence_ids.as_deref().unwrap_or_default();
        let subscription_id = self.outbound_id;
        if presence::send_slack_presence_subscription(&mut self.write, &mut self.outbound_id, ids)
            .await
            .is_err()
        {
            return Some(self.reconnect(SlackSocketReconnectReason::InitialPresenceSend));
        }
        self.context
            .record_presence_subscription(true, subscription_id, ids.len(), self.hello_at);
        self.sent_presence_ids = self.desired_presence_ids.clone();
        self.initial_presence_sent = true;
        None
    }

    async fn update_presence(&mut self, receiver_open: bool) -> Option<SlackSocketOutcome> {
        if !receiver_open {
            return Some(self.stop().await);
        }
        self.refresh_presence();
        if !self.connected
            || !self.initial_presence_sent
            || self.desired_presence_ids == self.sent_presence_ids
        {
            return None;
        }
        let ids = self.desired_presence_ids.as_deref().unwrap_or_default();
        let subscription_id = self.outbound_id;
        if presence::send_slack_presence_subscription(&mut self.write, &mut self.outbound_id, ids)
            .await
            .is_err()
        {
            return Some(self.reconnect(SlackSocketReconnectReason::ReplacementPresenceSend));
        }
        self.context
            .record_presence_subscription(false, subscription_id, ids.len(), None);
        self.sent_presence_ids = self.desired_presence_ids.clone();
        None
    }

    async fn send_heartbeat(&mut self) -> Option<SlackSocketOutcome> {
        if self.last_pong.elapsed() >= super::super::SLACK_REALTIME_PONG_TIMEOUT {
            return Some(self.reconnect(SlackSocketReconnectReason::PongTimeout));
        }
        let ping_id = self.outbound_id;
        if presence::send_slack_ping(&mut self.write, &mut self.outbound_id)
            .await
            .is_err()
        {
            return Some(self.reconnect(SlackSocketReconnectReason::PingSend));
        }
        self.context.record_ping(ping_id);
        None
    }

    async fn handle_message(
        &mut self,
        message: Option<Result<Message, tungstenite::Error>>,
    ) -> Option<SlackSocketOutcome> {
        let message = match message {
            Some(Ok(message)) => message,
            Some(Err(error)) => {
                return Some(
                    self.reconnect(SlackSocketReconnectReason::StreamRead((&error).into())),
                )
            }
            None => return Some(self.reconnect(SlackSocketReconnectReason::StreamEnded)),
        };
        match message {
            Message::Text(body) => self.handle_text(body.as_str()),
            Message::Ping(payload) => {
                if self.write.send(Message::Pong(payload)).await.is_err() {
                    Some(self.reconnect(SlackSocketReconnectReason::PongSend))
                } else {
                    None
                }
            }
            Message::Pong(_) => {
                self.last_pong = time::Instant::now();
                None
            }
            Message::Close(frame) => Some(self.reconnect(SlackSocketReconnectReason::CloseFrame(
                frame.map(|frame| u16::from(frame.code)),
            ))),
            _ => None,
        }
    }

    fn handle_text(&mut self, body: &str) -> Option<SlackSocketOutcome> {
        match wire::parse_slack_realtime_event(body) {
            Ok(action) => self.handle_action(action),
            Err(_) => {
                handle_slack_realtime_parse_error(
                    self.context,
                    self.connected,
                    &mut self.malformed_resync_sent,
                    &mut self.accumulator,
                );
                None
            }
        }
    }

    fn handle_action(&mut self, action: SlackRealtimeWireAction) -> Option<SlackSocketOutcome> {
        match action {
            SlackRealtimeWireAction::Hello => self.handle_hello(),
            SlackRealtimeWireAction::Reconnect(url) => {
                self.reconnect_url = Some(url);
                None
            }
            SlackRealtimeWireAction::Pong => {
                self.last_pong = time::Instant::now();
                None
            }
            SlackRealtimeWireAction::Goodbye => {
                Some(self.reconnect(SlackSocketReconnectReason::Goodbye))
            }
            SlackRealtimeWireAction::Impact(batch) => {
                self.handle_impact(batch);
                None
            }
            SlackRealtimeWireAction::MalformedPresence | SlackRealtimeWireAction::Ignore => None,
        }
    }

    fn handle_hello(&mut self) -> Option<SlackSocketOutcome> {
        self.connected = true;
        if !self.hello_seen {
            self.hello_at = self.context.record_hello();
            self.presence_send_pending = true;
            self.presence_send.as_mut().reset(
                tokio_time::Instant::now() + super::super::SLACK_REALTIME_PRESENCE_SUB_DELAY,
            );
        }
        self.hello_seen = true;
        let mut batch = if self.context.resync_on_hello {
            model::SlackRealtimeBatch::resync()
        } else {
            model::SlackRealtimeBatch::default()
        };
        batch.connection_state = Some(model::SlackRealtimeConnectionState::Connected);
        self.accumulator.merge(batch);
        None
    }

    fn handle_impact(&mut self, batch: model::SlackRealtimeBatch) {
        self.context
            .record_presence_changes(&batch.presence_changes);
        let (desired_presence_ids, batch) = presence::record_desired_presence_changes(
            self.presence_commit,
            self.presence_subscription_ids,
            self.presence_snapshot,
            batch,
        );
        self.desired_presence_ids = desired_presence_ids;
        self.accumulator.merge(batch);
    }

    fn refresh_presence(&mut self) {
        let (desired_presence_ids, snapshot) = presence::refresh_presence_roster(
            self.presence_commit,
            self.presence_subscription_ids,
            self.presence_snapshot,
        );
        self.desired_presence_ids = desired_presence_ids;
        if let Some(snapshot) = snapshot {
            self.accumulator
                .merge(presence::presence_snapshot_batch(snapshot));
        }
    }

    fn reconnect(&mut self, reason: SlackSocketReconnectReason) -> SlackSocketOutcome {
        accumulator::flush_accumulator(&mut self.accumulator, self.sender);
        SlackSocketOutcome::Reconnect {
            reconnect_url: self.reconnect_url.take(),
            connected: self.connected,
            reason,
        }
    }
}

fn handle_slack_realtime_parse_error(
    context: SlackSocketRunContext<'_>,
    connected: bool,
    malformed_resync_sent: &mut bool,
    accumulator: &mut accumulator::SlackRealtimeAccumulator,
) {
    context.record_malformed_frame(connected);
    if connected && !*malformed_resync_sent {
        *malformed_resync_sent = true;
        accumulator.merge(model::SlackRealtimeBatch::resync());
    }
}
