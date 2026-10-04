use std::collections::{HashMap, HashSet};

use super::{
    ParsedSlackFlannelEmoji, ParsedSlackFlannelEmojiAlias, ParsedSlackFlannelEmojiAsset,
    SLACK_FLANNEL_EMOJI_LIST_METHOD,
};

#[derive(Clone, Copy)]
enum ResolvedAliasPresentation<'a> {
    Platform,
    ImageUrl(&'a str),
}

struct ResolvedAlias<'a> {
    path: Vec<(&'a str, &'a ParsedSlackFlannelEmojiAlias)>,
    presentation: ResolvedAliasPresentation<'a>,
}

enum AliasTraversal<'a> {
    Resolved(ResolvedAliasPresentation<'a>),
    Continue(&'a str),
}

pub(super) fn validate_alias_graph(catalog: &[ParsedSlackFlannelEmoji]) -> Result<(), String> {
    let by_name = catalog
        .iter()
        .map(|emoji| (emoji.name.as_str(), &emoji.asset))
        .collect::<HashMap<_, _>>();
    let mut resolved = HashMap::new();

    for emoji in catalog {
        if !matches!(emoji.asset, ParsedSlackFlannelEmojiAsset::Alias(_))
            || resolved.contains_key(emoji.name.as_str())
        {
            continue;
        }
        let resolution = resolve_alias(emoji, catalog.len(), &by_name, &resolved)?;
        for (name, alias) in resolution.path {
            validate_resolved_alias_presentation(name, alias, resolution.presentation)?;
            resolved.insert(name, resolution.presentation);
        }
    }

    Ok(())
}

fn resolve_alias<'a>(
    emoji: &'a ParsedSlackFlannelEmoji,
    catalog_len: usize,
    by_name: &HashMap<&'a str, &'a ParsedSlackFlannelEmojiAsset>,
    resolved: &HashMap<&'a str, ResolvedAliasPresentation<'a>>,
) -> Result<ResolvedAlias<'a>, String> {
    let mut current = emoji.name.as_str();
    let mut path = Vec::new();
    let mut visiting = HashSet::new();
    let mut traversal_count = 0;
    loop {
        traversal_count += 1;
        if traversal_count > catalog_len {
            return Err(format!(
                "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} alias {} exceeded the catalog traversal bound",
                emoji.name
            ));
        }
        if let Some(presentation) = resolved.get(current) {
            return Ok(ResolvedAlias {
                path,
                presentation: *presentation,
            });
        }
        if !visiting.insert(current) {
            return Err(format!(
                "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} alias {} contains a cycle at {current}",
                emoji.name
            ));
        }
        let Some(ParsedSlackFlannelEmojiAsset::Alias(alias)) = by_name.get(current) else {
            return Err(format!(
                "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} alias graph changed type at {current}"
            ));
        };
        path.push((current, alias));
        match alias_traversal(current, alias, by_name)? {
            AliasTraversal::Resolved(presentation) => {
                return Ok(ResolvedAlias { path, presentation });
            }
            AliasTraversal::Continue(target) => current = target,
        }
    }
}

fn alias_traversal<'a>(
    current: &str,
    alias: &'a ParsedSlackFlannelEmojiAlias,
    by_name: &HashMap<&'a str, &'a ParsedSlackFlannelEmojiAsset>,
) -> Result<AliasTraversal<'a>, String> {
    if emojis::get_by_shortcode(&alias.target).is_some() {
        return Ok(AliasTraversal::Resolved(
            ResolvedAliasPresentation::Platform,
        ));
    }
    match by_name.get(alias.target.as_str()) {
        Some(ParsedSlackFlannelEmojiAsset::ImageUrl(url)) => Ok(AliasTraversal::Resolved(
            ResolvedAliasPresentation::ImageUrl(url),
        )),
        Some(ParsedSlackFlannelEmojiAsset::Alias(_)) => {
            Ok(AliasTraversal::Continue(&alias.target))
        }
        Some(ParsedSlackFlannelEmojiAsset::Platform) => Err(format!(
            "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} alias {current} targets platform emoji {}, which is absent from the shared shortcode catalog",
            alias.target
        )),
        None => Err(format!(
            "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} alias {current} has unresolved target {}",
            alias.target
        )),
    }
}

fn validate_resolved_alias_presentation(
    name: &str,
    alias: &ParsedSlackFlannelEmojiAlias,
    presentation: ResolvedAliasPresentation<'_>,
) -> Result<(), String> {
    let Some(resolved_image_url) = alias.resolved_image_url.as_deref() else {
        return Ok(());
    };
    match presentation {
        ResolvedAliasPresentation::ImageUrl(target_image_url)
            if resolved_image_url == target_image_url =>
        {
            Ok(())
        }
        ResolvedAliasPresentation::ImageUrl(_) => Err(format!(
            "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} alias {name} returned an image that did not match its target"
        )),
        ResolvedAliasPresentation::Platform => Err(format!(
            "Slack {SLACK_FLANNEL_EMOJI_LIST_METHOD} alias {name} returned an image for a platform target"
        )),
    }
}
