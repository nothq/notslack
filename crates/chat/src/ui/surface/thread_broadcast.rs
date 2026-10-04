use gpui::SharedString;

use crate::ui::SlackConversationKind;

pub(crate) fn slack_thread_broadcast_label(
    conversation_kind: SlackConversationKind,
    conversation_name: &str,
) -> Option<SharedString> {
    match conversation_kind {
        SlackConversationKind::Channel | SlackConversationKind::PrivateChannel => {
            let conversation_name = conversation_name
                .strip_prefix('#')
                .unwrap_or(conversation_name);
            Some(format!("Also send to #{conversation_name}").into())
        }
        SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage => {
            Some("Also send as direct message".into())
        }
        SlackConversationKind::Unknown => None,
    }
}
