//! Bundled OFL fallback faces. No runtime download, system-font scan, or disk I/O.
use egui::{Context, FontData, FontDefinitions, FontFamily};

const CJK: &[u8] = include_bytes!("../../../assets/fonts/NotoSansCJKjp-Regular.otf");
const ARABIC: &[u8] = include_bytes!("../../../assets/fonts/NotoSansArabic.ttf");

/// Install once during application creation, before the first UI pass.
pub fn install(ctx: &Context) {
    ctx.set_fonts(definitions());
}

fn definitions() -> FontDefinitions {
    let mut definitions = FontDefinitions::default();
    for (name, data) in [("Noto Sans CJK JP", CJK), ("Noto Sans Arabic", ARABIC)] {
        definitions
            .font_data
            .insert(name.into(), FontData::from_static(data).into());
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            definitions
                .families
                .entry(family)
                .or_default()
                .push(name.into());
        }
    }
    // ponytail: one Japanese CJK face bounds asset cost; add regional Han faces
    // when locale-specific glyph forms are implemented and measured.
    definitions
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::FontId;
    use skrifa::MetadataProvider;

    #[test]
    fn bundled_fallbacks_cover_multilingual_text_with_a_fixed_asset_budget() {
        assert!(CJK.len() + ARABIC.len() <= 17 * 1024 * 1024);
        let definitions = definitions();
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            let faces: Vec<_> = definitions.families[&family]
                .iter()
                .map(|name| {
                    let data = &definitions.font_data[name];
                    skrifa::FontRef::from_index(&data.font, data.index).expect("valid bundled font")
                })
                .collect();
            for c in "Hello, 日本語かなカナ 中文汉字繁體 한국어 العربية مَرْحَبًا é e\u{301}".chars()
            {
                assert!(
                    faces.iter().any(|face| {
                        face.charmap()
                            .map(c)
                            .is_some_and(|id| id != skrifa::GlyphId::NOTDEF)
                    }),
                    "missing glyph: {c} ({c:?})"
                );
            }
        }
        let ctx = Context::default();
        install(&ctx);
        let output = ctx.run_ui(Default::default(), |ui| {
            ui.fonts_mut(|fonts| {
                for family in [FontFamily::Proportional, FontFamily::Monospace] {
                    let font = FontId::new(14.0, family);
                    // egui 0.36.2 has_glyph incorrectly returns false for all
                    // primary-face glyphs. Check every scalar through its font
                    // parser above, then check the actual fallback path here.
                    for c in "日本語かなカナ中文汉字繁體한국어العربية".chars()
                    {
                        assert!(fonts.has_glyph(&font, c), "missing glyph: {c} ({c:?})");
                    }
                }
            });
        });
        output.drop_without_applying_deltas();
    }
}
