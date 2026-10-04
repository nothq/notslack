use std::collections::{HashSet, VecDeque};

use crate::ui::{SlackLaterHydrationTarget, SlackLaterItemKey};

#[derive(Default)]
pub(crate) struct SlackLaterHydrationQueue {
    pending: VecDeque<SlackLaterHydrationTarget>,
    queued_keys: HashSet<SlackLaterItemKey>,
    in_flight_keys: HashSet<SlackLaterItemKey>,
}

impl SlackLaterHydrationQueue {
    pub(crate) fn reset(&mut self) {
        self.pending.clear();
        self.queued_keys.clear();
        self.in_flight_keys.clear();
    }

    pub(crate) fn enqueue(&mut self, target: SlackLaterHydrationTarget, priority: bool) {
        let key = target.key();
        if self.in_flight_keys.contains(key) {
            return;
        }
        if self.queued_keys.contains(key) {
            if priority {
                let queued_index = self
                    .pending
                    .iter()
                    .position(|queued| queued.key() == key)
                    .expect("queued Slack Later hydration key must have a pending target");
                let queued = self
                    .pending
                    .remove(queued_index)
                    .expect("queued Slack Later hydration target disappeared");
                self.pending.push_front(queued);
            }
            return;
        }
        self.queued_keys.insert(key.clone());
        if priority {
            self.pending.push_front(target);
        } else {
            self.pending.push_back(target);
        }
    }

    pub(crate) fn start_next(&mut self, max_in_flight: usize) -> Option<SlackLaterHydrationTarget> {
        if self.in_flight_keys.len() >= max_in_flight {
            return None;
        }
        let target = self.pending.pop_front()?;
        self.queued_keys.remove(target.key());
        self.in_flight_keys.insert(target.key().clone());
        Some(target)
    }

    pub(crate) fn finish(&mut self, key: &SlackLaterItemKey) {
        self.in_flight_keys.remove(key);
    }
}
