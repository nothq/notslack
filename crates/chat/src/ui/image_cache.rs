use std::{
    collections::{HashMap, VecDeque},
    io::Cursor,
    ops::Deref,
    sync::Arc,
};

use gpui::{Context, Image, ImageFormat, RenderImage, SvgRenderer};
use image::{
    codecs::jpeg::JpegEncoder, GenericImageView, ImageFormat as RasterImageFormat, ImageReader,
    Limits,
};

use crate::ui::OnceLock;

mod remote_images;

pub(crate) use remote_images::{
    build_slack_conversation_remote_images, build_slack_message_remote_images,
    build_slack_remote_image_from_bytes, build_slack_remote_image_from_parts,
    build_slack_remote_images,
    build_slack_shell_remote_images, build_slack_sidebar_remote_images,
    slack_legacy_attachment_file_image_cache_key, slack_remote_image_cache_key,
};

const SLACK_ATTACHMENT_PREVIEW_MAX_CACHE_WIDTH: u32 = 1456;
const SLACK_ATTACHMENT_PREVIEW_MAX_CACHE_HEIGHT: u32 = 944;
const SLACK_REMOTE_IMAGE_CACHE_CAPACITY: usize = 256;
const SLACK_REMOTE_IMAGE_CACHE_MAX_ENCODED_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const SLACK_COMPOSER_LOCAL_PREVIEW_MAX_BYTES: usize = 0x1900000;
const SLACK_COMPOSER_LOCAL_PREVIEW_MAX_SIDE: u32 = 4096;
const SLACK_COMPOSER_LOCAL_PREVIEW_MAX_PIXELS: u64 = 16_777_216;
const SLACK_COMPOSER_LOCAL_PREVIEW_SIZE: u32 = 124;

pub(crate) struct IconAsset {
    image: Arc<Image>,
    rendered: OnceLock<Arc<RenderImage>>,
}

#[derive(Default)]
pub(crate) struct SlackRemoteImageCache {
    entries: HashMap<String, Arc<Image>>,
    insertion_order: VecDeque<String>,
    encoded_bytes: usize,
}

impl SlackRemoteImageCache {
    pub(crate) fn insert(&mut self, key: String, image: Arc<Image>) {
        if let Some(previous) = self.entries.remove(key.as_str()) {
            self.encoded_bytes = self.encoded_bytes.saturating_sub(previous.bytes().len());
            if let Some(index) = self.insertion_order.iter().position(|entry| entry == &key) {
                self.insertion_order.remove(index);
            }
        }
        self.encoded_bytes = self.encoded_bytes.saturating_add(image.bytes().len());
        self.insertion_order.push_back(key.clone());
        self.entries.insert(key, image);
        self.evict_excess();
    }

    pub(crate) fn extend(&mut self, images: impl IntoIterator<Item = (String, Arc<Image>)>) {
        for (key, image) in images {
            self.insert(key, image);
        }
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.insertion_order.clear();
        self.encoded_bytes = 0;
    }

    pub(crate) fn snapshot(&self) -> HashMap<String, Arc<Image>> {
        self.entries.clone()
    }

    fn evict_excess(&mut self) {
        while self.entries.len() > SLACK_REMOTE_IMAGE_CACHE_CAPACITY
            || self.encoded_bytes > SLACK_REMOTE_IMAGE_CACHE_MAX_ENCODED_BYTES
        {
            let key = self
                .insertion_order
                .pop_front()
                .expect("non-empty Slack remote image cache must have an oldest key");
            if let Some(image) = self.entries.remove(key.as_str()) {
                self.encoded_bytes = self.encoded_bytes.saturating_sub(image.bytes().len());
            }
        }
    }
}

impl From<HashMap<String, Arc<Image>>> for SlackRemoteImageCache {
    fn from(images: HashMap<String, Arc<Image>>) -> Self {
        let mut cache = Self::default();
        cache.extend(images);
        cache
    }
}

impl Deref for SlackRemoteImageCache {
    type Target = HashMap<String, Arc<Image>>;

    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl IconAsset {
    pub(crate) fn new(image: Arc<Image>) -> Self {
        Self {
            image,
            rendered: OnceLock::new(),
        }
    }

    pub(crate) fn render<T: 'static>(&self, cx: &mut Context<T>) -> Arc<RenderImage> {
        self.render_with(&cx.svg_renderer())
    }

    pub(crate) fn render_with(&self, renderer: &SvgRenderer) -> Arc<RenderImage> {
        self.rendered
            .get_or_init(|| render_image(self.image.clone(), renderer))
            .clone()
    }
}

fn render_image(image: Arc<Image>, renderer: &SvgRenderer) -> Arc<RenderImage> {
    if image.format() == ImageFormat::Svg {
        return renderer
            .render_single_frame(image.bytes(), 1.0)
            .expect("failed to render svg icon");
    }

    image
        .to_image_data(renderer.clone())
        .expect("failed to render image")
}

pub(crate) fn svg_from_body(view_box: &str, body: String) -> Arc<Image> {
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}">{body}</svg>"#)
            .into_bytes(),
    ))
}

