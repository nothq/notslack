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

pub(super) fn push_slack_legacy_message_text(block: &mut SlackMessageBodyBlockBuilder, body: &str) {
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
                block.push_decoded_text(&body[index..next_start]);
                if let Some(autolink) = slack_autolink_at(body, next_start) {
                    block.push_target(&autolink.display, autolink.style, autolink.target);
                    index = autolink.end;
                } else {
                    block.push_text("<", SlackMessageTextStyle::default());
                    index = next_start + 1;
                }
            }
            Some(next_start) if url_start == Some(next_start) => {
                block.push_decoded_text(&body[index..next_start]);
                let url_end = slack_bare_url_end(body, next_start);
                let url = slack_decode_text_entities(&body[next_start..url_end]);
                block.push_target(
                    &url,
                    SlackMessageTextStyle {
                        link: true,
                        ..Default::default()
                    },
                    Some(SlackMessageBodyTarget::Url(url.clone().into())),
                );
                index = url_end;
            }
            Some(next_start) => {
                let emoji = emoji
                    .as_ref()
                    .filter(|emoji| emoji.start == next_start)
                    .expect("next legacy message token should be an emoji");
                block.push_decoded_text(&body[index..emoji.start]);
                block.push_text(&emoji.display, SlackMessageTextStyle::default());
                index = emoji.end;
            }
            None => {
                block.push_decoded_text(&body[index..]);
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
