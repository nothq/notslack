use super::{ContiguousTextEdit, SlackComposerEntityRun, SlackComposerLinkRun};

pub(super) fn remap_link_runs(
    links: &[SlackComposerLinkRun],
    edit: &ContiguousTextEdit,
) -> Vec<SlackComposerLinkRun> {
    let new_end = edit.old_range.start + edit.replacement_len;
    let replacement_url = if edit.replacement_len > 0 {
        links
            .iter()
            .find(|link| {
                if edit.old_range.is_empty() {
                    link.range.start < edit.old_range.start && edit.old_range.start < link.range.end
                } else {
                    link.range.start <= edit.old_range.start && edit.old_range.end <= link.range.end
                }
            })
            .map(|link| link.url.clone())
    } else {
        None
    };
    let mut updated = Vec::with_capacity(links.len() + usize::from(replacement_url.is_some()));
    for link in links {
        if link.range.end <= edit.old_range.start {
            updated.push(link.clone());
            continue;
        }
        if link.range.start >= edit.old_range.end {
            updated.push(SlackComposerLinkRun {
                range: shift_after_edit(link.range.start, edit.old_range.end, new_end)
                    ..shift_after_edit(link.range.end, edit.old_range.end, new_end),
                url: link.url.clone(),
            });
            continue;
        }
        if link.range.start < edit.old_range.start {
            updated.push(SlackComposerLinkRun {
                range: link.range.start..edit.old_range.start,
                url: link.url.clone(),
            });
        }
        if link.range.end > edit.old_range.end {
            updated.push(SlackComposerLinkRun {
                range: new_end..shift_after_edit(link.range.end, edit.old_range.end, new_end),
                url: link.url.clone(),
            });
        }
    }
    if let Some(url) = replacement_url {
        updated.push(SlackComposerLinkRun {
            range: edit.old_range.start..new_end,
            url,
        });
    }
    updated
}

pub(super) fn remap_entity_runs(
    entities: &[SlackComposerEntityRun],
    edit: &ContiguousTextEdit,
) -> Vec<SlackComposerEntityRun> {
    let new_end = edit.old_range.start + edit.replacement_len;
    let mut updated = Vec::with_capacity(entities.len());
    for entity in entities {
        if entity.range.end <= edit.old_range.start {
            updated.push(entity.clone());
            continue;
        }
        if entity.range.start >= edit.old_range.end {
            updated.push(SlackComposerEntityRun {
                range: shift_after_edit(entity.range.start, edit.old_range.end, new_end)
                    ..shift_after_edit(entity.range.end, edit.old_range.end, new_end),
                entity: entity.entity.clone(),
            });
        }
    }
    updated
}

pub(super) fn contiguous_text_edit(old: &str, new: &str) -> Option<ContiguousTextEdit> {
    if old == new {
        return None;
    }

    let common_len = old.len().min(new.len());
    let mut prefix = 0;
    while prefix < common_len && old.as_bytes()[prefix] == new.as_bytes()[prefix] {
        prefix += 1;
    }
    while prefix > 0 && (!old.is_char_boundary(prefix) || !new.is_char_boundary(prefix)) {
        prefix -= 1;
    }

    let max_suffix = (old.len() - prefix).min(new.len() - prefix);
    let mut suffix = 0;
    while suffix < max_suffix
        && old.as_bytes()[old.len() - suffix - 1] == new.as_bytes()[new.len() - suffix - 1]
    {
        suffix += 1;
    }
    while suffix > 0
        && (!old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix))
    {
        suffix -= 1;
    }

    Some(ContiguousTextEdit {
        old_range: prefix..old.len() - suffix,
        replacement_len: new.len() - prefix - suffix,
    })
}

pub(super) fn shift_after_edit(offset: usize, old_end: usize, new_end: usize) -> usize {
    if new_end >= old_end {
        offset + (new_end - old_end)
    } else {
        offset - (old_end - new_end)
    }
}
