//! Search the bundled Unicode palette or the selected server's bounded catalog.
use crate::avatars::Avatars;
use client_core::State;
use model::Id;
use std::sync::OnceLock;

const NAMES: &str = include_str!("../../../assets/twemoji/names.tsv");
const CELL: f32 = 40.0;

/// Replace the composer's scalar-index selection without exceeding its character or RAM budget.
pub(crate) fn insert(
	draft: &mut String,
	text: &str,
	range: Option<egui::text::CCursorRange>,
	remaining: usize,
) -> Option<usize> {
	let count = draft.chars().count();
	let (start, end) = range.map_or((count, count), |range| {
		let range = range.as_sorted_char_range();
		(range.start.0.min(count), range.end.0.min(count))
	});
	let inserted = text.chars().count();
	if count - (end - start) + inserted > client_core::MAX_CONTENT {
		return None;
	}
	let byte_start = draft
		.char_indices()
		.nth(start)
		.map_or(draft.len(), |(i, _)| i);
	let byte_end = draft
		.char_indices()
		.nth(end)
		.map_or(draft.len(), |(i, _)| i);
	let bytes = draft.len() - (byte_end - byte_start) + text.len();
	let budget = draft.capacity().saturating_add(remaining);
	if bytes > budget {
		return None;
	}
	// draft_bytes measures capacity, so avoid String's geometric growth crossing the budget.
	if bytes > draft.capacity() {
		let mut replacement = String::with_capacity(bytes);
		if replacement.capacity() > budget {
			return None;
		}
		replacement.push_str(&draft[..byte_start]);
		replacement.push_str(text);
		replacement.push_str(&draft[byte_end..]);
		*draft = replacement;
	} else {
		draft.replace_range(byte_start..byte_end, text);
	}
	Some(start + inserted)
}

fn standard() -> &'static [(&'static str, &'static str)] {
	static ENTRIES: OnceLock<Vec<(&'static str, &'static str)>> = OnceLock::new();
	ENTRIES.get_or_init(|| {
		NAMES
			.lines()
			.map(|line| line.split_once('\t').expect("bundled emoji name"))
			.collect()
	})
}

pub(crate) struct Picker {
	open: bool,
	pending_open: bool,
	focus: bool,
	channel: Option<Id>,
	generation: u64,
	server: bool,
	query: String,
	matches: Vec<usize>,
}

impl Default for Picker {
	fn default() -> Self {
		// Initialize the static catalog during application creation, outside rendering.
		Self {
			open: false,
			pending_open: false,
			focus: false,
			channel: None,
			generation: 0,
			server: false,
			query: String::new(),
			matches: (0..standard().len()).collect(),
		}
	}
}

impl Picker {
	/// Fixture-only: open the popout on the next frame regardless of navigation resets.
	pub(crate) fn preview(&mut self) {
		self.pending_open = true;
	}
	fn filter(&mut self) {
		let query = self.query.trim().to_lowercase();
		self.matches.clear();
		self.matches.extend(
			standard()
				.iter()
				.enumerate()
				.filter(|(_, (text, name))| name.contains(&query) || text.contains(&query))
				.map(|(index, _)| index),
		);
	}

