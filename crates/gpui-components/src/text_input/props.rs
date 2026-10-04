use std::{ops::Range, rc::Rc, sync::Arc};

use gpui::{
    px, rgb, Bounds, ClipboardItem, Context, ElementId, FontStyle, FontWeight, Hsla, KeyDownEvent,
    Modifiers, Pixels, Point, SharedString, StrikethroughStyle, UnderlineStyle, Window,
};

use super::TextInput;

mod builders;

pub type TextInputChange = Rc<dyn Fn(String, &mut Window, &mut Context<TextInput>)>;
pub type TextInputAction = Rc<dyn Fn(&mut Window, &mut Context<TextInput>)>;
pub type TextInputLayoutChange = Rc<dyn Fn(&mut Window, &mut Context<TextInput>)>;
pub type TextInputStateChange = Rc<dyn Fn(TextInputSnapshot, &mut Window, &mut Context<TextInput>)>;
pub type TextInputKeyAction =
    Rc<dyn Fn(TextInputSnapshot, Modifiers, &mut Window, &mut Context<TextInput>)>;
pub type TextInputKeyPreAction =
    Rc<dyn Fn(TextInputSnapshot, Modifiers, &mut Window, &mut Context<TextInput>) -> bool>;
pub type TextInputKeyDownPreAction =
    Rc<dyn Fn(TextInputSnapshot, &KeyDownEvent, &mut Window, &mut Context<TextInput>) -> bool>;
pub type TextInputVerticalBoundaryAction =
    Rc<dyn Fn(TextInputVerticalBoundary, Modifiers, &mut Window, &mut Context<TextInput>) -> bool>;
pub type TextInputPointerSelectionChange =
    Rc<dyn Fn(TextInputPointerSelection, &mut Window, &mut Context<TextInput>)>;
pub type TextInputPreMutationActionHandler = Rc<
    dyn Fn(
        TextInputPreMutationAction,
        &mut Window,
        &mut Context<TextInput>,
    ) -> TextInputPreMutationDecision,
>;
/// Invoked with the index of the clicked atom and its window-space bounds.
pub type TextInputAtomClick =
    Rc<dyn Fn(usize, Bounds<Pixels>, &mut Window, &mut Context<TextInput>)>;

/// Inline token drawn as one non-editable unit in place of a content range.
///
/// The caret never rests strictly inside `range`: navigation snaps to its
/// edges, deletion removes the whole range, and double-click selects it.
#[derive(Clone, Debug, PartialEq)]
pub struct TextInputAtom {
    /// UTF-8 byte range of the content replaced on screen by `display`.
    /// Non-empty, on char boundaries; atoms are sorted and non-overlapping.
    pub range: Range<usize>,
    /// Text drawn instead of the content range.
    pub display: SharedString,
    /// Byte length of the leading part of `display` drawn in `prefix_color`
    /// (the "@" of "@Today"); 0 for none.
    pub prefix_len: usize,
    pub color: Hsla,
    pub prefix_color: Hsla,
    /// Underline the part after the prefix while the pointer hovers the atom.
    pub hover_underline: bool,
}

