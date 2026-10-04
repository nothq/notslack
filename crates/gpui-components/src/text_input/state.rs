use std::{ops::Range, sync::Arc};

use gpui::{px, Context, FocusHandle, SharedString, Window};

use super::{
    atoms::assert_atoms_fit, hard_line_count, offsets::is_grapheme_boundary, sanitize_for_mode,
    TextInput, TextInputAtom, TextInputCallbacks, TextInputGhost, TextInputMode,
    TextInputPreMutationAction, TextInputPreMutationDecision, TextInputProps,
    TextInputReplacementOrigin,
};

mod layout_state;

impl TextInput {
    pub fn new(props: TextInputProps, cx: &mut Context<Self>) -> Self {
        assert!(
            !props.obscured || props.mode == TextInputMode::SingleLine,
            "obscured text inputs must be single-line"
        );
        let content: SharedString = sanitize_for_mode(props.mode, props.value.as_ref()).into();
        let focus_handle = cx
            .focus_handle()
            .tab_stop(props.accessibility.is_some() && !props.disabled);
        let callbacks = TextInputCallbacks::from_props(&props);
        assert_atoms_fit(content.as_ref(), &props.atoms, props.ghost.as_ref());
        Self::from_initial_props(props, content, focus_handle, callbacks, cx)
    }

    fn from_initial_props(
        props: TextInputProps,
        content: SharedString,
        focus_handle: FocusHandle,
        callbacks: TextInputCallbacks,
        cx: &Context<Self>,
    ) -> Self {
        let offset = content.len();
        let measured_visual_line_count = hard_line_count(content.as_ref());
        Self {
            root_id: gpui::ElementId::NamedInteger(
                "gpui-components-text-input".into(),
                cx.entity_id().as_u64(),
            ),
            focus_handle,
            content,
            placeholder: props.placeholder,
            selected_range: offset..offset,
            selection_reversed: false,
            marked_range: None,
            pre_mutation_ime_active: false,
            layout_revision: 0,
            layout_cache: None,
            measured_visual_line_count,
            last_bounds: None,
            scroll_x: px(0.0),
            scroll_y: px(0.0),
            desired_x: None,
            is_selecting: false,
            pointer_selection_anchor: None,
            handling_key_down: false,
            mode: props.mode,
            style: props.style,
            bordered: props.bordered,
            disabled: props.disabled,
            obscured: props.obscured,
            request_focus: props.request_focus,
            fill_width: props.fill_width,
            wrap_mode: props.wrap_mode,
            wrap_at_hyphens: props.wrap_at_hyphens,
            font_weight: props.font_weight,
            tab_text: props.tab_text,
            highlights: props.highlights,
            atoms: props.atoms,
            ghost: props.ghost,
            hovered_atom: None,
            enter_behavior: props.enter_behavior,
            accessibility: props.accessibility,
            callbacks,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            caret_visible: true,
            blink_scheduled: false,
            blink_generation: 0,
            focused_last_render: false,
        }
    }

    pub fn apply_props(&mut self, props: TextInputProps, cx: &mut Context<Self>) {
        assert!(
            !props.obscured || props.mode == TextInputMode::SingleLine,
            "obscured text inputs must be single-line"
        );
        let layout_properties_changed = self.placeholder != props.placeholder
            || self.mode != props.mode
            || self.style != props.style
            || self.font_weight != props.font_weight
            || self.obscured != props.obscured
            || self.fill_width != props.fill_width
            || self.wrap_mode != props.wrap_mode
            || self.wrap_at_hyphens != props.wrap_at_hyphens
            || (!Arc::ptr_eq(&self.highlights, &props.highlights)
                && self.highlights != props.highlights)
            || (!Arc::ptr_eq(&self.atoms, &props.atoms) && self.atoms != props.atoms)
            || self.ghost != props.ghost;
        let callbacks = TextInputCallbacks::from_props(&props);
        self.focus_handle = self
            .focus_handle
            .clone()
            .tab_stop(props.accessibility.is_some() && !props.disabled);
        self.placeholder = props.placeholder;
        self.mode = props.mode;
        self.style = props.style;
        self.bordered = props.bordered;
        self.disabled = props.disabled;
        self.obscured = props.obscured;
        self.request_focus = props.request_focus;
        self.fill_width = props.fill_width;
        self.wrap_mode = props.wrap_mode;
        self.wrap_at_hyphens = props.wrap_at_hyphens;
        self.font_weight = props.font_weight;
        self.tab_text = props.tab_text;
        self.highlights = props.highlights;
        self.enter_behavior = props.enter_behavior;
        self.accessibility = props.accessibility;
        self.callbacks = callbacks;
        if layout_properties_changed {
            self.invalidate_layout();
        }
        self.set_text(props.value, cx);
        self.set_atoms(props.atoms, props.ghost, cx);
    }

    /// Adopt the atoms and ghost text the caller derived from the new value.
    /// The props own both, so they replace whatever the edits left behind.
    fn set_atoms(
        &mut self,
        atoms: Arc<[TextInputAtom]>,
        ghost: Option<TextInputGhost>,
        cx: &mut Context<Self>,
    ) {
        assert_atoms_fit(self.content.as_ref(), &atoms, ghost.as_ref());
        if *self.atoms == *atoms && self.ghost == ghost {
            return;
        }
        self.atoms = atoms;
        self.ghost = ghost;
        self.hovered_atom = None;
        if self.snap_selection_to_atoms() {
            self.desired_x = None;
        }
        self.invalidate_layout();
        cx.notify();
    }

