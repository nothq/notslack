use crate::ui::surface::{
    SlackMessageBody, SlackMessageBodyBlockKind, SlackMessageBodyTarget, SlackMessageTextStyle,
};

use super::super::reactions::slack_reaction_display;
use super::builder::{
    slack_decode_text_entities, slack_supported_link_url, SlackMessageBodyBlockBuilder,
    SlackMessageBodyBuilder,
};

pub(super) fn prepare_slack_legacy_message_body(body: &str, id_stem: &str) -> SlackMessageBody {
    let mut block = SlackMessageBodyBlockBuilder::new(SlackMessageBodyBlockKind::Paragraph);
    push_slack_legacy_message_text(&mut block, body);
    finish_slack_legacy_message_body(block, id_stem)
}

/// Pushes Slack mrkdwn: `*bold*`, `_italic_`, `~strike~` and `` `code` `` spans
/// around text that may hold links, mentions and emoji.
pub(super) fn push_slack_legacy_message_text(block: &mut SlackMessageBodyBlockBuilder, body: &str) {
    for span in slack_mrkdwn_spans(body) {
        push_slack_legacy_span(block, &body[span.range], span.style);
    }
}

struct SlackMrkdwnSpan {
    range: std::ops::Range<usize>,
    style: SlackMessageTextStyle,
}

/// Splits `body` into unstyled runs and the insides of styled spans. Spans do
/// not nest, stay on one line, and never start or end inside a `<...>` link.
fn slack_mrkdwn_spans(body: &str) -> Vec<SlackMrkdwnSpan> {
    let mut spans = Vec::new();
    let mut plain_start = 0;
    let mut index = 0;
    while index < body.len() {
        let character = body[index..]
            .chars()
            .next()
            .expect("index should stay on a character boundary");
        if character == '<' {
            index = slack_angle_token_end(body, index);
            continue;
        }
        let span = slack_mrkdwn_style(character)
            .filter(|_| slack_mrkdwn_opens_at(body, index, character))
            .and_then(|style| {
                slack_mrkdwn_close(body, index + 1, character).map(|close| (style, close))
            });
        let Some((style, close)) = span else {
            index += character.len_utf8();
            continue;
        };
        spans.push(SlackMrkdwnSpan {
            range: plain_start..index,
            style: SlackMessageTextStyle::default(),
        });
        spans.push(SlackMrkdwnSpan {
            range: index + 1..close,
            style,
        });
        index = close + 1;
        plain_start = index;
    }
    spans.push(SlackMrkdwnSpan {
        range: plain_start..body.len(),
        style: SlackMessageTextStyle::default(),
    });
    spans
}

fn slack_mrkdwn_style(delimiter: char) -> Option<SlackMessageTextStyle> {
    let style = SlackMessageTextStyle::default();
    match delimiter {
        '*' => Some(SlackMessageTextStyle {
            bold: true,
            ..style
        }),
        '_' => Some(SlackMessageTextStyle {
            italic: true,
            ..style
        }),
        '~' => Some(SlackMessageTextStyle {
            strike: true,
            ..style
        }),
        '`' => Some(SlackMessageTextStyle {
            code: true,
            ..style
        }),
        _ => None,
    }
}

/// A delimiter opens a span at the start of a word, with text right after it.
fn slack_mrkdwn_opens_at(body: &str, index: usize, delimiter: char) -> bool {
    let before = body[..index].chars().next_back();
    let after = body[index + delimiter.len_utf8()..].chars().next();
    before.is_none_or(|before| !before.is_alphanumeric() && before != delimiter)
        && after.is_some_and(|after| !after.is_whitespace() && after != delimiter)
}

/// The closing delimiter on the same line: after text, at the end of a word.
fn slack_mrkdwn_close(body: &str, start: usize, delimiter: char) -> Option<usize> {
    let mut index = start;
    while index < body.len() {
        let character = body[index..].chars().next()?;
        if character == '\n' {
            return None;
        }
        if character == '<' && delimiter != '`' {
            index = slack_angle_token_end(body, index);
            continue;
        }
        if character == delimiter && index > start {
            let before = body[..index].chars().next_back();
            let after = body[index + delimiter.len_utf8()..].chars().next();
            let ends_word = after.is_none_or(|after| !after.is_alphanumeric());
            if delimiter == '`'
                || (before.is_some_and(|before| !before.is_whitespace()) && ends_word)
            {
                return Some(index);
            }
        }
        index += character.len_utf8();
    }
    None
}

/// The end of the `<...>` token opening at `start`, or just past the `<` when
/// it never closes on its line.
fn slack_angle_token_end(body: &str, start: usize) -> usize {
    body[start..]
        .find(['>', '\n'])
        .filter(|offset| body[start + offset..].starts_with('>'))
        .map_or(start + 1, |offset| start + offset + 1)
}

