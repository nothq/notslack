use super::super::{
    PreparedSlackThreadReplySendResult, SlackMessageTimestamp, SlackThreadReplySendRequest,
};

pub(super) fn parse_slack_thread_reply_timestamp(
    parent_message_id: &str,
) -> Result<SlackMessageTimestamp, String> {
    SlackMessageTimestamp::parse(parent_message_id)
        .map_err(|error| format!("Cannot reply to this Slack thread: {error}"))
}

pub(super) fn validate_slack_thread_reply_send_result(
    request: &SlackThreadReplySendRequest,
    prepared: &PreparedSlackThreadReplySendResult,
) -> Result<(), String> {
    match prepared {
        PreparedSlackThreadReplySendResult::Message(prepared) => {
            if prepared.receipt.team_id != request.team_id
                || prepared.receipt.conversation_id != request.conversation_id
                || prepared.receipt.thread_timestamp != request.parent_message_id
                || prepared.receipt.broadcast != request.broadcast
                || prepared.receipt.reply.id == request.parent_message_id
                || prepared.receipt.reply.client_message_id.as_ref()
                    != Some(request.payload.client_message_id())
                || SlackMessageTimestamp::parse(&prepared.receipt.reply.id).is_err()
            {
                return Err(
                    "Slack reply response did not match the requested workspace, conversation, and thread."
                        .to_string(),
                );
            }
        }
        PreparedSlackThreadReplySendResult::Share(prepared) => {
            if prepared.snapshot.team_id != request.team_id
                || prepared.snapshot.conversation_id != request.conversation_id
                || prepared.snapshot.thread_timestamp != request.parent_message_id
            {
                return Err(
                    "Slack file-share response did not match the requested workspace, conversation, and thread."
                        .to_string(),
                );
            }
        }
    }
    Ok(())
}
