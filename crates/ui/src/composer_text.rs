//! Inline artwork with the original wire text retained in egui's editing/undo model.
use crate::{avatars::Avatars, emoji};
use egui::{Color32, Image, text::CCursor, text::LayoutJob, text::TextFormat};
use model::User;
use std::{
	hash::{DefaultHasher, Hash, Hasher},
	ops::Range,
	sync::Arc,
};
use unicode_segmentation::UnicodeSegmentation;

struct Inline {
	source: Range<usize>,
	projected: usize,
	width: f32,
	label: Option<Arc<egui::Galley>>,
	image: Option<Image<'static>>,
}

#[derive(Default)]
pub(crate) struct Layout {
	inlines: Vec<Inline>,
	cache: Option<(u64, Arc<egui::Galley>)>,
}

impl Layout {
	pub fn galley(
		&mut self,
		ui: &egui::Ui,
		text: &str,
		width: f32,
		users: &[User],
		avatars: &mut Avatars,
		demo: bool,
	) -> Arc<egui::Galley> {
		let mut key = DefaultHasher::new();
		text.hash(&mut key);
		width.to_bits().hash(&mut key);
		ui.ctx()
			.fonts(|fonts| fonts.definitions().font_data.len())
			.hash(&mut key);
		ui.ctx().pixels_per_point().to_bits().hash(&mut key);
		egui::TextStyle::Body.resolve(ui.style()).hash(&mut key);
		let colors = crate::design::palette(ui);
		colors.text.hash(&mut key);
		colors.accent.hash(&mut key);
		colors.muted.hash(&mut key);
		avatars.revision.hash(&mut key);
		emoji::ready(ui.ctx()).hash(&mut key);
		demo.hash(&mut key);
		if text.contains("<:") || text.contains("<a:") {
			static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
			(START
				.get_or_init(std::time::Instant::now)
				.elapsed()
				.as_secs() / 60)
				.hash(&mut key);
		}
		for user in users {
			user.id.hash(&mut key);
			user.name.hash(&mut key);
		}
		let key = key.finish();
		if let Some((cached, galley)) = &self.cache
			&& *cached == key
		{
			return galley.clone();
		}
		self.inlines.clear();
		let font = egui::TextStyle::Body.resolve(ui.style());
		let colors = crate::design::palette(ui);
		let format = TextFormat::simple(font.clone(), colors.text);
		let size = font.size + 5.0;
		let mut job = LayoutJob::default();
		job.wrap.max_width = width;
		if text.is_empty() {
			job.append("", 0.0, format.clone());
		}
		let mut byte = 0;
		let mut source = 0;
		let mut projected = 0;
		while byte < text.len() {
			let tail = &text[byte..];
			let mut label = None;
			let mut image = None;
			let length = if let Some((id, len)) = model::user_mention_prefix(tail)
				&& let Some(user) = users.iter().find(|u| u.id == id)
			{
				label = Some((format!("@{}", user.name), colors.accent));
				len
			} else if let Some((id, len)) = emoji::custom_prefix(tail) {
				image = avatars.custom_image(ui.ctx(), id, size, demo);
				// A useful name remains visible while artwork is unavailable/loading.
				if image.is_none() {
					let name = tail[..len]
						.trim_start_matches("<a:")
						.trim_start_matches("<:")
						.split(':')
						.next()
						.unwrap_or("emoji");
					label = Some((format!(":{name}:"), colors.muted));
				}
				len
			} else {
				let grapheme = tail.graphemes(true).next().expect("nonempty tail");
				image = emoji::image(ui.ctx(), grapheme, size);
				grapheme.len()
			};
			let label = label.map(|(text, color)| {
				let mut job = LayoutJob::simple_singleline(text, font.clone(), color);
				job.wrap.max_width = (width - 6.0).max(1.0);
				job.wrap.max_rows = 1;
				ui.fonts_mut(|f| f.layout_job(job))
			});
			let raw = &tail[..length];
			let count = raw.chars().count();
			if label.is_some() || image.is_some() {
				let slot = label.as_ref().map_or(size, |g| g.size().x + 6.0);
				// One zero-width glyph plus leading space forms an unbroken inline object.
				// Expand its character slots below, so native selection/copy/undo use wire text.
				job.append(
					"\u{200b}",
					slot,
					TextFormat {
						color: Color32::TRANSPARENT,
						line_height: Some(size),
						..format.clone()
					},
				);
				self.inlines.push(Inline {
					source: source..source + count,
					projected,
					width: slot,
					label,
					image,
				});
				projected += 1;
			} else {
				job.append(raw, 0.0, format.clone());
				projected += count;
			}
			byte += length;
			source += count;
		}
		if self.inlines.is_empty() {
			let galley = ui.fonts_mut(|f| f.layout_job(job));
			self.cache = Some((key, galley.clone()));
			return galley;
		}
		let mut galley = ui.fonts_mut(|f| f.layout_job(job));
		let galley_mut = Arc::make_mut(&mut galley);
		let chars: Vec<_> = text.chars().collect();
		let mut projected = 0;
		let mut next = 0;
		for placed in &mut galley_mut.rows {
			let row = Arc::make_mut(&mut placed.row);
			let mut glyphs = Vec::with_capacity(row.glyphs.len());
			for glyph in &row.glyphs {
				if let Some(inline) = self.inlines.get(next).filter(|i| i.projected == projected) {
					let count = inline.source.len();
					for (index, chr) in chars[inline.source.clone()].iter().enumerate() {
						let mut slot = *glyph;
						slot.chr = *chr;
						slot.pos.x =
							glyph.pos.x - inline.width + inline.width * index as f32 / count as f32;
						slot.advance_width = inline.width / count as f32;
						glyphs.push(slot);
					}
					next += 1;
				} else {
					glyphs.push(*glyph);
				}
				projected += 1;
			}
			row.glyphs = glyphs;
			projected += usize::from(placed.ends_with_newline);
		}
		// TextEdit's galley and TextBuffer must expose exactly the same original characters.
		galley_mut.job = Arc::new(LayoutJob::simple(text.to_owned(), font, colors.text, width));
		self.cache = Some((key, galley.clone()));
		galley
	}

