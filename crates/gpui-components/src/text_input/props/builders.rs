use std::sync::Arc;

use gpui::{ElementId, FontWeight, SharedString};

use super::{
    TextInputAccessibility, TextInputAction, TextInputAtom, TextInputAtomClick, TextInputChange,
    TextInputEnterBehavior, TextInputGhost, TextInputHighlight, TextInputKeyAction,
    TextInputKeyDownPreAction, TextInputKeyPreAction, TextInputLayoutChange, TextInputMode,
    TextInputPointerSelectionChange, TextInputPreMutationActionHandler, TextInputProps,
    TextInputStateChange, TextInputStyle, TextInputVerticalBoundaryAction, TextInputWrapMode,
};

impl TextInputProps {
    pub fn single_line(value: impl Into<SharedString>) -> Self {
        Self {
            value: value.into(),
            placeholder: SharedString::default(),
            mode: TextInputMode::SingleLine,
            style: TextInputStyle::default(),
            bordered: true,
            disabled: false,
            obscured: false,
            request_focus: false,
            fill_width: false,
            wrap_mode: TextInputWrapMode::SoftWrap,
            wrap_at_hyphens: false,
            font_weight: FontWeight::NORMAL,
            tab_text: "    ".into(),
            highlights: Arc::default(),
            atoms: Arc::default(),
            ghost: None,
            enter_behavior: TextInputEnterBehavior::ModeDefault,
            accessibility: None,
            on_change: None,
            on_change_with_state: None,
            on_selection_change: None,
            on_submit: None,
            on_submit_with_state: None,
            on_key_down_before_default: None,
            on_enter_before_default: None,
            on_escape: None,
            on_up: None,
            on_down: None,
            on_home: None,
            on_end: None,
            on_left_before_default: None,
            on_right_before_default: None,
            on_left_at_start: None,
            on_right_at_end: None,
            on_up_at_first_visual_line: None,
            on_down_at_last_visual_line: None,
            on_tab: None,
            on_tab_with_state: None,
            on_undo_with_state: None,
            on_redo_with_state: None,
            on_platform_shift_up_with_state: None,
            on_platform_shift_down_with_state: None,
            on_select_all_again_with_state: None,
            on_pointer_selection: None,
            on_pre_mutation_action: None,
            on_atom_click: None,
            on_focus: None,
            on_backspace_when_empty: None,
            on_backspace_at_start: None,
            on_delete_at_end: None,
            on_layout_change: None,
        }
    }

