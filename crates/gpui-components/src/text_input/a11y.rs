use std::ops::Range;

use gpui::{accesskit, A11ySubtreeBuilder, Context, ElementId, Window};
use unicode_segmentation::UnicodeSegmentation;

use super::{obscured_text, TextInput};

const MAX_CHARS_PER_TEXT_RUN: usize = 255;

struct A11yCharacter {
    bytes: Range<usize>,
    starts_word: bool,
}

struct A11yTextRun {
    node_id: accesskit::NodeId,
    characters: Range<usize>,
}

#[derive(Clone, Copy)]
struct A11yRunSource<'a> {
    text: &'a str,
    characters: &'a [A11yCharacter],
}

#[derive(Clone, Copy)]
struct A11yLineRunPosition {
    index: usize,
    count: usize,
}

pub(super) fn text_input_a11y_state(
    input: &TextInput,
    state_key: ElementId,
    window: &mut Window,
    cx: &mut Context<TextInput>,
) -> (String, impl FnOnce(&mut A11ySubtreeBuilder) + use<>) {
    let state = window.is_a11y_active().then(|| {
        let (text, selection_anchor, selection_focus) = a11y_text_and_selection(input);
        let is_focused = input.focus_handle.is_focused(window);
        let a11y_value = window.use_keyed_state((state_key, "a11y-value"), cx, {
            let text = text.clone();
            move |_, _| text
        });
        if !is_focused && *a11y_value.read(cx) != text {
            *a11y_value.as_mut(cx) = text.clone();
        }
        (
            a11y_value.read(cx).clone(),
            text,
            selection_anchor,
            selection_focus,
        )
    });

    let (value, text_state) = match state {
        Some((value, text, selection_anchor, selection_focus)) => (
            value,
            Some((text, selection_anchor, selection_focus, input.disabled)),
        ),
        None => (String::new(), None),
    };
    let text_runs = move |builder: &mut A11ySubtreeBuilder| {
        if let Some((text, selection_anchor, selection_focus, disabled)) = text_state {
            push_a11y_text_runs(builder, &text, selection_anchor, selection_focus);
            if disabled {
                builder.parent_node().set_disabled();
            }
        }
    };
    (value, text_runs)
}