	pub fn paint(&self, ui: &mut egui::Ui, output: &egui::text_edit::TextEditOutput) {
		let painter = ui.painter().with_clip_rect(output.text_clip_rect);
		for inline in &self.inlines {
			let mut cursor = CCursor::new(inline.source.start);
			cursor.prefer_next_row = true;
			let position = output
				.galley
				.pos_from_cursor(cursor)
				.translate(output.galley_pos.to_vec2());
			let rect = egui::Rect::from_min_size(
				position.min,
				egui::vec2(inline.width, position.height()),
			);
			if !rect.intersects(output.text_clip_rect) {
				continue;
			}
			if let Some(label) = &inline.label {
				painter.rect_filled(
					rect,
					3,
					crate::design::palette(ui).accent.gamma_multiply(0.12),
				);
				painter.galley(
					egui::pos2(rect.left() + 3.0, rect.center().y - label.size().y / 2.0),
					label.clone(),
					Color32::PLACEHOLDER,
				);
			} else if let Some(image) = &inline.image {
				let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
				child.set_clip_rect(output.text_clip_rect);
				image.paint_at(
					&child,
					egui::Rect::from_center_size(
						rect.center(),
						image.calc_size(egui::Vec2::splat(inline.width), image.size()),
					),
				);
			}
		}
	}

	/// Keep mouse/arrow selection endpoints outside encoded inline objects.
	pub fn snap_cursor(&self, output: &mut egui::text_edit::TextEditOutput, ctx: &egui::Context) {
		let Some(mut range) = output.cursor_range else {
			return;
		};
		let original = range;
		let direction = ctx.input(|i| {
			if i.key_pressed(egui::Key::ArrowLeft) {
				-1
			} else if i.key_pressed(egui::Key::ArrowRight) {
				1
			} else {
				0
			}
		});
		for cursor in [&mut range.primary, &mut range.secondary] {
			for inline in &self.inlines {
				let index = cursor.index.0;
				if inline.source.start < index && index < inline.source.end {
					cursor.index = if direction < 0
						|| (direction == 0
							&& index - inline.source.start < inline.source.end - index)
					{
						inline.source.start.into()
					} else {
						inline.source.end.into()
					};
					break;
				}
			}
		}
		if range != original {
			output.cursor_range = Some(range);
			output.state.cursor.set_char_range(Some(range));
			output.state.clone().store(ctx, output.response.id);
		}
	}

