mod controls;
mod status;
mod transport_icons;

use gpui::{
    div, img, prelude::FluentBuilder, px, relative, rgb, AnyElement, Context, Div,
    InteractiveElement, IntoElement, MouseButton, MouseDownEvent, ParentElement, Render, Styled,
    Window,
};
use gpui_components::alpha;
use theme::ActiveTheme;

use crate::{player::VideoPlayer, session::VideoSession, source::video_clock_label};

use self::{
    controls::{transport_button_opacity, FractionBarConfig},
    transport_icons::{video_transport_icon_image, VideoTransportIcon},
};

/// Foreground of the controls drawn over the always-dark video stage.
const STAGE_FOREGROUND: u32 = 0xf4f7fb;
const STAGE_MUTED_FOREGROUND: u32 = 0x9aa7b8;

impl Render for VideoPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync(window, cx);
        let colors = cx.theme().colors();
        div()
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .overflow_hidden()
            .flex()
            .bg(colors.background)
            .text_color(colors.text)
            .child(self.render_stage(cx))
            .when(self.config.show_playlist, |this| {
                this.child(self.render_playlist(cx))
            })
    }
}

impl VideoPlayer {
    fn render_stage(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex_grow(1.0)
            .min_w(px(0.0))
            .min_h(px(0.0))
            .relative()
            .overflow_hidden()
            .bg(rgb(0x030507))
            .child(self.render_frame_or_placeholder())
            .when(self.config.show_controls, |this| {
                this.child(
                    div()
                        .absolute()
                        .left(px(16.0))
                        .right(px(16.0))
                        .bottom(px(16.0))
                        .child(self.render_transport(cx)),
                )
            })
    }