fn push_slack_legacy_span(
    block: &mut SlackMessageBodyBlockBuilder,
    body: &str,
    base: SlackMessageTextStyle,
) {
    if base.code {
        block.push_text(&slack_decode_text_entities(body), base);
        return;
    }
    let mut index = 0;
    while index < body.len() {
        let angle_start = body[index..].find('<').map(|offset| index + offset);
        let url_start = slack_next_bare_url_start(body, index);
        let emoji = slack_next_legacy_emoji(body, index);
        let next_start = [
            angle_start,
            url_start,
            emoji.as_ref().map(|emoji| emoji.start),
        ]
        .into_iter()
        .flatten()
        .min();
        match next_start {
            Some(next_start) if angle_start == Some(next_start) => {
                block.push_text(&slack_decode_text_entities(&body[index..next_start]), base);
                if let Some(autolink) = slack_autolink_at(body, next_start) {
                    let style = SlackMessageTextStyle {
                        link: autolink.style.link,
                        mention: autolink.style.mention,
                        ..base
                    };
                    block.push_target(&autolink.display, style, autolink.target);
                    index = autolink.end;
                } else {
                    block.push_text("<", base);
                    index = next_start + 1;
                }
            }
            Some(next_start) if url_start == Some(next_start) => {
                block.push_text(&slack_decode_text_entities(&body[index..next_start]), base);
                let url_end = slack_bare_url_end(body, next_start);
                let url = slack_decode_text_entities(&body[next_start..url_end]);
                block.push_target(
                    &url,
                    SlackMessageTextStyle { link: true, ..base },
                    Some(SlackMessageBodyTarget::Url(url.clone().into())),
                );
                index = url_end;
            }
            Some(next_start) => {
                let emoji = emoji
                    .as_ref()
                    .filter(|emoji| emoji.start == next_start)
                    .expect("next legacy message token should be an emoji");
                block.push_text(&slack_decode_text_entities(&body[index..emoji.start]), base);
                block.push_text(&emoji.display, base);
                index = emoji.end;
            }
            None => {
                block.push_text(&slack_decode_text_entities(&body[index..]), base);
                break;
            }
        }
    }
}

fn finish_slack_legacy_message_body(
    block: SlackMessageBodyBlockBuilder,
    id_stem: &str,
) -> SlackMessageBody {
    let mut prepared = SlackMessageBodyBuilder::new(id_stem);
    prepared.push_block(block);
    prepared.finish()
}

struct SlackLegacyEmoji {
    start: usize,
    end: usize,
    display: String,
}

fn slack_next_legacy_emoji(body: &str, start: usize) -> Option<SlackLegacyEmoji> {
    let mut search_start = start;
    while search_start < body.len() {
        let emoji_start = body[search_start..]
            .find(':')
            .map(|offset| search_start + offset)?;
        let name_start = emoji_start + 1;
        let name_end = body[name_start..]
            .find(':')
            .map(|offset| name_start + offset)?;
        let name = &body[name_start..name_end];
        search_start = name_start;
        if name.is_empty()
            || name.len() > 64
            || !name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || "_+-".contains(character))
        {
            continue;
        }
        let mut shortcode = name.to_string();
        let mut emoji_end = name_end + 1;
        if let Some(suffix) = body[emoji_end..].strip_prefix(":skin-tone-") {
            if let Some((skin_tone, _)) = suffix.split_once(':') {
                if matches!(skin_tone, "2" | "3" | "4" | "5" | "6") {
                    let suffix = format!(":skin-tone-{skin_tone}:");
                    debug_assert!(body[emoji_end..].starts_with(&suffix));
                    shortcode.push_str("::skin-tone-");
                    shortcode.push_str(skin_tone);
                    emoji_end += suffix.len();
                }
            }
        }
        let display = slack_reaction_display(&shortcode);
        if display != format!(":{shortcode}:") {
            return Some(SlackLegacyEmoji {
                start: emoji_start,
                end: emoji_end,
                display,
            });
        }
    }
    None
}

struct SlackLegacyAutolink {
    end: usize,
    display: String,
    target: Option<SlackMessageBodyTarget>,
    style: SlackMessageTextStyle,
}

struct SlackLegacyAutolinkContent {
    display: String,
    target: Option<SlackMessageBodyTarget>,
    link: bool,
    mention: bool,
}

fn slack_autolink_at(body: &str, start: usize) -> Option<SlackLegacyAutolink> {
    let close = body[start..].find('>').map(|offset| start + offset)?;
    let inner = &body[start + 1..close];
    let (raw_target, raw_display) = inner.split_once('|').unwrap_or((inner, ""));
    let content = slack_legacy_autolink_content(raw_target.trim(), raw_display)?;
    Some(SlackLegacyAutolink {
        end: close + 1,
        display: content.display,
        target: content.target,
        style: SlackMessageTextStyle {
            link: content.link,
            mention: content.mention,
            ..Default::default()
        },
    })
}

