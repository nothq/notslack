use crate::ui::surface::{
    div, px, rgb, slack_icon, AnyElement, Context, Div, FluentBuilder, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, SlackAttachmentRow,
    SlackAttachmentSelection, SlackMediaHostId, SlackMediaPlayback, SlackShellIcon,
    StatefulInteractiveElement, Styled, SurfaceState,
};
use gpui::Role;

struct SlackAudioActivationCard<'a> {
    attachment: &'a SlackAttachmentRow,
    host: SlackMediaHostId,
    status: &'a str,
    action_label: &'a str,
    enabled: bool,
    failed: bool,
}

impl SurfaceState {
    pub(crate) fn render_slack_inline_audio_attachment(
        &self,
        attachment_row: &SlackAttachmentRow,
        host: SlackMediaHostId,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let media = attachment_row
            .attachment
            .media
            .as_ref()
            .expect("inline Slack audio attachment must have typed media");
        debug_assert_eq!(media.kind(), crate::ui::SlackAttachmentMediaKind::Audio);
        let playback = self.slack_media_playback.as_ref().filter(|playback| {
            playback
                .target()
                .matches(attachment_row.attachment_id.as_ref(), media.file_id())
                && playback.host() == &host
        });
        if let Some(SlackMediaPlayback::Audio { player, .. }) = playback {
            return player.clone().into_any_element();
        }
        let (status, action_label, enabled, failed) =
            self.slack_audio_activation_state(attachment_row, playback);
        self.render_slack_audio_activation_card(
            SlackAudioActivationCard {
                attachment: attachment_row,
                host,
                status,
                action_label,
                enabled,
                failed,
            },
            cx,
        )
        .into_any_element()
    }

    fn slack_audio_activation_state<'a>(
        &self,
        attachment: &'a SlackAttachmentRow,
        playback: Option<&SlackMediaPlayback>,
    ) -> (&'a str, &'static str, bool, bool) {
        match playback {
            Some(SlackMediaPlayback::Loading { .. }) => {
                ("Preparing…", "Preparing audio", false, false)
            }
            Some(SlackMediaPlayback::Failed { .. }) => {
                ("Playback failed", "Retry audio", true, true)
            }
            _ if self
                .slack_workspace_api_capabilities
                .prepare_attachment_media =>
            {
                (
                    attachment
                        .recording_duration_label
                        .as_deref()
                        .unwrap_or("0:00"),
                    "Play audio",
                    true,
                    false,
                )
            }
            _ => ("Unavailable", "Audio unavailable", false, true),
        }
    }

    fn render_slack_audio_activation_card(
        &self,
        card: SlackAudioActivationCard<'_>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let action_label = format!("{} {}", card.action_label, card.attachment.title);
        div()
            .id(format!(
                "slack-audio-attachment-{}",
                card.attachment.attachment_id
            ))
            .role(Role::Group)
            .aria_label(action_label.clone())
            .w(px(440.0))
            .max_w_full()
            .h(px(78.0))
            .rounded(px(8.0))
            .px(px(8.0))
            .py(px(6.0))
            .flex()
            .flex_col()
            .justify_center()
            .gap(px(4.0))
            .child(slack_audio_activation_title(card.attachment))
            .child(
                div()
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(self.render_slack_audio_activation_button(&card, action_label, cx))
                    .child(slack_audio_activation_track())
                    .child(slack_audio_activation_status(card.status, card.failed)),
            )
    }

    fn render_slack_audio_activation_button(
        &self,
        card: &SlackAudioActivationCard<'_>,
        action_label: String,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selection = SlackAttachmentSelection::from_row(card.attachment);
        let keyboard_selection = selection.clone();
        let host = card.host.clone();
        let keyboard_host = card.host.clone();
        div()
            .id(format!(
                "slack-audio-activate-{}",
                card.attachment.attachment_id
            ))
            .role(Role::Button)
            .aria_label(action_label)
            .focusable()
            .tab_stop(true)
            .size(px(36.0))
            .flex_none()
            .rounded(px(18.0))
            .bg(rgb(if card.failed { 0x611f27 } else { 0x1264a3 }))
            .opacity(if card.enabled { 1.0 } else { 0.62 })
            .when(card.enabled, |this| {
                this.cursor_pointer()
                    .hover(|style| style.bg(rgb(if card.failed { 0x7a2832 } else { 0x0b4c8c })))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.stop_propagation();
                        this.activate_slack_attachment_media(&selection, host.clone(), cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                        if slack_audio_activation_key(event) {
                            window.prevent_default();
                            cx.stop_propagation();
                            this.activate_slack_attachment_media(
                                &keyboard_selection,
                                keyboard_host.clone(),
                                cx,
                            );
                        }
                    }))
            })
            .focus_visible(|style| style.border_2().border_color(rgb(0xffffff)))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(SlackShellIcon::Play, 0xffffff, 16.0, cx))
            .into_any_element()
    }
}

fn slack_audio_activation_title(attachment: &SlackAttachmentRow) -> Div {
    div()
        .h(px(18.0))
        .min_w(px(0.0))
        .overflow_hidden()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_size(px(15.0))
        .line_height(px(18.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xd1d2d3))
        .child(attachment.title.clone())
}

fn slack_audio_activation_track() -> Div {
    div()
        .min_w(px(0.0))
        .h(px(4.0))
        .flex_grow(1.0)
        .rounded(px(2.0))
        .bg(rgb(0x56585e))
}

fn slack_audio_activation_status(status: &str, failed: bool) -> Div {
    div()
        .w(px(92.0))
        .flex_none()
        .whitespace_nowrap()
        .text_size(px(13.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(if failed { 0xe01e5a } else { 0xd1d2d3 }))
        .child(status.to_string())
}

fn slack_audio_activation_key(event: &KeyDownEvent) -> bool {
    !event.keystroke.modifiers.modified()
        && matches!(event.keystroke.key.as_str(), "enter" | "space")
}
