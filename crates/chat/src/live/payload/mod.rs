mod message;
mod message_draft;
mod search;
mod sidebar;
pub(crate) mod sidebar_dom;
mod thread;
mod users;
mod util;
mod workspace;

use crate::live::api::load_remote_image;
pub(crate) use message::slack_message_body;
pub(crate) use message::slack_message_from_value_with_context_in_timezone;
pub(in crate::live) use message::slack_rich_text_body_from_blocks;
pub(in crate::live) use message_draft::slack_draft_blocks_json;
pub(in crate::live) use util::{
    select_slack_avatar_image_url, SlackAvatarImageUrls, SlackAvatarPurpose,
};
pub(crate) use workspace::SlackFileShareMutationOutcome;
pub use workspace::{load_slack_live_workspace, SlackAttachmentPreview, SlackLiveWorkspaceLoader};

const SLACK_CONVERSATION_HISTORY_PAGE_SIZE: usize = 30;

pub(crate) fn slack_activity_actor_from_user_payload(
    expected_user_id: &str,
    user: &serde_json::Value,
) -> Result<crate::model::SlackActivityActor, String> {
    let user_id = util::string_at(user, &["id"]).ok_or_else(|| {
        format!("Slack Activity reaction actor {expected_user_id} omitted its user id")
    })?;
    if user_id != expected_user_id {
        return Err(format!(
            "Slack Activity reaction actor {user_id} did not match requested user {expected_user_id}"
        ));
    }
    let display_label = util::slack_user_display_name(user).ok_or_else(|| {
        format!("Slack Activity reaction actor {expected_user_id} omitted its display label")
    })?;
    Ok(crate::model::SlackActivityActor {
        user_id,
        display_label,
        avatar_image_url: util::slack_user_avatar_image_url(
            user,
            util::SlackAvatarPurpose::Message,
        ),
    })
}

pub fn load_slack_remote_image(url: &str) -> Result<SlackAttachmentPreview, String> {
    let (base64, mimetype) = load_remote_image(url, std::time::Duration::from_secs(15))?;
    Ok(SlackAttachmentPreview { base64, mimetype })
}
