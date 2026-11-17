//! The image viewer's controls: zoom, rotation, moving a zoomed image, the
//! full-size original, and the actions beside it (download, thread, saved,
//! copy link).

use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use gpui::{point, px, AppContext, Bounds, ClipboardItem, Image, ImageFormat, Pixels, Point, Size};

use crate::ui::surface::slack_lightbox_shows_image;
use crate::ui::surface::{
    Context, SlackAttachmentSelection, SlackLightboxOriginal, SlackLightboxOriginalImage,
    SlackLightboxTurnedImage, SlackLightboxView, SlackMessageRow, SurfaceState,
    SLACK_LIGHTBOX_MAX_ZOOM, SLACK_LIGHTBOX_MIN_ZOOM, SLACK_LIGHTBOX_ZOOM_STEP,
};
use crate::ui::SlackAttachment;

const SLACK_LIGHTBOX_NOTICE_DURATION: Duration = Duration::from_secs(3);

/// Where the image sits in the viewer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SlackLightboxLayout {
    pub(crate) origin: Point<Pixels>,
    pub(crate) size: Size<Pixels>,
}

impl SurfaceState {
    /// The viewer state of the open preview.
    pub(crate) fn slack_lightbox_view(&self) -> Option<&SlackLightboxView> {
        let expanded = self.slack_expanded_attachment.as_ref()?;
        self.slack_lightbox
            .as_ref()
            .filter(|view| view.attachment_id == expanded.attachment_id)
    }

    fn slack_lightbox_view_mut(&mut self) -> Option<&mut SlackLightboxView> {
        let attachment_id = self
            .slack_expanded_attachment
            .as_ref()?
            .attachment_id
            .clone();
        self.slack_lightbox
            .as_mut()
            .filter(|view| view.attachment_id == attachment_id)
    }

    pub(crate) fn begin_slack_lightbox_view(
        &mut self,
        selection: &SlackAttachmentSelection,
        cx: &mut Context<Self>,
    ) {
        self.slack_lightbox = Some(SlackLightboxView::new(
            selection.attachment_id.clone(),
            cx.focus_handle(),
        ));
        if slack_lightbox_shows_image(&selection.attachment) {
            self.load_slack_lightbox_original(cx);
        }
    }

    /// The message the open preview belongs to.
    pub(crate) fn slack_lightbox_message(&self) -> Option<&SlackMessageRow> {
        let expanded = self.slack_expanded_attachment.as_ref()?;
        self.find_slack_message_owning_attachment(expanded.attachment_id.as_ref())
    }

    /// The image to draw: turned when a turn is ready, otherwise the
    /// original, otherwise the thumbnail.
    pub(crate) fn slack_lightbox_image(
        &self,
        attachment: &SlackAttachment,
    ) -> Option<(Arc<Image>, Option<(u32, u32)>, bool)> {
        let view = self.slack_lightbox_view();
        if let Some(turned) =
            view.and_then(|view| view.turned.as_ref().filter(|t| t.rotation == view.rotation))
        {
            return Some((
                turned.image.clone(),
                Some((turned.width, turned.height)),
                true,
            ));
        }
        let unturned = match view.map(|view| &view.original) {
            Some(SlackLightboxOriginal::Loaded(original)) => Some((
                original.image.clone(),
                Some((original.width, original.height)),
            )),
            _ => self
                .slack_attachment_preview_image_by_key(
                    crate::ui::surface::slack_remote_image_cache_key(attachment).as_deref(),
                )
                .map(|image| {
                    let size = attachment.original_width.zip(attachment.original_height);
                    (image, size)
                }),
        }?;
        let matches_rotation = view.is_none_or(|view| view.rotation == 0);
        Some((unturned.0, unturned.1, matches_rotation))
    }

    /// Where the image goes, from the viewer's size and the image's.
    pub(crate) fn slack_lightbox_layout(
        &self,
        image_size: Option<(u32, u32)>,
    ) -> Option<SlackLightboxLayout> {
        let view = self.slack_lightbox_view()?;
        let viewport = view.viewport?;
        let (width, height) = image_size?;
        Some(slack_lightbox_layout(
            viewport,
            (width, height),
            view.zoom,
            view.pan,
        ))
    }

