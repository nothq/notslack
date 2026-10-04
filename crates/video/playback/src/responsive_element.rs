use gpui::{
    size, AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Style, Window,
};

use crate::{
    ffmpeg_player::{video as gpui_video, Video as NativeVideo},
    source::VideoFrameFit,
};

const PLAYER_FRAME_BUFFER_CAPACITY: usize = 30;

pub(crate) struct ResponsiveVideoElement {
    video: NativeVideo,
    element_id: ElementId,
    frame_fit: VideoFrameFit,
}

impl ResponsiveVideoElement {
    pub(crate) fn new(
        video: NativeVideo,
        element_id: impl Into<ElementId>,
        frame_fit: VideoFrameFit,
    ) -> Self {
        Self {
            video,
            element_id: element_id.into(),
            frame_fit,
        }
    }
}

impl IntoElement for ResponsiveVideoElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ResponsiveVideoElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<AnyElement>;

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = Style {
            size: size(gpui::relative(1.0).into(), gpui::relative(1.0).into()),
            min_size: size(gpui::px(0.0).into(), gpui::px(0.0).into()),
            ..Style::default()
        };
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let mut element = gpui_video(self.video.clone())
            .id(self.element_id.clone())
            .buffer_capacity(PLAYER_FRAME_BUFFER_CAPACITY)
            .frame_fit(self.frame_fit)
            .size(bounds.size.width, bounds.size.height)
            .into_any_element();
        element.layout_as_root(
            size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::Definite(bounds.size.height),
            ),
            window,
            cx,
        );
        let _ = element.prepaint_at(bounds.origin, window, cx);
        Some(element)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(element) = prepaint.as_mut() {
            element.paint(window, cx);
        }
    }
}
