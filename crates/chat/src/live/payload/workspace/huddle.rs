use crate::model::{SlackHuddleJoinReceipt, SlackHuddleJoinRequest, SlackHuddleLeaveRequest};

use super::SlackLiveWorkspaceLoader;

impl SlackLiveWorkspaceLoader {
    pub fn join_huddle(
        &self,
        request: &SlackHuddleJoinRequest,
    ) -> Result<SlackHuddleJoinReceipt, String> {
        self.api.join_huddle(request)
    }

    pub fn leave_huddle(&self, request: &SlackHuddleLeaveRequest) -> Result<(), String> {
        self.api.leave_huddle(request)
    }
}
