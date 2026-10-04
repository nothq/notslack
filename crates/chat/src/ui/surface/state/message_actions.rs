mod menu;
mod saved;
mod unread;

use super::{build_slack_dm_rows, prepare_slack_conversation_snapshot, Context, SurfaceState};
use crate::ui::surface::{
    SlackConversationLiveTarget, SlackConversationReadReadiness, SlackMessageMarkUnreadRequest,
    SlackMessageMenuAction, SlackMessageMenuState, SlackMessagePermalinkRequest,
    SlackMessageSavedRequest,
};
use crate::ui::{SlackLaterState, SlackMessageTimestamp, SlackSavedMessageMutation};
use gpui::ClipboardItem;

const SLACK_MESSAGE_MENU_WIDTH: f32 = 300.0;
const SLACK_MESSAGE_MENU_ROW_HEIGHT: f32 = 28.0;
const SLACK_MESSAGE_MENU_VERTICAL_PADDING: f32 = 24.0;
const SLACK_MESSAGE_MENU_GROUP_GAP: f32 = 17.0;
const SLACK_MESSAGE_MENU_EDGE_INSET: f32 = 4.0;
const SLACK_MESSAGE_MENU_ANCHOR_GAP: f32 = 8.0;
const SLACK_HISTORY_START_TIMESTAMP: &str = "0000000000.000000";

fn mark_slack_sidebar_conversation_unread(
    sections: &mut [crate::ui::SlackSidebarSection],
    conversation_id: &str,
) -> bool {
    sections
        .iter_mut()
        .flat_map(|section| section.items.iter_mut())
        .find(|item| item.target_id == conversation_id)
        .is_some_and(|item| {
            let changed = !item.unread;
            item.unread = true;
            changed
        })
}
