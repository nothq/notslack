use std::time::Instant;

use crate::model::{SlackRealtimePresenceChange, SlackUserPresence};

use super::super::diagnostics::{SlackRealtimeDiagnostic, SlackRealtimeDiagnosticRecord};

#[derive(Clone, Copy)]
pub(in crate::live::events::worker) struct SlackSocketRunContext<'a> {
    pub(super) resync_on_hello: bool,
    pub(super) diagnostic: Option<&'a SlackRealtimeDiagnostic>,
    pub(super) socket_sequence: u64,
}

impl<'a> SlackSocketRunContext<'a> {
    pub(in crate::live::events::worker) fn new(
        resync_on_hello: bool,
        diagnostic: Option<&'a SlackRealtimeDiagnostic>,
        socket_sequence: u64,
    ) -> Self {
        Self {
            resync_on_hello,
            diagnostic,
            socket_sequence,
        }
    }
    pub(super) fn record_hello(self) -> Option<Instant> {
        let diagnostic = self.diagnostic?;
        let hello_at = Instant::now();
        diagnostic.record(SlackRealtimeDiagnosticRecord::Hello {
            socket_sequence: self.socket_sequence,
            reconnect: self.resync_on_hello,
        });
        Some(hello_at)
    }

    pub(super) fn record_presence_subscription(
        self,
        initial: bool,
        id: u64,
        users: usize,
        hello_at: Option<Instant>,
    ) {
        let Some(diagnostic) = self.diagnostic else {
            return;
        };
        diagnostic.record(SlackRealtimeDiagnosticRecord::PresenceSubscription {
            socket_sequence: self.socket_sequence,
            initial,
            id,
            users,
            hello_delay_ms: hello_at.map(|started| started.elapsed().as_millis()),
        });
    }

    pub(super) fn record_ping(self, id: u64) {
        let Some(diagnostic) = self.diagnostic else {
            return;
        };
        diagnostic.record(SlackRealtimeDiagnosticRecord::Ping {
            socket_sequence: self.socket_sequence,
            id,
        });
    }

    pub(super) fn record_presence_changes(self, changes: &[SlackRealtimePresenceChange]) {
        let Some(diagnostic) = self.diagnostic else {
            return;
        };
        let active = changes
            .iter()
            .filter(|change| change.presence == SlackUserPresence::Active)
            .count();
        let away = changes.len().saturating_sub(active);
        if active == 0 && away == 0 {
            return;
        }
        diagnostic.record(SlackRealtimeDiagnosticRecord::PresenceChange {
            socket_sequence: self.socket_sequence,
            active,
            away,
        });
    }

    pub(super) fn record_malformed_frame(self, connected: bool) {
        let Some(diagnostic) = self.diagnostic else {
            return;
        };
        diagnostic.record(SlackRealtimeDiagnosticRecord::MalformedFrame {
            socket_sequence: self.socket_sequence,
            connected,
        });
    }
}