	pub fn show(
		&mut self,
		ui: &mut egui::Ui,
		state: &State,
		channel: Id,
		avatars: &mut Avatars,
	) -> Option<String> {
		if self.channel != Some(channel) || self.generation != state.generation {
			self.channel = Some(channel);
			self.generation = state.generation;
			self.open = false;
			self.server = false;
			self.query.clear();
			self.filter();
		}
		if std::mem::take(&mut self.pending_open) {
			self.open = true;
		}
		let trigger = crate::icons::toggle(
			ui,
			crate::icons::Icon::Smile,
			28.0,
			self.open,
			"Insert an emoji",
		);
		if trigger.clicked() {
			self.open = !self.open;
			self.focus = self.open;
		}
		if !self.open {
			return None;
		}
		if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
			self.open = false;
			trigger.request_focus();
			return None;
		}
		let colors = crate::design::palette(ui);
		let guild_id = state
			.channels
			.iter()
			.find(|c| c.id == channel)
			.and_then(|c| c.guild);
		let guild = guild_id.and_then(|id| state.guilds.iter().find(|g| g.id == id));
		if guild.is_none() {
			self.server = false;
		}
		let bounds = ui.ctx().content_rect().shrink(8.0);
		let width = WIDTH.min(bounds.width());
		let height = HEIGHT.min(bounds.height());
		// Anchor above the composer with the right edge on the trigger, like a Discord popout.
		let x = (trigger.rect.right() - width)
			.min(bounds.right() - width)
			.max(bounds.left());
		let y = (trigger.rect.top() - 8.0 - height).max(bounds.top());
		let mut selected = None;
		let mut hovered: Option<(Option<egui::Image<'static>>, String, String)> = None;
		let area = egui::Area::new(egui::Id::unique("emoji-picker"))
			.kind(egui::UiKind::Popup)
			.order(egui::Order::Foreground)
			.fixed_pos(egui::pos2(x, y))
			.constrain_to(bounds)
			.interactable(true)
			.show(ui.ctx(), |ui| {
				egui::Frame::new()
					.fill(colors.sidebar)
					.stroke(egui::Stroke::new(1.0, colors.border))
					.corner_radius(8)
					.shadow(egui::epaint::Shadow {
						offset: [0, 8],
						blur: 24,
						spread: 0,
						color: egui::Color32::from_black_alpha(96),
					})
					.show(ui, |ui| {
						let (rect, _) =
							ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
						ui.style_mut().interaction.selectable_labels = false;
						const TOP: f32 = 64.0;
						const FOOTER: f32 = 48.0;
						const RAIL: f32 = 48.0;
						let search_rect =
							egui::Rect::from_min_size(rect.min, egui::vec2(width, TOP));
						let footer_rect = egui::Rect::from_min_size(
							egui::pos2(rect.left(), rect.bottom() - FOOTER),
							egui::vec2(width, FOOTER),
						);
						let body_rect = egui::Rect::from_min_max(
							egui::pos2(rect.left(), search_rect.bottom()),
							egui::pos2(rect.right(), footer_rect.top()),
						);
						let rail_rect = egui::Rect::from_min_max(
							body_rect.min,
							egui::pos2(body_rect.left() + RAIL, body_rect.bottom()),
						);
						let grid_rect = egui::Rect::from_min_max(
							egui::pos2(rail_rect.right(), body_rect.top()),
							body_rect.max,
						);

						// Search field.
						ui.scope_builder(
							egui::UiBuilder::new().max_rect(search_rect.shrink(16.0)),
							|ui| {
								egui::Frame::new()
									.fill(colors.raised)
									.corner_radius(6)
									.stroke(egui::Stroke::new(1.0, colors.border))
									.inner_margin(egui::Margin::symmetric(10, 0))
									.show(ui, |ui| {
										ui.set_width(ui.available_width());
										ui.set_height(30.0);
										ui.horizontal_centered(|ui| {
											ui.spacing_mut().item_spacing.x = 8.0;
											crate::icons::inline(
												ui,
												crate::icons::Icon::Search,
												16.0,
												colors.muted,
											);
											let search = ui.add(
												egui::TextEdit::singleline(&mut self.query)
													.char_limit(64)
													.frame(egui::Frame::NONE)
													.hint_text("Find the perfect emoji")
													.desired_width(ui.available_width()),
											);
											search.widget_info(|| {
												egui::WidgetInfo::labeled(
													egui::WidgetType::TextEdit,
													true,
													"Search emoji by name",
												)
											});
											if self.focus {
												search.request_focus();
												self.focus = false;
											}
											if search.changed() {
												self.filter();
											}
										});
									});
							},
						);
						ui.painter().hline(
							rect.x_range(),
							search_rect.bottom(),
							egui::Stroke::new(1.0, colors.border),
						);

						// Category rail.
						ui.painter().rect_filled(rail_rect, 0, colors.base);
						ui.scope_builder(
							egui::UiBuilder::new()
								.max_rect(rail_rect.shrink2(egui::vec2(8.0, 8.0)))
								.layout(egui::Layout::top_down(egui::Align::Center)),
							|ui| {
								ui.spacing_mut().item_spacing.y = 6.0;
								let unicode = crate::icons::toggle(
									ui,
									crate::icons::Icon::Smile,
									32.0,
									!self.server,
									"Standard emoji",
								);
								if unicode.clicked() {
									self.server = false;
								}
								if let Some(guild) = guild {
									let (tab, response) = ui.allocate_exact_size(
										egui::Vec2::splat(32.0),
										egui::Sense::click(),
									);
									if self.server || response.hovered() || response.has_focus() {
										ui.painter().rect_filled(tab, 6, colors.hover);
									}
									let inner = tab.shrink(3.0);
									ui.scope_builder(
										egui::UiBuilder::new().max_rect(inner),
										|ui| {
											avatars.show_icon(
												ui,
												guild.icon_key(),
												inner.width(),
												state.demo,
												&guild.name,
											);
										},
									);
									response.widget_info(|| {
										egui::WidgetInfo::selected(
											egui::WidgetType::Button,
											true,
											self.server,
											&guild.name,
										)
									});
									if response.on_hover_text(&guild.name).clicked() {
										self.server = true;
									}
								}
							},
						);

						// Grid.
						ui.scope_builder(
							egui::UiBuilder::new()
								.max_rect(grid_rect.shrink2(egui::vec2(8.0, 8.0)))
								.layout(egui::Layout::top_down(egui::Align::Min)),
							|ui| {
								ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
								let heading = if self.server {
									guild.map_or("This server", |g| g.name.as_str())
								} else {
									"Emoji"
								};
								ui.label(
									crate::design::semibold(ui, heading.to_uppercase(), 12.0)
										.color(colors.muted),
								);
								ui.add_space(6.0);
								let columns = ((ui.available_width() - 12.0) / CELL)
									.floor()
									.clamp(1.0, 12.0) as usize;
								let grid_height = ui.available_height();
								if self.server {
									let Some(emojis) = guild.and_then(|g| g.emojis.as_deref())
									else {
										ui.label(
											egui::RichText::new(
												"This server's emoji list is not loaded yet.",
											)
											.color(colors.muted),
										);
										return;
									};
									let query = self.query.trim().to_lowercase();
									let matching: Vec<_> = emojis
										.iter()
										.filter(|emoji| {
											query.is_empty()
												|| emoji.name.to_lowercase().contains(&query)
										})
										.collect();
									if matching.is_empty() {
										ui.label(
											egui::RichText::new(if emojis.is_empty() {
												"This server has no custom emoji."
											} else {
												"No matching emoji."
											})
											.color(colors.muted),
										);
									}
									egui::ScrollArea::vertical()
										.id_salt(("server-emoji", channel, &self.query))
										.max_height(grid_height)
										.auto_shrink([false, false])
										.show_rows(
											ui,
											CELL,
											matching.len().div_ceil(columns),
											|ui, rows| {
												for row in rows {
													ui.horizontal(|ui| {
														for emoji in matching
															.iter()
															.skip(row * columns)
															.take(columns)
														{
															let image = avatars.custom_image(
																ui.ctx(),
																emoji.id,
																32.0,
																state.demo,
															);
															let response = cell(
																ui,
																image.clone(),
																&emoji.name,
																emoji.usable(),
																&colors,
															);
															if response.hovered() {
																hovered = Some((
																	image,
																	emoji.name.clone(),
																	format!(":{}:", emoji.name),
																));
															}
															if response.clicked() && emoji.usable()
															{
																selected = Some(emoji.markup());
															}
														}
													});
												}
											},
										);
								} else {
									if self.matches.is_empty() {
										ui.label(
											egui::RichText::new("No matching emoji.")
												.color(colors.muted),
										);
									}
									egui::ScrollArea::vertical()
										.id_salt(("unicode-emoji", channel, &self.query))
										.max_height(grid_height)
										.auto_shrink([false, false])
										.show_rows(
											ui,
											CELL,
											self.matches.len().div_ceil(columns),
											|ui, rows| {
												for row in rows {
													ui.horizontal(|ui| {
														for &index in self
															.matches
															.iter()
															.skip(row * columns)
															.take(columns)
														{
															let (text, name) = standard()[index];
															let image = crate::emoji::image(
																ui.ctx(),
																text,
																32.0,
															);
															let response = cell(
																ui,
																image.clone(),
																name,
																true,
																&colors,
															);
															if response.hovered() {
																hovered = Some((
																	image,
																	text.to_owned(),
																	shortcode(name),
																));
															}
															if response.clicked() {
																selected = Some(text.to_owned());
															}
														}
													});
												}
											},
										);
								}
							},
						);

						// Footer: hovered emoji preview.
						ui.painter().rect_filled(
							footer_rect,
							egui::CornerRadius {
								nw: 0,
								ne: 0,
								sw: 8,
								se: 8,
							},
							colors.base,
						);
						ui.scope_builder(
							egui::UiBuilder::new()
								.max_rect(footer_rect.shrink2(egui::vec2(16.0, 8.0)))
								.layout(egui::Layout::left_to_right(egui::Align::Center)),
							|ui| {
								ui.spacing_mut().item_spacing.x = 12.0;
								match &hovered {
									Some((image, fallback, code)) => {
										let (rect, _) = ui.allocate_exact_size(
											egui::Vec2::splat(32.0),
											egui::Sense::hover(),
										);
										paint_emoji(ui, rect, image.as_ref(), fallback);
										ui.label(
											crate::design::semibold(ui, code, 15.0)
												.color(colors.text_strong),
										);
									}
									None => {
										crate::icons::inline(
											ui,
											crate::icons::Icon::Smile,
											28.0,
											colors.muted,
										);
										ui.label(
											egui::RichText::new("Hover an emoji to preview it")
												.color(colors.muted),
										);
									}
								}
							},
						);
					});
			});
		// Click anywhere outside the popout (except the trigger) dismisses it.
		let clicked_outside = ui.input(|i| {
			i.pointer.any_pressed()
				&& i.pointer.interact_pos().is_some_and(|pos| {
					!area.response.rect.contains(pos) && !trigger.rect.contains(pos)
				})
		});
		self.open = !clicked_outside && selected.is_none();
		if selected.is_some() {
			trigger.request_focus();
		}
		selected
	}
}