    pub fn set_text(&mut self, value: impl Into<SharedString>, cx: &mut Context<Self>) {
        let value = sanitize_for_mode(self.mode, value.into().as_ref());
        if self.content.as_ref() == value {
            return;
        }
        let previous_cursor = self.cursor_offset();
        self.replace_content(value.into());
        let cursor = self.floor_grapheme_boundary(previous_cursor);
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = None;
        self.pre_mutation_ime_active = false;
        self.scroll_x = px(0.0);
        self.scroll_y = px(0.0);
        self.desired_x = None;
        self.pointer_selection_anchor = None;
        self.is_selecting = false;
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.invalidate_layout();
        self.reset_caret_blink();
        cx.notify();
    }

    pub fn set_text_and_move_cursor_to_end(
        &mut self,
        value: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) {
        self.set_text(value, cx);
        let end = self.content.len();
        self.move_cursor_after_set_text(end, cx);
    }

    pub fn set_text_and_move_cursor(
        &mut self,
        value: impl Into<SharedString>,
        offset: usize,
        cx: &mut Context<Self>,
    ) {
        self.set_text(value, cx);
        assert!(
            is_grapheme_boundary(self.content.as_ref(), offset),
            "text input cursor offset must follow Unicode grapheme boundaries"
        );
        self.move_cursor_after_set_text(offset, cx);
    }

    pub fn set_text_and_mark_range(
        &mut self,
        value: impl Into<SharedString>,
        marked_range: Range<usize>,
        cx: &mut Context<Self>,
    ) {
        self.set_text(value, cx);
        assert!(
            marked_range.start < marked_range.end
                && marked_range.end <= self.content.len()
                && self.content.is_char_boundary(marked_range.start)
                && self.content.is_char_boundary(marked_range.end),
            "text input marked range must be non-empty, fit the text, and follow UTF-8 boundaries"
        );
        let cursor = marked_range.end;
        self.selected_range = cursor..cursor;
        self.selection_reversed = false;
        self.marked_range = Some(marked_range);
        self.pre_mutation_ime_active = false;
        self.desired_x = None;
        self.invalidate_layout();
        self.reset_caret_blink();
        cx.notify();
    }

    fn move_cursor_after_set_text(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selected_range != (offset..offset) || self.selection_reversed {
            self.selected_range = offset..offset;
            self.selection_reversed = false;
            self.marked_range = None;
            self.desired_x = None;
            self.reset_caret_blink();
            cx.notify();
        }
    }

    pub fn text(&self) -> &str {
        self.content.as_ref()
    }

    pub fn selection_range(&self) -> Range<usize> {
        self.selected_range.clone()
    }

    pub fn transform_selection(
        &mut self,
        transform: impl FnOnce(&str) -> (String, Range<usize>),
        cx: &mut Context<Self>,
    ) -> SharedString {
        let selected_range = self.selected_range.clone();
        let (replacement, selection_in_replacement) =
            transform(&self.content[selected_range.clone()]);
        assert!(
            selection_in_replacement.start <= selection_in_replacement.end
                && selection_in_replacement.end <= replacement.len()
                && is_grapheme_boundary(&replacement, selection_in_replacement.start)
                && is_grapheme_boundary(&replacement, selection_in_replacement.end),
            "text input selection transform returned an invalid replacement selection"
        );
        let content = format!(
            "{}{}{}",
            &self.content[..selected_range.start],
            replacement,
            &self.content[selected_range.end..]
        );
        if self.content.as_ref() != content {
            self.push_undo_snapshot();
        }
        self.splice_content(selected_range.clone(), &replacement);
        self.selected_range = selected_range.start + selection_in_replacement.start
            ..selected_range.start + selection_in_replacement.end;
        self.selection_reversed = false;
        self.marked_range = None;
        self.desired_x = None;
        self.invalidate_layout();
        self.reset_caret_blink();
        cx.notify();
        self.content.clone()
    }

    pub fn focus_handle_clone(&self) -> FocusHandle {
        self.focus_handle.clone()
    }

    pub fn request_focus(&mut self, cx: &mut Context<Self>) {
        if self.request_focus {
            return;
        }
        self.request_focus = true;
        cx.notify();
    }

    pub(super) fn set_text_from_accessibility(
        &mut self,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.disabled {
            return;
        }
        let value = sanitize_for_mode(self.mode, &value);
        if self.content.as_ref() == value {
            return;
        }
        if self.dispatch_pre_mutation_action(
            TextInputPreMutationAction::Replace {
                snapshot: self.snapshot(),
                range: 0..self.content.len(),
                text: value.clone(),
                origin: TextInputReplacementOrigin::Accessibility,
            },
            window,
            cx,
        ) == TextInputPreMutationDecision::Handled
        {
            return;
        }
        let previous_selection = self.selected_range.clone();
        let previous_selection_reversed = self.selection_reversed;
        self.push_undo_snapshot();
        self.replace_content(value.into());
        let offset = self.content.len();
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.marked_range = None;
        self.scroll_x = px(0.0);
        self.scroll_y = px(0.0);
        self.desired_x = None;
        self.invalidate_layout();
        self.refresh_layout_for_last_width(window, cx);
        self.reset_caret_blink();
        cx.notify();
        self.emit_change(window, cx);
        self.emit_selection_change_if_needed(
            previous_selection,
            previous_selection_reversed,
            window,
            cx,
        );
    }
}
