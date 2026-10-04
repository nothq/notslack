use gpui::{
    Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Window,
};

use crate::ui::VideoClipPreview;

pub(super) struct SlackVideoClipPreviewElement {
    #[cfg(target_os = "macos")]
    preview: VideoClipPreview,
    element_id: ElementId,
}

impl SlackVideoClipPreviewElement {
    pub(super) fn new(
        #[cfg(target_os = "macos")] preview: VideoClipPreview,
        #[cfg(not(target_os = "macos"))] _: VideoClipPreview,
        element_id: impl Into<ElementId>,
    ) -> Self {
        Self {
            #[cfg(target_os = "macos")]
            preview,
            element_id: element_id.into(),
        }
    }

    #[cfg(target_os = "macos")]
    fn fitted_cover_bounds(
        &self,
        bounds: gpui::Bounds<gpui::Pixels>,
    ) -> gpui::Bounds<gpui::Pixels> {
        let frame_width = self.preview.surface().width() as f32;
        let frame_height = self.preview.surface().height() as f32;
        let container_width = bounds.size.width.as_f32();
        let container_height = bounds.size.height.as_f32();
        let scale = (container_width / frame_width).max(container_height / frame_height);
        let width = frame_width * scale;
        let height = frame_height * scale;
        gpui::Bounds::new(
            gpui::point(
                bounds.origin.x + gpui::px((container_width - width) * 0.5),
                bounds.origin.y + gpui::px((container_height - height) * 0.5),
            ),
            gpui::size(gpui::px(width), gpui::px(height)),
        )
    }
}

impl Element for SlackVideoClipPreviewElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
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
        let style = gpui::Style {
            size: gpui::size(gpui::relative(1.0).into(), gpui::relative(1.0).into()),
            min_size: gpui::size(gpui::px(0.0).into(), gpui::px(0.0).into()),
            ..Default::default()
        };
        (window.request_layout(style, None, cx), ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut gpui::App,
    ) -> Self::PrepaintState {
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        #[cfg(target_os = "macos")] bounds: gpui::Bounds<gpui::Pixels>,
        #[cfg(not(target_os = "macos"))] _: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        _prepaint_state: &mut Self::PrepaintState,
        #[cfg(target_os = "macos")] window: &mut Window,
        #[cfg(not(target_os = "macos"))] _: &mut Window,
        _cx: &mut gpui::App,
    ) {
        #[cfg(target_os = "macos")]
        window.paint_surface(
            self.fitted_cover_bounds(bounds),
            self.preview.surface().clone_pixel_buffer(),
        );
    }
}

impl IntoElement for SlackVideoClipPreviewElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