    pub(crate) fn set_slack_lightbox_viewport(&mut self, bounds: Bounds<Pixels>) -> bool {
        let Some(view) = self.slack_lightbox_view_mut() else {
            return false;
        };
        if view.viewport == Some(bounds) {
            return false;
        }
        view.viewport = Some(bounds);
        true
    }

    pub(crate) fn set_slack_lightbox_slider(&mut self, bounds: Bounds<Pixels>) {
        if let Some(view) = self.slack_lightbox_view_mut() {
            view.slider = Some(bounds);
        }
    }

    pub(crate) fn step_slack_lightbox_zoom(&mut self, steps: f32, cx: &mut Context<Self>) {
        let Some(zoom) = self.slack_lightbox_view().map(|view| view.zoom) else {
            return;
        };
        self.set_slack_lightbox_zoom(zoom + steps * SLACK_LIGHTBOX_ZOOM_STEP, cx);
    }

    pub(crate) fn set_slack_lightbox_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        let size = self.slack_lightbox_current_image_size();
        let Some(view) = self.slack_lightbox_view_mut() else {
            return;
        };
        let zoom = zoom.clamp(SLACK_LIGHTBOX_MIN_ZOOM, SLACK_LIGHTBOX_MAX_ZOOM);
        if (zoom - view.zoom).abs() < f32::EPSILON {
            return;
        }
        view.zoom = zoom;
        view.pan = clamp_slack_lightbox_pan(view, size, view.pan);
        cx.notify();
    }

    /// Sets the zoom from a point on the slider.
    pub(crate) fn set_slack_lightbox_zoom_at(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let Some(slider) = self.slack_lightbox_view().and_then(|view| view.slider) else {
            return;
        };
        let width = f32::from(slider.size.width).max(1.0);
        let fraction = ((f32::from(x) - f32::from(slider.origin.x)) / width).clamp(0.0, 1.0);
        let zoom = SLACK_LIGHTBOX_MIN_ZOOM
            + fraction * (SLACK_LIGHTBOX_MAX_ZOOM - SLACK_LIGHTBOX_MIN_ZOOM);
        self.set_slack_lightbox_zoom(zoom, cx);
    }

    pub(crate) fn set_slack_lightbox_slider_dragging(&mut self, dragging: bool) {
        if let Some(view) = self.slack_lightbox_view_mut() {
            view.slider_dragging = dragging;
        }
    }

    pub(crate) fn reset_slack_lightbox(&mut self, cx: &mut Context<Self>) {
        let Some(view) = self.slack_lightbox_view_mut() else {
            return;
        };
        if !view.is_adjusted() && view.pan == Point::default() {
            return;
        }
        view.zoom = SLACK_LIGHTBOX_MIN_ZOOM;
        view.rotation = 0;
        view.pan = Point::default();
        view.drag_from = None;
        cx.notify();
    }

    /// Turns the image a quarter clockwise.
    pub(crate) fn rotate_slack_lightbox(&mut self, cx: &mut Context<Self>) {
        let Some(attachment) = self
            .slack_expanded_attachment
            .as_ref()
            .map(|selection| selection.attachment.clone())
        else {
            return;
        };
        let source = match self.slack_lightbox_view().map(|view| &view.original) {
            Some(SlackLightboxOriginal::Loaded(original)) => Some(original.image.clone()),
            _ => self.slack_attachment_preview_image_by_key(
                crate::ui::surface::slack_remote_image_cache_key(&attachment).as_deref(),
            ),
        };
        let Some(view) = self.slack_lightbox_view_mut() else {
            return;
        };
        view.rotation = (view.rotation + 1) % 4;
        view.pan = Point::default();
        let rotation = view.rotation;
        cx.notify();
        if rotation == 0 {
            view.turned = None;
            return;
        }
        let Some(source) = source else {
            return;
        };
        view.turning = Some(rotation);
        let attachment_id = view.attachment_id.clone();
        self.spawn_background_task(
            (source, rotation),
            cx,
            |(source, rotation)| turn_slack_lightbox_image(&source, rotation),
            move |this, turned, cx| {
                let Some(view) = this
                    .slack_lightbox_view_mut()
                    .filter(|view| view.attachment_id == attachment_id)
                else {
                    return;
                };
                if view.turning == Some(rotation) {
                    view.turning = None;
                }
                if let Some(turned) = turned {
                    view.turned = Some(turned);
                }
                cx.notify();
            },
        );
    }

    pub(crate) fn begin_slack_lightbox_drag(&mut self, position: Point<Pixels>) {
        if let Some(view) = self.slack_lightbox_view_mut() {
            if view.zoom > SLACK_LIGHTBOX_MIN_ZOOM {
                view.drag_from = Some((position, view.pan));
            }
        }
    }

    pub(crate) fn drag_slack_lightbox(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self
            .slack_lightbox_view()
            .is_some_and(|view| view.slider_dragging)
        {
            self.set_slack_lightbox_zoom_at(position.x, cx);
            return;
        }
        let size = self.slack_lightbox_current_image_size();
        let Some(view) = self.slack_lightbox_view_mut() else {
            return;
        };
        let Some((start, pan)) = view.drag_from else {
            return;
        };
        let wanted = point(pan.x + position.x - start.x, pan.y + position.y - start.y);
        let pan = clamp_slack_lightbox_pan(view, size, wanted);
        if pan != view.pan {
            view.pan = pan;
            cx.notify();
        }
    }

    pub(crate) fn end_slack_lightbox_drag(&mut self) {
        if let Some(view) = self.slack_lightbox_view_mut() {
            view.drag_from = None;
            view.slider_dragging = false;
        }
    }

    fn slack_lightbox_current_image_size(&self) -> Option<(u32, u32)> {
        let attachment = &self.slack_expanded_attachment.as_ref()?.attachment;
        let (_, size, turned) = self.slack_lightbox_image(attachment)?;
        let (width, height) = size?;
        let rotation = self.slack_lightbox_view()?.rotation;
        // Until the turned image is ready, lay out the size it will have.
        Some(if !turned && rotation % 2 == 1 {
            (height, width)
        } else {
            (width, height)
        })
    }

    pub(crate) fn slack_lightbox_display_size(&self) -> Option<(u32, u32)> {
        self.slack_lightbox_current_image_size()
    }

    fn load_slack_lightbox_original(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = self.slack_expanded_attachment.as_ref() else {
            return;
        };
        let attachment = &selection.attachment;
        let Some(url) = attachment
            .full_image_url
            .clone()
            .or_else(|| attachment.preview_image_url.clone())
        else {
            return;
        };
        let Some(workspace_api) = self.active_slack_workspace_api() else {
            return;
        };
        let Some(view) = self.slack_lightbox_view_mut() else {
            return;
        };
        if matches!(
            view.original,
            SlackLightboxOriginal::Loading | SlackLightboxOriginal::Loaded(_)
        ) {
            return;
        }
        view.original = SlackLightboxOriginal::Loading;
        let attachment_id = view.attachment_id.clone();
        self.spawn_background_task(
            url,
            cx,
            move |url| {
                let image = workspace_api.load_slack_remote_image(&url).ok().flatten()?;
                slack_lightbox_original(image.bytes, image.mimetype)
            },
            move |this, original, cx| {
                let Some(view) = this
                    .slack_lightbox_view_mut()
                    .filter(|view| view.attachment_id == attachment_id)
                else {
                    return;
                };
                let download = std::mem::take(&mut view.download_when_loaded);
                view.original = match original {
                    Some(original) => SlackLightboxOriginal::Loaded(Arc::new(original)),
                    None => SlackLightboxOriginal::Failed,
                };
                // A turn asked for before the original arrived is redone
                // at full size.
                let rotation = view.rotation;
                if rotation != 0 {
                    view.rotation = (rotation + 3) % 4;
                    this.rotate_slack_lightbox(cx);
                }
                if download {
                    this.download_slack_lightbox_image(cx);
                }
                cx.notify();
            },
        );
    }

    pub(crate) fn toggle_slack_lightbox_thread(&mut self, cx: &mut Context<Self>) {
        let shown = self.slack_lightbox_thread_panel().is_some();
        if let Some(view) = self.slack_lightbox_view_mut() {
            view.menu_open = false;
        }
        if shown {
            if let Some(view) = self.slack_lightbox_view_mut() {
                view.thread_shown = false;
            }
            cx.notify();
            return;
        }
        let Some(target) = self
            .slack_lightbox_message()
            .and_then(|row| row.action_target.clone())
        else {
            return;
        };
        if let Some(view) = self.slack_lightbox_view_mut() {
            view.thread_shown = true;
        }
        // Opening a thread closes previews elsewhere; here the thread goes
        // beside the image, so the preview stays when the panel opened in
        // place.
        let expanded = self.slack_expanded_attachment.clone();
        self.open_slack_message_thread_target(&target, cx);
        let opened_here = self.slack_thread_panel.as_ref().is_some_and(|panel| {
            panel.conversation_id == target.conversation_id()
                && panel.parent_message_id == target.root_timestamp().as_str()
        });
        if opened_here && self.slack_expanded_attachment.is_none() {
            self.slack_expanded_attachment = expanded;
        }
        cx.notify();
    }

    /// The thread panel the viewer shows beside the image, when the viewer
    /// was asked to show the thread and it is open.
    pub(crate) fn slack_lightbox_thread_panel(
        &self,
    ) -> Option<&crate::ui::surface::SlackThreadPanelState> {
        if !self.slack_lightbox_view()?.thread_shown {
            return None;
        }
        let target = self.slack_lightbox_message()?.action_target.clone()?;
        self.slack_thread_panel.as_ref().filter(|panel| {
            panel.conversation_id == target.conversation_id()
                && panel.parent_message_id == target.root_timestamp().as_str()
        })
    }

    pub(crate) fn toggle_slack_lightbox_menu(&mut self, cx: &mut Context<Self>) {
        if let Some(view) = self.slack_lightbox_view_mut() {
            view.menu_open = !view.menu_open;
            cx.notify();
        }
    }

    pub(crate) fn close_slack_lightbox_menu(&mut self, cx: &mut Context<Self>) -> bool {
        match self.slack_lightbox_view_mut() {
            Some(view) if view.menu_open => {
                view.menu_open = false;
                cx.notify();
                true
            }
            _ => false,
        }
    }

    pub(crate) fn toggle_slack_lightbox_message_saved(&mut self, cx: &mut Context<Self>) {
        self.close_slack_lightbox_menu(cx);
        let Some((target, saved_state)) = self
            .slack_lightbox_message()
            .and_then(|row| Some((row.action_target.clone()?, row.saved_state)))
        else {
            return;
        };
        let notice = if saved_state.is_some() {
            "Removed from saved items"
        } else {
            "Added to saved items"
        };
        self.toggle_slack_message_saved_state_for_target(target, saved_state, cx);
        self.show_slack_lightbox_notice(notice, cx);
    }

    pub(crate) fn copy_slack_lightbox_file_link(&mut self, cx: &mut Context<Self>) {
        self.close_slack_lightbox_menu(cx);
        let Some(link) = self
            .slack_expanded_attachment
            .as_ref()
            .map(|selection| selection.attachment.link_url.clone())
            .filter(|link| !link.is_empty())
        else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(link));
        self.show_slack_lightbox_notice("Link copied to clipboard", cx);
    }

    /// Saves the original image where the person chooses.
    pub(crate) fn download_slack_lightbox_image(&mut self, cx: &mut Context<Self>) {
        let Some(title) = self
            .slack_expanded_attachment
            .as_ref()
            .map(|selection| selection.attachment.title.clone())
        else {
            return;
        };
        let original = match self.slack_lightbox_view().map(|view| view.original.clone()) {
            Some(SlackLightboxOriginal::Loaded(original)) => original,
            Some(SlackLightboxOriginal::Failed) => {
                self.show_slack_lightbox_notice("The image could not be downloaded", cx);
                return;
            }
            Some(_) => {
                if let Some(view) = self.slack_lightbox_view_mut() {
                    view.download_when_loaded = true;
                }
                self.load_slack_lightbox_original(cx);
                return;
            }
            None => return,
        };
        let directory = self
            .local_file_api
            .as_ref()
            .and_then(|api| api.default_download_directory().ok())
            .or_else(dirs::download_dir)
            .or_else(dirs::home_dir)
            .unwrap_or_default();
        let name = slack_lightbox_file_name(&title, &original.mimetype);
        let path = cx.prompt_for_new_path(&directory, Some(&name));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = path.await else {
                return;
            };
            let bytes = original.bytes.clone();
            let saved_path = path.clone();
            let result = cx
                .background_spawn(async move { std::fs::write(&saved_path, &bytes) })
                .await;
            let notice = match result {
                Ok(()) => format!(
                    "Saved {}",
                    path.file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or(name)
                ),
                Err(error) => format!("Could not save the image: {error}"),
            };
            this.update(cx, |this, cx| this.show_slack_lightbox_notice(notice, cx))
                .ok();
        })
        .detach();
    }

    pub(crate) fn show_slack_lightbox_notice(
        &mut self,
        notice: impl Into<gpui::SharedString>,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.slack_lightbox_view_mut() else {
            return;
        };
        view.notice = Some(notice.into());
        view.notice_generation += 1;
        let generation = view.notice_generation;
        cx.notify();
        self.spawn_timer_task(
            generation,
            SLACK_LIGHTBOX_NOTICE_DURATION,
            cx,
            |this, generation, cx| {
                if let Some(view) = this
                    .slack_lightbox_view_mut()
                    .filter(|view| view.notice_generation == generation)
                {
                    view.notice = None;
                    cx.notify();
                }
            },
        );
    }
}

