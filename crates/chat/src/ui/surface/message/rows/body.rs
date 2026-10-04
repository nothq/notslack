use crate::ui::surface::SlackMessageBody;
use crate::ui::{SlackMessage, SlackMessageClientId, SlackMessageDraft};
use gpui::SharedString;
use gpui_components::selectable_text::{SelectableTextMessage, SelectableTextPosition};
use std::cell::Cell;

mod builder;
mod legacy;
mod render;
mod rich;

pub(super) use builder::slack_message_element_ids;
pub(in crate::ui::surface) use render::slack_message_body_block;
pub(in crate::ui::surface) use render::slack_message_body_block_in_document;
pub(in crate::ui::surface::message) use render::{
    slack_prepared_message_body, SlackPreparedMessageBodyStyle,
};

pub(in crate::ui::surface) struct SlackMessageSelectionContext {
    document_id: SharedString,
    row_index: usize,
    message_index: usize,
    message_id: SharedString,
    next_fragment_index: Cell<usize>,
}

impl SlackMessageSelectionContext {
    pub(in crate::ui::surface) fn new(
        document_id: SharedString,
        row_index: usize,
        message_index: usize,
        message_id: impl Into<SharedString>,
    ) -> Self {
        Self {
            document_id,
            row_index,
            message_index,
            message_id: message_id.into(),
            next_fragment_index: Cell::new(0),
        }
    }

    pub(in crate::ui::surface) fn next_position(
        &self,
        fragment_id: SharedString,
    ) -> SelectableTextPosition {
        self.next_position_with_separator(fragment_id, "\n")
    }

    pub(in crate::ui::surface) fn next_position_with_separator(
        &self,
        fragment_id: SharedString,
        separator_within_message: &'static str,
    ) -> SelectableTextPosition {
        let fragment_index = self.next_fragment_index.get();
        self.next_fragment_index.set(fragment_index + 1);
        let separator_before = if fragment_index > 0 || self.row_index > 0 || self.message_index > 0
        {
            separator_within_message
        } else {
            ""
        };
        SelectableTextPosition::new(
            self.document_id.clone(),
            SelectableTextMessage {
                row_index: self.row_index,
                message_index: self.message_index,
                message_id: self.message_id.clone(),
            },
            fragment_index,
            fragment_id,
            separator_before,
        )
    }
}

pub(crate) fn prepare_slack_message_body(body: &str) -> SlackMessageBody {
    legacy::prepare_slack_legacy_message_body(body, &legacy::slack_legacy_body_id_stem(body))
}

pub(crate) fn prepare_slack_message_body_from_message(message: &SlackMessage) -> SlackMessageBody {
    let id_stem = format!("slack-message-body-{}", message.id);
    if let Some(rich_body) = message.rich_body.as_deref() {
        let prepared = rich::prepare_slack_rich_message_body(rich_body, &id_stem);
        if !prepared.blocks.is_empty() || message.body.is_empty() {
            return prepared;
        }
    }
    legacy::prepare_slack_legacy_message_body(&message.body, &id_stem)
}

pub(super) fn prepare_slack_message_body_from_local_delivery(
    client_message_id: &SlackMessageClientId,
    draft: Option<&SlackMessageDraft>,
) -> SlackMessageBody {
    let Some(draft) = draft else {
        return SlackMessageBody::default();
    };
    let id_stem = format!("slack-local-delivery-body-{}", client_message_id.as_str());
    if let Some(rich_body) = draft.rich_text() {
        let prepared = rich::prepare_slack_rich_message_body(rich_body, &id_stem);
        if !prepared.text.is_empty() || draft.fallback_text().is_empty() {
            return prepared;
        }
    }
    legacy::prepare_slack_legacy_message_body(draft.fallback_text(), &id_stem)
}
