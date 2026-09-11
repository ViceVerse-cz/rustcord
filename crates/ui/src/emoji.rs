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
	ctx.data_mut(|data| data.insert_temp(egui::Id::unique("twemoji"), texture));
	entries();
	Ok(())
}

pub(crate) fn ready(ctx: &Context) -> bool {
	ctx.data(|data| {
		data.get_temp::<TextureHandle>(egui::Id::unique("twemoji"))
			.is_some()
	})
}

/// Startup decoding uses a single bounded worker; the context is thread-safe.
pub fn install_async(ctx: &Context) -> std::io::Result<()> {
	let ctx = ctx.clone();
	std::thread::Builder::new()
		.name("emoji-atlas".into())
		.spawn(move || {
			if install(&ctx).is_ok() {
				ctx.request_repaint();
			}
		})
		.map(|_| ())
}

pub(crate) fn inline_size(ui: &egui::Ui) -> f32 {
	egui::TextStyle::Body.resolve(ui.style()).size * 1.6
}

pub(crate) fn lookup(text: &str) -> Option<usize> {
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
	let texture = ctx.data(|data| data.get_temp::<TextureHandle>(egui::Id::unique("twemoji")))?;
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

/// Keep original text in egui's selection model while drawing artwork in its place.
pub(crate) fn selectable(
	ui: &mut egui::Ui,
	text: &str,
	image: impl FnOnce(&mut egui::Ui) -> Option<Image<'static>>,
	size: f32,
	link: bool,
) -> egui::Response {
	type Cache = std::sync::Arc<
		std::sync::Mutex<std::collections::HashMap<egui::Id, std::sync::Arc<egui::Galley>>>,
	>;
	let key = egui::Id::unique((
		"emoji-selectable",
		text,
		size.to_bits(),
		ui.ctx().pixels_per_point().to_bits(),
		ui.ctx().fonts(|fonts| fonts.definitions().font_data.len()),
	));
	let cache = ui.ctx().data_mut(|data| {
		data.get_temp_mut_or_default::<Cache>(egui::Id::unique("emoji-selection-cache"))
			.clone()
	});
	let cached = cache.lock().expect("emoji cache").get(&key).cloned();
	let galley = cached.unwrap_or_else(|| {
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
		// At most 64 short emoji layouts, independent of message history length.
		if text.len() <= 128 {
			let mut cache = cache.lock().expect("emoji cache");
			if cache.len() >= 64 {
				cache.clear();
			}
			cache.insert(key, galley.clone());
		}
		galley
	});
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
