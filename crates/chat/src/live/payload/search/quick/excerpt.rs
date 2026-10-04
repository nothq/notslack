use std::{collections::HashMap, ops::Range};

use serde_json::Value;

use crate::{live::payload::sidebar_dom::SlackSidebarSnapshot, model::SlackQuickSearchHighlight};

use super::super::normalized_slack_token;

pub(super) fn normalize_slack_quick_search_excerpt(
    source: &str,
    highlights: &[SlackQuickSearchHighlight],
    users: &HashMap<String, Value>,
    sidebar: &SlackSidebarSnapshot,
) -> (String, Vec<SlackQuickSearchHighlight>) {
    let (text, ranges) = normalized_segments(source, highlights, users, sidebar);
    let highlights = merge_highlight_ranges(ranges)
        .into_iter()
        .map(|range| SlackQuickSearchHighlight {
            start: range.start,
            end: range.end,
        })
        .collect();
    (text, highlights)
}

fn normalized_segments(
    source: &str,
    highlights: &[SlackQuickSearchHighlight],
    users: &HashMap<String, Value>,
    sidebar: &SlackSidebarSnapshot,
) -> (String, Vec<Range<usize>>) {
    let mut text = String::with_capacity(source.len());
    let mut ranges = Vec::with_capacity(highlights.len());
    let mut source_index = 0;
    while let Some(open_offset) = source[source_index..].find('<') {
        let open = source_index + open_offset;
        append_plain_segment(
            source,
            source_index..open,
            highlights,
            &mut text,
            &mut ranges,
        );
        let Some(close_offset) = source[open..].find('>') else {
            append_plain_segment(
                source,
                open..source.len(),
                highlights,
                &mut text,
                &mut ranges,
            );
            return (text, ranges);
        };
        let close = open + close_offset;
        let token_range = open..close + 1;
        if let Some(replacement) = normalized_slack_token(&source[open + 1..close], users, sidebar)
        {
            append_replacement(
                &replacement,
                token_range,
                highlights,
                &mut text,
                &mut ranges,
            );
        } else {
            append_plain_segment(source, token_range, highlights, &mut text, &mut ranges);
        }
        source_index = close + 1;
    }
    append_plain_segment(
        source,
        source_index..source.len(),
        highlights,
        &mut text,
        &mut ranges,
    );
    (text, ranges)
}

fn append_replacement(
    replacement: &str,
    source_range: Range<usize>,
    highlights: &[SlackQuickSearchHighlight],
    text: &mut String,
    ranges: &mut Vec<Range<usize>>,
) {
    let output_start = text.len();
    text.push_str(replacement);
    if highlights
        .iter()
        .any(|highlight| ranges_intersect(highlight.start..highlight.end, source_range.clone()))
    {
        ranges.push(output_start..text.len());
    }
}

fn append_plain_segment(
    source: &str,
    source_range: Range<usize>,
    highlights: &[SlackQuickSearchHighlight],
    output: &mut String,
    ranges: &mut Vec<Range<usize>>,
) {
    if source_range.is_empty() {
        return;
    }
    let output_start = output.len();
    output.extend(source[source_range.clone()].chars().map(|character| {
        if matches!(character, '\n' | '\r' | '\t') {
            ' '
        } else {
            character
        }
    }));
    debug_assert_eq!(output.len() - output_start, source_range.len());
    for highlight in highlights {
        let start = highlight.start.max(source_range.start);
        let end = highlight.end.min(source_range.end);
        if start < end {
            ranges.push(
                output_start + start - source_range.start..output_start + end - source_range.start,
            );
        }
    }
}

fn merge_highlight_ranges(mut ranges: Vec<Range<usize>>) -> Vec<Range<usize>> {
    ranges.sort_unstable_by_key(|range| range.start);
    let mut merged = Vec::<Range<usize>>::with_capacity(ranges.len());
    for range in ranges {
        if let Some(previous) = merged
            .last_mut()
            .filter(|previous| range.start <= previous.end)
        {
            previous.end = previous.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    merged
}

fn ranges_intersect(left: Range<usize>, right: Range<usize>) -> bool {
    left.start < right.end && right.start < left.end
}
