use std::{cell::RefCell, collections::HashMap, sync::Arc};

use gpui::{Context, RenderImage};
use gpui_components::{render_svg_image, svg_from_body};

use crate::player::VideoPlayer;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum VideoTransportIcon {
    Play,
    Pause,
    Volume,
    VolumeMuted,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct VideoTransportIconCacheKey {
    icon: VideoTransportIcon,
    color: u32,
}

type VideoTransportIconCache = RefCell<HashMap<VideoTransportIconCacheKey, Arc<RenderImage>>>;

std::thread_local! {
    static VIDEO_TRANSPORT_ICON_CACHE: VideoTransportIconCache = RefCell::new(HashMap::new());
}

pub(super) fn video_transport_icon_image(
    icon: VideoTransportIcon,
    color: u32,
    cx: &mut Context<VideoPlayer>,
) -> Arc<RenderImage> {
    VIDEO_TRANSPORT_ICON_CACHE.with(|cache| {
        let key = VideoTransportIconCacheKey { icon, color };
        let mut cache = cache.borrow_mut();
        cache
            .entry(key)
            .or_insert_with(|| {
                render_svg_image(
                    svg_from_body("0 0 24 24", video_transport_icon_body(icon, color)),
                    cx,
                )
            })
            .clone()
    })
}

fn video_transport_icon_body(icon: VideoTransportIcon, color: u32) -> String {
    let color = format!("#{color:06x}");
    let body = match icon {
        VideoTransportIcon::Play => r#"<path d="M8 5V19L19 12L8 5Z" fill="{color}"/>"#,
        VideoTransportIcon::Pause => {
            r#"<path d="M8 5H10.75V19H8V5Z" fill="{color}"/><path d="M13.25 5H16V19H13.25V5Z" fill="{color}"/>"#
        }
        VideoTransportIcon::Volume => {
            r#"<path d="M4 10V14H8L13 19V5L8 10H4Z" fill="none" stroke="{color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/><path d="M16 9C16.8 9.8 17.25 10.85 17.25 12C17.25 13.15 16.8 14.2 16 15" fill="none" stroke="{color}" stroke-width="2" stroke-linecap="round"/><path d="M18.5 6.5C20 8 20.75 9.85 20.75 12C20.75 14.15 20 16 18.5 17.5" fill="none" stroke="{color}" stroke-width="2" stroke-linecap="round"/>"#
        }
        VideoTransportIcon::VolumeMuted => {
            r#"<path d="M4 10V14H8L13 19V5L8 10H4Z" fill="none" stroke="{color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/><path d="M17 9L21 13" fill="none" stroke="{color}" stroke-width="2" stroke-linecap="round"/><path d="M21 9L17 13" fill="none" stroke="{color}" stroke-width="2" stroke-linecap="round"/>"#
        }
    };
    body.replace("{color}", color.as_str())
}
