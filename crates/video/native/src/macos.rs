use core_foundation::{
    base::{CFType, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFMutableDictionary},
    number::CFNumber,
    string::CFString,
};
use core_video::{
    pixel_buffer::{
        kCVPixelFormatType_420YpCbCr8BiPlanarFullRange, CVPixelBuffer, CVPixelBufferKeys,
    },
    pixel_buffer_pool::{CVPixelBufferPool, CVPixelBufferPoolKeys},
    r#return::kCVReturnSuccess,
};

const MIN_PIXEL_BUFFER_POOL_SURFACES: i32 = 8;

pub fn nv12_frame_to_pixel_buffer(
    nv12_data: &[u8],
    width: u32,
    height: u32,
    uv_row_width: u32,
) -> Option<Nv12PixelBuffer> {
    let layout = Nv12Layout::from_frame(nv12_data, width, height, uv_row_width)?;
    let attrs = pixel_buffer_attributes();
    let pixel_buffer = CVPixelBuffer::new(
        kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
        layout.surface_width,
        layout.surface_height,
        Some(&attrs),
    )
    .ok()?;
    let pixel_layout = PixelBufferLayout::from_pixel_buffer(&pixel_buffer, layout)?;

    if pixel_buffer.lock_base_address(0) != kCVReturnSuccess {
        return None;
    }
    // SAFETY: The CVPixelBuffer is locked, its plane layout was validated against
    // the packed NV12 source frame, and each copy is bounded by the checked row sizes.
    let copied =
        unsafe { copy_nv12_to_pixel_buffer(nv12_data, layout, &pixel_buffer, pixel_layout) };
    let _ = pixel_buffer.unlock_base_address(0);

    if !copied {
        return None;
    }

    Some(Nv12PixelBuffer {
        pixel_buffer,
        surface_width: u32::try_from(layout.surface_width).ok()?,
        surface_height: u32::try_from(layout.surface_height).ok()?,
    })
}

pub struct Nv12PixelBufferPool {
    pool: CVPixelBufferPool,
    layout: Nv12Layout,
}

impl Nv12PixelBufferPool {
    pub fn new(width: u32, height: u32, uv_row_width: u32) -> Option<Self> {
        let layout = Nv12Layout::from_dimensions(width, height, uv_row_width)?;
        let pool_attributes = pixel_buffer_pool_attributes();
        let pixel_buffer_attributes = pixel_buffer_pool_pixel_attributes(layout);
        let pool =
            CVPixelBufferPool::new(Some(&pool_attributes), Some(&pixel_buffer_attributes)).ok()?;
        Some(Self { pool, layout })
    }

    pub const fn supports_frame(&self, width: u32, height: u32, uv_row_width: u32) -> bool {
        self.layout.width == width as usize
            && self.layout.height == height as usize
            && self.layout.uv_row_width == uv_row_width as usize
    }

    pub fn copy_frame(&self, nv12_data: &[u8]) -> Option<Nv12PixelBuffer> {
        if nv12_data.len() < self.layout.byte_len {
            return None;
        }
        let pixel_buffer = self.pool.create_pixel_buffer().ok()?;
        let pixel_layout = PixelBufferLayout::from_pixel_buffer(&pixel_buffer, self.layout)?;

        if pixel_buffer.lock_base_address(0) != kCVReturnSuccess {
            return None;
        }
        // SAFETY: The CVPixelBuffer came from a pool created for this layout,
        // the source frame length was checked, and copies are bounded by the
        // validated plane strides.
        let copied = unsafe {
            copy_nv12_to_pixel_buffer(nv12_data, self.layout, &pixel_buffer, pixel_layout)
        };
        let _ = pixel_buffer.unlock_base_address(0);

        if !copied {
            return None;
        }

        Some(Nv12PixelBuffer {
            pixel_buffer,
            surface_width: u32::try_from(self.layout.surface_width).ok()?,
            surface_height: u32::try_from(self.layout.surface_height).ok()?,
        })
    }
}

impl std::fmt::Debug for Nv12PixelBufferPool {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Nv12PixelBufferPool")
            .field("width", &self.layout.width)
            .field("height", &self.layout.height)
            .field("surface_width", &self.layout.surface_width)
            .field("surface_height", &self.layout.surface_height)
            .field("uv_row_width", &self.layout.uv_row_width)
            .finish()
    }
}

