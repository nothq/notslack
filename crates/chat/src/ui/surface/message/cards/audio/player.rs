use std::time::Duration;

use crate::ui::surface::{
    slack_icon, SlackAudioPlayer, SlackAudioPlayerEvent, SlackMediaTarget, SlackShellIcon,
};
use crate::ui::{
    div, px, rgb, Context, Div, FontWeight, InteractiveElement, KeyDownEvent, ParentElement,
    Render, StatefulInteractiveElement, Styled, Window,
};
use gpui::{EventEmitter, IntoElement, MouseButton, MouseDownEvent, Role};

const SLACK_AUDIO_REFRESH_INTERVAL: Duration = Duration::from_millis(100);
const SLACK_AUDIO_SCRUBBER_SEGMENT_COUNT: usize = 40;

impl SlackAudioPlayer {
    pub(crate) fn new(
        player: crate::ui::AudioPreviewPlayer,
        target: SlackMediaTarget,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            player,
            target,
            refresh_scheduled: false,
            failure_emitted: false,
        }
    }

    pub(crate) fn toggle(&mut self, cx: &mut Context<Self>) {
        self.player.toggle();
        self.schedule_refresh(cx);
        cx.notify();
    }

    pub(crate) fn play(&mut self, cx: &mut Context<Self>) {
        self.player.play();
        self.schedule_refresh(cx);
        cx.notify();
    }

    pub(crate) fn pause(&mut self, cx: &mut Context<Self>) {
        self.player.pause();
        cx.notify();
    }

    pub(crate) fn seek_fraction(&mut self, fraction: f64, cx: &mut Context<Self>) {
        self.player.seek_fraction(fraction);
        self.schedule_refresh(cx);
        cx.notify();
    }

    pub(crate) fn seek(&mut self, position: Duration, cx: &mut Context<Self>) {
        self.player.seek(position);
        self.schedule_refresh(cx);
        cx.notify();
    }

    pub(crate) fn toggle_muted(&mut self, cx: &mut Context<Self>) {
        self.player.toggle_muted();
        cx.notify();
    }

    pub(crate) fn set_muted(&mut self, muted: bool, cx: &mut Context<Self>) {
        self.player.set_muted(muted);
        cx.notify();
    }

    pub(crate) fn stop(&mut self, cx: &mut Context<Self>) {
        self.player.stop();
        cx.notify();
    }

    fn seek_relative(&mut self, delta: Duration, forwards: bool, cx: &mut Context<Self>) {
        let snapshot = self.player.snapshot();
        let position = if forwards {
            snapshot.position.saturating_add(delta)
        } else {
            snapshot.position.saturating_sub(delta)
        };
        self.player.seek(position.min(snapshot.duration));
        self.schedule_refresh(cx);
        cx.notify();
    }

    fn schedule_refresh(&mut self, cx: &mut Context<Self>) {
        if self.player.error_message().is_some() {
            if !self.failure_emitted {
                self.failure_emitted = true;
                cx.emit(SlackAudioPlayerEvent::Failed);
            }
            return;
        }
        if self.refresh_scheduled || self.player.state() != crate::ui::AudioPreviewState::Playing {
            return;
        }
        self.refresh_scheduled = true;
        crate::ui::spawn_timer_task_for_entity(
            (),
            SLACK_AUDIO_REFRESH_INTERVAL,
            cx,
            |this, (), cx| {
                this.refresh_scheduled = false;
                cx.notify();
                this.schedule_refresh(cx);
            },
        );
    }

    fn render_play_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.player.state();
        let icon = if state == crate::ui::AudioPreviewState::Playing {
            SlackShellIcon::Pause
        } else {
            SlackShellIcon::Play
        };
        let action = match state {
            crate::ui::AudioPreviewState::Idle | crate::ui::AudioPreviewState::Paused => "Play",
            crate::ui::AudioPreviewState::Playing => "Pause",
            crate::ui::AudioPreviewState::Finished => "Replay",
        };
        div()
            .id(format!(
                "slack-audio-playback-{}",
                self.target.attachment_id
            ))
            .role(Role::Button)
            .aria_label(format!("{action} {}", self.target.title))
            .focusable()
            .tab_stop(true)
            .size(px(36.0))
            .flex_none()
            .rounded(px(18.0))
            .bg(rgb(0x1264a3))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x0b4c8c)))
            .focus_visible(|style| style.border_2().border_color(rgb(0xffffff)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.toggle(cx);
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space")
                    && !event.keystroke.modifiers.modified()
                {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle(cx);
                }
            }))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(icon, 0xffffff, 16.0, cx))
    }

    fn render_scrubber(&self, progress_fraction: f64, cx: &mut Context<Self>) -> impl IntoElement {
        let segments = (0..SLACK_AUDIO_SCRUBBER_SEGMENT_COUNT)
            .map(|index| {
                let seek_fraction = (index + 1) as f64 / SLACK_AUDIO_SCRUBBER_SEGMENT_COUNT as f64;
                div().flex_1().h_full().cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        this.seek_fraction(seek_fraction, cx);
                    }),
                )
            })
            .collect::<Vec<_>>();
        div()
            .id(format!(
                "slack-audio-scrubber-{}",
                self.target.attachment_id
            ))
            .role(Role::Button)
            .aria_label(format!("Seek {}", self.target.title))
            .focusable()
            .tab_stop(true)
            .min_w(px(0.0))
            .h(px(18.0))
            .flex_grow(1.0)
            .relative()
            .items_center()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_slack_audio_scrubber_key(event, window, cx);
            }))
            .child(slack_audio_scrubber_track())
            .child(slack_audio_scrubber_progress(progress_fraction))
            .child(slack_audio_scrubber_thumb(progress_fraction))
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .right(px(0.0))
                    .top(px(0.0))
                    .bottom(px(0.0))
                    .flex()
                    .children(segments),
            )
    }

    fn handle_slack_audio_scrubber_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let forwards = match event.keystroke.key.as_str() {
            "left" => false,
            "right" => true,
            _ => return,
        };
        window.prevent_default();
        cx.stop_propagation();
        self.seek_relative(Duration::from_secs(5), forwards, cx);
    }

    fn render_mute_button(&self, muted: bool, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id(format!("slack-audio-mute-{}", self.target.attachment_id))
            .role(Role::Button)
            .aria_label(format!(
                "{} {}",
                if muted { "Unmute" } else { "Mute" },
                self.target.title
            ))
            .focusable()
            .tab_stop(true)
            .size(px(32.0))
            .flex_none()
            .rounded(px(16.0))
            .cursor_pointer()
            .hover(|style| style.bg(rgb(0x2a2d31)))
            .focus_visible(|style| style.bg(rgb(0x2a2d31)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.toggle_muted(cx);
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if matches!(event.keystroke.key.as_str(), "enter" | "space")
                    && !event.keystroke.modifiers.modified()
                {
                    window.prevent_default();
                    cx.stop_propagation();
                    this.toggle_muted(cx);
                }
            }))
            .flex()
            .items_center()
            .justify_center()
            .child(slack_icon(
                if muted {
                    SlackShellIcon::VolumeMuted
                } else {
                    SlackShellIcon::Volume
                },
                0xb9babd,
                17.0,
                cx,
            ))
    }
}

