use std::sync::Arc;

use super::{
    alpha, div, img, px, rgb, slack_icon, slack_palette, AnyElement, AppearanceMode, Context, Div,
    FontWeight, Image, InteractiveElement, IntoElement, KeyDownEvent, ListSizingBehavior,
    ParentElement, SlackPalette, SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::surface::{SlackBookmarkFolderRow, SlackBookmarkFolderRowKind};
use gpui::{uniform_list, Role, Stateful};

const SLACK_BOOKMARK_FOLDER_ROW_HEIGHT: f32 = 62.0;

impl SurfaceState {
    pub(crate) fn render_slack_bookmark_folder_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let title = self
            .slack_active_bookmark_folder_label
            .clone()
            .unwrap_or_else(|| "Folder".into());
        let content =
            if self.slack_bookmark_folder_rows.is_empty() && self.slack_bookmark_folder_loading {
                self.render_slack_bookmark_folder_loading(&title)
            } else if self.slack_bookmark_folder_rows.is_empty() {
                if let Some(error) = self.slack_bookmark_folder_error.as_ref() {
                    self.render_slack_bookmark_folder_error(error, cx)
                } else {
                    self.render_slack_bookmark_folder_empty()
                }
            } else {
                self.render_slack_bookmark_folder_list(&title, cx)
            };
        let palette = slack_palette(self.appearance_mode);
        let panel_background = match self.appearance_mode {
            AppearanceMode::Dark => palette.main_bg,
            AppearanceMode::Light => 0xf8f8f8,
        };
        div()
            .id("slack-bookmark-folder-panel")
            .aria_label(format!("{title} folder"))
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .pt(px(20.0))
            .pb(px(24.0))
            .bg(rgb(panel_background))
            .child(
                div()
                    .id("slack-bookmark-folder-title")
                    .role(Role::Heading)
                    .aria_level(1)
                    .h(px(36.0))
                    .flex_none()
                    .mx(px(32.0))
                    .mb(px(12.0))
                    .pl(px(12.0))
                    .flex()
                    .items_center()
                    .text_size(px(22.0))
                    .line_height(px(30.0))
                    .font_weight(FontWeight::BLACK)
                    .text_color(rgb(palette.main_text))
                    .child(title),
            )
            .child(content)
            .into_any_element()
    }

    fn render_slack_bookmark_folder_list(
        &self,
        title: &gpui::SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = self.slack_bookmark_folder_rows.clone();
        let row_count = rows.len();
        let scroll_handle = self.slack_bookmark_folder_scroll_handle.clone();
        let view = cx.entity();
        div()
            .id("slack-bookmark-folder-list")
            .role(Role::List)
            .aria_label(format!("{title} folder items"))
            .flex_grow(1.0)
            .min_h(px(0.0))
            .mx(px(32.0))
            .mb(px(32.0))
            .overflow_hidden()
            .child(
                uniform_list(
                    "slack-bookmark-folder-rows",
                    row_count,
                    move |range, _window, cx| {
                        let rows = rows.clone();
                        view.update(cx, |this, cx| {
                            range
                                .map(|index| {
                                    let row = rows
                                        .get(index)
                                        .expect("Slack bookmark folder row index must exist");
                                    this.render_slack_bookmark_folder_row(row, cx)
                                })
                                .collect::<Vec<_>>()
                        })
                    },
                )
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .track_scroll(&scroll_handle)
                .size_full(),
            )
            .into_any_element()
    }

    fn render_slack_bookmark_folder_row(
        &self,
        row: &SlackBookmarkFolderRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let target_url = row.target_url.clone();
        let keyboard_target_url = target_url.clone();
        let icon = match row.kind {
            SlackBookmarkFolderRowKind::Link => SlackShellIcon::FormatLink,
            SlackBookmarkFolderRowKind::File => SlackShellIcon::Files,
        };
        let remote_icon = row
            .icon_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned());
        let bookmark_icon = self.render_slack_bookmark_folder_icon(
            remote_icon,
            icon,
            palette.main_secondary_text,
            cx,
        );
        div()
            .id(row.element_id.clone())
            .role(Role::ListItem)
            .aria_label(row.title.clone())
            .focusable()
            .tab_stop(true)
            .h(px(SLACK_BOOKMARK_FOLDER_ROW_HEIGHT))
            .flex_none()
            .cursor_pointer()
            .child(
                div()
                    .w_full()
                    .h(px(SLACK_BOOKMARK_FOLDER_ROW_HEIGHT))
                    .rounded(px(8.0))
                    .border_1()
                    .border_color(alpha(0x5e5d60, 0.13))
                    .bg(rgb(palette.attachment_bg))
                    .p(px(12.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(bookmark_icon)
                    .child(self.render_slack_bookmark_folder_copy(row, &palette)),
            )
            .focus_visible(|style| style.border_2().border_color(rgb(palette.link)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_slack_bookmark_folder_item(target_url.as_ref(), cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if slack_bookmark_folder_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_bookmark_folder_item(keyboard_target_url.as_ref(), cx);
                }
            }))
            .into_any_element()
    }

    fn render_slack_bookmark_folder_icon(
        &self,
        image: Option<Arc<Image>>,
        icon: SlackShellIcon,
        icon_color: u32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(image) = image {
            return div()
                .size(px(36.0))
                .flex_none()
                .p(px(4.0))
                .rounded(px(8.0))
                .overflow_hidden()
                .child(img(image).size_full())
                .into_any_element();
        }
        div()
            .size(px(36.0))
            .flex_none()
            .rounded(px(8.0))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(icon, icon_color, 20.0, cx))
            .into_any_element()
    }

    fn render_slack_bookmark_folder_copy(
        &self,
        row: &SlackBookmarkFolderRow,
        palette: &SlackPalette,
    ) -> Div {
        div()
            .h(px(38.0))
            .flex_grow(1.0)
            .min_w(px(0.0))
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .h(px(18.0))
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(15.0))
                    .line_height(px(18.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.attachment_text))
                    .child(row.title.clone()),
            )
            .child(
                div()
                    .h(px(16.0))
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .text_size(px(13.0))
                    .line_height(px(16.0))
                    .text_color(rgb(palette.attachment_muted_text))
                    .child(row.context.clone()),
            )
    }

    fn render_slack_bookmark_folder_loading(&self, title: &gpui::SharedString) -> AnyElement {
        self.render_slack_bookmark_folder_terminal(
            "slack-bookmark-folder-loading",
            format!("Loading {title}…"),
            format!("Loading {title} folder"),
        )
    }

    fn render_slack_bookmark_folder_empty(&self) -> AnyElement {
        self.render_slack_bookmark_folder_terminal(
            "slack-bookmark-folder-empty",
            "No links or files in this folder",
            "This folder has no links or files",
        )
    }

    fn render_slack_bookmark_folder_terminal(
        &self,
        id: &'static str,
        message: impl Into<gpui::SharedString>,
        accessibility_label: impl Into<gpui::SharedString>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id(id)
            .aria_label(accessibility_label.into())
            .flex_grow(1.0)
            .min_h(px(0.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(15.0))
            .text_color(rgb(palette.main_muted_text))
            .child(message.into())
            .into_any_element()
    }

    fn render_slack_bookmark_folder_error(
        &self,
        error: &gpui::SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-bookmark-folder-error")
            .aria_label(format!("Could not load folder: {error}"))
            .flex_grow(1.0)
            .min_h(px(0.0))
            .px(px(32.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .child(
                div()
                    .max_w(px(520.0))
                    .text_size(px(13.0))
                    .text_color(rgb(palette.main_muted_text))
                    .child(error.clone()),
            )
            .child(self.render_slack_bookmark_folder_retry(cx))
            .into_any_element()
    }

    fn render_slack_bookmark_folder_retry(&self, cx: &mut Context<Self>) -> Stateful<gpui::Div> {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-bookmark-folder-retry")
            .role(Role::Button)
            .aria_label("Retry loading folder")
            .focusable()
            .tab_stop(true)
            .h(px(32.0))
            .px(px(14.0))
            .rounded(px(6.0))
            .bg(rgb(0x1264a3))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x0b4c8c)))
            .focus_visible(move |style| style.border_1().border_color(rgb(palette.main_text)))
            .flex()
            .items_center()
            .text_size(px(13.0))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(0xffffff))
            .child("Retry")
            .on_click(cx.listener(|this, _, _, cx| {
                this.retry_slack_bookmark_folder(cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if slack_bookmark_folder_action_key(event) {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.retry_slack_bookmark_folder(cx);
                }
            }))
    }
}

fn slack_bookmark_folder_action_key(event: &KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