pub(crate) fn svg_with_paths(view_box: &str, paths: &[&str], fill: u32) -> Arc<Image> {
    let body = paths
        .iter()
        .map(|path| format!("<path fill=\"#{fill:06x}\" d=\"{path}\"/>"))
        .collect::<String>();
    svg_from_body(view_box, body)
}

pub(crate) fn build_slack_composer_local_preview(
    bytes: &[u8],
    mimetype: &str,
) -> Option<Arc<Image>> {
    if bytes.len() >= SLACK_COMPOSER_LOCAL_PREVIEW_MAX_BYTES {
        return None;
    }
    let format = ImageFormat::from_mime_type(mimetype)?;
    let raster_format = raster_image_format(format)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(SLACK_COMPOSER_LOCAL_PREVIEW_MAX_SIDE);
    limits.max_image_height = Some(SLACK_COMPOSER_LOCAL_PREVIEW_MAX_SIDE);
    limits.max_alloc = Some(SLACK_COMPOSER_LOCAL_PREVIEW_MAX_PIXELS.saturating_mul(4));
    let mut reader = ImageReader::with_format(Cursor::new(bytes), raster_format);
    reader.limits(limits);
    let decoded = reader.decode().ok()?;
    let (width, height) = decoded.dimensions();
    if u64::from(width).saturating_mul(u64::from(height)) > SLACK_COMPOSER_LOCAL_PREVIEW_MAX_PIXELS
    {
        return None;
    }
    let preview = if width > SLACK_COMPOSER_LOCAL_PREVIEW_SIZE
        || height > SLACK_COMPOSER_LOCAL_PREVIEW_SIZE
    {
        decoded.resize(
            SLACK_COMPOSER_LOCAL_PREVIEW_SIZE,
            SLACK_COMPOSER_LOCAL_PREVIEW_SIZE,
            image::imageops::FilterType::Triangle,
        )
    } else {
        decoded
    };
    let mut encoded = Vec::new();
    let image = match format {
        ImageFormat::Jpeg => {
            JpegEncoder::new_with_quality(&mut encoded, 80)
                .encode_image(&preview)
                .ok()?;
            Image::from_bytes(ImageFormat::Jpeg, encoded)
        }
        _ => {
            preview
                .write_to(&mut Cursor::new(&mut encoded), RasterImageFormat::Png)
                .ok()?;
            Image::from_bytes(ImageFormat::Png, encoded)
        }
    };
    Some(Arc::new(image))
}

pub(crate) fn slack_composer_local_preview_supported(mimetype: &str) -> bool {
    ImageFormat::from_mime_type(mimetype)
        .and_then(raster_image_format)
        .is_some()
}

pub(super) fn downscale_slack_attachment_preview_image(
    format: ImageFormat,
    bytes: Vec<u8>,
) -> Image {
    let Some(raster_format) = raster_image_format(format) else {
        return Image::from_bytes(format, bytes);
    };
    let Ok(decoded) = image::load_from_memory_with_format(&bytes, raster_format) else {
        return Image::from_bytes(format, bytes);
    };
    let (width, height) = decoded.dimensions();
    if width <= SLACK_ATTACHMENT_PREVIEW_MAX_CACHE_WIDTH
        && height <= SLACK_ATTACHMENT_PREVIEW_MAX_CACHE_HEIGHT
    {
        return Image::from_bytes(format, bytes);
    }

    let resized = decoded.resize(
        SLACK_ATTACHMENT_PREVIEW_MAX_CACHE_WIDTH,
        SLACK_ATTACHMENT_PREVIEW_MAX_CACHE_HEIGHT,
        image::imageops::FilterType::Triangle,
    );
    let mut encoded = Vec::new();
    let encoded_image = match format {
        ImageFormat::Jpeg => JpegEncoder::new_with_quality(&mut encoded, 80)
            .encode_image(&resized)
            .ok()
            .map(|_| Image::from_bytes(ImageFormat::Jpeg, encoded)),
        _ => resized
            .write_to(&mut Cursor::new(&mut encoded), RasterImageFormat::Png)
            .ok()
            .map(|_| Image::from_bytes(ImageFormat::Png, encoded)),
    };

    encoded_image.unwrap_or_else(|| Image::from_bytes(format, bytes))
}

fn raster_image_format(format: ImageFormat) -> Option<RasterImageFormat> {
    match format {
        ImageFormat::Png => Some(RasterImageFormat::Png),
        ImageFormat::Jpeg => Some(RasterImageFormat::Jpeg),
        ImageFormat::Webp => Some(RasterImageFormat::WebP),
        ImageFormat::Gif => Some(RasterImageFormat::Gif),
        ImageFormat::Bmp => Some(RasterImageFormat::Bmp),
        ImageFormat::Tiff => Some(RasterImageFormat::Tiff),
        ImageFormat::Ico => Some(RasterImageFormat::Ico),
        ImageFormat::Pnm => Some(RasterImageFormat::Pnm),
        ImageFormat::Svg => None,
    }
}