    fn render_transport(&self, cx: &mut Context<Self>) -> Div {
        let has_audio = self.session.as_ref().is_some_and(VideoSession::has_audio);
        div()
            .min_h(px(46.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(cx.theme().colors().border)
            .bg(alpha(0x05070b, 0.82))
            .px(px(12.0))
            .py(px(9.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .child(self.render_play_button(cx))
            .child(self.render_clock(self.current_time_label(), true))
            .child(self.render_progress_bar(cx).flex_grow(1.0))
            .child(self.render_clock(self.duration_label(), false))
            .when(has_audio, |this| {
                this.child(self.render_mute_button(cx))
                    .child(self.render_volume_bar(cx))
            })
    }

    fn render_play_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let icon = if self.is_playing() {
            VideoTransportIcon::Pause
        } else {
            VideoTransportIcon::Play
        };
        let enabled = self.session.is_some();
        self.render_transport_button(icon, enabled, true, cx)
            .when(enabled, |this| {
                this.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, _, cx| this.toggle_playback(cx)),
                )
            })
            .into_any_element()
    }

    fn render_mute_button(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = self.session.as_ref().is_some_and(VideoSession::muted);
        let icon = if muted {
            VideoTransportIcon::VolumeMuted
        } else {
            VideoTransportIcon::Volume
        };
        let enabled = self.session.is_some();
        self.render_transport_button(icon, enabled, false, cx)
            .when(enabled, |this| {
                this.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseDownEvent, _, cx| this.toggle_muted(cx)),
                )
            })
            .into_any_element()
    }

    fn render_transport_button(
        &self,
        icon: VideoTransportIcon,
        enabled: bool,
        emphasis: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let opacity = transport_button_opacity(enabled, emphasis);
        div()
            .w(px(40.0))
            .h(px(30.0))
            .flex_none()
            .rounded(px(6.0))
            .border_1()
            .border_color(alpha(STAGE_FOREGROUND, if emphasis { 0.42 } else { 0.18 }))
            .bg(alpha(STAGE_FOREGROUND, opacity))
            .flex()
            .items_center()
            .justify_center()
            .opacity(if enabled { 1.0 } else { 0.34 })
            .when(enabled, |this| this.cursor_pointer())
            .child(img(video_transport_icon_image(icon, STAGE_FOREGROUND, cx)).size(px(16.0)))
    }

    fn render_clock(&self, label: String, primary: bool) -> Div {
        div()
            .w(px(48.0))
            .flex_none()
            .whitespace_nowrap()
            .text_size(px(12.0))
            .line_height(px(16.0))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(alpha(STAGE_FOREGROUND, if primary { 0.9 } else { 0.62 }))
            .child(label)
    }

    fn render_progress_bar(&self, cx: &mut Context<Self>) -> Div {
        let enabled = self.session.is_some();
        let weak = cx.entity().downgrade();
        div()
            .relative()
            .min_w(px(0.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .on_children_prepainted(move |bounds, window, cx| {
                let Some(bounds) = bounds.first().copied() else {
                    return;
                };
                let should_refresh = weak
                    .update(cx, |this, cx| this.set_progress_bar_bounds(bounds, cx))
                    .unwrap_or(false);
                if should_refresh {
                    window.refresh();
                }
            })
            .when(enabled, |this| {
                this.cursor_pointer().on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                        this.seek_to_progress_position(event.position, cx);
                    }),
                )
            })
            .child(
                div()
                    .relative()
                    .w_full()
                    .h_full()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .w_full()
                            .h(px(4.0))
                            .rounded(px(999.0))
                            .bg(alpha(STAGE_FOREGROUND, 0.22)),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(0.0))
                            .top(px(10.0))
                            .h(px(4.0))
                            .w(relative(self.progress_fraction().clamp(0.0, 1.0)))
                            .rounded(px(999.0))
                            .bg(alpha(STAGE_FOREGROUND, 0.94)),
                    ),
            )
    }

    fn render_volume_bar(&self, cx: &mut Context<Self>) -> Div {
        self.render_fraction_bar(
            FractionBarConfig {
                fraction: self.volume_fraction(),
                segment_count: 20,
                track_color: alpha(STAGE_FOREGROUND, 0.18),
                fill_color: alpha(STAGE_FOREGROUND, 0.72),
                action: |this, value, cx| this.set_volume(value, cx),
            },
            cx,
        )
        .w(px(72.0))
        .flex_none()
    }

    fn render_fraction_bar(&self, config: FractionBarConfig, cx: &mut Context<Self>) -> Div {
        let enabled = self.session.is_some();
        div()
            .relative()
            .min_w(px(0.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .child(
                div()
                    .w_full()
                    .h(px(4.0))
                    .rounded(px(999.0))
                    .bg(config.track_color),
            )
            .child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(10.0))
                    .h(px(4.0))
                    .w(relative(config.fraction.clamp(0.0, 1.0)))
                    .rounded(px(999.0))
                    .bg(config.fill_color),
            )
            .child(
                div()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .right(px(0.0))
                    .bottom(px(0.0))
                    .flex()
                    .children((0..config.segment_count).map(|segment_index| {
                        let value = (segment_index + 1) as f32 / config.segment_count as f32;
                        let action = config.action;
                        div().flex_1().h_full().when(enabled, |this| {
                            this.cursor_pointer().on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _: &MouseDownEvent, _, cx| {
                                    action(this, value, cx);
                                }),
                            )
                        })
                    })),
            )
    }

    pub(super) fn current_time_label(&self) -> String {
        video_clock_label(
            self.session
                .as_ref()
                .map(VideoSession::current_time_seconds)
                .unwrap_or(0.0),
        )
    }

    pub(super) fn duration_label(&self) -> String {
        self.session
            .as_ref()
            .and_then(VideoSession::duration_seconds)
            .map(video_clock_label)
            .unwrap_or_else(|| "--:--".to_string())
    }

    pub(super) fn progress_fraction(&self) -> f32 {
        self.playback_progress_fraction()
    }

    pub(super) fn volume_fraction(&self) -> f32 {
        let Some(session) = self.session.as_ref() else {
            return 0.0;
        };
        if session.muted() {
            0.0
        } else {
            session.volume()
        }
    }

    pub(super) fn is_playing(&self) -> bool {
        self.session.as_ref().is_some_and(VideoSession::is_playing)
    }
}
