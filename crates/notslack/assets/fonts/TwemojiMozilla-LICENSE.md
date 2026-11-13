# Twemoji Mozilla

`TwemojiMozilla.ttf` is Twemoji Mozilla 0.7.0
(https://github.com/mozilla/twemoji-colr/releases/tag/v0.7.0), bundled on
Linux as the color emoji font. It is modified in two ways so GPUI's Linux text
system renders it (see `crates/chat/src/ui/emoji_font.rs`):

- its PostScript name is `NotoColorEmoji`, the only name GPUI draws in color;
- `m` maps to the empty U+200D glyph, because GPUI drops fonts without an `m`.

The emoji artwork is Twemoji, copyright Twitter, Inc and other contributors,
licensed under CC-BY 4.0: https://creativecommons.org/licenses/by/4.0/

The font build code is copyright Mozilla Foundation, licensed under the
Apache License 2.0: https://www.apache.org/licenses/LICENSE-2.0
