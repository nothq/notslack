use crate::model::{
    SlackConversationFilesRequest, SlackConversationFilesSort, SlackFilesRequest, SlackFilesScope,
    SlackFilesSort, SlackFilesTypeFilter,
};

use super::SlackConversationFileSource;

pub(super) fn validate_files_request(request: &SlackFilesRequest) -> Result<(), String> {
    if request.team_id.trim().is_empty() {
        return Err("Slack Files request requires a team id".to_string());
    }
    if request.self_user_id.trim().is_empty() {
        return Err("Slack Files request requires a self user id".to_string());
    }
    if request.page == 0 {
        return Err("Slack Files page must be positive".to_string());
    }
    Ok(())
}

pub(super) fn validate_conversation_files_request(
    request: &SlackConversationFilesRequest,
    page: u32,
) -> Result<(), String> {
    if request.team_id.trim().is_empty() {
        return Err("Slack conversation Files request requires a team id".to_string());
    }
    if request.conversation_id.trim().is_empty() {
        return Err("Slack conversation Files request requires a conversation id".to_string());
    }
    if page == 0 {
        return Err("Slack conversation Files page must be positive".to_string());
    }
    Ok(())
}

pub(super) fn conversation_files_query(
    request: &SlackConversationFilesRequest,
    source: SlackConversationFileSource,
) -> String {
    let source_filter = match source {
        SlackConversationFileSource::Files => "-type:images -type:videos -type:folders",
        SlackConversationFileSource::Media => "type:images type:videos",
    };
    let filters = format!("in:<#{}> {source_filter}", request.conversation_id.trim());
    let search = request.search_query.trim();
    if search.is_empty() {
        filters
    } else {
        format!("{search} {filters}")
    }
}

pub(super) fn conversation_files_sort(
    sort: SlackConversationFilesSort,
) -> (&'static str, &'static str) {
    match sort {
        SlackConversationFilesSort::Newest => ("timestamp", "desc"),
        SlackConversationFilesSort::Oldest => ("timestamp", "asc"),
        SlackConversationFilesSort::Relevant => ("score", "desc"),
    }
}

pub(super) fn files_query(request: &SlackFilesRequest) -> String {
    let filters = match request.scope {
        SlackFilesScope::All => files_type_query(&request.type_filters),
        SlackFilesScope::CreatedByYou => {
            let type_query = files_type_query(&request.type_filters);
            if type_query.is_empty() {
                format!("creator:<@{}>", request.self_user_id)
            } else {
                format!("{type_query} creator:<@{}>", request.self_user_id)
            }
        }
        SlackFilesScope::SharedWithYou => {
            let type_query = files_type_query(&request.type_filters);
            if type_query.is_empty() {
                format!("-from:<@{}>", request.self_user_id)
            } else {
                format!("-from:<@{}> {type_query}", request.self_user_id)
            }
        }
    };
    let search = request.search_query.trim();
    if search.is_empty() {
        filters
    } else if filters.is_empty() {
        search.to_string()
    } else {
        format!("{search} {filters}")
    }
}

fn files_type_query(filters: &[SlackFilesTypeFilter]) -> String {
    filters
        .iter()
        .flat_map(|filter| match filter {
            SlackFilesTypeFilter::Lists => ["type:lists", ""].into_iter(),
            SlackFilesTypeFilter::CanvasesAndDocuments => {
                ["type:quip", "type:documents"].into_iter()
            }
            SlackFilesTypeFilter::Spreadsheets => ["type:spreadsheets", ""].into_iter(),
            SlackFilesTypeFilter::Presentations => ["type:presentations", ""].into_iter(),
            SlackFilesTypeFilter::Pdfs => ["type:pdfs", ""].into_iter(),
            SlackFilesTypeFilter::Audio => ["type:audio", ""].into_iter(),
            SlackFilesTypeFilter::Videos => ["type:videos", ""].into_iter(),
            SlackFilesTypeFilter::Images => ["type:images", ""].into_iter(),
            SlackFilesTypeFilter::Snippets => ["type:snippets", ""].into_iter(),
            SlackFilesTypeFilter::Emails => ["type:emails", ""].into_iter(),
        })
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn files_sort(
    request: &SlackFilesRequest,
) -> (&'static str, Option<&'static str>, bool) {
    if !request.search_query.trim().is_empty() {
        return (
            "score",
            Some("desc"),
            request.scope == SlackFilesScope::SharedWithYou,
        );
    }
    match request.scope {
        SlackFilesScope::CreatedByYou => ("timestamp", Some("desc"), false),
        SlackFilesScope::SharedWithYou => ("score", Some("desc"), true),
        SlackFilesScope::All => match request.sort {
            SlackFilesSort::RecentlyViewed => ("last_engaged", None, false),
            SlackFilesSort::LastUpdated => ("timestamp", Some("desc"), false),
        },
    }
}
