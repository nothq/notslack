//! GPUI SVG construction and rendering.

use std::{collections::HashMap, sync::Arc};

use gpui::{Context, Global, Image, ImageFormat, RenderImage};

const SVG_CACHE_BYTES: usize = 16 * 1024 * 1024;
const SVG_CACHE_ENTRIES: usize = 512;

#[derive(Default)]
struct SvgImageCache {
    entries: HashMap<Vec<u8>, CachedSvg>,
    bytes: usize,
    access: u64,
}

struct CachedSvg {
    image: Arc<RenderImage>,
    bytes: usize,
    last_used: u64,
}

impl Global for SvgImageCache {}

impl SvgImageCache {
    fn get(&mut self, source: &[u8]) -> Option<Arc<RenderImage>> {
        let entry = self.entries.get_mut(source)?;
        self.access += 1;
        entry.last_used = self.access;
        Some(entry.image.clone())
    }

    fn insert(&mut self, source: &[u8], image: Arc<RenderImage>) {
        let bytes = source.len() + image.as_bytes(0).map_or(0, <[u8]>::len);
        if bytes > SVG_CACHE_BYTES {
            return;
        }
        while self.bytes + bytes > SVG_CACHE_BYTES || self.entries.len() >= SVG_CACHE_ENTRIES {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
                .expect("a full SVG cache has entries");
            self.bytes -= self.entries.remove(&oldest).unwrap().bytes;
        }
        self.access += 1;
        self.bytes += bytes;
        self.entries.insert(
            source.to_vec(),
            CachedSvg {
                image,
                bytes,
                last_used: self.access,
            },
        );
    }
}

pub fn svg_from_body(view_box: &str, body: impl Into<String>) -> Arc<Image> {
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view_box}">{}</svg>"#,
            body.into()
        )
        .into_bytes(),
    ))
}

pub fn render_svg_image<T: 'static>(image: Arc<Image>, cx: &mut Context<T>) -> Arc<RenderImage> {
    if image.format() == ImageFormat::Svg {
        // Callers recreate SVG sources during render. Cache by contents so scrolling
        // reuses both the rasterized pixels and GPUI's texture identity.
        if !cx.has_global::<SvgImageCache>() {
            cx.set_global(SvgImageCache::default());
        }
        if let Some(rendered) = cx.global_mut::<SvgImageCache>().get(image.bytes()) {
            return rendered;
        }
        let rendered = cx
            .svg_renderer()
            .render_single_frame(image.bytes(), 1.0)
            .expect("failed to render svg icon");
        cx.global_mut::<SvgImageCache>()
            .insert(image.bytes(), rendered.clone());
        return rendered;
    }

    image
        .to_image_data(cx.svg_renderer())
        .expect("failed to render image")
}
