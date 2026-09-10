//! Bundled Twemoji plus bounded OS color fallback. No Unicode emoji network requests.
use egui::{Context, Image, TextureHandle};
use std::sync::OnceLock;

const ATLAS: &[u8] = include_bytes!("../../../assets/twemoji/atlas.png");
const INDEX: &str = include_str!("../../../assets/twemoji/index.tsv");
static ENTRIES: OnceLock<Vec<(&'static str, usize)>> = OnceLock::new();
#[cfg(target_os = "macos")]
mod native;
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod native_macos;

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
    #[cfg(target_os = "macos")]
    native::install(ctx);
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
    let Some(cell) = lookup(text) else {
        #[cfg(target_os = "macos")]
        return native::image(ctx, text, size);
        #[cfg(not(target_os = "macos"))]
        return None;
    };
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

/// A single truncated label, with the same color emoji artwork as messages.
/// Keep its full original accessible name even when the visual text is elided.
pub(crate) fn label(
    ui: &mut egui::Ui,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
) -> egui::Response {
    label_rows(ui, text, font, color, 1)
}

pub(crate) fn label_wrapped(
    ui: &mut egui::Ui,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
) -> egui::Response {
    label_rows(ui, text, font, color, usize::MAX)
}

fn label_rows(
    ui: &mut egui::Ui,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
    rows: usize,
) -> egui::Response {
    use unicode_segmentation::UnicodeSegmentation;
    let size = font.size + 2.0;
    let format = egui::text::TextFormat::simple(font, color);
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = ui.available_width();
    job.wrap.max_rows = rows;
    let mut artwork = Vec::new();
    let mut index: usize = 0;
    let mut plain = String::new();
    for cluster in text.graphemes(true) {
        if let Some(image) = image(ui.ctx(), cluster, size) {
            job.append(&plain, 0.0, format.clone());
            plain.clear();
            let mut slot_font = format.font_id.clone();
            let advance = ui.fonts_mut(|f| f.glyph_width(&slot_font, '\u{a0}'));
            slot_font.size *= size / advance.max(1.0);
            job.append(
                "\u{a0}",
                0.0,
                egui::text::TextFormat {
                    font_id: slot_font,
                    color: egui::Color32::TRANSPARENT,
                    line_height: Some(size),
                    ..format.clone()
                },
            );
            artwork.push((index, image));
            index += 1;
        } else {
            plain.push_str(cluster);
            index += cluster.chars().count();
        }
    }
    job.append(&plain, 0.0, format);
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    let response = ui.add(
        egui::Label::new(galley.clone())
            .selectable(false)
            .show_tooltip_when_elided(false),
    );
    let response = if galley.elided {
        response.on_hover_text(text)
    } else {
        response
    };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(response.rect));
    child.set_clip_rect(ui.clip_rect().intersect(response.rect));
    let mut row_start = 0;
    for row in &galley.rows {
        for (index, image) in &artwork {
            if let Some(glyph) = index
                .checked_sub(row_start)
                .and_then(|i| row.glyphs.get(i))
                .filter(|g| g.chr == '\u{a0}')
            {
                let rect = egui::Rect::from_min_size(
                    egui::pos2(
                        response.rect.left() + row.pos.x + glyph.pos.x,
                        response.rect.top() + row.pos.y + (row.size.y - size) / 2.0,
                    ),
                    egui::Vec2::splat(size),
                );
                image.paint_at(&child, rect);
            }
        }
        row_start += row.glyphs.len() + usize::from(row.ends_with_newline);
    }
    response
        .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), text));
    response
}