const WIDTH: f32 = 424.0;
const HEIGHT: f32 = 440.0;

/// Discord-style `:short_code:` rendered from the bundled CLDR name.
fn shortcode(name: &str) -> String {
	let mut code = String::with_capacity(name.len() + 2);
	code.push(':');
	let mut last_underscore = true;
	for c in name.chars() {
		if c.is_alphanumeric() {
			code.extend(c.to_lowercase());
			last_underscore = false;
		} else if !last_underscore {
			code.push('_');
			last_underscore = true;
		}
	}
	if code.ends_with('_') {
		code.pop();
	}
	code.push(':');
	code
}

fn paint_emoji(
	ui: &mut egui::Ui,
	rect: egui::Rect,
	image: Option<&egui::Image<'static>>,
	text: &str,
) {
	if let Some(image) = image {
		image.paint_at(ui, rect);
	} else {
		ui.painter().text(
			rect.center(),
			egui::Align2::CENTER_CENTER,
			text,
			egui::FontId::proportional((rect.height() * 0.7).max(10.0)),
			ui.visuals().text_color(),
		);
	}
}

/// One grid cell: hover highlight plus the emoji image (or its name when no image exists).
fn cell(
	ui: &mut egui::Ui,
	image: Option<egui::Image<'static>>,
	name: &str,
	enabled: bool,
	colors: &crate::design::Palette,
) -> egui::Response {
	let (rect, response) = ui.allocate_exact_size(
		egui::Vec2::splat(CELL),
		if enabled {
			egui::Sense::click()
		} else {
			egui::Sense::hover()
		},
	);
	if ui.is_rect_visible(rect) {
		if response.hovered() || response.has_focus() {
			ui.painter().rect_filled(rect, 6, colors.hover);
		}
		let inner = rect.shrink(4.0);
		match image {
			Some(image) => {
				let image = if enabled {
					image
				} else {
					image.tint(egui::Color32::from_white_alpha(96))
				};
				image.paint_at(ui, inner);
			}
			None => {
				let short: String = name.chars().take(3).collect();
				ui.painter().text(
					rect.center(),
					egui::Align2::CENTER_CENTER,
					short,
					egui::FontId::proportional(11.0),
					colors.muted,
				);
			}
		}
	}
	response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
	if enabled {
		response
	} else {
		response.on_hover_text(format!("{name} — unavailable or permission not verified"))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn insertion_replaces_unicode_selection_and_respects_character_and_capacity_budgets() {
		use egui::text::{CCursor, CCursorRange};
		let mut draft = "前👩🏽‍💻後".to_owned();
		let selection = Some(CCursorRange::two(CCursor::new(5), CCursor::new(1)));
		assert_eq!(insert(&mut draft, "❤️", selection, 0), Some(3));
		assert_eq!(draft, "前❤️後");
		let markup = "<a:party_blob:123456789>";
		assert_eq!(
			insert(&mut draft, markup, None, 100),
			Some(4 + markup.len())
		);
		assert_eq!(draft, format!("前❤️後{markup}"));
		let end = draft.chars().count();
		assert_eq!(
			insert(
				&mut draft,
				"😀",
				Some(CCursorRange::one(CCursor::new(usize::MAX))),
				100
			),
			Some(end + 1)
		);
		assert!(draft.ends_with("😀"));

		let mut draft = "a".repeat(client_core::MAX_CONTENT);
		assert_eq!(insert(&mut draft, "😀", None, 100), None);
		assert_eq!(draft.len(), client_core::MAX_CONTENT);
		let mut draft = String::new();
		assert_eq!(insert(&mut draft, "😀", None, 3), None);
		assert!(draft.is_empty());
		assert_eq!(insert(&mut draft, "😀", None, 4), Some(1));
		assert_eq!(draft.capacity(), 4);
		let selection = Some(CCursorRange::two(CCursor::new(0), CCursor::new(1)));
		assert_eq!(insert(&mut draft, "👍", selection, 0), Some(1));
		assert_eq!(draft, "👍");
		assert_eq!(draft.capacity(), 4);
	}

	#[test]
	fn palette_search_preserves_complete_sequences_and_is_bounded() {
		let atlas = include_str!("../../../assets/twemoji/index.tsv");
		let atlas: std::collections::BTreeSet<_> = atlas
			.lines()
			.map(|l| l.split_once('\t').unwrap().0)
			.collect();
		assert_eq!(standard().len(), 3953);
		assert!(NAMES.len() < 300_000);
		for (text, name) in standard() {
			assert!(atlas.contains(text.replace('\u{fe0f}', "").as_str()));
			assert!(!name.is_empty());
		}
		let mut picker = Picker {
			query: "WOMAN TECHNOLOGIST".into(),
			..Default::default()
		};
		picker.filter();
		assert!(picker.matches.iter().any(|&i| standard()[i].0 == "👩🏽‍💻"));
		picker.query = "❤️".into();
		picker.filter();
		assert!(picker.matches.iter().any(|&i| standard()[i].0 == "❤️"));
		picker.query = "not an emoji name".into();
		picker.filter();
		assert!(picker.matches.is_empty());
	}

	#[test]
	fn server_grid_only_requests_visible_images_and_resets_on_navigation() {
		let state = State {
			guilds: vec![model::Guild {
				id: Id(1),
				name: "Synthetic server".into(),
				icon: None,
				emojis: Some(
					(1..=1000)
						.map(|id| model::CustomEmoji {
							id: Id(id),
							name: format!("emoji_{id}"),
							animated: false,
							available: true,
							managed: false,
							roles: Some(vec![]),
						})
						.collect(),
				),
			}],
			channels: vec![model::Channel {
				id: Id(2),
				guild: Some(Id(1)),
				parent_id: None,
				position: 0,
				name: "Synthetic channel".into(),
				kind: 0,
				recipients: vec![],
				member_list_id: None,
				last_message: None,
			}],
			..State::default()
		};
		let mut picker = Picker {
			open: true,
			server: true,
			channel: Some(Id(2)),
			generation: state.generation,
			..Picker::default()
		};
		let mut avatars = Avatars::default();
		let ctx = egui::Context::default();
		for _ in 0..3 {
			let mut output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(640.0, 480.0),
					)),
					..Default::default()
				},
				|ui| {
					assert!(picker.show(ui, &state, Id(2), &mut avatars).is_none());
				},
			);
			output.textures_delta.clear();
		}
		let requests = avatars.take_requests();
		assert!(!requests.is_empty());
		assert!(
			requests.len() < 100,
			"offscreen emoji must not queue image requests"
		);
		picker.query = "old query".into();
		let mut output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(640.0, 480.0),
				)),
				..Default::default()
			},
			|ui| {
				assert!(picker.show(ui, &state, Id(3), &mut avatars).is_none());
			},
		);
		output.textures_delta.clear();
		assert!(!picker.open && !picker.server && picker.query.is_empty());
	}
}
