use gpui::SharedString;

use crate::ui::surface::{
    div, px, rgb, slack_icon, slack_palette, Context, Div, FontWeight, ParentElement,
    SlackMainComposerNotice, SlackShellIcon, Styled, SurfaceState,
};

use super::slack_dm_peer_local_time_label;

pub(super) enum SlackPreparedMainComposerNotice {
    Message(SharedString),
    NotificationsPaused {
        conversation_label: SharedString,
    },
    PeerLocalTime {
        conversation_label: SharedString,
        local_time: String,
    },
}

pub(super) fn prepare_slack_main_composer_notice(
    notice: &SlackMainComposerNotice,
) -> Option<SlackPreparedMainComposerNotice> {
    match notice {
        SlackMainComposerNotice::Message(message) => {
            Some(SlackPreparedMainComposerNotice::Message(message.clone()))
        }
        SlackMainComposerNotice::NotificationsPaused { conversation_label } => {
            Some(SlackPreparedMainComposerNotice::NotificationsPaused {
                conversation_label: conversation_label.clone(),
            })
        }
        SlackMainComposerNotice::PeerLocalTime {
            conversation_label,
            timezone,
        } => slack_dm_peer_local_time_label(*timezone, chrono::Utc::now()).map(|local_time| {
            SlackPreparedMainComposerNotice::PeerLocalTime {
                conversation_label: conversation_label.clone(),
                local_time,
            }
        }),
    }
}

impl SurfaceState {
    pub(super) fn render_slack_composer_notice(
        &self,
        notice: &SlackPreparedMainComposerNotice,
        cx: &mut Context<Self>,
    ) -> Div {
        let palette = slack_palette(self.appearance_mode);
        let (icon, icon_color) = match notice {
            SlackPreparedMainComposerNotice::NotificationsPaused { .. } => {
                (SlackShellIcon::NotificationsDndFilled, 0xABABAD)
            }
            SlackPreparedMainComposerNotice::PeerLocalTime { .. } => {
                (SlackShellIcon::Clock, palette.composer_icon)
            }
            SlackPreparedMainComposerNotice::Message(_) => {
                (SlackShellIcon::Bell, palette.composer_icon)
            }
        };
        let content = slack_composer_notice_content(notice);
        div()
            .h(px(42.0))
            .px(px(12.0))
            .rounded_t(px(8.0))
            .bg(rgb(0x25272a))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(slack_icon(icon, icon_color, 16.0, cx))
            .child(
                content
                    .text_size(px(13.0))
                    .text_color(rgb(palette.main_text)),
            )
    }
}

fn slack_composer_notice_content(notice: &SlackPreparedMainComposerNotice) -> Div {
    match notice {
        SlackPreparedMainComposerNotice::NotificationsPaused { conversation_label } => div()
            .flex()
            .items_center()
            .child(conversation_label.clone())
            .child(" has ")
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child("paused their notifications"),
            ),
        SlackPreparedMainComposerNotice::PeerLocalTime {
            conversation_label,
            local_time,
        } => div()
            .flex()
            .items_center()
            .child("It’s ")
            .child(
                div()
                    .font_weight(FontWeight::BOLD)
                    .child(local_time.clone()),
            )
            .child(" for ")
            .child(conversation_label.clone()),
        SlackPreparedMainComposerNotice::Message(message) => div().child(message.clone()),
    }
}
