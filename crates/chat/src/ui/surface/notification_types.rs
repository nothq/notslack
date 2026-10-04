use crate::ui::SlackChannelNotificationMutation;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackChannelNotificationLoadRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackChannelNotificationMutationRequest {
    pub(crate) generation: u64,
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) mutation: SlackChannelNotificationMutation,
}
