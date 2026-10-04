use serde::{Deserialize, Serialize};

const DEFAULT_APP_WIDTH: u32 = 1495;
const DEFAULT_APP_HEIGHT: u32 = 1056;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Viewport {
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale_factor: f64,
    #[serde(default)]
    pub viewport_x: i64,
    #[serde(default)]
    pub viewport_y: i64,
    #[serde(default)]
    pub macos_chrome_top_inset: f32,
    #[serde(default)]
    pub macos_notch_center_x: f32,
    #[serde(default)]
    pub macos_notch_center_width: f32,
    #[serde(default)]
    pub macos_notch_center_height: f32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            logical_width: DEFAULT_APP_WIDTH,
            logical_height: DEFAULT_APP_HEIGHT,
            scale_factor: 2.0,
            viewport_x: 0,
            viewport_y: 0,
            macos_chrome_top_inset: 0.0,
            macos_notch_center_x: 0.0,
            macos_notch_center_width: 0.0,
            macos_notch_center_height: 0.0,
        }
    }
}

impl Viewport {
    pub fn app_width(&self) -> f32 {
        self.logical_width as f32
    }

    pub fn app_height(&self) -> f32 {
        self.logical_height as f32
    }

    pub fn chrome_top_inset(&self) -> f32 {
        self.macos_chrome_top_inset.max(0.0)
    }

    pub fn board_viewport_width(&self) -> f32 {
        (self.app_width() - 168.0).max(0.0)
    }

    pub fn card_peek_height(&self) -> f32 {
        self.app_height() - 120.0
    }
}