fn slack_lightbox_original(bytes: Vec<u8>, mimetype: String) -> Option<SlackLightboxOriginalImage> {
    let format = gpui_components::image_format_from_mime_type(&mimetype)
        .or_else(|| slack_lightbox_guess_format(&bytes))?;
    let (width, height) = image::ImageReader::new(Cursor::new(&bytes))
        .with_guessed_format()
        .ok()?
        .into_dimensions()
        .ok()?;
    let bytes: Arc<[u8]> = bytes.into();
    Some(SlackLightboxOriginalImage {
        image: Arc::new(Image::from_bytes(format, bytes.to_vec())),
        bytes,
        mimetype,
        width,
        height,
    })
}

fn slack_lightbox_guess_format(bytes: &[u8]) -> Option<ImageFormat> {
    Some(match image::guess_format(bytes).ok()? {
        image::ImageFormat::Png => ImageFormat::Png,
        image::ImageFormat::Jpeg => ImageFormat::Jpeg,
        image::ImageFormat::Gif => ImageFormat::Gif,
        image::ImageFormat::WebP => ImageFormat::Webp,
        image::ImageFormat::Bmp => ImageFormat::Bmp,
        image::ImageFormat::Tiff => ImageFormat::Tiff,
        _ => return None,
    })
}

/// The image turned `rotation` quarters clockwise.
fn turn_slack_lightbox_image(source: &Image, rotation: u8) -> Option<SlackLightboxTurnedImage> {
    let decoded = image::load_from_memory(source.bytes()).ok()?;
    let turned = match rotation % 4 {
        1 => decoded.rotate90(),
        2 => decoded.rotate180(),
        3 => decoded.rotate270(),
        _ => decoded,
    };
    let turned = image::DynamicImage::ImageRgba8(turned.to_rgba8());
    let mut bytes = Vec::new();
    turned
        .write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Bmp)
        .ok()?;
    Some(SlackLightboxTurnedImage {
        rotation,
        width: turned.width(),
        height: turned.height(),
        image: Arc::new(Image::from_bytes(ImageFormat::Bmp, bytes)),
    })
}

