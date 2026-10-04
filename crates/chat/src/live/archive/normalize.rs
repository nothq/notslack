use crate::model::{SlackConversationKind, SlackSidebarItem, SlackWorkspace};

use crate::live::conversation::{conversation_kind_for_label, normalize_conversation_label};

pub(super) fn normalize_slack_archive(mut workspace: SlackWorkspace) -> SlackWorkspace {
    let raw_channel_name = workspace.channel_name.clone();
    if workspace.team_id.trim().is_empty() {
        workspace.team_id = "T_ARCHIVE".to_string();
    }
    if matches!(workspace.channel_kind, SlackConversationKind::Unknown) {
        workspace.channel_kind = conversation_kind_for_label(&raw_channel_name);
    }
    if workspace.conversation_id.trim().is_empty() {
        workspace.conversation_id =
            archive_conversation_id(&raw_channel_name, workspace.channel_kind);
    }
    workspace.channel_name =
        normalize_conversation_label(&raw_channel_name, workspace.channel_kind);
    if workspace.composer_placeholder.trim().is_empty() {
        workspace.composer_placeholder = workspace
            .channel_kind
            .composer_placeholder(&workspace.channel_name);
    }

    let active_conversation_id = workspace.conversation_id.clone();
    for item in workspace
        .sections
        .iter_mut()
        .flat_map(|section| section.items.iter_mut())
    {
        normalize_sidebar_item(item);
        item.active = item.target_id == active_conversation_id;
    }
    workspace
}

fn normalize_sidebar_item(item: &mut SlackSidebarItem) {
    if matches!(item.target_kind, SlackConversationKind::Unknown) {
        item.target_kind = conversation_kind_for_item(item);
    }
    if item.target_id.trim().is_empty() {
        item.target_id = archive_conversation_id(&item.label, item.target_kind);
    }
    item.label = normalize_conversation_label(&item.label, item.target_kind);
    match item.target_kind {
        SlackConversationKind::DirectMessage | SlackConversationKind::GroupMessage => {
            item.icon = Some("person".to_string());
        }
        SlackConversationKind::PrivateChannel => {
            item.icon = Some("lock".to_string());
        }
        SlackConversationKind::Channel => {
            item.icon = Some("hash".to_string());
        }
        SlackConversationKind::Unknown => {}
    }
}

fn conversation_kind_for_item(item: &SlackSidebarItem) -> SlackConversationKind {
    match item.icon.as_deref() {
        Some("person") => SlackConversationKind::DirectMessage,
        Some("lock") => SlackConversationKind::PrivateChannel,
        Some("hash") => conversation_kind_for_label(&item.label),
        _ => conversation_kind_for_label(&item.label),
    }
}

fn archive_conversation_id(label: &str, kind: SlackConversationKind) -> String {
    let prefix = match kind {
        SlackConversationKind::Channel => "C_ARCHIVE_",
        SlackConversationKind::PrivateChannel => "G_ARCHIVE_",
        SlackConversationKind::DirectMessage => "D_ARCHIVE_",
        SlackConversationKind::GroupMessage => "M_ARCHIVE_",
        SlackConversationKind::Unknown => "X_ARCHIVE_",
    };
    let normalized = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();
    if normalized.is_empty() {
        format!("{prefix}UNKNOWN")
    } else {
        format!("{prefix}{normalized}")
    }
}
