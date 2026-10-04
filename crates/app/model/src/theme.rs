use crate::AppearanceMode;

pub const DARK_APP_BACKGROUND: u32 = 0x27292d;
pub const EMBEDDED_SHELL_FONT_FAMILY: &str = "Lato";
pub const EMBEDDED_SHELL_ROW_HEIGHT: f32 = 38.0;

pub const fn if_light(appearance_mode: AppearanceMode, light: f32, dark: f32) -> f32 {
    match appearance_mode {
        AppearanceMode::Light => light,
        AppearanceMode::Dark => dark,
    }
}

pub const fn if_light_hex(appearance_mode: AppearanceMode, light: u32, dark: u32) -> u32 {
    match appearance_mode {
        AppearanceMode::Light => light,
        AppearanceMode::Dark => dark,
    }
}

pub const fn blend_hex(
    foreground: u32,
    background: u32,
    foreground_weight: u32,
    total_weight: u32,
) -> u32 {
    let background_weight = total_weight.saturating_sub(foreground_weight);
    let red = blend_channel(
        (foreground >> 16) & 0xff,
        (background >> 16) & 0xff,
        foreground_weight,
        background_weight,
        total_weight,
    );
    let green = blend_channel(
        (foreground >> 8) & 0xff,
        (background >> 8) & 0xff,
        foreground_weight,
        background_weight,
        total_weight,
    );
    let blue = blend_channel(
        foreground & 0xff,
        background & 0xff,
        foreground_weight,
        background_weight,
        total_weight,
    );
    (red << 16) | (green << 8) | blue
}

const fn blend_channel(
    foreground: u32,
    background: u32,
    foreground_weight: u32,
    background_weight: u32,
    total_weight: u32,
) -> u32 {
    if total_weight == 0 {
        return foreground;
    }
    (foreground * foreground_weight + background * background_weight + total_weight / 2)
        / total_weight
}

#[derive(Clone, Copy, PartialEq)]
pub struct SurfaceColorSpec {
    pub hex: u32,
    pub opacity: f32,
}

impl SurfaceColorSpec {
    pub const fn new(hex: u32, opacity: f32) -> Self {
        Self { hex, opacity }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct SurfaceTheme {
    pub app_bg: u32,
    pub elevated_surface_bg: u32,
    pub surface_border: SurfaceColorSpec,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub text_hint: u32,
    pub command_menu_active_bg: SurfaceColorSpec,
    pub card_fill: u32,
    pub card_border: SurfaceColorSpec,
    pub card_shadow: SurfaceColorSpec,
    pub favorite_active_bg: SurfaceColorSpec,
    pub page_icon_bg: SurfaceColorSpec,
}

impl SurfaceTheme {
    pub const fn for_appearance_mode(appearance_mode: AppearanceMode) -> Self {
        match appearance_mode {
            AppearanceMode::Light => Self {
                app_bg: 0xffffff,
                elevated_surface_bg: 0xffffff,
                surface_border: SurfaceColorSpec::new(0x37352f, 0.075),
                text_primary: 0x2c2c2b,
                text_secondary: 0x4a4947,
                text_muted: 0x9b9a97,
                text_hint: 0x9b9a97,
                command_menu_active_bg: SurfaceColorSpec::new(0x37352f, 0.08),
                card_fill: 0xffffff,
                card_border: SurfaceColorSpec::new(0x37352f, 0.05),
                card_shadow: SurfaceColorSpec::new(0x37352f, 0.014),
                favorite_active_bg: SurfaceColorSpec::new(0x2a1c00, 0.07),
                page_icon_bg: SurfaceColorSpec::new(0x37352f, 0.04),
            },
            AppearanceMode::Dark => Self {
                app_bg: DARK_APP_BACKGROUND,
                elevated_surface_bg: 0x252525,
                surface_border: SurfaceColorSpec::new(0xffffff, 0.08),
                text_primary: 0xf0efed,
                text_secondary: 0xd8d6d1,
                text_muted: 0xada9a3,
                text_hint: 0x65645e,
                command_menu_active_bg: SurfaceColorSpec::new(0xffffff, 0.06),
                card_fill: 0x2c2c2b,
                card_border: SurfaceColorSpec::new(0xfffff3, 0.082),
                card_shadow: SurfaceColorSpec::new(0x191919, 0.08),
                favorite_active_bg: SurfaceColorSpec::new(0xffffff, 0.08),
                page_icon_bg: SurfaceColorSpec::new(0xffffff, 0.04),
            },
        }
    }
}

impl Default for SurfaceTheme {
    fn default() -> Self {
        Self::for_appearance_mode(AppearanceMode::Dark)
    }
}
