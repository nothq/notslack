use gpui::Context;

use crate::player::VideoPlayer;

pub(super) type FractionBarAction = fn(&mut VideoPlayer, f32, &mut Context<VideoPlayer>);

pub(super) struct FractionBarConfig {
    pub(super) fraction: f32,
    pub(super) segment_count: usize,
    pub(super) track_color: gpui::Hsla,
    pub(super) fill_color: gpui::Hsla,
    pub(super) action: FractionBarAction,
}

pub(super) fn transport_button_opacity(enabled: bool, emphasis: bool) -> f32 {
    match (enabled, emphasis) {
        (true, true) => 0.18,
        (false, true) => 0.06,
        (true, false) => 0.08,
        (false, false) => 0.03,
    }
}