/// Keep original text in egui's selection model while drawing artwork in its place.
pub(crate) fn selectable(
    ui: &mut egui::Ui,
    text: &str,
    image: impl FnOnce(&mut egui::Ui) -> Option<Image<'static>>,
    size: f32,
    link: bool,
) -> egui::Response {
    let mut galley = ui.fonts_mut(|fonts| {
        fonts.layout_no_wrap(
            text.to_owned(),
            egui::FontId::proportional(size),
            egui::Color32::TRANSPARENT,
        )
    });
    let glyphs = std::sync::Arc::make_mut(&mut galley);
    glyphs.rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::splat(size));
    glyphs.mesh_bounds = glyphs.rect;
    glyphs.num_vertices = 0;
    glyphs.num_indices = 0;
    for placed in &mut glyphs.rows {
        let row = std::sync::Arc::make_mut(&mut placed.row);
        // One hit target: selection endpoints must never split a Unicode sequence or markup.
        for glyph in &mut row.glyphs {
            glyph.pos.x = 0.0;
            glyph.advance_width = size;
            glyph.first_vertex = 0;
        }
        row.size = egui::Vec2::splat(size);
        row.visuals = Default::default();
    }
    let mut label = egui::Label::new(galley).selectable(true);
    if link {
        label = label.sense(egui::Sense::click());
    }
    let response = ui.add(label);
    if let Some(image) = ui
        .is_rect_visible(response.rect)
        .then(|| image(ui))
        .flatten()
    {
        let size = image.calc_size(response.rect.size(), image.size());
        image.paint_at(
            ui,
            egui::Rect::from_center_size(response.rect.center(), size),
        );
    } else {
        ui.painter().text(
            response.rect.center(),
            egui::Align2::CENTER_CENTER,
            "?",
            egui::FontId::proportional(size),
            ui.visuals().weak_text_color(),
        );
    }
    let response = response.on_hover_text(text);
    response.context_menu(|ui| {
        if ui.button("Copy emoji").clicked() {
            ui.ctx().copy_text(text.to_owned());
            ui.close();
        }
    });
    response
}

pub(crate) fn custom_prefix(text: &str) -> Option<(model::Id, usize)> {
    let body = text
        .strip_prefix("<:")
        .or_else(|| text.strip_prefix("<a:"))?;
    let end = body.as_bytes().iter().take(54).position(|b| *b == b'>')?;
    let (name, id) = body[..end].split_once(':')?;
    if !(2..=32).contains(&name.len())
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return None;
    }
    Some((id.parse().ok()?, text.len() - body.len() + end + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_labels_draw_color_emoji_and_clip_long_narrow_names() {
        let ctx = Context::default();
        install(&ctx).unwrap();
        let texture = ctx
            .data(|data| data.get_temp::<TextureHandle>(egui::Id::new("twemoji")))
            .unwrap();
        let mut bounds = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.set_width(90.0);
            bounds = label(
                ui,
                "🌙 night-chat-with-a-long-name 👍",
                egui::FontId::proportional(15.0),
                egui::Color32::WHITE,
            )
            .rect;
        });
        assert!(bounds.width() <= 90.0);
        output.textures_delta.clear();
        let artwork: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect) if rect.fill_texture_id() == texture.id() => {
                    Some((shape.clip_rect, rect.rect))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            artwork.len(),
            1,
            "the elided final emoji must not be painted"
        );
        assert!(bounds.contains_rect(artwork[0].1));
        assert!(bounds.contains_rect(artwork[0].0));
        output.drop_without_applying_deltas();
    }
    #[test]
    fn wrapped_label_keeps_emoji_slots_at_row_starts_and_ends() {
        let ctx = Context::default();
        install(&ctx).unwrap();
        let texture = ctx
            .data(|data| data.get_temp::<TextureHandle>(egui::Id::new("twemoji")))
            .unwrap();
        let mut bounds = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.set_width(60.0);
            bounds = label_wrapped(
                ui,
                "a long 🌙 status\n👍",
                egui::FontId::proportional(15.0),
                egui::Color32::WHITE,
            )
            .rect;
        });
        output.textures_delta.clear();
        let artwork: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect) if rect.fill_texture_id() == texture.id() => {
                    Some(rect.rect)
                }
                _ => None,
            })
            .collect();
        assert_eq!(artwork.len(), 2);
        assert!(artwork.iter().all(|rect| bounds.contains_rect(*rect)));
        assert!(artwork[1].top() >= artwork[0].bottom() - 0.1, "{artwork:?}");
        output.drop_without_applying_deltas();
    }
    #[test]
    fn custom_markup_is_bounded_and_never_an_arbitrary_url() {
        for token in ["<:serein_wave:9001>", "<a:serein_party:9001>"] {
            assert_eq!(
                custom_prefix(&format!("{token} trailing")),
                Some((model::Id(9001), token.len()))
            );
        }
        for token in [
            "<:x:1>",
            "<:hello:0>",
            "<:hello:-1>",
            "<:hello:18446744073709551616>",
            "<:../x:1>",
            "<:hello:1/2>",
            "<a:hello:1",
            "<:hello:https://example.com>",
        ] {
            assert!(custom_prefix(token).is_none(), "{token}");
        }
    }
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
