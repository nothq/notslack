use std::{ops::Range, sync::Arc};

use gpui::{Bounds, ElementId, FocusHandle, FontWeight, Pixels, SharedString};

mod a11y;
mod atoms;
mod callbacks;
mod caret;
mod element;
mod enter;
mod ime;
mod input;
mod layout;
mod navigation;
mod offsets;
mod pointer;
mod selection;

use callbacks::TextInputCallbacks;
use layout::{hard_line_count, shape_text_layout, TextLayoutCache, TextLayoutLine};
pub use offsets::{ceil_grapheme_boundary, floor_grapheme_boundary};

const CARET_BLINK_MS: u64 = 530;

fn text_input_caret_width() -> Pixels {
    gpui::px(1.5)
}

pub const TEXT_INPUT_AUTOMATION_IME_SET_KEY: &str = "notslack-automation-ime-set";
pub const TEXT_INPUT_AUTOMATION_IME_COMMIT_KEY: &str = "notslack-automation-ime-commit";
pub const TEXT_INPUT_AUTOMATION_IME_CANCEL_KEY: &str = "notslack-automation-ime-cancel";

#[derive(Clone)]
struct TextInputUndoSnapshot {
    content: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
}

mod props;
mod state;

pub use props::{
    TextInputAccessibility, TextInputAction, TextInputAtom, TextInputAtomClick, TextInputChange,
    TextInputEnterBehavior, TextInputGhost, TextInputHighlight, TextInputKeyAction,
    TextInputKeyDownPreAction, TextInputKeyPreAction, TextInputLayoutChange, TextInputMode,
    TextInputPointerSelection, TextInputPointerSelectionChange, TextInputPointerSelectionPhase,
    TextInputPreMutationAction, TextInputPreMutationActionHandler, TextInputPreMutationDecision,
    TextInputProps, TextInputReplacementOrigin, TextInputSnapshot, TextInputStateChange,
    TextInputStyle, TextInputVerticalBoundary, TextInputVerticalBoundaryAction,
    TextInputVisualLine, TextInputWrapMode,
};

pub struct TextInput {
    root_id: ElementId,
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    pre_mutation_ime_active: bool,
    layout_revision: u64,
    layout_cache: Option<TextLayoutCache>,
    measured_visual_line_count: usize,
    last_bounds: Option<Bounds<Pixels>>,
    scroll_x: Pixels,
    scroll_y: Pixels,
    desired_x: Option<Pixels>,
    is_selecting: bool,
    pointer_selection_anchor: Option<usize>,
    handling_key_down: bool,
    mode: TextInputMode,
    style: TextInputStyle,
    bordered: bool,
    disabled: bool,
    obscured: bool,
    request_focus: bool,
    fill_width: bool,
    wrap_mode: TextInputWrapMode,
    wrap_at_hyphens: bool,
    font_weight: FontWeight,
    tab_text: SharedString,
    highlights: Arc<[TextInputHighlight]>,
    atoms: Arc<[TextInputAtom]>,
    ghost: Option<TextInputGhost>,
    /// Index into `atoms` of the atom under the pointer.
    hovered_atom: Option<usize>,
    enter_behavior: TextInputEnterBehavior,
    accessibility: Option<TextInputAccessibility>,
    callbacks: TextInputCallbacks,
    undo_stack: Vec<TextInputUndoSnapshot>,
    redo_stack: Vec<TextInputUndoSnapshot>,
    caret_visible: bool,
    blink_scheduled: bool,
    blink_generation: u64,
    focused_last_render: bool,
}

fn sanitize_for_mode(mode: TextInputMode, text: &str) -> String {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    match mode {
        TextInputMode::SingleLine => text.replace('\n', " "),
        TextInputMode::Multiline { .. } => text,
    }
}

#[derive(Clone, Copy)]
pub(super) struct TextDisplayOffset {
    pub(super) content: usize,
    pub(super) display: usize,
}

pub(super) fn obscured_text(text: &str) -> (String, Arc<[TextDisplayOffset]>) {
    const MASK: char = '\u{2022}';

    let mut display = String::with_capacity(text.chars().count() * MASK.len_utf8());
    let mut offsets = Vec::with_capacity(text.chars().count() + 1);
    offsets.push(TextDisplayOffset {
        content: 0,
        display: 0,
    });
    for (content, character) in text.char_indices() {
        display.push(MASK);
        offsets.push(TextDisplayOffset {
            content: content + character.len_utf8(),
            display: display.len(),
        });
    }
    (display, offsets.into())
}