#[derive(Clone)]
pub struct Nv12PixelBuffer {
    pixel_buffer: CVPixelBuffer,
    surface_width: u32,
    surface_height: u32,
}

impl Nv12PixelBuffer {
    pub const fn surface_width(&self) -> u32 {
        self.surface_width
    }

    pub const fn surface_height(&self) -> u32 {
        self.surface_height
    }

    pub fn clone_pixel_buffer(&self) -> CVPixelBuffer {
        self.pixel_buffer.clone()
    }

    pub fn into_pixel_buffer(self) -> CVPixelBuffer {
        self.pixel_buffer
    }
}

impl std::fmt::Debug for Nv12PixelBuffer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Nv12PixelBuffer")
            .field("surface_width", &self.surface_width)
            .field("surface_height", &self.surface_height)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy)]
struct Nv12Layout {
    width: usize,
    height: usize,
    surface_width: usize,
    surface_height: usize,
    y_size: usize,
    uv_height: usize,
    uv_row_width: usize,
    byte_len: usize,
}

impl Nv12Layout {
    fn from_frame(nv12_data: &[u8], width: u32, height: u32, uv_row_width: u32) -> Option<Self> {
        let layout = Self::from_dimensions(width, height, uv_row_width)?;
        (nv12_data.len() >= layout.byte_len).then_some(layout)
    }

    fn from_dimensions(width: u32, height: u32, uv_row_width: u32) -> Option<Self> {
        let width = width as usize;
        let height = height as usize;
        if width == 0 || height == 0 {
            return None;
        }
        let surface_width = width + (width % 2);
        let surface_height = height + (height % 2);
        let uv_row_width = uv_row_width as usize;
        if uv_row_width < surface_width || !uv_row_width.is_multiple_of(2) {
            return None;
        }
        let y_size = width.checked_mul(height)?;
        let uv_height = surface_height / 2;
        let uv_size = uv_row_width.checked_mul(uv_height)?;
        let byte_len = y_size.checked_add(uv_size)?;
        Some(Self {
            width,
            height,
            surface_width,
            surface_height,
            y_size,
            uv_height,
            uv_row_width,
            byte_len,
        })
    }
}

#[derive(Clone, Copy)]
struct PixelBufferLayout {
    y_stride: usize,
    uv_stride: usize,
}

impl PixelBufferLayout {
    fn from_pixel_buffer(pixel_buffer: &CVPixelBuffer, frame: Nv12Layout) -> Option<Self> {
        if pixel_buffer.get_pixel_format() != kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
            || !pixel_buffer.is_planar()
            || pixel_buffer.get_plane_count() != 2
            || pixel_buffer.get_width() != frame.surface_width
            || pixel_buffer.get_height() != frame.surface_height
            || pixel_buffer.get_width_of_plane(0) != frame.surface_width
            || pixel_buffer.get_height_of_plane(0) != frame.surface_height
            || pixel_buffer.get_width_of_plane(1) * 2 < frame.surface_width
            || pixel_buffer.get_height_of_plane(1) < frame.uv_height
        {
            return None;
        }

        let y_stride = pixel_buffer.get_bytes_per_row_of_plane(0);
        let uv_stride = pixel_buffer.get_bytes_per_row_of_plane(1);
        (y_stride >= frame.surface_width && uv_stride >= frame.uv_row_width).then_some(Self {
            y_stride,
            uv_stride,
        })
    }
}

fn pixel_buffer_attributes() -> CFDictionary<CFString, CFType> {
    let mut attrs: CFMutableDictionary<CFString, CFType> = CFMutableDictionary::new();
    attrs.add(
        &CVPixelBufferKeys::MetalCompatibility.into(),
        &CFBoolean::true_value().as_CFType(),
    );
    let empty_iosurface: CFDictionary<CFString, CFType> = CFDictionary::from_CFType_pairs(&[]);
    attrs.add(
        &CVPixelBufferKeys::IOSurfaceProperties.into(),
        &empty_iosurface.as_CFType(),
    );
    attrs.to_immutable()
}

