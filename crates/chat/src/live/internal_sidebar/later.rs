use crate::live::payload::slack_draft_blocks_json;
use crate::model::{
    SlackLaterCursor, SlackLaterFilter, SlackLaterSnapshot, SlackMessageDraft,
    SlackReminderMutation,
};

use super::SlackInternalSidebarClient;
use wire::{decode_saved_list, decode_saved_mutation};

mod wire;

const SAVED_LIST: &str = "saved.list";
const SAVED_ADD: &str = "saved.add";
const SAVED_UPDATE: &str = "saved.update";
const SAVED_DELETE: &str = "saved.delete";
const LATER_PAGE_SIZE: usize = 15;

impl SlackInternalSidebarClient {
    pub(crate) fn load_later_page(
        &self,
        filter: SlackLaterFilter,
        cursor: Option<&SlackLaterCursor>,
    ) -> Result<SlackLaterSnapshot, String> {
        let mut params = vec![
            ("filter", filter.as_api_value().to_string()),
            ("include_tombstones", "true".to_string()),
            ("limit", LATER_PAGE_SIZE.to_string()),
            ("_x_reason", "saved-api/savedList".to_string()),
            ("_x_mode", "online".to_string()),
            ("_x_sonic", "true".to_string()),
            ("_x_app_name", "client".to_string()),
        ];
        if let Some(cursor) = cursor {
            params.push(("cursor", cursor.as_str().to_string()));
        }
        let body = self.post_internal_method(SAVED_LIST, params)?;
        decode_saved_list(filter, &body)
    }

    pub(crate) fn mutate_reminder(&self, mutation: &SlackReminderMutation) -> Result<(), String> {
        let (method, mut params) = match mutation {
            SlackReminderMutation::Create { client_id, draft } => (
                SAVED_ADD,
                vec![
                    ("item_type", "reminder".to_string()),
                    (
                        "description",
                        encode_reminder_description(draft.description())?,
                    ),
                    ("client_id", client_id.as_hyphenated_string()),
                    ("date_due", draft.due_at().to_string()),
                    ("_x_reason", "saved-api/saveReminder".to_string()),
                ],
            ),
            SlackReminderMutation::Edit { reminder_id, draft } => (
                SAVED_UPDATE,
                vec![
                    ("item_type", "reminder".to_string()),
                    ("item_id", reminder_id.as_str().to_string()),
                    (
                        "description",
                        encode_reminder_description(draft.description())?,
                    ),
                    ("date_due", draft.due_at().to_string()),
                    ("_x_reason", "saved-api/updateSavedReminder".to_string()),
                ],
            ),
            SlackReminderMutation::Complete { reminder_id } => (
                SAVED_UPDATE,
                vec![
                    ("item_type", "reminder".to_string()),
                    ("item_id", reminder_id.as_str().to_string()),
                    ("mark", "completed".to_string()),
                    ("todo_state", "completed".to_string()),
                    ("date_due", "0".to_string()),
                    ("_x_reason", "saved-api/toggleReminderComplete".to_string()),
                ],
            ),
            SlackReminderMutation::Delete { reminder_id } => (
                SAVED_DELETE,
                vec![
                    ("item_type", "reminder".to_string()),
                    ("item_id", reminder_id.as_str().to_string()),
                    ("_x_reason", "saved-api/removeReminder".to_string()),
                ],
            ),
        };
        params.extend([
            ("_x_mode", "online".to_string()),
            ("_x_sonic", "true".to_string()),
            ("_x_app_name", "client".to_string()),
        ]);
        let body = self.post_internal_method(method, params)?;
        decode_saved_mutation(method, &body)
    }
}

fn encode_reminder_description(description: &str) -> Result<String, String> {
    let draft = SlackMessageDraft::plain_text(description.to_string())?;
    slack_draft_blocks_json(&draft)
        .map_err(|error| format!("failed to encode Slack reminder description: {error}"))
}
