use super::{state::VideoFrameData, video::Video};
use gpui::{
    Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Window,
};

#[cfg(target_os = "linux")]
use std::sync::Arc;

use crate::source::VideoFrameFit;

/// A video element that implements Element trait similar to GPUI's img element.
pub struct VideoElement {
    video: Video,
    display_width: Option<gpui::Pixels>,
    display_height: Option<gpui::Pixels>,
    element_id: Option<ElementId>,
    fill: bool,
    frame_fit: VideoFrameFit,
}

#[cfg(target_os = "linux")]
#[derive(Default)]
struct LinuxRenderImageState(Option<Arc<gpui::RenderImage>>);

impl VideoElement {
    pub fn new(video: Video) -> Self {
        Self {
            video,
            display_width: None,
            display_height: None,
            element_id: None,
            fill: false,
            frame_fit: VideoFrameFit::default(),
        }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.element_id = Some(id.into());
        self
    }

    pub fn fill(mut self) -> Self {
        self.fill = true;
        self
    }

    pub fn frame_fit(mut self, frame_fit: VideoFrameFit) -> Self {
        self.frame_fit = frame_fit;
        self
    }

    pub fn size(mut self, width: gpui::Pixels, height: gpui::Pixels) -> Self {
        self.fill = false;
        self.display_width = Some(width);
        self.display_height = Some(height);
        self
    }

    /// Set only width; height is inferred via aspect ratio.
    pub fn width(mut self, width: gpui::Pixels) -> Self {
        self.fill = false;
        self.display_width = Some(width);
        self.display_height = None;
        self
    }

    /// Set only height; width is inferred via aspect ratio.
    pub fn height(mut self, height: gpui::Pixels) -> Self {
        self.fill = false;
        self.display_height = Some(height);
        self.display_width = None;
        self
    }

    /// Configure how many frames to buffer inside the underlying `Video`.
    /// 0 disables buffering and behaves like immediate rendering.
    pub fn buffer_capacity(self, capacity: usize) -> Self {
        self.video.set_frame_buffer_capacity(capacity);
        self
    }

    /// Get the current display dimensions, falling back to video's effective display size.
    fn get_display_size(&self) -> (gpui::Pixels, gpui::Pixels) {
        match (self.display_width, self.display_height) {
            (Some(w), Some(h)) => (w, h),
            _ => {
                let (w, h) = self.video.display_size();
                (gpui::px(w as f32), gpui::px(h as f32))
            }
        }
    }

    /// Compute fitted destination bounds inside the given container `bounds`.
    fn fitted_bounds(
        &self,
        bounds: gpui::Bounds<gpui::Pixels>,
        frame_width: u32,
        frame_height: u32,
    ) -> gpui::Bounds<gpui::Pixels> {
        let container_w: f32 = bounds.size.width.into();
        let container_h: f32 = bounds.size.height.into();
        let frame_w = frame_width as f32;
        let frame_h = frame_height as f32;

        let scale = if frame_w > 0.0 && frame_h > 0.0 {
            let width_scale = container_w / frame_w;
            let height_scale = container_h / frame_h;
            match self.frame_fit {
                VideoFrameFit::Contain => width_scale.min(height_scale),
                VideoFrameFit::Cover => width_scale.max(height_scale),
            }
        } else {
            1.0
        };

        let dest_w = (frame_w * scale).max(0.0);
        let dest_h = (frame_h * scale).max(0.0);
        let offset_x = (container_w - dest_w) * 0.5;
        let offset_y = (container_h - dest_h) * 0.5;

        gpui::Bounds::new(
            gpui::point(
                bounds.origin.x + gpui::px(offset_x),
                bounds.origin.y + gpui::px(offset_y),
            ),
            gpui::size(gpui::px(dest_w), gpui::px(dest_h)),
        )
    }

    #[cfg(target_os = "macos")]
    fn paint_surface_macos(
        &self,
        window: &mut Window,
        bounds: gpui::Bounds<gpui::Pixels>,
        surface: video_native::Nv12PixelBuffer,
    ) {
        let dest_bounds =
            self.fitted_bounds(bounds, surface.surface_width(), surface.surface_height());
        window.paint_surface(dest_bounds, surface.into_pixel_buffer());
    }

