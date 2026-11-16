//! The window title, which names what is open the way Slack's does:
//! "ge-builds (Channel) - Genius Education - notslack".

use crate::ui::surface::{SlackMainRoute, SlackRailView, SurfaceState};
use crate::ui::{SlackConversationKind, Window};

impl SurfaceState {
    pub(crate) fn sync_slack_window_title(&mut self, window: &mut Window) {
        let title = self.slack_window_title();
        if title != self.slack_window_title {
            window.set_window_title(&title);
            self.slack_window_title = title;
        }
    }

    fn slack_window_title(&self) -> String {
        let Some(workspace) = self.slack_workspace() else {
            return "notslack".to_string();
        };
        let view = match self.slack_active_rail_view {
            SlackRailView::Activity => Some("Activity".to_string()),
            SlackRailView::Files => Some("Files".to_string()),
            SlackRailView::Later => Some("Later".to_string()),
            SlackRailView::DraftsSent => Some("Drafts & sent".to_string()),
            SlackRailView::Home
            | SlackRailView::Dms
            | SlackRailView::More
            | SlackRailView::Admin => match self.slack_main_route {
                SlackMainRoute::Conversation => {
                    slack_conversation_title(&workspace.channel_name, workspace.channel_kind)
                }
                SlackMainRoute::AllThreads => Some("Threads".to_string()),
                SlackMainRoute::Directory => Some("Directories".to_string()),
                SlackMainRoute::NewMessage => Some("New message".to_string()),
            },
        };
        slack_window_title(view.as_deref(), &workspace.workspace_name)
    }
}

fn slack_conversation_title(name: &str, kind: SlackConversationKind) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some(match kind {
        SlackConversationKind::Channel | SlackConversationKind::PrivateChannel => {
            format!("{name} (Channel)")
        }
        SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage => {
            format!("{name} (DM)")
        }
        SlackConversationKind::Unknown => name.to_string(),
    })
}

fn slack_window_title(view: Option<&str>, workspace_name: &str) -> String {
    [view.unwrap_or_default(), workspace_name.trim(), "notslack"]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" - ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_name_the_conversation_its_kind_and_the_workspace() {
        let channel = slack_conversation_title("ge-builds", SlackConversationKind::Channel);
        assert_eq!(
            slack_window_title(channel.as_deref(), "Genius Education"),
            "ge-builds (Channel) - Genius Education - notslack"
        );
        let dm = slack_conversation_title("Cyril John", SlackConversationKind::DirectMessage);
        assert_eq!(
            slack_window_title(dm.as_deref(), "Genius Education"),
            "Cyril John (DM) - Genius Education - notslack"
        );
        assert_eq!(
            slack_window_title(Some("Files"), "Genius Education"),
            "Files - Genius Education - notslack"
        );
        assert_eq!(slack_window_title(None, ""), "notslack");
    }
}
