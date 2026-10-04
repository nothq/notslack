use std::{ops::Range, sync::Arc};

use gpui::SharedString;

use super::super::{SlackMessageBody, SlackSearchTextHighlight, SlackSearchTextHighlightKind};

pub(super) fn prepare_slack_search_text_highlights(
    body: &SlackMessageBody,
    body_preview: &str,
    preview_source_end: usize,
    query: &str,
) -> Arc<[SlackSearchTextHighlight]> {
    let query_highlights = slack_search_query_highlights(&body.text, query);
    let events = slack_search_highlight_events(body, &query_highlights, preview_source_end);
    let highlights = slack_search_highlights_from_events(events);
    for highlight in &highlights {
        assert_slack_search_text_highlight_range(body_preview, &highlight.range);
    }
    for highlights in highlights.windows(2) {
        assert!(
            highlights[0].range.end <= highlights[1].range.start,
            "prepared Slack search highlights must be sorted and disjoint"
        );
    }
    highlights.into()
}

fn slack_search_highlight_events(
    body: &SlackMessageBody,
    query_highlights: &[Range<usize>],
    preview_source_end: usize,
) -> Vec<(usize, i32, i32)> {
    let mut events = Vec::with_capacity((body.links.len() + query_highlights.len()) * 2);
    for range in body.links.iter().map(|link| &link.range) {
        assert_slack_search_text_highlight_range(&body.text, range);
        let preview_range = range.start..range.end.min(preview_source_end);
        if preview_range.start < preview_range.end {
            events.push((preview_range.start, 1_i32, 0_i32));
            events.push((preview_range.end, -1_i32, 0_i32));
        }
    }
    for range in query_highlights {
        assert_slack_search_text_highlight_range(&body.text, range);
        let preview_range = range.start..range.end.min(preview_source_end);
        if preview_range.start < preview_range.end {
            events.push((preview_range.start, 0_i32, 1_i32));
            events.push((preview_range.end, 0_i32, -1_i32));
        }
    }
    events.sort_unstable_by_key(|event| event.0);
    events
}

fn slack_search_highlights_from_events(
    events: Vec<(usize, i32, i32)>,
) -> Vec<SlackSearchTextHighlight> {
    let mut highlights = Vec::<SlackSearchTextHighlight>::with_capacity(events.len() / 2);
    let mut event_index = 0;
    let mut previous_position = events.first().map_or(0, |event| event.0);
    let mut link_depth = 0_i32;
    let mut query_depth = 0_i32;
    while event_index < events.len() {
        let position = events[event_index].0;
        let kind = match (link_depth > 0, query_depth > 0) {
            (true, true) => Some(SlackSearchTextHighlightKind::LinkAndQueryMatch),
            (true, false) => Some(SlackSearchTextHighlightKind::Link),
            (false, true) => Some(SlackSearchTextHighlightKind::QueryMatch),
            (false, false) => None,
        };
        if previous_position < position {
            if let Some(kind) = kind {
                let extends_previous = highlights.last().is_some_and(|previous| {
                    previous.range.end == previous_position && previous.kind == kind
                });
                if extends_previous {
                    highlights
                        .last_mut()
                        .expect("prepared Slack search highlight disappeared")
                        .range
                        .end = position;
                } else {
                    highlights.push(SlackSearchTextHighlight {
                        range: previous_position..position,
                        kind,
                    });
                }
            }
        }

        while event_index < events.len() && events[event_index].0 == position {
            link_depth += events[event_index].1;
            query_depth += events[event_index].2;
            event_index += 1;
        }
        assert!(
            link_depth >= 0 && query_depth >= 0,
            "Slack search highlight event depths must remain non-negative"
        );
        previous_position = position;
    }
    assert_eq!(
        (link_depth, query_depth),
        (0, 0),
        "Slack search highlight event depths must be balanced"
    );
    highlights
}

pub(super) fn prepare_slack_search_body_preview(
    body: &str,
    max_chars: usize,
) -> (SharedString, usize) {
    assert!(
        max_chars > 0,
        "Slack search body preview limit must be positive"
    );
    let source_end = body
        .char_indices()
        .nth(max_chars)
        .map_or(body.len(), |(index, _)| index);
    assert!(
        body.is_char_boundary(source_end),
        "Slack search body preview must end at a UTF-8 boundary"
    );
    let mut preview = body[..source_end]
        .chars()
        .map(|character| match character {
            '\n' | '\r' | '\t' => ' ',
            character => character,
        })
        .collect::<String>();
    assert_eq!(
        preview.len(),
        source_end,
        "Slack search body preview normalization must preserve byte offsets"
    );
    if source_end < body.len() {
        preview.push('…');
    }
    (preview.into(), source_end)
}

fn assert_slack_search_text_highlight_range(body: &str, range: &Range<usize>) {
    assert!(
        range.start < range.end,
        "Slack search highlight range must not be empty"
    );
    assert!(
        range.end <= body.len(),
        "Slack search highlight range must fit the prepared body"
    );
    assert!(
        body.is_char_boundary(range.start) && body.is_char_boundary(range.end),
        "Slack search highlight range must follow UTF-8 boundaries"
    );
}

fn slack_search_query_highlights(body: &str, query: &str) -> Vec<Range<usize>> {
    let query = query.trim();
    if query.is_empty() || !query.is_ascii() {
        return Vec::new();
    }
    let body_lower = body.to_ascii_lowercase();
    let query_lower = query.to_ascii_lowercase();
    body_lower
        .match_indices(&query_lower)
        .map(|(start, matched)| start..start + matched.len())
        .collect()
}