    #[cfg(target_os = "linux")]
    fn paint_render_image(
        &self,
        window: &mut Window,
        cx: &mut gpui::App,
        bounds: gpui::Bounds<gpui::Pixels>,
        frame: super::state::RenderedVideoFrame,
    ) {
        let last_render_image = window.use_state(cx, |_, cx| {
            cx.on_release(|state: &mut LinuxRenderImageState, cx| {
                if let Some(image) = state.0.take() {
                    cx.drop_image(image, None);
                }
            })
            .detach();
            LinuxRenderImageState::default()
        });
        let previous = last_render_image.update(cx, |current, _| {
            if current
                .0
                .as_ref()
                .is_some_and(|image| image.id == frame.image.id)
            {
                None
            } else {
                current.0.replace(frame.image.clone())
            }
        });
        let dest_bounds = self.fitted_bounds(bounds, frame.width, frame.height);
        if let Err(error) =
            window.paint_image(dest_bounds, gpui::Corners::default(), frame.image, 0, false)
        {
            log::error!("failed to paint video frame: {error}");
        }
        if let Some(previous) = previous {
            cx.drop_image(previous, Some(window));
        }
    }

    fn frame_to_render(&self, needs_current_frame: bool) -> Option<VideoFrameData> {
        let buffered = self.video.buffered_len();
        if buffered == 0 {
            return needs_current_frame
                .then(|| self.video.current_frame_data())
                .flatten();
        }

        let mut frame_to_render = None;
        for _ in 0..buffered {
            if let Some(frame) = self.video.pop_buffered_frame() {
                frame_to_render = Some(frame);
            }
        }
        if frame_to_render.is_some() {
            log::debug!("Painting frame from buffer (buffered_len before drain: {buffered})");
        }
        frame_to_render
    }
}

impl Element for VideoElement {
    type RequestLayoutState = ();
    type PrepaintState = bool;

    fn id(&self) -> Option<ElementId> {
        self.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        if self.fill {
            let style = gpui::Style {
                size: gpui::size(gpui::relative(1.0).into(), gpui::relative(1.0).into()),
                min_size: gpui::size(gpui::px(0.0).into(), gpui::px(0.0).into()),
                ..Default::default()
            };
            return (window.request_layout(style, None, cx), ());
        }

        let (mut width, mut height) = self.get_display_size();

        if self.display_width.is_none() || self.display_height.is_none() {
            let (vw, vh) = self.video.display_size();
            if self.display_width.is_none() {
                width = gpui::px(vw as f32);
            }
            if self.display_height.is_none() {
                height = gpui::px(vh as f32);
            }
        }

        let style = gpui::Style {
            size: gpui::Size {
                width: gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                    gpui::AbsoluteLength::Pixels(width),
                )),
                height: gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                    gpui::AbsoluteLength::Pixels(height),
                )),
            },
            ..Default::default()
        };

        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut gpui::App,
    ) -> Self::PrepaintState {
        let is_playing = !self.video.eos() && !self.video.paused();
        let has_new_frame = self.video.take_frame_ready();
        if is_playing || has_new_frame {
            window.request_animation_frame();
        }
        has_new_frame
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        _has_new_frame: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut gpui::App,
    ) {
        #[cfg(target_os = "macos")]
        let cached_surface = self.video.cached_render_surface();
        #[cfg(target_os = "macos")]
        let needs_current_frame = *_has_new_frame || cached_surface.is_none();
        #[cfg(not(target_os = "macos"))]
        let needs_current_frame = false;

        let _frame_to_render = self.frame_to_render(needs_current_frame);

        #[cfg(target_os = "macos")]
        match _frame_to_render.as_ref() {
            Some(frame) => {
                if let Some(surface) = self.video.update_render_surface(frame) {
                    self.paint_surface_macos(_window, _bounds, surface);
                }
            }
            None => {
                if let Some(surface) = cached_surface {
                    self.paint_surface_macos(_window, _bounds, surface);
                }
            }
        }

        #[cfg(target_os = "linux")]
        if let Some(frame) = self.video.latest_render_frame() {
            self.paint_render_image(_window, _cx, _bounds, frame);
        }
    }
}

impl IntoElement for VideoElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Helper function to create a video element.
pub fn video(video: Video) -> VideoElement {
    VideoElement::new(video)
}
