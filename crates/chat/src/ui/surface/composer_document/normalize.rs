use super::{SlackComposerEntityRun, SlackComposerLinkRun, SlackComposerStyleRun};

pub(super) fn normalize_runs(
    mut runs: Vec<SlackComposerStyleRun>,
    text: &str,
) -> Vec<SlackComposerStyleRun> {
    runs.retain(|run| !run.range.is_empty() && !run.style.is_empty());
    runs.sort_unstable_by_key(|run| (run.range.start, run.range.end));
    let mut normalized: Vec<SlackComposerStyleRun> = Vec::with_capacity(runs.len());
    for run in runs {
        assert!(
            run.range.end <= text.len()
                && text.is_char_boundary(run.range.start)
                && text.is_char_boundary(run.range.end),
            "Slack composer style range must follow UTF-8 boundaries"
        );
        if let Some(previous) = normalized.last_mut() {
            assert!(
                previous.range.end <= run.range.start,
                "Slack composer style ranges must not overlap"
            );
            if previous.range.end == run.range.start && previous.style == run.style {
                previous.range.end = run.range.end;
                continue;
            }
        }
        normalized.push(run);
    }
    normalized
}

pub(super) fn normalize_link_runs(
    mut links: Vec<SlackComposerLinkRun>,
    text: &str,
) -> Vec<SlackComposerLinkRun> {
    links.retain(|link| !link.range.is_empty());
    links.sort_unstable_by_key(|link| (link.range.start, link.range.end));
    let mut normalized: Vec<SlackComposerLinkRun> = Vec::with_capacity(links.len());
    for link in links {
        assert!(
            link.range.end <= text.len()
                && text.is_char_boundary(link.range.start)
                && text.is_char_boundary(link.range.end),
            "Slack composer link range must follow UTF-8 boundaries"
        );
        if let Some(previous) = normalized.last_mut() {
            assert!(
                previous.range.end <= link.range.start,
                "Slack composer link ranges must not overlap"
            );
            if previous.range.end == link.range.start && previous.url == link.url {
                previous.range.end = link.range.end;
                continue;
            }
        }
        normalized.push(link);
    }
    normalized
}

pub(super) fn normalize_entity_runs(
    mut entities: Vec<SlackComposerEntityRun>,
    text: &str,
) -> Vec<SlackComposerEntityRun> {
    entities.sort_unstable_by_key(|entity| (entity.range.start, entity.range.end));
    let mut normalized: Vec<SlackComposerEntityRun> = Vec::with_capacity(entities.len());
    for entity in entities {
        entity.entity.assert_canonical();
        assert!(
            !entity.range.is_empty()
                && entity.range.end <= text.len()
                && text.is_char_boundary(entity.range.start)
                && text.is_char_boundary(entity.range.end),
            "Slack composer entity range must be non-empty and follow UTF-8 boundaries"
        );
        assert!(
            entity
                .entity
                .matches_display_text(&text[entity.range.clone()]),
            "Slack composer entity range must match its canonical display text"
        );
        if let Some(previous) = normalized.last() {
            assert!(
                previous.range.end <= entity.range.start,
                "Slack composer entity ranges must not overlap"
            );
        }
        normalized.push(entity);
    }
    normalized
}
