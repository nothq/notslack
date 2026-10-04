use std::{sync::atomic::Ordering, time::Duration};

use super::{
    error::Error,
    state::{Position, VideoFrameData},
    video::Video,
};

#[cfg(target_os = "macos")]
const RETAINED_RENDER_SURFACES: usize = 8;

impl Video {
    /// Get the size/resolution of the video as `(width, height)`.
    pub fn size(&self) -> (i32, i32) {
        (self.read().width, self.read().height)
    }

    /// Get the natural aspect ratio (width / height) of the video as f32.
    pub fn aspect_ratio(&self) -> f32 {
        let (w, h) = self.size();
        if w <= 0 || h <= 0 {
            return 1.0;
        }
        w as f32 / h as f32
    }

    /// Set an override display width in pixels. Pass `None` to clear.
    pub fn set_display_width(&self, width: Option<u32>) {
        self.write().display_width_override = width;
    }

    /// Set an override display height in pixels. Pass `None` to clear.
    pub fn set_display_height(&self, height: Option<u32>) {
        self.write().display_height_override = height;
    }

    /// Set override display size in pixels. Any value set to `None` is cleared.
    pub fn set_display_size(&self, width: Option<u32>, height: Option<u32>) {
        let mut inner = self.write();
        inner.display_width_override = width;
        inner.display_height_override = height;
    }