fn slack_legacy_autolink_content(
    raw_target: &str,
    raw_display: &str,
) -> Option<SlackLegacyAutolinkContent> {
    let (display, target, link, mention) = if let Some(user_id) = raw_target.strip_prefix('@') {
        (
            slack_legacy_autolink_label('@', user_id, raw_display),
            Some(SlackMessageBodyTarget::User(user_id.to_string().into())),
            false,
            true,
        )
    } else if let Some(channel_id) = raw_target.strip_prefix('#') {
        (
            slack_legacy_autolink_label('#', channel_id, raw_display),
            Some(SlackMessageBodyTarget::Channel(
                channel_id.to_string().into(),
            )),
            false,
            true,
        )
    } else if let Some(broadcast) = raw_target.strip_prefix('!') {
        let broadcast = broadcast
            .split_once('^')
            .map_or(broadcast, |(broadcast, _)| broadcast);
        let display = if raw_display.is_empty() {
            format!("@{broadcast}")
        } else {
            slack_decode_text_entities(raw_display)
        };
        (display, None, false, true)
    } else if slack_supported_link_url(raw_target) {
        let target = slack_decode_text_entities(raw_target);
        let display = if raw_display.is_empty() {
            target.clone()
        } else {
            slack_decode_text_entities(raw_display)
        };
        (
            display,
            Some(SlackMessageBodyTarget::Url(target.into())),
            true,
            false,
        )
    } else {
        return None;
    };
    Some(SlackLegacyAutolinkContent {
        display,
        target,
        link,
        mention,
    })
}

fn slack_legacy_autolink_label(prefix: char, target: &str, raw_display: &str) -> String {
    if raw_display.is_empty() {
        format!("{prefix}{target}")
    } else {
        format!(
            "{prefix}{}",
            slack_decode_text_entities(raw_display).trim_start_matches(prefix)
        )
    }
}

pub(super) fn slack_legacy_body_id_stem(body: &str) -> String {
    let hash = body
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            hash.wrapping_mul(0x100000001b3) ^ u64::from(*byte)
        });
    format!("slack-message-body-legacy-{hash:016x}")
}

fn slack_next_bare_url_start(body: &str, start: usize) -> Option<usize> {
    let mut search_start = start;
    while search_start < body.len() {
        let rest = &body[search_start..];
        let http = rest.find("http://");
        let https = rest.find("https://");
        let relative_start = match (http, https) {
            (Some(http), Some(https)) => http.min(https),
            (Some(http), None) => http,
            (None, Some(https)) => https,
            (None, None) => return None,
        };
        let candidate = search_start + relative_start;
        if slack_bare_url_boundary(body, candidate) {
            return Some(candidate);
        }
        search_start = candidate + 1;
    }
    None
}

fn slack_bare_url_boundary(body: &str, start: usize) -> bool {
    body[..start]
        .chars()
        .next_back()
        .is_none_or(|character| character.is_whitespace() || "([{\"'".contains(character))
}

fn slack_bare_url_end(body: &str, start: usize) -> usize {
    let raw_end = body[start..]
        .find(|character: char| character.is_whitespace() || character == '<' || character == '>')
        .map(|offset| start + offset)
        .unwrap_or(body.len());
    let mut end = raw_end;
    while end > start {
        let character = body[..end]
            .chars()
            .next_back()
            .expect("url end should stay on a character boundary");
        if ".,;:!?)]}".contains(character) {
            end -= character.len_utf8();
        } else {
            break;
        }
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    fn styled(body: &str) -> Vec<(&str, SlackMessageTextStyle)> {
        slack_mrkdwn_spans(body)
            .into_iter()
            .filter(|span| !span.range.is_empty())
            .map(|span| (&body[span.range], span.style))
            .collect()
    }

    fn plain() -> SlackMessageTextStyle {
        SlackMessageTextStyle::default()
    }

    #[test]
    fn mrkdwn_markers_style_the_text_between_them() {
        let bold = SlackMessageTextStyle {
            bold: true,
            ..plain()
        };
        let code = SlackMessageTextStyle {
            code: true,
            ..plain()
        };
        assert_eq!(
            styled("*Testing deployed* by `abc_def` now"),
            [
                ("Testing deployed", bold),
                (" by ", plain()),
                ("abc_def", code),
                (" now", plain()),
            ]
        );
    }

    #[test]
    fn a_styled_span_may_hold_a_link() {
        let italic = SlackMessageTextStyle {
            italic: true,
            ..plain()
        };
        assert_eq!(
            styled("_<https://example.com/a_b|the run>_"),
            [("<https://example.com/a_b|the run>", italic)]
        );
    }

    #[test]
    fn markers_inside_words_links_and_arithmetic_are_text() {
        for body in [
            "snake_case_name and 2*3*4",
            "a * b * c",
            "<https://example.com/a_b_c> ~ unfinished",
            "*open\nclose*",
        ] {
            assert_eq!(styled(body), [(body, plain())], "{body}");
        }
    }
}
