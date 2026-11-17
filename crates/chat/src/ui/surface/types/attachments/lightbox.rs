//! What the image viewer remembers while it is open: zoom, rotation, the
//! full-size image and the controls around it.

use std::sync::Arc;

use gpui::{Bounds, FocusHandle, Image, Pixels, Point, SharedString};

/// The smallest zoom: the image fits the viewer.
pub(crate) const SLACK_LIGHTBOX_MIN_ZOOM: f32 = 1.0;
/// The largest zoom, twice the fitted size, like Slack.
pub(crate) const SLACK_LIGHTBOX_MAX_ZOOM: f32 = 2.0;
/// How far one press of − or + moves the zoom.
pub(crate) const SLACK_LIGHTBOX_ZOOM_STEP: f32 = 0.25;

#[derive(Clone)]
pub(crate) struct SlackLightboxView {
    pub(crate) attachment_id: SharedString,
    /// Takes the keys (space, + and −) while the viewer is open.
    pub(crate) focus: FocusHandle,
    pub(crate) focus_pending: bool,
    pub(crate) zoom: f32,
    /// Quarter turns clockwise, 0 to 3.
    pub(crate) rotation: u8,
    pub(crate) pan: Point<Pixels>,
    /// Where a drag to move the zoomed image began: the pointer, then the
    /// pan at that moment.
    pub(crate) drag_from: Option<(Point<Pixels>, Point<Pixels>)>,
    pub(crate) slider_dragging: bool,
    pub(crate) viewport: Option<Bounds<Pixels>>,
    pub(crate) slider: Option<Bounds<Pixels>>,
    pub(crate) thread_shown: bool,
    pub(crate) menu_open: bool,
    pub(crate) original: SlackLightboxOriginal,
    /// The image turned by `rotation`, once it is ready.
    pub(crate) turned: Option<SlackLightboxTurnedImage>,
    pub(crate) turning: Option<u8>,
    pub(crate) download_when_loaded: bool,
    pub(crate) notice: Option<SharedString>,
    pub(crate) notice_generation: u64,
}

impl SlackLightboxView {
    pub(crate) fn new(attachment_id: SharedString, focus: FocusHandle) -> Self {
        Self {
            attachment_id,
            focus,
            focus_pending: true,
            zoom: SLACK_LIGHTBOX_MIN_ZOOM,
            rotation: 0,
            pan: Point::default(),
            drag_from: None,
            slider_dragging: false,
            viewport: None,
            slider: None,
            thread_shown: false,
            menu_open: false,
            original: SlackLightboxOriginal::NotLoaded,
            turned: None,
            turning: None,
            download_when_loaded: false,
            notice: None,
            notice_generation: 0,
        }
    }

    /// Zoomed or turned, so there is something to reset.
    pub(crate) fn is_adjusted(&self) -> bool {
        self.zoom > SLACK_LIGHTBOX_MIN_ZOOM || self.rotation != 0
    }

    pub(crate) fn zoom_percent(&self) -> u32 {
        (self.zoom * 100.0).round() as u32
    }
}

/// The image file at full size.
#[derive(Clone)]
pub(crate) enum SlackLightboxOriginal {
    NotLoaded,
    Loading,
    Loaded(Arc<SlackLightboxOriginalImage>),
    Failed,
}

pub(crate) struct SlackLightboxOriginalImage {
    pub(crate) bytes: Arc<[u8]>,
    pub(crate) mimetype: String,
    pub(crate) image: Arc<Image>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Clone)]
pub(crate) struct SlackLightboxTurnedImage {
    pub(crate) rotation: u8,
    pub(crate) image: Arc<Image>,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// Whether the preview is an image the viewer can zoom and turn.
pub(crate) fn slack_lightbox_shows_image(attachment: &crate::ui::SlackAttachment) -> bool {
    attachment.media.is_none()
        && (attachment.full_image_url.is_some()
            || attachment.preview_image_url.is_some()
            || attachment.preview_image_base64.is_some())
}
