use gpui::Role;

use super::{
    alpha, div, img, list, px, rgb, slack_icon, slack_palette, AnyElement, Context, Div,
    FluentBuilder, FontWeight, InteractiveElement, IntoElement, ListSizingBehavior, ParentElement,
    SlackShellIcon, StatefulInteractiveElement, Styled, SurfaceState,
};
use crate::ui::surface::{SlackConversationFilesMenu, SLACK_CONVERSATION_FILES_STATE_HEIGHT};
use crate::ui::SlackConversationFilesFilter;

const SLACK_CONVERSATION_MEDIA_TILE_HEIGHT: f32 = 140.0;

mod controls;
mod rows;

impl SurfaceState {
    pub(crate) fn render_slack_conversation_files_panel(
        &self,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        div()
            .id("slack-conversation-files")
            .relative()
            .flex_grow(1.0)
            .min_h(px(0.0))
            .pt(px(20.0))
            .flex()
            .flex_col()
            .bg(rgb(palette.topic_bg))
            .child(self.render_slack_conversation_files_search(cx))
            .child(self.render_slack_conversation_files_explorer(cx))
            .when(
                self.slack_conversation_files_menu == Some(SlackConversationFilesMenu::Sort),
                |this| this.child(self.render_slack_conversation_files_sort_menu(cx)),
            )
            .into_any_element()
    }

    fn render_slack_conversation_files_skeleton(&self) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_CONVERSATION_FILES_STATE_HEIGHT))
            .flex_none()
            .mx(px(32.0))
            .rounded(px(8.0))
            .overflow_hidden()
            .flex()
            .flex_col()
            .children((0..6).map(|index| {
                div()
                    .h(px(SLACK_CONVERSATION_FILES_STATE_HEIGHT / 6.0))
                    .flex_none()
                    .px(px(12.0))
                    .border_b_1()
                    .border_color(alpha(palette.main_text, 0.1))
                    .bg(rgb(palette.attachment_bg))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .size(px(36.0))
                            .rounded(px(8.0))
                            .bg(alpha(palette.main_text, 0.11)),
                    )
                    .child(
                        div()
                            .flex_grow(1.0)
                            .flex()
                            .flex_col()
                            .gap(px(7.0))
                            .child(
                                div()
                                    .w(px(180.0 + index as f32 * 17.0))
                                    .h(px(11.0))
                                    .rounded(px(4.0))
                                    .bg(alpha(palette.main_text, 0.11)),
                            )
                            .child(
                                div()
                                    .w(px(240.0))
                                    .h(px(9.0))
                                    .rounded(px(4.0))
                                    .bg(alpha(palette.main_text, 0.08)),
                            ),
                    )
            }))
    }

    fn render_slack_conversation_files_error(&self, message: &str, cx: &mut Context<Self>) -> Div {
        let palette = slack_palette(self.appearance_mode);
        div()
            .h(px(SLACK_CONVERSATION_FILES_STATE_HEIGHT))
            .flex_none()
            .mx(px(32.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(rgb(0x9b3c37))
            .bg(rgb(palette.attachment_bg))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.0))
            .text_size(px(14.0))
            .text_color(rgb(palette.main_text))
            .child(message.to_string())
            .child(
                div()
                    .id("slack-conversation-files-retry")
                    .role(Role::Button)
                    .aria_label("Retry loading files and links")
                    .focusable()
                    .tab_stop(true)
                    .h(px(30.0))
                    .px(px(14.0))
                    .rounded(px(5.0))
                    .bg(rgb(0x1264a3))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.retry_slack_conversation_files(cx);
                    }))
                    .on_key_down(cx.listener(|this, event, window, cx| {
                        if slack_conversation_files_action_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.retry_slack_conversation_files(cx);
                        }
                    }))
                    .flex()
                    .items_center()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child("Retry"),
            )
    }

    fn render_slack_conversation_files_empty(&self) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let message = match self.slack_conversation_files_filter {
            SlackConversationFilesFilter::All => "No files or links found for this channel.",
            SlackConversationFilesFilter::Files => "No files found for this channel.",
            SlackConversationFilesFilter::Media => "No media found for this channel.",
            SlackConversationFilesFilter::Links => "No links found for this channel.",
        };
        div()
            .h(px(SLACK_CONVERSATION_FILES_STATE_HEIGHT))
            .flex_none()
            .mx(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(15.0))
            .text_color(rgb(palette.main_secondary_text))
            .child(message)
    }
}

fn slack_conversation_files_action_key(event: &gpui::KeyDownEvent) -> bool {
    matches!(event.keystroke.key.as_str(), "enter" | "space")
        && !event.keystroke.modifiers.modified()
}
