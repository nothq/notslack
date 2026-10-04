use crate::ui::AppearanceMode;

mod overlay;
mod results;

#[derive(Clone, Copy)]
struct SlackSearchOverlayPalette {
    surface: u32,
    border: u32,
    text: u32,
    strong_text: u32,
    secondary_text: u32,
    selected_bg: u32,
    hover_bg: u32,
    key_bg: u32,
    key_hover_bg: u32,
}

fn slack_search_overlay_palette(appearance_mode: AppearanceMode) -> SlackSearchOverlayPalette {
    match appearance_mode {
        AppearanceMode::Light => SlackSearchOverlayPalette {
            surface: 0xffffff,
            border: 0xd8d8d8,
            text: 0x1d1c1d,
            strong_text: 0x1d1c1d,
            secondary_text: 0x5e5d60,
            selected_bg: 0xe8e8e8,
            hover_bg: 0xf4f4f4,
            key_bg: 0xe8e8e8,
            key_hover_bg: 0xd8d8d8,
        },
        AppearanceMode::Dark => SlackSearchOverlayPalette {
            surface: 0x1a1d21,
            border: 0x34373b,
            text: 0xd1d2d3,
            strong_text: 0xf8f8f8,
            secondary_text: 0xb9babd,
            selected_bg: 0x222529,
            hover_bg: 0x25282d,
            key_bg: 0x34373b,
            key_hover_bg: 0x42454a,
        },
    }
}

fn consume_slack_search_action_key(
    event: &gpui::KeyDownEvent,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> bool {
    let key = event.keystroke.key.as_str();
    if (key == "enter" || key == "space") && !event.keystroke.modifiers.modified() {
        window.prevent_default();
        cx.stop_propagation();
        return true;
    }
    false
}
