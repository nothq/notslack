use std::ops::Range;

const SLACK_SEARCH_HIGHLIGHT_START: char = '\u{e000}';
const SLACK_SEARCH_HIGHLIGHT_END: char = '\u{e001}';

pub(in crate::live::api::search) struct SlackSearchHighlightedText {
    pub(in crate::live::api::search) text: String,
    pub(in crate::live::api::search) ranges: Vec<Range<usize>>,
}

pub(in crate::live::api::search) fn strip_search_highlight_markers(
    method: &str,
    field: &str,
    text: String,
) -> Result<String, String> {
    Ok(parse_search_highlighted_text_ranges(method, field, text)?.text)
}

pub(in crate::live::api::search) fn parse_search_highlighted_text_ranges(
    method: &str,
    field: &str,
    text: String,
) -> Result<SlackSearchHighlightedText, String> {
    if !text.contains(SLACK_SEARCH_HIGHLIGHT_START) && !text.contains(SLACK_SEARCH_HIGHLIGHT_END) {
        return Ok(SlackSearchHighlightedText {
            text,
            ranges: Vec::new(),
        });
    }

    let mut body = String::with_capacity(text.len());
    let mut highlight_start = None;
    let mut ranges = Vec::new();
    for character in text.chars() {
        match character {
            SLACK_SEARCH_HIGHLIGHT_START if highlight_start.is_some() => {
                return Err(format!(
                    "Slack {method} response returned nested {field} highlight markers"
                ));
            }
            SLACK_SEARCH_HIGHLIGHT_START => highlight_start = Some(body.len()),
            SLACK_SEARCH_HIGHLIGHT_END if highlight_start.is_none() => {
                return Err(format!(
                    "Slack {method} response returned an unmatched {field} highlight end marker"
                ));
            }
            SLACK_SEARCH_HIGHLIGHT_END => {
                let start = highlight_start
                    .take()
                    .expect("validated Slack search highlight start must exist");
                if start == body.len() {
                    return Err(format!(
                        "Slack {method} response returned an empty {field} highlight"
                    ));
                }
                ranges.push(start..body.len());
            }
            _ => body.push(character),
        }
    }
    if highlight_start.is_some() {
        return Err(format!(
            "Slack {method} response returned an unclosed {field} highlight start marker"
        ));
    }
    Ok(SlackSearchHighlightedText { text: body, ranges })
}
