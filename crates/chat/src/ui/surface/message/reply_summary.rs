use super::rows::{bind_slack_profile_click, slack_message_meta};
use crate::ui::surface::{
    alpha, div, img, px, rgb, slack_base_icon_radius, slack_palette, AnyElement, Context,
    FluentBuilder, FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement,
    SlackMessageRow, SlackReplyParticipantRow, SlackReplySummaryRow, StatefulInteractiveElement,
    Styled, SurfaceState,
};
use gpui::Role;

impl SurfaceState {
    pub(super) fn render_slack_message_reply_summary(
        &self,
        row: &SlackMessageRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(reply_summary) = row.reply_summary.as_ref() else {
            return div().into_any_element();
        };
        let has_reply_participants = !row.reply_participants.is_empty();
        let description = self.render_slack_reply_summary_description(row, reply_summary, cx);
        div()
            .w_full()
            .max_w(px(600.0))
            .h(px(34.0))
            .flex_none()
            .ml(px(-5.0))
            .mb(px(4.0))
            .border_1()
            .border_color(alpha(0x000000, 0.0))
            .p(px(4.0))
            .flex()
            .items_center()
            .when(has_reply_participants, |this| {
                this.children(
                    row.reply_participants
                        .iter()
                        .map(|participant| self.render_slack_reply_participant(participant, cx)),
                )
            })
            .when(
                !has_reply_participants && row.latest_reply_avatar_text.is_some(),
                |this| {
                    let avatar_text = row
                        .latest_reply_avatar_text
                        .clone()
                        .expect("checked latest reply avatar text");
                    this.child(self.render_slack_reply_summary_avatar(row, avatar_text, cx))
                },
            )
            .child(description)
            .into_any_element()
    }

    fn render_slack_reply_summary_description(
        &self,
        row: &SlackMessageRow,
        summary: &SlackReplySummaryRow,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let palette = slack_palette(self.appearance_mode);
        let description = div()
            .h_full()
            .min_w(px(0.0))
            .flex()
            .items_center()
            .ml(px(3.0))
            .text_size(px(13.0))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(palette.link))
            .child(summary.count_label.clone())
            .child(
                slack_message_meta(summary.latest_reply_label.clone(), self.appearance_mode)
                    .ml(px(8.0)),
            );
        if !self.slack_workspace_api_capabilities.load_thread && row.replies.is_empty() {
            return description.into_any_element();
        }
        let message_id = row.id.clone();
        let keyboard_message_id = message_id.clone();
        description
            .id(summary.element_id.clone())
            .role(Role::Button)
            .aria_label(summary.accessibility_label.clone())
            .focusable()
            .tab_stop(true)
            .cursor_pointer()
            .focus_visible(|style| style.bg(alpha(palette.send_disabled_border, 0.10)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.open_slack_thread_panel(&message_id, cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    cx.stop_propagation();
                    this.open_slack_thread_panel(&keyboard_message_id, cx);
                }
            }))
            .into_any_element()
    }

    fn render_slack_reply_participant(
        &self,
        participant: &SlackReplyParticipantRow,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let image = participant
            .avatar_image_url
            .as_ref()
            .and_then(|url| self.slack_remote_images.get(url.as_ref()).cloned());
        let has_image = image.is_some();
        let clicked_user_id = participant.user_id.clone();
        let keyboard_user_id = participant.user_id.clone();
        div()
            .id(participant.profile_element_id.clone())
            .role(Role::Button)
            .aria_label(participant.profile_accessibility_label.clone())
            .focusable()
            .tab_stop(true)
            .size(px(24.0))
            .flex_none()
            .mr(px(4.0))
            .rounded(slack_base_icon_radius(24.0))
            .overflow_hidden()
            .bg(rgb(participant.avatar_fill))
            .cursor_pointer()
            .focus_visible(|style| style.border_1().border_color(rgb(0x1d9bd1)))
            .when(!has_image, |this| {
                this.flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(10.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child(participant.avatar_text.clone())
            })
            .when_some(image, |this, image| {
                this.child(
                    img(image)
                        .size(px(24.0))
                        .rounded(slack_base_icon_radius(24.0)),
                )
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.open_slack_profile(clicked_user_id.as_ref(), cx);
            }))
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.open_slack_profile(keyboard_user_id.as_ref(), cx);
                }
            }))
    }

    fn render_slack_reply_summary_avatar(
        &self,
        row: &SlackMessageRow,
        avatar_text: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let avatar = if let Some(image) = row
            .latest_reply_avatar_image_url
            .as_deref()
            .and_then(|url| self.slack_remote_images.get(url).cloned())
        {
            div()
                .size(px(24.0))
                .rounded(slack_base_icon_radius(24.0))
                .overflow_hidden()
                .child(
                    img(image)
                        .w_full()
                        .h_full()
                        .rounded(slack_base_icon_radius(24.0)),
                )
        } else {
            div()
                .size(px(24.0))
                .rounded(slack_base_icon_radius(24.0))
                .bg(rgb(row.latest_reply_avatar_fill))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(10.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(0xffffff))
                .child(avatar_text)
        };
        bind_slack_profile_click(
            avatar.mr(px(4.0)),
            row.latest_reply_user_id.clone(),
            format!("slack-message-latest-reply-avatar-{}", row.id),
            format!(
                "Open profile for {}",
                row.latest_reply_author
                    .as_deref()
                    .unwrap_or("latest reply author")
            ),
            cx,
        )
    }
}