/// Fits the image inside the viewer without enlarging it, then applies the
/// zoom and the pan around the centre.
pub(crate) fn slack_lightbox_layout(
    viewport: Bounds<Pixels>,
    (width, height): (u32, u32),
    zoom: f32,
    pan: Point<Pixels>,
) -> SlackLightboxLayout {
    let view_width = f32::from(viewport.size.width).max(1.0);
    let view_height = f32::from(viewport.size.height).max(1.0);
    let width = width.max(1) as f32;
    let height = height.max(1) as f32;
    let fit = (view_width / width).min(view_height / height).min(1.0);
    let size = Size {
        width: px(width * fit * zoom),
        height: px(height * fit * zoom),
    };
    SlackLightboxLayout {
        origin: point(
            px((view_width - f32::from(size.width)) / 2.0) + pan.x,
            px((view_height - f32::from(size.height)) / 2.0) + pan.y,
        ),
        size,
    }
}

/// Keeps a zoomed image from being dragged past its own edges.
fn clamp_slack_lightbox_pan(
    view: &SlackLightboxView,
    image_size: Option<(u32, u32)>,
    pan: Point<Pixels>,
) -> Point<Pixels> {
    let (Some(viewport), Some(size)) = (view.viewport, image_size) else {
        return Point::default();
    };
    let layout = slack_lightbox_layout(viewport, size, view.zoom, Point::default());
    let limit_x = ((f32::from(layout.size.width) - f32::from(viewport.size.width)) / 2.0).max(0.0);
    let limit_y =
        ((f32::from(layout.size.height) - f32::from(viewport.size.height)) / 2.0).max(0.0);
    point(
        px(f32::from(pan.x).clamp(-limit_x, limit_x)),
        px(f32::from(pan.y).clamp(-limit_y, limit_y)),
    )
}