impl EventEmitter<SlackAudioPlayerEvent> for SlackAudioPlayer {}

impl Render for SlackAudioPlayer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = self.player.snapshot();
        let error = self.player.error_message();
        self.schedule_refresh(cx);
        div()
            .id(format!("slack-audio-player-{}", self.target.attachment_id))
            .role(Role::Group)
            .aria_label(format!("Audio message {}", self.target.title))
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
            .child(slack_audio_player_title(self.target.title.clone()))
            .child(
                div()
                    .h(px(36.0))
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(self.render_play_button(cx))
                    .child(self.render_scrubber(snapshot.progress_fraction(), cx))
                    .child(slack_audio_player_status(
                        snapshot.position,
                        snapshot.duration,
                        error.is_some(),
                    ))
                    .child(self.render_mute_button(snapshot.muted, cx)),
            )
    }
}

fn slack_audio_scrubber_track() -> Div {
    div()
        .absolute()
        .left(px(0.0))
        .right(px(0.0))
        .top(px(7.0))
        .h(px(4.0))
        .rounded(px(2.0))
        .bg(rgb(0x56585e))
}

fn slack_audio_scrubber_progress(progress_fraction: f64) -> Div {
    div()
        .absolute()
        .left(px(0.0))
        .top(px(7.0))
        .h(px(4.0))
        .w(gpui::relative(progress_fraction.clamp(0.0, 1.0) as f32))
        .rounded(px(2.0))
        .bg(rgb(0x1264a3))
}

fn slack_audio_scrubber_thumb(progress_fraction: f64) -> Div {
    div()
        .absolute()
        .left(gpui::relative(progress_fraction.clamp(0.0, 1.0) as f32))
        .ml(px(-6.0))
        .top(px(3.0))
        .size(px(12.0))
        .rounded(px(6.0))
        .bg(rgb(0x1264a3))
}

fn slack_audio_player_title(title: gpui::SharedString) -> Div {
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
        .child(title)
}

fn slack_audio_player_status(position: Duration, duration: Duration, failed: bool) -> Div {
    let label = if failed {
        "Failed".to_string()
    } else {
        format!(
            "{} / {}",
            slack_audio_time_label(position),
            slack_audio_time_label(duration)
        )
    };
    div()
        .w(px(72.0))
        .flex_none()
        .whitespace_nowrap()
        .text_size(px(13.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(if failed { 0xe01e5a } else { 0xd1d2d3 }))
        .child(label)
}

fn slack_audio_time_label(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3_600;
    let minutes = total_seconds % 3_600 / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
