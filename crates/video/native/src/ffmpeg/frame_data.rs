use std::ffi::c_int;

use rsmpeg::ffi;

use super::{FfmpegError, Nv12FrameData};

pub(super) fn nv12_frame_data_with_visible_size(
    frame: *const ffi::AVFrame,
    visible_width: i32,
    visible_height: i32,
) -> Result<Nv12FrameData, FfmpegError> {
    // SAFETY: `frame` is a live FFmpeg frame.
    if unsafe { (*frame).format } != ffi::AV_PIX_FMT_NV12 as c_int {
        return Err(FfmpegError::Caps);
    }
    let layout = nv12_frame_layout(frame, visible_width, visible_height)?;
    let mut data = Vec::with_capacity(layout.y_size + layout.uv_size);
    append_frame_plane_rows(
        &mut data,
        frame,
        FramePlaneRows {
            plane: 0,
            stride: layout.y_stride,
            row_width: layout.width_usize,
            rows: layout.height_usize,
        },
    )?;
    append_frame_plane_rows(
        &mut data,
        frame,
        FramePlaneRows {
            plane: 1,
            stride: layout.uv_stride,
            row_width: layout.uv_row_width,
            rows: layout.uv_height,
        },
    )?;
    Ok(Nv12FrameData {
        nv12_data: data,
        width: layout.width,
        height: layout.height,
        uv_row_width: u32::try_from(layout.uv_row_width).map_err(|_| FfmpegError::Caps)?,
    })
}

pub(super) fn yuv420p_frame_data(frame: *const ffi::AVFrame) -> Result<Nv12FrameData, FfmpegError> {
    // SAFETY: `frame` is a live FFmpeg frame.
    if unsafe { (*frame).format } != ffi::AV_PIX_FMT_YUV420P as c_int {
        return Err(FfmpegError::Caps);
    }
    let layout = yuv420p_frame_layout(frame)?;
    let capacity = layout
        .y_size
        .checked_add(layout.uv_size)
        .ok_or(FfmpegError::Caps)?;
    let mut data = Vec::with_capacity(capacity);
    append_frame_plane_rows(
        &mut data,
        frame,
        FramePlaneRows {
            plane: 0,
            stride: layout.y_stride,
            row_width: layout.width_usize,
            rows: layout.height_usize,
        },
    )?;
    append_yuv420p_chroma_rows(
        &mut data,
        frame,
        Yuv420pChromaRows {
            u_stride: layout.u_stride,
            v_stride: layout.v_stride,
            row_width: layout.chroma_width,
            rows: layout.chroma_height,
        },
    )?;
    Ok(Nv12FrameData {
        nv12_data: data,
        width: layout.width,
        height: layout.height,
        uv_row_width: u32::try_from(layout.surface_width).map_err(|_| FfmpegError::Caps)?,
    })
}

struct Nv12FrameLayout {
    width: u32,
    height: u32,
    width_usize: usize,
    height_usize: usize,
    uv_row_width: usize,
    uv_height: usize,
    y_stride: usize,
    uv_stride: usize,
    y_size: usize,
    uv_size: usize,
}

fn nv12_frame_layout(
    frame: *const ffi::AVFrame,
    visible_width: i32,
    visible_height: i32,
) -> Result<Nv12FrameLayout, FfmpegError> {
    // SAFETY: `frame` is a live FFmpeg frame.
    let frame_width = u32::try_from(unsafe { (*frame).width }).map_err(|_| FfmpegError::Caps)?;
    // SAFETY: `frame` is a live FFmpeg frame.
    let frame_height = u32::try_from(unsafe { (*frame).height }).map_err(|_| FfmpegError::Caps)?;
    let width = u32::try_from(visible_width).map_err(|_| FfmpegError::Caps)?;
    let height = u32::try_from(visible_height).map_err(|_| FfmpegError::Caps)?;
    if width == 0 || height == 0 || width > frame_width || height > frame_height {
        return Err(FfmpegError::Caps);
    }
    let width_usize = width as usize;
    let height_usize = height as usize;
    let uv_height = height_usize.div_ceil(2);
    let uv_row_width = width_usize + (width_usize % 2);
    let y_stride = frame_stride(frame, 0)?;
    let uv_stride = frame_stride(frame, 1)?;
    if y_stride < width_usize || uv_stride < uv_row_width {
        return Err(FfmpegError::Caps);
    }
    Ok(Nv12FrameLayout {
        width,
        height,
        width_usize,
        height_usize,
        uv_row_width,
        uv_height,
        y_stride,
        uv_stride,
        y_size: width_usize
            .checked_mul(height_usize)
            .ok_or(FfmpegError::Caps)?,
        uv_size: uv_row_width
            .checked_mul(uv_height)
            .ok_or(FfmpegError::Caps)?,
    })
}

struct Yuv420pFrameLayout {
    width: u32,
    height: u32,
    width_usize: usize,
    height_usize: usize,
    surface_width: usize,
    chroma_width: usize,
    chroma_height: usize,
    y_stride: usize,
    u_stride: usize,
    v_stride: usize,
    y_size: usize,
    uv_size: usize,
}