/// The file name to suggest: the title, with an extension for its type
/// when it has none.
fn slack_lightbox_file_name(title: &str, mimetype: &str) -> String {
    let name: String = title
        .chars()
        .map(|character| match character {
            '/' | '\\' | '\0' => '_',
            character => character,
        })
        .collect();
    let name = if name.trim().is_empty() {
        "image".to_string()
    } else {
        name
    };
    if std::path::Path::new(&name).extension().is_some() {
        return name;
    }
    let extension = match mimetype {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        _ => return name,
    };
    format!("{name}.{extension}")
}

#[cfg(test)]
mod tests {
    use gpui::{point, px, size, Bounds};

    use super::{slack_lightbox_file_name, slack_lightbox_layout};

    #[test]
    fn images_fit_without_growing_and_zoom_from_the_centre() {
        let viewport = Bounds::new(point(px(0.0), px(0.0)), size(px(1000.0), px(500.0)));
        let fitted = slack_lightbox_layout(viewport, (2000, 500), 1.0, point(px(0.0), px(0.0)));
        assert_eq!(fitted.size, size(px(1000.0), px(250.0)));
        assert_eq!(fitted.origin, point(px(0.0), px(125.0)));
        let small = slack_lightbox_layout(viewport, (100, 50), 1.0, point(px(0.0), px(0.0)));
        assert_eq!(small.size, size(px(100.0), px(50.0)));
        let zoomed = slack_lightbox_layout(viewport, (2000, 500), 2.0, point(px(10.0), px(0.0)));
        assert_eq!(zoomed.size, size(px(2000.0), px(500.0)));
        assert_eq!(zoomed.origin, point(px(-490.0), px(0.0)));
    }

    #[test]
    fn download_names_keep_or_gain_an_extension() {
        assert_eq!(
            slack_lightbox_file_name("image.png", "image/png"),
            "image.png"
        );
        assert_eq!(
            slack_lightbox_file_name("Screenshot", "image/jpeg"),
            "Screenshot.jpg"
        );
        assert_eq!(slack_lightbox_file_name("a/b", "image/png"), "a_b.png");
    }
}