fn pixel_buffer_pool_attributes() -> CFDictionary<CFString, CFType> {
    let minimum_count_key = CFString::from(CVPixelBufferPoolKeys::MinimumBufferCount);
    let minimum_count = CFNumber::from(MIN_PIXEL_BUFFER_POOL_SURFACES);
    CFDictionary::from_CFType_pairs(&[(minimum_count_key, minimum_count.into_CFType())])
}

fn pixel_buffer_pool_pixel_attributes(layout: Nv12Layout) -> CFDictionary<CFString, CFType> {
    let width_key = CFString::from(CVPixelBufferKeys::Width);
    let height_key = CFString::from(CVPixelBufferKeys::Height);
    let pixel_format_key = CFString::from(CVPixelBufferKeys::PixelFormatType);
    let metal_key = CFString::from(CVPixelBufferKeys::MetalCompatibility);
    let iosurface_key = CFString::from(CVPixelBufferKeys::IOSurfaceProperties);

    let width = CFNumber::from(layout.surface_width as i32);
    let height = CFNumber::from(layout.surface_height as i32);
    let pixel_format = CFNumber::from(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange as i64);
    let metal_compatible = CFBoolean::true_value();
    let iosurface_properties = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[]);

    CFDictionary::from_CFType_pairs(&[
        (width_key, width.into_CFType()),
        (height_key, height.into_CFType()),
        (pixel_format_key, pixel_format.into_CFType()),
        (metal_key, metal_compatible.into_CFType()),
        (iosurface_key, iosurface_properties.into_CFType()),
    ])
}

unsafe fn copy_nv12_to_pixel_buffer(
    nv12_data: &[u8],
    frame_layout: Nv12Layout,
    pixel_buffer: &CVPixelBuffer,
    pixel_layout: PixelBufferLayout,
) -> bool {
    // SAFETY: CoreVideo exposes plane base addresses only while the pixel buffer is locked.
    let y_dst = unsafe { pixel_buffer.get_base_address_of_plane(0) as *mut u8 };
    // SAFETY: CoreVideo exposes plane base addresses only while the pixel buffer is locked.
    let uv_dst = unsafe { pixel_buffer.get_base_address_of_plane(1) as *mut u8 };
    if y_dst.is_null() || uv_dst.is_null() || nv12_data.len() < frame_layout.byte_len {
        return false;
    }

    let src = nv12_data.as_ptr();
    for row in 0..frame_layout.height {
        let src_off = row * frame_layout.width;
        let dst_off = row * pixel_layout.y_stride;
        // SAFETY: Source length and destination stride were validated before copying rows.
        unsafe {
            std::ptr::copy_nonoverlapping(src.add(src_off), y_dst.add(dst_off), frame_layout.width);
            if frame_layout.surface_width > frame_layout.width {
                let pad_value = *src.add(src_off + frame_layout.width - 1);
                for column in frame_layout.width..frame_layout.surface_width {
                    *y_dst.add(dst_off + column) = pad_value;
                }
            }
        }
    }
    for row in frame_layout.height..frame_layout.surface_height {
        let src_off = (frame_layout.height - 1) * frame_layout.width;
        let dst_off = row * pixel_layout.y_stride;
        // SAFETY: Source length and destination stride were validated before copying rows.
        unsafe {
            std::ptr::copy_nonoverlapping(src.add(src_off), y_dst.add(dst_off), frame_layout.width);
            if frame_layout.surface_width > frame_layout.width {
                let pad_value = *src.add(src_off + frame_layout.width - 1);
                for column in frame_layout.width..frame_layout.surface_width {
                    *y_dst.add(dst_off + column) = pad_value;
                }
            }
        }
    }
    for row in 0..frame_layout.uv_height {
        let src_off = frame_layout.y_size + row * frame_layout.uv_row_width;
        let dst_off = row * pixel_layout.uv_stride;
        // SAFETY: Source length and destination stride were validated before copying rows.
        unsafe {
            std::ptr::copy_nonoverlapping(
                src.add(src_off),
                uv_dst.add(dst_off),
                frame_layout.uv_row_width,
            );
        }
    }
    true
}
