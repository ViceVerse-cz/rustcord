//! Bundled Twemoji. One fixed atlas, no runtime requests or per-message image cache.
use egui::{Context, Image, TextureHandle};
use std::sync::OnceLock;

const ATLAS: &[u8] = include_bytes!("../../../assets/twemoji/atlas.png");
const INDEX: &str = include_str!("../../../assets/twemoji/index.tsv");
static ENTRIES: OnceLock<Vec<(&'static str, usize)>> = OnceLock::new();

fn entries() -> &'static [(&'static str, usize)] {
    ENTRIES.get_or_init(|| {
        INDEX
            .lines()
            .map(|line| {
                let (text, index) = line.split_once('\t').expect("bundled emoji index");
                (text, index.parse().expect("bundled atlas cell"))
            })
            .collect()
    })
}

/// Decode once during application creation, outside the render callback.
pub fn install(ctx: &Context) -> Result<(), image::ImageError> {
    let image = image::load_from_memory_with_format(ATLAS, image::ImageFormat::Png)?.into_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let texture = ctx.load_texture(
        "Twemoji 17.0.3",
        egui::ColorImage::from_rgba_unmultiplied(size, &image),
        egui::TextureOptions::LINEAR,
    );
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("twemoji"), texture));
    entries();
    Ok(())
}

fn lookup(text: &str) -> Option<usize> {
    // Explicit text presentation must stay text. Do not partially match unknown sequences.
    if text.is_ascii() || text.contains('\u{fe0e}') || text.len() > 128 {
        return None;
    }
    let normalized;
    let key = if text.contains('\u{fe0f}') {
        normalized = text.replace('\u{fe0f}', "");
        normalized.as_str()
    } else {
        text
    };
    entries()
        .binary_search_by_key(&key, |(name, _)| *name)
        .ok()
        .map(|index| entries()[index].1)
}

pub(crate) fn image(ctx: &Context, text: &str, size: f32) -> Option<Image<'static>> {
    let cell = lookup(text)?;
    let texture = ctx.data(|data| data.get_temp::<TextureHandle>(egui::Id::new("twemoji")))?;
    let [width, height] = texture.size();
    let x = (cell % 64) as f32 * 32.0;
    let y = (cell / 64) as f32 * 32.0;
    let uv = egui::Rect::from_min_max(
        egui::pos2(x / width as f32, y / height as f32),
        egui::pos2((x + 32.0) / width as f32, (y + 32.0) / height as f32),
    );
    Some(
        Image::new((texture.id(), egui::Vec2::splat(size)))
            .uv(uv)
            .alt_text(text),
    )
}

pub(crate) fn button(ctx: &Context, emoji: &str, text: String) -> egui::Button<'static> {
    if let Some(image) = image(ctx, emoji, 18.0) {
        egui::Button::image_and_text(image, text).image_tint_follows_text_color(false)
    } else {
        egui::Button::new(format!("{emoji} {text}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atlas_is_bounded_and_matches_complete_sequences() {
        let image = image::load_from_memory(ATLAS).unwrap();
        assert!(image.width() as usize * image.height() as usize * 4 <= 16 * 1024 * 1024);
        assert!(ATLAS.len() <= 8 * 1024 * 1024);
        assert!(entries().len() <= 4096);
        assert!(entries().windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert!(
            entries()
                .iter()
                .all(|(_, cell)| *cell < (image.width() / 32 * (image.height() / 32)) as usize)
        );
        for text in ["👍", "👍🏽", "❤️", "👩🏽‍💻", "🇨🇿", "1️⃣", "🏳️‍🌈", "🏳‍🌈", "👩‍⚕", "👨‍👩‍👧‍👦"]
        {
            assert!(lookup(text).is_some(), "missing {text}");
        }
        for text in ["hello", "1", "©\u{fe0e}", "👩\u{200d}🦀", "<:custom:123>"] {
            assert!(lookup(text).is_none(), "unexpected {text}");
        }
    }
}
