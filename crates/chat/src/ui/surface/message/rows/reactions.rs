use emojis::{get_by_shortcode, SkinTone};

const SLACK_STANDARD_EMOJI_ASSET_ROOT: &str =
    "https://a.slack-edge.com/production-standard-emoji-assets/16.0/apple-small";

pub(super) fn slack_reaction_display(emoji_name: &str) -> String {
    slack_reaction_unicode(emoji_name)
        .map(str::to_string)
        .unwrap_or_else(|| format!(":{emoji_name}:"))
}

pub(super) fn slack_reaction_presentation(emoji_name: &str) -> (String, Option<String>) {
    let Some(unicode) = slack_reaction_unicode(emoji_name) else {
        return (format!(":{emoji_name}:"), None);
    };
    let codepoints = unicode
        .chars()
        .map(|character| format!("{:04x}", u32::from(character)))
        .collect::<Vec<_>>()
        .join("-");
    (
        unicode.to_string(),
        Some(format!(
            "{SLACK_STANDARD_EMOJI_ASSET_ROOT}/{codepoints}@2x.png"
        )),
    )
}

pub(super) fn slack_reaction_group_shortcode(emoji_name: &str) -> &str {
    slack_reaction_skin_tone(emoji_name).map_or(emoji_name, |(shortcode, _)| shortcode)
}

fn slack_reaction_unicode(emoji_name: &str) -> Option<&'static str> {
    let (shortcode, skin_tone) = slack_reaction_skin_tone(emoji_name)
        .map_or((emoji_name, None), |(shortcode, skin_tone)| {
            (shortcode, Some(skin_tone))
        });
    let emoji = get_by_shortcode(shortcode)?;
    let emoji = skin_tone
        .and_then(|skin_tone| emoji.with_skin_tone(skin_tone))
        .unwrap_or(emoji);
    Some(emoji.as_str())
}

fn slack_reaction_skin_tone(emoji_name: &str) -> Option<(&str, SkinTone)> {
    let (shortcode, suffix) = emoji_name.rsplit_once("::skin-tone-")?;
    let skin_tone = match suffix {
        "2" => SkinTone::Light,
        "3" => SkinTone::MediumLight,
        "4" => SkinTone::Medium,
        "5" => SkinTone::MediumDark,
        "6" => SkinTone::Dark,
        _ => return None,
    };
    Some((shortcode, skin_tone))
}