/// Display-only text drawn at a content offset without being part of the value.
#[derive(Clone, Debug, PartialEq)]
pub struct TextInputGhost {
    /// Content byte offset at which the ghost text is drawn (display only).
    pub offset: usize,
    pub text: SharedString,
    pub color: Hsla,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextInputSnapshot {
    pub text: String,
    pub selection: Range<usize>,
    pub cursor: usize,
    pub is_composing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputReplacementOrigin {
    Typing,
    Accessibility,
    DeleteBackward,
    DeleteForward,
    ImeCommit,
    ImeMarked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextInputPreMutationAction {
    Copy {
        snapshot: TextInputSnapshot,
    },
    Cut {
        snapshot: TextInputSnapshot,
    },
    Paste {
        snapshot: TextInputSnapshot,
        range: Range<usize>,
        text: String,
        item: ClipboardItem,
    },
    Replace {
        snapshot: TextInputSnapshot,
        range: Range<usize>,
        text: String,
        origin: TextInputReplacementOrigin,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputPreMutationDecision {
    Continue,
    Handled,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextInputVerticalBoundary {
    pub snapshot: TextInputSnapshot,
    /// Absolute window x for the shaped caret's retained vertical-navigation column.
    pub window_x: Pixels,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputVisualLine {
    First,
    Last,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputPointerSelectionPhase {
    Begin,
    Update,
    End,
    EndOutside,
}

#[derive(Clone, Debug)]
pub struct TextInputPointerSelection {
    pub phase: TextInputPointerSelectionPhase,
    /// Window-relative pointer position.
    pub position: Point<Pixels>,
    /// Fixed byte offset in this input from which the drag selection started.
    pub anchor: usize,
    pub modifiers: Modifiers,
    pub snapshot: TextInputSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextInputMode {
    SingleLine,
    Multiline { max_visible_lines: Option<usize> },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextInputWrapMode {
    #[default]
    SoftWrap,
    NoWrap,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextInputEnterBehavior {
    /// Preserve the existing mode-specific behavior: single-line inputs submit
    /// and multiline inputs insert a newline.
    #[default]
    ModeDefault,
    /// Submit on Enter. Multiline inputs continue to insert a newline when
    /// Shift-Enter is pressed.
    SubmitOnEnter,
}

/// Stable identity and label for an input exposed to assistive technology.
///
/// The id must remain stable across frames and be unique among sibling
/// elements.
#[derive(Clone)]
pub struct TextInputAccessibility {
    pub id: ElementId,
    pub label: SharedString,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextInputHighlight {
    pub range: Range<usize>,
    pub color: Hsla,
    pub background: Option<Hsla>,
    /// Font family override for this range. Takes precedence over `monospace`.
    pub font_family: Option<SharedString>,
    /// Corner radius for the background quad. Has no effect without `background`.
    pub background_corner_radius: Pixels,
    /// Horizontal background-only expansion around the highlighted glyph range.
    pub background_padding_x: Pixels,
    /// Vertical background-only inset within each visual line.
    pub background_inset_y: Pixels,
    pub font_weight: Option<FontWeight>,
    pub font_style: Option<FontStyle>,
    pub underline: Option<UnderlineStyle>,
    pub strikethrough: Option<StrikethroughStyle>,
    pub monospace: bool,
}

#[derive(Clone, PartialEq)]
pub struct TextInputStyle {
    pub height: Pixels,
    pub min_height: Pixels,
    pub padding_x: Pixels,
    pub padding_y: Pixels,
    pub radius: Pixels,
    pub background: Hsla,
    pub border: Hsla,
    pub focused_border: Hsla,
    pub text: Hsla,
    pub placeholder: Hsla,
    pub selection: Hsla,
    pub caret: Hsla,
    pub font_size: Pixels,
    pub line_height: Pixels,
    pub font_family: Option<SharedString>,
}

impl Default for TextInputStyle {
    fn default() -> Self {
        Self {
            height: px(36.0),
            min_height: px(36.0),
            padding_x: px(10.0),
            padding_y: px(7.0),
            radius: px(5.0),
            background: rgb(0x191919).into(),
            border: Hsla::from(rgb(0xffffff)).opacity(0.08),
            focused_border: Hsla::from(rgb(0x78bced)).opacity(0.86),
            text: rgb(0xf0efed).into(),
            placeholder: rgb(0x65645e).into(),
            selection: Hsla::from(rgb(0x78bced)).opacity(0.28),
            caret: rgb(0x78bced).into(),
            font_size: px(12.0),
            line_height: px(16.0),
            font_family: None,
        }
    }
}

#[derive(Clone)]
pub struct TextInputProps {
    pub value: SharedString,
    pub placeholder: SharedString,
    pub mode: TextInputMode,
    pub style: TextInputStyle,
    pub bordered: bool,
    pub disabled: bool,
    pub obscured: bool,
    pub request_focus: bool,
    pub fill_width: bool,
    pub wrap_mode: TextInputWrapMode,
    pub wrap_at_hyphens: bool,
    pub font_weight: FontWeight,
    pub tab_text: SharedString,
    pub highlights: Arc<[TextInputHighlight]>,
    /// Sorted, non-overlapping inline tokens replacing content ranges on screen.
    pub atoms: Arc<[TextInputAtom]>,
    pub ghost: Option<TextInputGhost>,
    pub enter_behavior: TextInputEnterBehavior,
    pub accessibility: Option<TextInputAccessibility>,
    pub on_change: Option<TextInputChange>,
    pub on_change_with_state: Option<TextInputStateChange>,
    pub on_selection_change: Option<TextInputStateChange>,
    pub on_submit: Option<TextInputAction>,
    pub on_submit_with_state: Option<TextInputKeyAction>,
    pub on_key_down_before_default: Option<TextInputKeyDownPreAction>,
    pub on_enter_before_default: Option<TextInputKeyPreAction>,
    pub on_escape: Option<TextInputAction>,
    pub on_up: Option<TextInputAction>,
    pub on_down: Option<TextInputAction>,
    pub on_home: Option<TextInputAction>,
    pub on_end: Option<TextInputAction>,
    pub on_left_before_default: Option<TextInputKeyPreAction>,
    pub on_right_before_default: Option<TextInputKeyPreAction>,
    pub on_left_at_start: Option<TextInputKeyAction>,
    pub on_right_at_end: Option<TextInputKeyAction>,
    pub on_up_at_first_visual_line: Option<TextInputVerticalBoundaryAction>,
    pub on_down_at_last_visual_line: Option<TextInputVerticalBoundaryAction>,
    pub on_tab: Option<TextInputAction>,
    pub on_tab_with_state: Option<TextInputKeyAction>,
    pub on_undo_with_state: Option<TextInputKeyAction>,
    pub on_redo_with_state: Option<TextInputKeyAction>,
    pub on_platform_shift_up_with_state: Option<TextInputKeyAction>,
    pub on_platform_shift_down_with_state: Option<TextInputKeyAction>,
    pub on_select_all_again_with_state: Option<TextInputKeyAction>,
    pub on_pointer_selection: Option<TextInputPointerSelectionChange>,
    pub on_pre_mutation_action: Option<TextInputPreMutationActionHandler>,
    pub on_atom_click: Option<TextInputAtomClick>,
    pub on_focus: Option<TextInputAction>,
    pub on_backspace_when_empty: Option<TextInputAction>,
    pub on_backspace_at_start: Option<TextInputKeyAction>,
    pub on_delete_at_end: Option<TextInputKeyAction>,
    pub on_layout_change: Option<TextInputLayoutChange>,
}
