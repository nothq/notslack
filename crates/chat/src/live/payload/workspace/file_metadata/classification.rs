use crate::model::{SlackFileSharing, SlackRemoteDraftFileDeletionEligibility};
use serde_json::Value;

// Authenticated Slack Desktop 4.51.180 module Nvgx defines canDraftFileBeDeleted as
// unshared && !Quip && !Post. Its removeDraftFile path separately preserves List files (except
// Polls) before loading files.info, so the native cleanup boundary applies both predicates.
pub(super) fn slack_remote_draft_file_deletion_eligibility(
    file: &Value,
) -> SlackRemoteDraftFileDeletionEligibility {
    let mode = file.get("mode").and_then(Value::as_str);
    let filetype = file.get("filetype").and_then(Value::as_str);
    let subtype = file.get("subtype").and_then(Value::as_str);
    let is_quip = mode == Some("quip") || filetype == Some("quip");
    let is_post = matches!(mode, Some("post" | "space" | "docs"));
    let is_list = (mode == Some("list") || filetype == Some("list")) && subtype != Some("poll");
    if is_quip || is_post || is_list {
        SlackRemoteDraftFileDeletionEligibility::Ineligible
    } else if mode.is_some() || filetype.is_some() {
        SlackRemoteDraftFileDeletionEligibility::Eligible
    } else {
        SlackRemoteDraftFileDeletionEligibility::Unknown
    }
}

pub(super) fn slack_file_sharing(file: &Value) -> SlackFileSharing {
    let has_shared_conversation = ["channels", "groups", "ims"].into_iter().any(|field| {
        file.get(field)
            .and_then(Value::as_array)
            .is_some_and(|values| !values.is_empty())
    });
    let has_public_share = file
        .get("is_public")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || file
            .get("public_url_shared")
            .and_then(Value::as_bool)
            .unwrap_or(false);
    let shares = file.get("shares").and_then(Value::as_object);
    if has_shared_conversation
        || has_public_share
        || shares.is_some_and(|shares| shares.values().any(slack_share_value_has_entries))
    {
        return SlackFileSharing::Shared;
    }
    if shares.is_some() {
        return SlackFileSharing::Unshared;
    }
    let has_complete_unshared_projection = ["channels", "groups", "ims"]
        .into_iter()
        .all(|field| file.get(field).and_then(Value::as_array).is_some())
        && file.get("is_public").and_then(Value::as_bool) == Some(false)
        && file.get("public_url_shared").and_then(Value::as_bool) == Some(false);
    if has_complete_unshared_projection {
        SlackFileSharing::Unshared
    } else {
        SlackFileSharing::Unknown
    }
}

fn slack_share_value_has_entries(value: &Value) -> bool {
    match value {
        Value::Array(values) => !values.is_empty(),
        Value::Object(values) => values.values().any(slack_share_value_has_entries),
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(_) | Value::String(_) => true,
    }
}
