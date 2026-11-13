//! The color emoji font text falls back to on Linux.
//!
//! GPUI draws text on Linux with swash, which renders COLRv0 and bitmap color
//! fonts but not COLRv1. Distributions now ship Noto Color Emoji as COLRv1
//! only, so its emoji come out blank. notslack bundles Twemoji Mozilla, a
//! COLRv0 font, and lists it ahead of the system fallbacks.
//!
//! GPUI draws a font in color only when its PostScript name is
//! `NotoColorEmoji`, and drops any font without a glyph for `m`, so the
//! bundled copy is renamed and maps `m` to an empty glyph (see
//! `crates/notslack/assets/fonts/TwemojiMozilla-LICENSE.md`).

use gpui::FontFallbacks;

/// The family name of the bundled emoji font.
pub const EMOJI_FONT_FAMILY: &str = "Twemoji Mozilla";

/// The fallbacks every text style carries: the bundled emoji font on Linux,
/// none elsewhere, where the system emoji font renders.
pub fn emoji_font_fallbacks() -> Option<FontFallbacks> {
    #[cfg(target_os = "linux")]
    {
        static FALLBACKS: std::sync::OnceLock<FontFallbacks> = std::sync::OnceLock::new();
        Some(
            FALLBACKS
                .get_or_init(|| FontFallbacks::from_fonts(vec![EMOJI_FONT_FAMILY.to_string()]))
                .clone(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// The bundled Twemoji has the emoji of Unicode 14.0; newer ones would come
/// out blank or as their component parts.
#[cfg(target_os = "linux")]
const EMOJI_FONT_UNICODE_VERSION: emojis::UnicodeVersion = emojis::UnicodeVersion::new(14, 0);

/// Whether the emoji font draws `emoji`, so pickers can leave out the ones it
/// cannot.
pub fn emoji_font_draws(emoji: &emojis::Emoji) -> bool {
    #[cfg(target_os = "linux")]
    {
        emoji.unicode_version() <= EMOJI_FONT_UNICODE_VERSION
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = emoji;
        true
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::emoji_font_draws;

    #[test]
    fn emoji_newer_than_the_bundled_font_are_left_out() {
        assert!(emoji_font_draws(emojis::get("🫠").unwrap()));
        assert!(!emoji_font_draws(emojis::get("🫨").unwrap()));
        assert!(!emoji_font_draws(emojis::get("🙂‍↔️").unwrap()));
    }
}
