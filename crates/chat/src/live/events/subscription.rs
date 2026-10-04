pub(crate) struct SlackRealtimeSubscription {
    receiver: tokio::sync::broadcast::Receiver<crate::model::SlackRealtimeBatch>,
    presence_snapshot: tokio::sync::watch::Receiver<crate::model::SlackRealtimePresenceSnapshot>,
    pending_resync: bool,
    pending_closed: bool,
}

impl SlackRealtimeSubscription {
    pub(crate) fn new(
        receiver: tokio::sync::broadcast::Receiver<crate::model::SlackRealtimeBatch>,
        presence_snapshot: tokio::sync::watch::Receiver<
            crate::model::SlackRealtimePresenceSnapshot,
        >,
    ) -> Self {
        Self {
            receiver,
            presence_snapshot,
            pending_resync: false,
            pending_closed: false,
        }
    }

    pub(crate) fn new_presentation(
        receiver: tokio::sync::broadcast::Receiver<crate::model::SlackRealtimeBatch>,
        presence_snapshot: tokio::sync::watch::Receiver<
            crate::model::SlackRealtimePresenceSnapshot,
        >,
        closed: bool,
    ) -> Self {
        Self {
            receiver,
            presence_snapshot,
            pending_resync: !closed,
            pending_closed: closed,
        }
    }
}

impl crate::model::SlackRealtimeSubscription for SlackRealtimeSubscription {
    fn recv(&mut self) -> crate::model::SlackRealtimeReceiveFuture<'_> {
        if std::mem::take(&mut self.pending_closed) {
            return Box::pin(async { Err(crate::model::SlackRealtimeRecvError::Closed) });
        }
        if std::mem::take(&mut self.pending_resync) {
            let snapshot = self.presence_snapshot.borrow().clone();
            return Box::pin(async move {
                Ok(crate::model::SlackRealtimeBatch::resync_with_presence(
                    snapshot,
                ))
            });
        }
        Box::pin(async move {
            match self.receiver.recv().await {
                Ok(batch)
                    if batch.connection_state
                        == Some(crate::model::SlackRealtimeConnectionState::Closed) =>
                {
                    Err(crate::model::SlackRealtimeRecvError::Closed)
                }
                Ok(batch) => Ok(batch),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    Ok(crate::model::SlackRealtimeBatch::resync_with_presence(
                        self.presence_snapshot.borrow().clone(),
                    ))
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    Err(crate::model::SlackRealtimeRecvError::Closed)
                }
            }
        })
    }
}
