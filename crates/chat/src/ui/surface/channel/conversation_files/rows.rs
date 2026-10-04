use super::{
    alpha, div, img, list, px, rgb, slack_conversation_files_action_key, slack_palette, AnyElement,
    Context, Div, FluentBuilder, FontWeight, InteractiveElement, IntoElement, ListSizingBehavior,
    ParentElement, StatefulInteractiveElement, Styled, SurfaceState,
    SLACK_CONVERSATION_MEDIA_TILE_HEIGHT,
};
use crate::ui::surface::{
    SlackConversationFilesExplorerRow, SlackConversationFilesRow, SlackConversationFilesRowVisual,
    SlackFileVisual, SLACK_CONVERSATION_FILES_MEDIA_GRID_HEIGHT,
    SLACK_CONVERSATION_FILES_MEDIA_HEADER_HEIGHT, SLACK_CONVERSATION_FILES_MEDIA_PREVIEW_HEIGHT,
    SLACK_CONVERSATION_FILES_ROW_HEIGHT, SLACK_CONVERSATION_FILES_SPACER_HEIGHT,
    SLACK_CONVERSATION_MEDIA_COLUMNS,
};
use crate::ui::SlackConversationFilesFilter;
use gpui::{ObjectFit, Role, StyledImage};

impl SurfaceState {
    pub(super) fn render_slack_conversation_files_explorer(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let view = cx.entity();
        let count = self.slack_conversation_files_explorer_rows.len();
        div()
            .id("slack-conversation-files-explorer")
            .role(Role::ListBox)
            .aria_label("Conversation files and links")
            .mt(px(1.0))
            .flex_grow(1.0)
            .min_h(px(0.0))
            .overflow_hidden()
            .child(
                list(
                    self.slack_conversation_files_list_state.clone(),
                    move |index, _window, cx| {
                        view.update(cx, |this, cx| {
                            let explorer_row = *this
                                .slack_conversation_files_explorer_rows
                                .get(index)
                                .expect("conversation Files explorer row index must exist");
                            this.render_slack_conversation_files_explorer_row(explorer_row, cx)
                        })
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .size_full(),
            )
            .when(self.slack_conversation_files_loading && count > 2, |this| {
                this.child(
                    div()
                        .absolute()
                        .right(px(40.0))
                        .bottom(px(8.0))
                        .text_size(px(12.0))
                        .child("Loading…"),
                )
            })
            .into_any_element()
    }

    fn render_slack_conversation_files_explorer_row(
        &self,
        explorer_row: SlackConversationFilesExplorerRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match explorer_row {
            SlackConversationFilesExplorerRow::Controls => self
                .render_slack_conversation_files_controls(cx)
                .into_any_element(),
            SlackConversationFilesExplorerRow::MediaHeader => self
                .render_slack_conversation_media_header(cx)
                .into_any_element(),
            SlackConversationFilesExplorerRow::MediaGrid {
                start,
                end,
                preview,
            } => self.render_slack_conversation_media_grid(start, end, preview, cx),
            SlackConversationFilesExplorerRow::Spacer => div()
                .h(px(SLACK_CONVERSATION_FILES_SPACER_HEIGHT))
                .flex_none()
                .into_any_element(),
            SlackConversationFilesExplorerRow::Result { source, index } => {
                let rows = self.slack_conversation_files_cache.rows(source);
                self.render_slack_conversation_files_row(
                    rows.get(index)
                        .expect("conversation Files result row index must exist"),
                    index + 1,
                    rows.len(),
                    cx,
                )
            }
            SlackConversationFilesExplorerRow::Loading => self
                .render_slack_conversation_files_skeleton()
                .into_any_element(),
            SlackConversationFilesExplorerRow::Empty => self
                .render_slack_conversation_files_empty()
                .into_any_element(),
            SlackConversationFilesExplorerRow::Error => self
                .render_slack_conversation_files_error(
                    self.slack_conversation_files_error
                        .as_deref()
                        .expect("conversation Files error row requires an error"),
                    cx,
                )
                .into_any_element(),
        }
    }

    fn render_slack_conversation_media_header(&self, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_CONVERSATION_FILES_MEDIA_HEADER_HEIGHT))
            .flex_none()
            .px(px(32.0))
            .pt(px(16.0))
            .pb(px(8.0))
            .flex()
            .items_center()
            .justify_between()
            .font_weight(FontWeight::BOLD)
            .text_size(px(15.0))
            .text_color(rgb(palette.main_secondary_text))
            .child("Photos and videos")
            .child(
                div()
                    .id("slack-conversation-files-see-all-media")
                    .role(Role::Button)
                    .aria_label("See all photos and videos")
                    .focusable()
                    .tab_stop(true)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.select_slack_conversation_files_filter(
                            SlackConversationFilesFilter::Media,
                            cx,
                        );
                    }))
                    .on_key_down(cx.listener(|this, event, window, cx| {
                        if slack_conversation_files_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.select_slack_conversation_files_filter(
                                SlackConversationFilesFilter::Media,
                                cx,
                            );
                        }
                    }))
                    .text_color(rgb(palette.link))
                    .child("See all"),
            )
    }

    fn render_slack_conversation_media_grid(
        &self,
        start: usize,
        end: usize,
        preview: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let height = if preview {
            SLACK_CONVERSATION_FILES_MEDIA_PREVIEW_HEIGHT
        } else {
            SLACK_CONVERSATION_FILES_MEDIA_GRID_HEIGHT
        };
        let rows = self
            .slack_conversation_files_cache
            .media
            .get(start..end)
            .expect("conversation Media grid range must exist");
        div()
            .h(px(height))
            .flex_none()
            .px(px(32.0))
            .pt(px(if preview { 1.0 } else { 0.0 }))
            .pb(px(if preview {
                0.0
            } else {
                SLACK_CONVERSATION_FILES_SPACER_HEIGHT
            }))
            .grid()
            .grid_cols(SLACK_CONVERSATION_MEDIA_COLUMNS as u16)
            .gap(px(16.0))
            .children(
                rows.iter()
                    .map(|row| self.render_slack_conversation_media_tile(row, cx)),
            )
            .into_any_element()
    }

    fn render_slack_conversation_files_row(
        &self,
        row: &SlackConversationFilesRow,
        position: usize,
        count: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let link_url = row.link_url.clone();
        div()
            .h(px(SLACK_CONVERSATION_FILES_ROW_HEIGHT))
            .flex_none()
            .px(px(32.0))
            .child(
                div()
                    .id(row.element_id.clone())
                    .role(Role::ListBoxOption)
                    .aria_label(row.accessibility_label.clone())
                    .aria_position_in_set(position)
                    .aria_size_of_set(count)
                    .focusable()
                    .tab_stop(true)
                    .h_full()
                    .w_full()
                    .px(px(12.0))
                    .border_l_1()
                    .border_r_1()
                    .border_b_1()
                    .when(position == 1, |this| this.border_t_1())
                    .border_color(alpha(palette.main_text, 0.12))
                    .when(position == 1, |this| {
                        this.rounded_tl(px(8.0)).rounded_tr(px(8.0))
                    })
                    .when(position == count, |this| {
                        this.rounded_bl(px(8.0)).rounded_br(px(8.0))
                    })
                    .bg(rgb(palette.attachment_bg))
                    .cursor_pointer()
                    .hover(move |style| style.bg(alpha(palette.main_text, 0.06)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_slack_link(link_url.as_ref(), cx);
                    }))
                    .on_key_down({
                        let link_url = row.link_url.clone();
                        cx.listener(move |this, event, window, cx| {
                            if slack_conversation_files_action_key(event) {
                                window.prevent_default();
                                cx.stop_propagation();
                                this.open_slack_link(link_url.as_ref(), cx);
                            }
                        })
                    })
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(self.render_slack_conversation_files_row_tile(row))
                    .child(self.render_slack_conversation_files_row_copy(row, &palette)),
            )
            .into_any_element()
    }

    fn render_slack_conversation_files_row_copy(
        &self,
        row: &SlackConversationFilesRow,
        palette: &super::super::SlackPalette,
    ) -> Div {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .w_full()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .font_weight(FontWeight::BOLD)
                    .text_size(px(15.0))
                    .text_color(rgb(palette.main_text))
                    .child(row.title.clone()),
            )
            .child(
                div()
                    .w_full()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(13.0))
                    .text_color(alpha(palette.main_text, 0.7))
                    .child(row.metadata.clone()),
            )
    }

    fn render_slack_conversation_files_row_tile(&self, row: &SlackConversationFilesRow) -> Div {
        let image = row
            .thumbnail_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned());
        let has_image = image.is_some();
        div()
            .size(px(36.0))
            .flex_none()
            .rounded(px(8.0))
            .overflow_hidden()
            .bg(rgb(row.visual.tile_fill()))
            .flex()
            .items_center()
            .justify_center()
            .when_some(image, |this, image| {
                this.child(img(image).size_full().object_fit(ObjectFit::Cover))
            })
            .when(!has_image, |this| {
                let glyph_size = match row.visual {
                    SlackConversationFilesRowVisual::File(SlackFileVisual::Pdf) => 8.0,
                    SlackConversationFilesRowVisual::File(_) => 14.0,
                    SlackConversationFilesRowVisual::Link => 17.0,
                };
                this.text_size(px(glyph_size))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(
                        if row.visual == SlackConversationFilesRowVisual::Link {
                            0x5e5d60
                        } else {
                            0xffffff
                        },
                    ))
                    .child(row.visual.glyph())
            })
    }

    fn render_slack_conversation_media_tile(
        &self,
        row: &SlackConversationFilesRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let link_url = row.link_url.clone();
        let image = row
            .thumbnail_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned());
        let has_image = image.is_some();
        div()
            .id(row.element_id.clone())
            .role(Role::Button)
            .aria_label(row.accessibility_label.clone())
            .focusable()
            .tab_stop(true)
            .h(px(SLACK_CONVERSATION_MEDIA_TILE_HEIGHT))
            .rounded(px(8.0))
            .overflow_hidden()
            .bg(alpha(palette.main_text, 0.1))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_slack_link(link_url.as_ref(), cx);
            }))
            .on_key_down({
                let link_url = row.link_url.clone();
                cx.listener(move |this, event, window, cx| {
                    if slack_conversation_files_action_key(event) {
                        window.prevent_default();
                        cx.stop_propagation();
                        this.open_slack_link(link_url.as_ref(), cx);
                    }
                })
            })
            .flex()
            .items_center()
            .justify_center()
            .when_some(image, |this, image| {
                this.child(img(image).size_full().object_fit(ObjectFit::Cover))
            })
            .when(!has_image, |this| {
                this.child(
                    div()
                        .text_size(px(24.0))
                        .text_color(rgb(palette.main_secondary_text))
                        .child(row.visual.glyph()),
                )
            })
            .into_any_element()
    }
}
