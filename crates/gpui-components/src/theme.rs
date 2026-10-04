use app_model::SurfaceColorSpec;
use gpui::{rgb, Hsla};

pub fn rgba(color: SurfaceColorSpec) -> Hsla {
    alpha(color.hex, color.opacity)
}

pub fn alpha(hex: u32, opacity: f32) -> Hsla {
    Hsla::from(rgb(hex)).opacity(opacity)
}
