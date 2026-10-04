use std::sync::Arc;

use gpui::RenderImage;
use image::{Frame as ImageFrame, RgbaImage};

use super::state::{RenderedVideoFrame, VideoFrameData};

pub(super) fn render_image_from_nv12(frame: &VideoFrameData) -> Result<RenderedVideoFrame, String> {
    let width = frame.width as usize;
    let height = frame.height as usize;
    let uv_row_width = frame.uv_row_width as usize;
    if width == 0 || height == 0 || uv_row_width < width + (width % 2) {
        return Err(format!(
            "invalid NV12 layout: width={}, height={}, uv_row_width={}",
            frame.width, frame.height, frame.uv_row_width
        ));
    }

    let y_size = width
        .checked_mul(height)
        .ok_or_else(|| "NV12 luma plane size overflowed".to_string())?;
    let uv_size = uv_row_width
        .checked_mul(height.div_ceil(2))
        .ok_or_else(|| "NV12 chroma plane size overflowed".to_string())?;
    let frame_size = y_size
        .checked_add(uv_size)
        .ok_or_else(|| "NV12 frame size overflowed".to_string())?;
    let data = frame.nv12_data.get(..frame_size).ok_or_else(|| {
        format!(
            "NV12 frame has {} bytes but requires {frame_size}",
            frame.nv12_data.len()
        )
    })?;
    let (y_plane, uv_plane) = data.split_at(y_size);
    let mut bgra = Vec::with_capacity(
        y_size
            .checked_mul(4)
            .ok_or_else(|| "BGRA frame size overflowed".to_string())?,
    );
    for row in 0..height {
        for column in 0..width {
            let y = i32::from(y_plane[row * width + column]).saturating_sub(16);
            let uv = (row / 2) * uv_row_width + (column / 2) * 2;
            let u = i32::from(uv_plane[uv]) - 128;
            let v = i32::from(uv_plane[uv + 1]) - 128;
            bgra.extend_from_slice(&[
                yuv_component(298 * y + 516 * u),
                yuv_component(298 * y - 100 * u - 208 * v),
                yuv_component(298 * y + 409 * v),
                u8::MAX,
            ]);
        }
    }

    let image = RgbaImage::from_raw(frame.width, frame.height, bgra)
        .ok_or_else(|| "failed to construct the BGRA render image".to_string())?;
    Ok(RenderedVideoFrame {
        image: Arc::new(RenderImage::new(vec![ImageFrame::new(image)])),
        width: frame.width,
        height: frame.height,
    })
}

fn yuv_component(value: i32) -> u8 {
    ((value + 128) >> 8).clamp(0, 255) as u8
}