    /// Get the effective display size honoring overrides.
    pub fn display_size(&self) -> (u32, u32) {
        let inner = self.read();
        let natural_w = inner.width.max(0) as u32;
        let natural_h = inner.height.max(0) as u32;
        let ar = if natural_h == 0 {
            1.0
        } else {
            natural_w as f32 / natural_h as f32
        };
        match (inner.display_width_override, inner.display_height_override) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, inferred_height(w, ar, natural_h)),
            (None, Some(h)) => (((h as f32) * ar).round() as u32, h),
            (None, None) => (natural_w, natural_h),
        }
    }

    /// Get the framerate of the video as frames per second.
    pub fn framerate(&self) -> f64 {
        self.read().framerate
    }

    /// Set the volume multiplier of the audio.
    pub fn set_volume(&self, volume: f64) {
        let mut inner = self.write();
        inner.volume = volume;
        if let Some(audio) = &inner.audio {
            audio.set_volume(volume);
        }
    }

    /// Get the volume multiplier of the audio.
    pub fn volume(&self) -> f64 {
        self.read().volume
    }

    /// Set if the audio is muted or not.
    pub fn set_muted(&self, muted: bool) {
        let mut inner = self.write();
        inner.muted = muted;
        if let Some(audio) = &inner.audio {
            audio.set_muted(muted);
        }
    }

    /// Get if the audio is muted or not.
    pub fn muted(&self) -> bool {
        self.read().muted
    }

    /// Report whether the media has an active audio stream.
    pub fn has_audio(&self) -> bool {
        self.read().audio.is_some()
    }

    /// Return the latest audio startup or runtime error, if audio could not play.
    pub fn audio_error(&self) -> Option<String> {
        let inner = self.read();
        inner
            .audio_start_error
            .clone()
            .or_else(|| inner.audio.as_ref().and_then(|audio| audio.error_message()))
    }

    /// Get if the stream ended or not.
    pub fn eos(&self) -> bool {
        self.read().is_eos.load(Ordering::Acquire)
    }

    /// Get if the media will loop or not.
    pub fn looping(&self) -> bool {
        self.read().looping.load(Ordering::SeqCst)
    }

    /// Set if the media will loop or not.
    pub fn set_looping(&self, looping: bool) {
        let inner = self.write();
        inner.looping.store(looping, Ordering::SeqCst);
        if let Some(audio) = &inner.audio {
            audio.set_looping(looping);
        }
    }

    /// Set if the media is paused or not.
    pub fn set_paused(&self, paused: bool) {
        self.write().set_paused(paused)
    }

    /// Get if the media is paused or not.
    pub fn paused(&self) -> bool {
        self.read().paused()
    }

    /// Jumps to a specific position in the media.
    pub fn seek(&self, position: impl Into<Position>, accurate: bool) -> Result<(), Error> {
        self.read().seek(position, accurate)
    }

    /// Set the playback speed of the media.
    pub fn set_speed(&self, speed: f64) -> Result<(), Error> {
        self.write().set_speed(speed)
    }

    /// Get the current playback speed.
    pub fn speed(&self) -> f64 {
        f64::from_bits(self.read().speed.load(Ordering::SeqCst))
    }

    /// Get the current playback position in time.
    pub fn position(&self) -> Duration {
        Duration::from_nanos(self.read().current_position_ns.load(Ordering::SeqCst))
    }

    /// Report whether the decoder is still resolving a seek target.
    pub(crate) fn has_pending_seek(&self) -> bool {
        self.read().pending_seek_generation.load(Ordering::Acquire) != 0
    }

    /// Get the media duration.
    pub fn duration(&self) -> Duration {
        self.read().duration
    }

    /// Restarts a stream.
    pub fn restart_stream(&self) -> Result<(), Error> {
        self.write().restart_stream()
    }

    /// Report whether a current NV12 frame is available.
    pub fn has_current_frame(&self) -> bool {
        self.read().frame.lock().is_some()
    }

    /// Get the current NV12 frame data if available.
    pub fn current_frame_data(&self) -> Option<VideoFrameData> {
        self.read().frame.lock().clone()
    }

    /// Returns true if a new frame arrived since last check and resets the flag.
    pub fn take_frame_ready(&self) -> bool {
        self.read().upload_frame.swap(false, Ordering::SeqCst)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn update_render_surface(
        &self,
        frame: &VideoFrameData,
    ) -> Option<video_native::Nv12PixelBuffer> {
        let mut inner = self.write();
        let needs_pool = inner
            .render_surface_pool
            .as_ref()
            .map(|pool| !pool.supports_frame(frame.width, frame.height, frame.uv_row_width))
            .unwrap_or(true);
        if needs_pool {
            inner.render_surface_pool = video_native::Nv12PixelBufferPool::new(
                frame.width,
                frame.height,
                frame.uv_row_width,
            );
            inner.render_surfaces.clear();
        }
        let surface = inner
            .render_surface_pool
            .as_ref()?
            .copy_frame(&frame.nv12_data)?;
        inner.render_surfaces.push_back(surface.clone());
        while inner.render_surfaces.len() > RETAINED_RENDER_SURFACES {
            inner.render_surfaces.pop_front();
        }
        Some(surface)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn cached_render_surface(&self) -> Option<video_native::Nv12PixelBuffer> {
        self.read().render_surfaces.back().cloned()
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn latest_render_frame(&self) -> Option<super::state::RenderedVideoFrame> {
        let inner = self.read();
        let frame = inner.rendered_frame.lock().clone();
        frame
    }

    /// Configure the frame buffer capacity (0 disables buffering).
    pub fn set_frame_buffer_capacity(&self, capacity: usize) {
        let inner = self.read();
        inner
            .frame_buffer_capacity
            .store(capacity, Ordering::SeqCst);
        if capacity == 0 {
            inner.frame_buffer.lock().clear();
            return;
        }
        let mut buf = inner.frame_buffer.lock();
        while buf.len() > capacity {
            buf.pop_front();
        }
    }

    /// Retrieve the current frame buffer capacity.
    pub fn frame_buffer_capacity(&self) -> usize {
        self.read().frame_buffer_capacity.load(Ordering::SeqCst)
    }

    /// Pop the oldest buffered frame, returning tightly packed NV12 bytes with width/height.
    pub fn pop_buffered_frame(&self) -> Option<VideoFrameData> {
        self.read().frame_buffer.lock().pop_front()
    }

    /// Number of frames currently buffered.
    pub fn buffered_len(&self) -> usize {
        self.read().frame_buffer.lock().len()
    }
}

fn inferred_height(width: u32, aspect_ratio: f32, natural_height: u32) -> u32 {
    if aspect_ratio == 0.0 {
        natural_height
    } else {
        (width as f32 / aspect_ratio).round() as u32
    }
}