	/// Let the native editor delete a selected token, preserving its normal undo history.
	pub fn select_deleted_inline(&self, ctx: &egui::Context, id: egui::Id) {
		if !ctx.memory(|m| m.has_focus(id)) {
			return;
		}
		let Some(mut state) = egui::text_edit::TextEditState::load(ctx, id) else {
			return;
		};
		let Some(range) = state.cursor.char_range().filter(|r| r.is_empty()) else {
			return;
		};
		let (back, forward) = ctx.input(|i| {
			(
				i.key_pressed(egui::Key::Backspace),
				i.key_pressed(egui::Key::Delete),
			)
		});
		if let Some(inline) = self.inlines.iter().find(|i| {
			(back && i.source.end == range.primary.index.0)
				|| (forward && i.source.start == range.primary.index.0)
		}) {
			state
				.cursor
				.set_char_range(Some(egui::text::CCursorRange::two(
					CCursor::new(inline.source.start),
					CCursor::new(inline.source.end),
				)));
			state.store(ctx, id);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn users() -> Vec<User> {
		vec![User {
			id: model::Id(42),
			name: "Zoë".into(),
			avatar: None,
			discriminator: 0,
		}]
	}

	#[test]
	fn nonsquare_emoji_keep_aspect_in_messages_and_composer() {
		for dimensions in [[64, 32], [32, 64]] {
			let ctx = egui::Context::default();
			let texture = ctx.load_texture(
				"synthetic-emoji",
				egui::ColorImage::filled(dimensions, Color32::WHITE),
				Default::default(),
			);
			let image = Image::new(&texture).fit_to_exact_size(egui::Vec2::splat(24.0));
			let mut layout = Layout::default();
			let mut avatars = Avatars::default();
			let mut text = "<:serein_leaf:9001>".to_owned();
			let mut output = ctx.run_ui(Default::default(), |ui| {
				emoji::selectable(ui, &text, |_| Some(image.clone()), 24.0, false);
				let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, width| {
					layout.galley(ui, buffer.as_str(), width, &[], &mut avatars, true)
				};
				let edit = egui::TextEdit::multiline(&mut text)
					.layouter(&mut layouter)
					.show(ui);
				layout.inlines[0].image = Some(image.clone());
				layout.paint(ui, &edit);
			});
			output.textures_delta.clear();
			let sizes: Vec<_> = output
				.shapes
				.iter()
				.filter_map(|shape| match &shape.shape {
					egui::Shape::Rect(rect) if rect.fill_texture_id() == texture.id() => {
						Some(rect.rect.size())
					}
					_ => None,
				})
				.collect();
			assert_eq!(sizes.len(), 2);
			for size in sizes {
				assert!(
					(size.x / size.y - dimensions[0] as f32 / dimensions[1] as f32).abs() < 0.01
				);
				assert!(size.x <= 24.0 && size.y <= 24.0);
			}
			output.drop_without_applying_deltas();
		}
	}

	#[test]
	fn artwork_keeps_wire_characters_and_cursor_geometry_across_wrapped_lines() {
		let ctx = egui::Context::default();
		emoji::install(&ctx).unwrap();
		let text = "Hi <@42> 👩🏽‍💻 <:serein_leaf:9001>\nagain <@!42> ❤️";
		let mut avatars = Avatars::default();
		let mut layout = Layout::default();
		let output = ctx.run_ui(Default::default(), |ui| {
			let galley = layout.galley(ui, text, 130.0, &users(), &mut avatars, true);
			assert_eq!(galley.job.text, text);
			let mut reconstructed = String::new();
			for row in &galley.rows {
				reconstructed.extend(row.glyphs.iter().map(|g| g.chr));
				if row.ends_with_newline {
					reconstructed.push('\n');
				}
				assert!(row.glyphs.iter().all(|g| g.pos.x >= -0.1));
				assert!(row.glyphs.windows(2).all(|g| g[0].pos.x <= g[1].pos.x));
			}
			assert_eq!(reconstructed, text);
			assert_eq!(layout.inlines.len(), 5);
			assert_eq!(
				layout.inlines.iter().filter(|i| i.image.is_some()).count(),
				3
			);
			assert_eq!(layout.inlines[0].label.as_ref().unwrap().job.text, "@Zoë");
			assert_eq!(galley.end().index.0, text.chars().count());
			for inline in &layout.inlines {
				let mut cursor = CCursor::new(inline.source.start);
				cursor.prefer_next_row = true;
				let start = galley.pos_from_cursor(cursor);
				cursor.index = inline.source.end.into();
				cursor.prefer_next_row = false;
				let end = galley.pos_from_cursor(cursor);
				assert!(
					(start.top() - end.top()).abs() < 0.1,
					"inline must not wrap internally"
				);
				assert!((end.left() - start.left() - inline.width).abs() < 1.0);
			}
		});
		output.drop_without_applying_deltas();
		assert!(avatars.take_requests().is_empty());
	}

	#[test]
	fn native_delete_undo_copy_and_ime_keep_wire_mentions() {
		let ctx = egui::Context::default();
		let mut avatars = Avatars::default();
		let mut layout = Layout::default();
		let mut text = "hi <@42>!".to_owned();
		let id = egui::Id::unique("rich-editor-test");
		let mut frame = |time, events: Vec<egui::Event>, cursor: Option<usize>| {
			if let Some(cursor) = cursor {
				ctx.memory_mut(|m| m.request_focus(id));
				let mut state = egui::text_edit::TextEditState::load(&ctx, id).unwrap_or_default();
				state
					.cursor
					.set_char_range(Some(egui::text::CCursorRange::one(CCursor::new(cursor))));
				state.store(&ctx, id);
			}
			let copying = events
				.iter()
				.any(|event| matches!(event, egui::Event::Copy));
			let composing = events
				.iter()
				.any(|event| matches!(event, egui::Event::Ime(_)));
			let output = ctx.run_ui(
				egui::RawInput {
					time: Some(time),
					events,
					..Default::default()
				},
				|ui| {
					layout.galley(ui, &text, 300.0, &users(), &mut avatars, true);
					layout.select_deleted_inline(&ctx, id);
					let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, width| {
						layout.galley(ui, buffer.as_str(), width, &users(), &mut avatars, true)
					};
					let mut edit = egui::TextEdit::multiline(&mut text)
						.id(id)
						.layouter(&mut layouter)
						.show(ui);
					if !composing {
						layout.snap_cursor(&mut edit, &ctx);
					}
					layout.paint(ui, &edit);
				},
			);
			if copying {
				assert!(output.platform_output.commands.iter().any(|command| {
					matches!(command, egui::OutputCommand::CopyText(copied) if copied == &text)
				}));
			}
			output.drop_without_applying_deltas();
			text.clone()
		};
		let key = |key, modifiers| egui::Event::Key {
			key,
			physical_key: None,
			pressed: true,
			repeat: false,
			modifiers,
		};
		assert_eq!(frame(0.0, vec![], Some(8)), "hi <@42>!");
		assert_eq!(
			frame(
				1.0,
				vec![key(egui::Key::Backspace, egui::Modifiers::NONE)],
				None
			),
			"hi !"
		);
		assert_eq!(
			frame(2.0, vec![key(egui::Key::Z, egui::Modifiers::COMMAND)], None),
			"hi <@42>!"
		);
		frame(
			2.1,
			vec![key(egui::Key::ArrowLeft, egui::Modifiers::NONE)],
			Some(8),
		);
		assert_eq!(
			egui::text_edit::TextEditState::load(&ctx, id)
				.unwrap()
				.cursor
				.char_range()
				.unwrap()
				.primary
				.index
				.0,
			3
		);
		frame(
			2.2,
			vec![key(egui::Key::ArrowRight, egui::Modifiers::NONE)],
			None,
		);
		assert_eq!(
			egui::text_edit::TextEditState::load(&ctx, id)
				.unwrap()
				.cursor
				.char_range()
				.unwrap()
				.primary
				.index
				.0,
			8
		);
		assert_eq!(
			frame(3.0, vec![egui::Event::Text("č".into())], Some(8)),
			"hi <@42>č!"
		);
		assert_eq!(
			frame(
				4.0,
				vec![egui::Event::Ime(egui::ImeEvent::Preedit {
					text: "ni".into(),
					active_range_chars: None,
				})],
				Some(9)
			),
			"hi <@42>čni!"
		);
		assert_eq!(
			frame(
				5.0,
				vec![egui::Event::Ime(egui::ImeEvent::Commit("你".into()))],
				None
			),
			"hi <@42>č你!"
		);
		assert_eq!(
			frame(
				6.0,
				vec![
					key(egui::Key::A, egui::Modifiers::COMMAND),
					egui::Event::Copy
				],
				None
			),
			"hi <@42>č你!"
		);
	}
}