fn yuv420p_frame_layout(frame: *const ffi::AVFrame) -> Result<Yuv420pFrameLayout, FfmpegError> {
    // SAFETY: `frame` is a live FFmpeg frame.
    let width = u32::try_from(unsafe { (*frame).width }).map_err(|_| FfmpegError::Caps)?;
    // SAFETY: `frame` is a live FFmpeg frame.
    let height = u32::try_from(unsafe { (*frame).height }).map_err(|_| FfmpegError::Caps)?;
    if width == 0 || height == 0 {
        return Err(FfmpegError::Caps);
    }
    let width_usize = width as usize;
    let height_usize = height as usize;
    let surface_width = width_usize
        .checked_add(width_usize % 2)
        .ok_or(FfmpegError::Caps)?;
    let surface_height = height_usize
        .checked_add(height_usize % 2)
        .ok_or(FfmpegError::Caps)?;
    let chroma_width = surface_width / 2;
    let chroma_height = surface_height / 2;
    let y_stride = frame_stride(frame, 0)?;
    let u_stride = frame_stride(frame, 1)?;
    let v_stride = frame_stride(frame, 2)?;
    if y_stride < width_usize || u_stride < chroma_width || v_stride < chroma_width {
        return Err(FfmpegError::Caps);
    }
    Ok(Yuv420pFrameLayout {
        width,
        height,
        width_usize,
        height_usize,
        surface_width,
        chroma_width,
        chroma_height,
        y_stride,
        u_stride,
        v_stride,
        y_size: width_usize
            .checked_mul(height_usize)
            .ok_or(FfmpegError::Caps)?,
        uv_size: surface_width
            .checked_mul(chroma_height)
            .ok_or(FfmpegError::Caps)?,
    })
}

fn frame_stride(frame: *const ffi::AVFrame, plane: usize) -> Result<usize, FfmpegError> {
    // SAFETY: `frame` is a live FFmpeg frame.
    usize::try_from(unsafe { (*frame).linesize[plane] }).map_err(|_| FfmpegError::Caps)
}

struct FramePlaneRows {
    plane: usize,
    stride: usize,
    row_width: usize,
    rows: usize,
}

fn append_frame_plane_rows(
    output: &mut Vec<u8>,
    frame: *const ffi::AVFrame,
    rows: FramePlaneRows,
) -> Result<(), FfmpegError> {
    // SAFETY: `frame` is a live FFmpeg frame.
    let ptr = unsafe { (*frame).data[rows.plane] };
    if ptr.is_null() {
        return Err(FfmpegError::Caps);
    }
    let len = rows
        .stride
        .checked_mul(rows.rows)
        .ok_or(FfmpegError::Caps)?;
    // SAFETY: Plane pointer and length are bounded by checked FFmpeg stride and row count.
    let plane = unsafe { std::slice::from_raw_parts(ptr.cast_const(), len) };
    for row in 0..rows.rows {
        let start = row.checked_mul(rows.stride).ok_or(FfmpegError::Caps)?;
        let end = start.checked_add(rows.row_width).ok_or(FfmpegError::Caps)?;
        output.extend_from_slice(plane.get(start..end).ok_or(FfmpegError::Caps)?);
    }
    Ok(())
}

struct Yuv420pChromaRows {
    u_stride: usize,
    v_stride: usize,
    row_width: usize,
    rows: usize,
}

fn append_yuv420p_chroma_rows(
    output: &mut Vec<u8>,
    frame: *const ffi::AVFrame,
    rows: Yuv420pChromaRows,
) -> Result<(), FfmpegError> {
    // SAFETY: `frame` is a live FFmpeg frame.
    let u_ptr = unsafe { (*frame).data[1] };
    // SAFETY: `frame` is a live FFmpeg frame.
    let v_ptr = unsafe { (*frame).data[2] };
    if u_ptr.is_null() || v_ptr.is_null() {
        return Err(FfmpegError::Caps);
    }
    let u_len = rows
        .u_stride
        .checked_mul(rows.rows)
        .ok_or(FfmpegError::Caps)?;
    let v_len = rows
        .v_stride
        .checked_mul(rows.rows)
        .ok_or(FfmpegError::Caps)?;
    // SAFETY: Plane pointers and lengths are bounded by checked FFmpeg stride and row count.
    let u_plane = unsafe { std::slice::from_raw_parts(u_ptr.cast_const(), u_len) };
    // SAFETY: Plane pointers and lengths are bounded by checked FFmpeg stride and row count.
    let v_plane = unsafe { std::slice::from_raw_parts(v_ptr.cast_const(), v_len) };
    for row in 0..rows.rows {
        let u_start = row.checked_mul(rows.u_stride).ok_or(FfmpegError::Caps)?;
        let v_start = row.checked_mul(rows.v_stride).ok_or(FfmpegError::Caps)?;
        let u_end = u_start
            .checked_add(rows.row_width)
            .ok_or(FfmpegError::Caps)?;
        let v_end = v_start
            .checked_add(rows.row_width)
            .ok_or(FfmpegError::Caps)?;
        let u_row = u_plane.get(u_start..u_end).ok_or(FfmpegError::Caps)?;
        let v_row = v_plane.get(v_start..v_end).ok_or(FfmpegError::Caps)?;
        for column in 0..rows.row_width {
            output.push(u_row[column]);
            output.push(v_row[column]);
        }
    }
    Ok(())
}
