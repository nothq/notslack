mod hub;
mod subscription;
mod wire;
mod worker;

use std::sync::Arc;

pub(crate) use hub::SlackRealtimeHub;
pub(crate) use subscription::SlackRealtimeSubscription;

type SlackPresenceSubscriptionIds = Option<Arc<[String]>>;
