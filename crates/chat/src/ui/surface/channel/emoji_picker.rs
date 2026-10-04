use super::{
    div, px, relative, rgb, AnyElement, AppearanceMode, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, SlackAuxPanelRow,
    SlackAuxPanelSection, SlackAuxPanelState, StatefulInteractiveElement, Styled, SurfaceState,
};

const SLACK_EMOJI_PICKER_COLUMNS: usize = 10;

#[derive(Clone, Copy)]
struct SlackEmojiPickerPalette {
    text: u32,
    muted_text: u32,
    cell_hover_bg: u32,
}

impl SurfaceState {
    pub(crate) fn render_slack_emoji_picker_popover(
        &self,
        panel: &SlackAuxPanelState,
        cx: &mut Context<Self>,
    ) -> Div {
        let query = panel.query.as_deref().unwrap_or("");
        let palette = slack_emoji_picker_palette(self.appearance_mode);
        div()
            .h_full()
            .flex()
            .flex_col()
            .text_color(rgb(palette.text))
            .child(self.render_slack_composer_popover_search_field(
                query,
                "Search all emoji",
                true,
                cx,
            ))
            .child(
                div()
                    .flex_grow(1.0)
                    .min_h(px(0.0))
                    .id("slack-emoji-picker-scroll")
                    .overflow_scroll()
                    .px(px(8.0))
                    .pt(px(8.0))
                    .pb(px(4.0))
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .children(
                        panel
                            .sections
                            .iter()
                            .take(4)
                            .map(|section| self.render_slack_emoji_picker_section(section, cx)),
                    ),
            )
    }

    fn render_slack_emoji_picker_section(
        &self,
        section: &SlackAuxPanelSection,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_emoji_picker_palette(self.appearance_mode);
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .when_some(slack_emoji_picker_section_title(section), |this, title| {
                this.child(
                    div()
                        .h(px(20.0))
                        .px(px(4.0))
                        .flex()
                        .items_center()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(rgb(palette.text))
                        .child(title),
                )
            })
            .child(self.render_slack_emoji_picker_rows(section, cx))
    }

    fn render_slack_emoji_picker_rows(
        &self,
        section: &SlackAuxPanelSection,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_emoji_picker_palette(self.appearance_mode);
        if section.rows.iter().all(|row| row.action.is_none()) {
            let label = section
                .rows
                .first()
                .map(|row| row.label.clone())
                .unwrap_or_default();
            return div()
                .h(px(32.0))
                .px(px(4.0))
                .flex()
                .items_center()
                .text_size(px(13.0))
                .text_color(rgb(palette.muted_text))
                .child(label);
        }
        div().flex().flex_col().children(
            section
                .rows
                .chunks(SLACK_EMOJI_PICKER_COLUMNS)
                .take(6)
                .map(|row_chunk| {
                    div().h(px(32.0)).flex().items_center().children(
                        row_chunk
                            .iter()
                            .map(|row| self.render_slack_emoji_picker_cell(row, cx)),
                    )
                }),
        )
    }

    fn render_slack_emoji_picker_cell(
        &self,
        row: &SlackAuxPanelRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_emoji_picker_palette(self.appearance_mode);
        let cell = div()
            .w(px(34.0))
            .h(px(32.0))
            .rounded(px(5.0))
            .flex()
            .items_center()
            .justify_center()
            .when(row.action.is_some(), |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(rgb(palette.cell_hover_bg)))
            })
            .child(self.render_slack_emoji_picker_cell_contents(row));
        let Some(action) = row.action.clone() else {
            return cell;
        };
        cell.on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                this.activate_slack_aux_panel_action(action.clone(), cx);
            }),
        )
    }

    fn render_slack_emoji_picker_cell_contents(&self, row: &SlackAuxPanelRow) -> AnyElement {
        div()
            .text_size(px(22.0))
            .text_color(rgb(slack_emoji_picker_palette(self.appearance_mode).text))
            .line_height(relative(1.0))
            .child(
                row.emoji_glyph
                    .clone()
                    .expect("actionable emoji row should have a prepared display glyph"),
            )
            .into_any_element()
    }
}

fn slack_emoji_picker_palette(appearance_mode: AppearanceMode) -> SlackEmojiPickerPalette {
    match appearance_mode {
        AppearanceMode::Dark => SlackEmojiPickerPalette {
            text: 0xf4f5f7,
            muted_text: 0x8f959c,
            cell_hover_bg: 0x2f3439,
        },
        AppearanceMode::Light => SlackEmojiPickerPalette {
            text: 0x1d1c1d,
            muted_text: 0x5e5d60,
            cell_hover_bg: 0xe8f5fa,
        },
    }
}

fn slack_emoji_picker_section_title(section: &SlackAuxPanelSection) -> Option<String> {
    section.title.as_deref().map(|title| match title {
        "Suggested" => "Frequently Used".to_string(),
        other => other.to_string(),
    })
}
