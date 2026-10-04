use std::sync::Arc;

use gpui::SharedString;

use crate::ui::{
    SlackBookmarkFolderRequest, SlackBookmarkFolderSnapshot, SlackBookmarkSource,
    SlackConversationTab, SlackConversationTabTarget,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackBookmarkFolderIdentity {
    pub(crate) team_id: SharedString,
    pub(crate) conversation_id: SharedString,
    pub(crate) tab_id: SharedString,
    pub(crate) folder_bookmark_id: SharedString,
}

impl SlackBookmarkFolderIdentity {
    pub(crate) fn from_tab(
        team_id: &str,
        conversation_id: &str,
        tab: &SlackConversationTab,
    ) -> Option<Self> {
        if tab.is_disabled {
            return None;
        }
        let SlackConversationTabTarget::Folder { bookmark_id } = &tab.target else {
            return None;
        };
        if team_id.is_empty()
            || conversation_id.is_empty()
            || tab.id.is_empty()
            || bookmark_id.is_empty()
        {
            return None;
        }
        Some(Self {
            team_id: team_id.to_string().into(),
            conversation_id: conversation_id.to_string().into(),
            tab_id: tab.id.clone().into(),
            folder_bookmark_id: bookmark_id.clone().into(),
        })
    }

    pub(crate) fn request(&self) -> SlackBookmarkFolderRequest {
        SlackBookmarkFolderRequest {
            team_id: self.team_id.to_string(),
            conversation_id: self.conversation_id.to_string(),
            folder_bookmark_id: self.folder_bookmark_id.to_string(),
        }
    }

    pub(crate) fn matches_workspace(&self, team_id: &str, conversation_id: &str) -> bool {
        self.team_id.as_ref() == team_id && self.conversation_id.as_ref() == conversation_id
    }

    pub(crate) fn matches_tab(&self, tab: &SlackConversationTab) -> bool {
        if tab.is_disabled || self.tab_id.as_ref() != tab.id {
            return false;
        }
        matches!(
            &tab.target,
            SlackConversationTabTarget::Folder { bookmark_id }
                if self.folder_bookmark_id.as_ref() == bookmark_id
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackBookmarkFolderLoad {
    pub(crate) generation: u64,
    pub(crate) identity: SlackBookmarkFolderIdentity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlackBookmarkFolderRowKind {
    Link,
    File,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SlackBookmarkFolderRow {
    pub(crate) element_id: SharedString,
    pub(crate) title: SharedString,
    pub(crate) context: SharedString,
    pub(crate) target_url: SharedString,
    pub(crate) icon_url: Option<SharedString>,
    pub(crate) kind: SlackBookmarkFolderRowKind,
}

pub(crate) struct PreparedSlackBookmarkFolderSnapshot {
    pub(crate) team_id: String,
    pub(crate) conversation_id: String,
    pub(crate) folder_bookmark_id: String,
    pub(crate) rows: Arc<[SlackBookmarkFolderRow]>,
}

pub(crate) fn prepare_slack_bookmark_folder_snapshot(
    snapshot: SlackBookmarkFolderSnapshot,
) -> PreparedSlackBookmarkFolderSnapshot {
    let folder_bookmark_id = snapshot.folder_bookmark_id.clone();
    let rows = snapshot
        .items
        .into_iter()
        .filter_map(|item| {
            let kind = match item.source {
                SlackBookmarkSource::Link { .. } => SlackBookmarkFolderRowKind::Link,
                SlackBookmarkSource::File { .. } => SlackBookmarkFolderRowKind::File,
                SlackBookmarkSource::PinnedMessage { .. }
                | SlackBookmarkSource::Unsupported { .. } => return None,
            };
            let title = item.title.filter(|title| !title.trim().is_empty())?;
            let target_url = item.link.filter(|link| !link.trim().is_empty())?;
            let hostname = url::Url::parse(&target_url)
                .ok()
                .and_then(|url| url.host_str().map(str::to_string));
            let context = match (kind, hostname) {
                (SlackBookmarkFolderRowKind::Link, Some(hostname)) => hostname,
                (SlackBookmarkFolderRowKind::Link, None) => "Link".to_string(),
                (SlackBookmarkFolderRowKind::File, Some(hostname)) => {
                    format!("File · {hostname}")
                }
                (SlackBookmarkFolderRowKind::File, None) => "File".to_string(),
            };
            Some(SlackBookmarkFolderRow {
                element_id: format!(
                    "slack-bookmark-folder-{folder_bookmark_id}-item-{}",
                    item.id
                )
                .into(),
                title: title.into(),
                context: context.into(),
                target_url: target_url.into(),
                icon_url: item.icon_url.map(Into::into),
                kind,
            })
        })
        .collect::<Vec<_>>()
        .into();
    PreparedSlackBookmarkFolderSnapshot {
        team_id: snapshot.team_id,
        conversation_id: snapshot.conversation_id,
        folder_bookmark_id: snapshot.folder_bookmark_id,
        rows,
    }
}