fn a11y_text_and_selection(input: &TextInput) -> (String, usize, usize) {
    let (selection_anchor, selection_focus) = if input.selection_reversed {
        (input.selected_range.end, input.selected_range.start)
    } else {
        (input.selected_range.start, input.selected_range.end)
    };
    if !input.obscured {
        return (input.content.to_string(), selection_anchor, selection_focus);
    }
    let (text, offsets) = obscured_text(input.content.as_ref());
    let display_offset = |content_offset| {
        offsets
            .binary_search_by_key(&content_offset, |boundary| boundary.content)
            .map(|index| offsets[index].display)
            .expect("obscured accessibility offset must follow a UTF-8 character boundary")
    };
    (
        text,
        display_offset(selection_anchor),
        display_offset(selection_focus),
    )
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn a11y_characters(text: &str) -> Vec<A11yCharacter> {
    let mut characters = Vec::new();
    let mut was_word_char = false;
    for (grapheme_start, grapheme) in text.grapheme_indices(true) {
        let grapheme_is_word = grapheme.chars().next().is_some_and(is_word_char);
        let starts_word = grapheme_is_word && !was_word_char;
        was_word_char = grapheme_is_word;

        if grapheme.len() <= u8::MAX as usize {
            characters.push(A11yCharacter {
                bytes: grapheme_start..grapheme_start + grapheme.len(),
                starts_word,
            });
            continue;
        }

        // AccessKit stores each selectable character's UTF-8 length in a u8.
        // An unusually long grapheme cannot be represented as one character,
        // so retain every byte by falling back to its scalar-value boundaries.
        for (scalar_index, (relative_start, scalar)) in grapheme.char_indices().enumerate() {
            let scalar_start = grapheme_start + relative_start;
            characters.push(A11yCharacter {
                bytes: scalar_start..scalar_start + scalar.len_utf8(),
                starts_word: starts_word && scalar_index == 0,
            });
        }
    }
    characters
}

fn text_position(
    text_len: usize,
    byte_offset: usize,
    characters: &[A11yCharacter],
    runs: &[A11yTextRun],
) -> accesskit::TextPosition {
    let byte_offset = byte_offset.min(text_len);
    let character_index =
        characters.partition_point(|character| character.bytes.end <= byte_offset);
    let run = runs
        .iter()
        .find(|run| run.characters.contains(&character_index))
        .unwrap_or_else(|| {
            runs.last()
                .expect("an accessibility text input has a text run")
        });
    accesskit::TextPosition {
        node: run.node_id,
        character_index: character_index.saturating_sub(run.characters.start),
    }
}

fn push_a11y_text_runs(
    builder: &mut A11ySubtreeBuilder,
    text: &str,
    selection_anchor: usize,
    selection_focus: usize,
) {
    let characters = a11y_characters(text);
    let line_starts = a11y_line_starts(text, &characters);
    let source = A11yRunSource {
        text,
        characters: &characters,
    };
    let mut runs: Vec<A11yTextRun> = Vec::new();
    for (line_index, line_start) in line_starts.iter().copied().enumerate() {
        let line_end = line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(characters.len());
        push_a11y_line_runs(builder, source, line_start..line_end, &mut runs);
    }

    let anchor = text_position(text.len(), selection_anchor, &characters, &runs);
    let focus = text_position(text.len(), selection_focus, &characters, &runs);
    builder
        .parent_node()
        .set_text_selection(accesskit::TextSelection { anchor, focus });
}

fn a11y_line_starts(text: &str, characters: &[A11yCharacter]) -> Vec<usize> {
    let mut line_starts = vec![0];
    line_starts.extend(
        characters
            .iter()
            .enumerate()
            .filter_map(|(index, character)| {
                (text[character.bytes.clone()] == *"\n").then_some(index + 1)
            }),
    );
    line_starts
}

fn push_a11y_line_runs(
    builder: &mut A11ySubtreeBuilder,
    source: A11yRunSource<'_>,
    line: Range<usize>,
    runs: &mut Vec<A11yTextRun>,
) {
    let line_run_count = line.len().div_ceil(MAX_CHARS_PER_TEXT_RUN).max(1);
    for line_run_index in 0..line_run_count {
        let character_start = line.start + line_run_index * MAX_CHARS_PER_TEXT_RUN;
        let character_end = (character_start + MAX_CHARS_PER_TEXT_RUN).min(line.end);
        push_a11y_text_run(
            builder,
            source,
            character_start..character_end,
            A11yLineRunPosition {
                index: line_run_index,
                count: line_run_count,
            },
            runs,
        );
    }
}

fn push_a11y_text_run(
    builder: &mut A11ySubtreeBuilder,
    source: A11yRunSource<'_>,
    character_range: Range<usize>,
    line_run: A11yLineRunPosition,
    runs: &mut Vec<A11yTextRun>,
) {
    let byte_start = source
        .characters
        .get(character_range.start)
        .map_or(source.text.len(), |character| character.bytes.start);
    let byte_end = source
        .characters
        .get(character_range.end.saturating_sub(1))
        .map_or(byte_start, |character| character.bytes.end);
    let node_id = builder.synthetic_node_id(runs.len() as u64);
    let mut node = accesskit::Node::new(accesskit::Role::TextRun);
    node.set_value(source.text[byte_start..byte_end].to_owned());
    let characters = &source.characters[character_range.clone()];
    node.set_character_lengths(
        characters
            .iter()
            .map(|character| {
                u8::try_from(character.bytes.len())
                    .expect("accessibility character length exceeds u8")
            })
            .collect::<Vec<_>>(),
    );
    node.set_word_starts(
        characters
            .iter()
            .enumerate()
            .filter(|(_, character)| character.starts_word)
            .map(|(index, _)| u8::try_from(index).expect("text run index exceeds u8"))
            .collect::<Vec<_>>(),
    );
    if line_run.index > 0 {
        node.set_previous_on_line(runs[runs.len() - 1].node_id);
    }
    if line_run.index + 1 < line_run.count {
        node.set_next_on_line(builder.synthetic_node_id(runs.len() as u64 + 1));
    }
    builder.push_child(node_id, node);
    runs.push(A11yTextRun {
        node_id,
        characters: character_range,
    });
}
