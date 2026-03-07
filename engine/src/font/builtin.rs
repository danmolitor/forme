//! Builtin Unicode fallback fonts.
//!
//! Noto Sans Regular (400) and Bold (700) are bundled so that non-Latin
//! text (Cyrillic, Greek, math symbols, etc.) renders correctly without
//! any user-side font configuration.

const NOTO_SANS_REGULAR: &[u8] = include_bytes!("../../fonts/NotoSans-Regular.ttf");
const NOTO_SANS_BOLD: &[u8] = include_bytes!("../../fonts/NotoSans-Bold.ttf");

/// Register the builtin Noto Sans fonts with a font registry.
pub fn register_builtin_fonts(registry: &mut super::FontRegistry) {
    registry.register("Noto Sans", 400, false, NOTO_SANS_REGULAR.to_vec());
    registry.register("Noto Sans", 700, false, NOTO_SANS_BOLD.to_vec());
}