    pub fn multiline(value: impl Into<SharedString>) -> Self {
        Self {
            mode: TextInputMode::Multiline {
                max_visible_lines: None,
            },
            ..Self::single_line(value)
        }
    }

    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn mode(mut self, mode: TextInputMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn style(mut self, style: TextInputStyle) -> Self {
        self.style = style;
        self
    }

    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Obscure a single-line input without changing the value supplied to callbacks.
    pub fn obscured(mut self, obscured: bool) -> Self {
        self.obscured = obscured;
        self
    }

    pub fn request_focus(mut self, request_focus: bool) -> Self {
        self.request_focus = request_focus;
        self
    }

    pub fn fill_width(mut self, fill_width: bool) -> Self {
        self.fill_width = fill_width;
        self
    }

    pub fn wrap_mode(mut self, wrap_mode: TextInputWrapMode) -> Self {
        self.wrap_mode = wrap_mode;
        self
    }

    /// Allow multiline text to wrap at hyphen boundaries, matching browser text layout.
    pub fn wrap_at_hyphens(mut self, wrap_at_hyphens: bool) -> Self {
        self.wrap_at_hyphens = wrap_at_hyphens;
        self
    }

    pub fn font_weight(mut self, font_weight: FontWeight) -> Self {
        self.font_weight = font_weight;
        self
    }

    pub fn tab_text(mut self, tab_text: impl Into<SharedString>) -> Self {
        self.tab_text = tab_text.into();
        self
    }

    pub fn highlights(mut self, highlights: impl Into<Arc<[TextInputHighlight]>>) -> Self {
        self.highlights = highlights.into();
        self
    }

    /// Replace content ranges with atomic inline tokens; see [`TextInputAtom`].
    pub fn atoms(mut self, atoms: impl Into<Arc<[TextInputAtom]>>) -> Self {
        self.atoms = atoms.into();
        self
    }

    /// Draw display-only ghost text at a content offset; see [`TextInputGhost`].
    pub fn ghost(mut self, ghost: impl Into<Option<TextInputGhost>>) -> Self {
        self.ghost = ghost.into();
        self
    }

    pub fn enter_behavior(mut self, enter_behavior: TextInputEnterBehavior) -> Self {
        self.enter_behavior = enter_behavior;
        self
    }

    pub fn accessibility(
        mut self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
    ) -> Self {
        self.accessibility = Some(TextInputAccessibility {
            id: id.into(),
            label: label.into(),
        });
        self
    }

    pub fn on_change(mut self, on_change: TextInputChange) -> Self {
        self.on_change = Some(on_change);
        self
    }

    pub fn on_change_with_state(mut self, on_change: TextInputStateChange) -> Self {
        self.on_change_with_state = Some(on_change);
        self
    }

    pub fn on_selection_change(mut self, on_selection_change: TextInputStateChange) -> Self {
        self.on_selection_change = Some(on_selection_change);
        self
    }

    pub fn on_pre_mutation_action(mut self, on_action: TextInputPreMutationActionHandler) -> Self {
        self.on_pre_mutation_action = Some(on_action);
        self
    }

    /// Handle a left click strictly inside an atom's display text instead of
    /// moving the caret or focus.
    pub fn on_atom_click(mut self, on_atom_click: TextInputAtomClick) -> Self {
        self.on_atom_click = Some(on_atom_click);
        self
    }

    pub fn on_submit(mut self, on_submit: TextInputAction) -> Self {
        self.on_submit = Some(on_submit);
        self
    }

    pub fn on_submit_with_state(mut self, on_submit: TextInputKeyAction) -> Self {
        self.on_submit_with_state = Some(on_submit);
        self
    }

    pub fn on_key_down_before_default(mut self, on_key_down: TextInputKeyDownPreAction) -> Self {
        self.on_key_down_before_default = Some(on_key_down);
        self
    }

    pub fn on_enter_before_default(mut self, on_enter: TextInputKeyPreAction) -> Self {
        self.on_enter_before_default = Some(on_enter);
        self
    }

    pub fn on_escape(mut self, on_escape: TextInputAction) -> Self {
        self.on_escape = Some(on_escape);
        self
    }

    pub fn on_up(mut self, on_up: TextInputAction) -> Self {
        self.on_up = Some(on_up);
        self
    }

    pub fn on_down(mut self, on_down: TextInputAction) -> Self {
        self.on_down = Some(on_down);
        self
    }

    pub fn on_home(mut self, on_home: TextInputAction) -> Self {
        self.on_home = Some(on_home);
        self
    }

    pub fn on_end(mut self, on_end: TextInputAction) -> Self {
        self.on_end = Some(on_end);
        self
    }

    pub fn on_left_before_default(mut self, on_left: TextInputKeyPreAction) -> Self {
        self.on_left_before_default = Some(on_left);
        self
    }

    pub fn on_right_before_default(mut self, on_right: TextInputKeyPreAction) -> Self {
        self.on_right_before_default = Some(on_right);
        self
    }

    pub fn on_left_at_start(mut self, on_left: TextInputKeyAction) -> Self {
        self.on_left_at_start = Some(on_left);
        self
    }

    pub fn on_right_at_end(mut self, on_right: TextInputKeyAction) -> Self {
        self.on_right_at_end = Some(on_right);
        self
    }

    pub fn on_up_at_first_visual_line(mut self, on_up: TextInputVerticalBoundaryAction) -> Self {
        self.on_up_at_first_visual_line = Some(on_up);
        self
    }

    pub fn on_down_at_last_visual_line(mut self, on_down: TextInputVerticalBoundaryAction) -> Self {
        self.on_down_at_last_visual_line = Some(on_down);
        self
    }

    pub fn on_tab(mut self, on_tab: TextInputAction) -> Self {
        self.on_tab = Some(on_tab);
        self
    }

    pub fn on_tab_with_state(mut self, on_tab: TextInputKeyAction) -> Self {
        self.on_tab_with_state = Some(on_tab);
        self
    }

    pub fn on_undo_with_state(mut self, on_undo: TextInputKeyAction) -> Self {
        self.on_undo_with_state = Some(on_undo);
        self
    }

    pub fn on_redo_with_state(mut self, on_redo: TextInputKeyAction) -> Self {
        self.on_redo_with_state = Some(on_redo);
        self
    }

    pub fn on_platform_shift_up_with_state(mut self, on_up: TextInputKeyAction) -> Self {
        self.on_platform_shift_up_with_state = Some(on_up);
        self
    }

    pub fn on_platform_shift_down_with_state(mut self, on_down: TextInputKeyAction) -> Self {
        self.on_platform_shift_down_with_state = Some(on_down);
        self
    }

    pub fn on_select_all_again_with_state(mut self, on_select_all: TextInputKeyAction) -> Self {
        self.on_select_all_again_with_state = Some(on_select_all);
        self
    }

    pub fn on_pointer_selection(
        mut self,
        on_pointer_selection: TextInputPointerSelectionChange,
    ) -> Self {
        self.on_pointer_selection = Some(on_pointer_selection);
        self
    }

    pub fn on_focus(mut self, on_focus: TextInputAction) -> Self {
        self.on_focus = Some(on_focus);
        self
    }

    pub fn on_backspace_when_empty(mut self, on_backspace_when_empty: TextInputAction) -> Self {
        self.on_backspace_when_empty = Some(on_backspace_when_empty);
        self
    }

    pub fn on_backspace_at_start(mut self, on_backspace_at_start: TextInputKeyAction) -> Self {
        self.on_backspace_at_start = Some(on_backspace_at_start);
        self
    }

    pub fn on_delete_at_end(mut self, on_delete_at_end: TextInputKeyAction) -> Self {
        self.on_delete_at_end = Some(on_delete_at_end);
        self
    }

    pub fn on_layout_change(mut self, on_layout_change: TextInputLayoutChange) -> Self {
        self.on_layout_change = Some(on_layout_change);
        self
    }
}
