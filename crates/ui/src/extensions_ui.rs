//! Native community extensions UI. The desktop owns all package IO and execution.
use crate::design;
use client_core::{MAX_CONTENT, MAX_DRAFT_BYTES, State};
use extensions::{Capability, Element, ExtensionKind, Invocation, Manifest, Output, Surface};
use model::Id;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
pub struct ExtensionEntry {
	pub description: String,
	pub preview: Option<extensions::Preview>,
	pub theme_preview: Option<extensions::Theme>,
	pub manifest: Manifest,
	pub reviewed: bool,
	pub sha256: String,
	pub download_bytes: u64,
	pub enabled: bool,
	pub cleanup_pending: bool,
	pub update_available: bool,
	pub update_manifest: Option<Manifest>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionContext {
	pub generation: u64,
	pub channel: Option<Id>,
	pub draft: Option<String>,
}
impl ExtensionContext {
	pub fn capture(state: &State, composer: bool) -> Self {
		Self {
			generation: state.generation,
			channel: state.selected,
			draft: composer.then(|| {
				state
					.selected
					.and_then(|id| state.drafts.get(&id))
					.cloned()
					.unwrap_or_default()
			}),
		}
	}
	pub fn is_current(&self, state: &State) -> bool {
		self.generation == state.generation
			&& self.channel == state.selected
			&& self.draft.as_ref().is_none_or(|draft| {
				state
					.selected
					.and_then(|id| state.drafts.get(&id))
					.map_or("", String::as_str)
					== draft
			})
	}
}

pub enum ExtensionRequest {
	Preview {
		id: String,
	},
	RefreshCatalog,
	Import,
	Enable {
		id: String,
		grants: Vec<Capability>,
		sha256: String,
		reviewed: bool,
	},
	Disable {
		id: String,
	},
	Invoke {
		id: String,
		invocation: Invocation,
		context: ExtensionContext,
	},
}

#[derive(Clone)]
pub(crate) struct MenuAction {
	pub plugin: String,
	pub action: String,
	pub label: String,
}

struct Consent {
	entry: ExtensionEntry,
	grants: Vec<Capability>,
}
struct ResultPanel {
	id: String,
	context: ExtensionContext,
	invocation: Invocation,
	output: Output,
	values: BTreeMap<String, String>,
}

// Eight 640x360 RGBA thumbnails: at most 7,372,800 bytes of image pixels.
const MAX_PREVIEWS: usize = 8;
struct PreviewImage {
	image: Option<egui::ColorImage>,
	texture: Option<egui::TextureHandle>,
	used: u64,
	loading: bool,
}
#[derive(Default)]
pub struct ExtensionUi {
	pub entries: Vec<ExtensionEntry>,
	pub status: String,
	pub busy: bool,
	pub requests: Vec<ExtensionRequest>,
	consent: Option<Consent>,
	result: Option<ResultPanel>,
	error: Option<String>,
	message_actions: Arc<Vec<MenuAction>>,
	themes: bool,
	query: String,
	previews: BTreeMap<String, PreviewImage>,
	preview_clock: u64,
	enlarged: Option<String>,
}
impl ExtensionUi {
	pub fn set_entries(&mut self, entries: Vec<ExtensionEntry>) {
		self.consent = None;
		self.previews.retain(|id, _| {
			let old = self.entries.iter().find(|entry| &entry.manifest.id == id);
			let new = entries.iter().find(|entry| &entry.manifest.id == id);
			matches!((old, new), (Some(old), Some(new)) if old.preview == new.preview)
		});
		self.entries = entries;
		self.message_actions = Arc::new(
			self.entries
				.iter()
				.filter(|entry| entry.enabled)
				.flat_map(|entry| {
					entry
						.manifest
						.actions
						.iter()
						.filter(|action| action.surface == Surface::Message)
						.map(|action| MenuAction {
							plugin: entry.manifest.id.clone(),
							action: action.id.clone(),
							label: format!("{} · {}", entry.manifest.name, action.label),
						})
				})
				.collect(),
		);
	}
	/// Select the catalog tab in an offline native fixture.
	#[cfg(any(test, feature = "demo"))]
	pub fn preview_themes(&mut self, themes: bool) {
		self.themes = themes;
	}
	pub fn receive_preview(&mut self, id: String, image: Option<egui::ColorImage>) {
		if !self.entries.iter().any(|entry| entry.manifest.id == id) {
			return;
		}
		if let Some(preview) = self.previews.get_mut(&id) {
			preview.loading = false;
			preview.image = image.filter(|image| {
				image.size[0] > 0
					&& image.size[1] > 0
					&& image.size[0] <= 640
					&& image.size[1] <= 360
					&& image.pixels.len() == image.size[0] * image.size[1]
			});
		}
	}
	pub fn retry_preview(&mut self, id: &str) {
		self.previews.remove(id);
	}
	#[cfg(any(test, feature = "demo"))]
	pub fn preview_fixture_image(&mut self, id: String, image: egui::ColorImage) {
		if self.reserve_preview(&id) {
			self.receive_preview(id, Some(image));
		}
	}
	fn reserve_preview(&mut self, id: &str) -> bool {
		if self.previews.contains_key(id) {
			return true;
		}
		if self.previews.len() >= MAX_PREVIEWS {
			let oldest = self
				.previews
				.iter()
				.filter(|(_, image)| image.used.saturating_add(1) < self.preview_clock)
				.min_by_key(|(_, image)| image.used)
				.map(|(id, _)| id.clone());
			let Some(oldest) = oldest else {
				return false;
			};
			self.previews.remove(&oldest);
		}
		self.previews.insert(
			id.into(),
			PreviewImage {
				image: None,
				texture: None,
				used: self.preview_clock,
				loading: true,
			},
		);
		true
	}
	fn preview_image(&mut self, ui: &mut egui::Ui, entry: &ExtensionEntry) {
		let colors = design::palette(ui);
		let size = egui::vec2(ui.available_width(), ui.available_width() * 9.0 / 16.0);
		let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
		if !ui.is_rect_visible(rect) {
			return;
		}
		if entry.theme_preview.is_some()
			|| entry
				.manifest
				.capabilities
				.contains(&Capability::DeletedMessages)
		{
			draw_native_preview(ui, rect, entry);
			let response = ui.interact(rect, ui.id().with("enlarge-preview"), egui::Sense::click());
			if response
				.on_hover_text(format!("Preview {}", entry.manifest.name))
				.clicked()
			{
				self.enlarged = Some(entry.manifest.id.clone());
			}
			return;
		}
		let id = &entry.manifest.id;
		if entry.preview.is_some()
			&& !self.previews.contains_key(id)
			&& !self.busy
			&& self.requests.len() < 4
			&& self
				.previews
				.values()
				.filter(|preview| preview.loading)
				.count() < 4
			&& self.reserve_preview(id)
		{
			self.queue(ui.ctx(), ExtensionRequest::Preview { id: id.clone() });
		}
		if let Some(preview) = self.previews.get_mut(id) {
			preview.used = self.preview_clock;
			if let Some(image) = preview.image.take() {
				preview.texture = Some(ui.ctx().load_texture(
					format!("extension-preview-{id}"),
					image,
					egui::TextureOptions::LINEAR,
				));
			}
			if let Some(texture) = &preview.texture {
				ui.painter().rect_filled(rect, 6, colors.base);
				let fit = texture.size_vec2()
					* (rect.width() / texture.size_vec2().x)
						.min(rect.height() / texture.size_vec2().y);
				egui::Image::new(texture)
					.corner_radius(6)
					.paint_at(ui, egui::Rect::from_center_size(rect.center(), fit));
				let response =
					ui.interact(rect, ui.id().with("enlarge-preview"), egui::Sense::click());
				response.widget_info(|| {
					egui::WidgetInfo::labeled(
						egui::WidgetType::Button,
						true,
						format!("Preview {}", entry.manifest.name),
					)
				});
				if response.on_hover_text("View preview").clicked() {
					self.enlarged = Some(id.clone());
				}
				return;
			}
		}
		ui.painter().rect_filled(rect, 6, colors.sidebar);
		let inset = rect.shrink(18.0);
		let icon = egui::Rect::from_center_size(
			inset.center() - egui::vec2(0.0, 14.0),
			egui::vec2(42.0, 32.0),
		);
		ui.painter().rect_stroke(
			icon,
			5,
			egui::Stroke::new(1.5, colors.muted),
			egui::StrokeKind::Inside,
		);
		ui.painter().circle_filled(icon.center(), 4.0, colors.muted);
		ui.painter().text(
			inset.center() + egui::vec2(0.0, 23.0),
			egui::Align2::CENTER_CENTER,
			if self.previews.get(id).is_some_and(|preview| preview.loading) {
				"Loading preview..."
			} else if entry.preview.is_some() && !self.previews.contains_key(id) {
				"Preview not loaded"
			} else {
				"Preview unavailable"
			},
			egui::FontId::proportional(12.0),
			colors.muted,
		);
	}
	fn preview_modal(&mut self, ctx: &egui::Context) {
		let Some(id) = self.enlarged.clone() else {
			return;
		};
		let Some(entry) = self.entries.iter().find(|entry| entry.manifest.id == id) else {
			self.enlarged = None;
			return;
		};
		let texture = self.previews.get(&id).and_then(|p| p.texture.as_ref());
		let native = entry.theme_preview.is_some()
			|| entry
				.manifest
				.capabilities
				.contains(&Capability::DeletedMessages);
		if texture.is_none() && !native {
			self.enlarged = None;
			return;
		}
		let mut close = false;
		let modal = egui::Modal::new(egui::Id::unique("extension-preview-modal")).show(ctx, |ui| {
			ui.label(design::semibold(ui, &entry.manifest.name, 20.0));
			let available = ctx.content_rect().size() - egui::vec2(64.0, 140.0);
			if native {
				let width = available
					.x
					.clamp(1.0, 800.0)
					.min((available.y * 16.0 / 9.0).max(1.0));
				let (rect, _) = ui.allocate_exact_size(
					egui::vec2(width, width * 9.0 / 16.0),
					egui::Sense::hover(),
				);
				draw_native_preview(ui, rect, entry);
				ui.weak(if entry.theme_preview.is_some() {
					"Theme palette preview"
				} else {
					"Example deleted-message appearance"
				});
			} else if let Some(texture) = texture {
				ui.add(
					egui::Image::new(texture)
						.max_size(available.max(egui::vec2(1.0, 1.0)))
						.corner_radius(8),
				);
				ui.weak("Creator preview");
			}
			close = ui.button("Close preview").clicked();
		});
		if close || modal.should_close() {
			self.enlarged = None;
		}
	}
	pub fn offer_import(&mut self, entry: ExtensionEntry) {
		self.enlarged = None;
		self.consent = Some(Consent {
			entry,
			grants: vec![],
		});
	}
	pub(crate) fn has_result(&self) -> bool {
		self.result.is_some() || self.error.is_some()
	}
	pub fn report_error(&mut self, message: String) {
		let message: String = message.chars().take(512).collect();
		self.status = message.clone();
		self.error = Some(message);
	}
	pub fn reset_runtime(&mut self) {
		self.error = None;
		self.result = None;
		self.consent = None;
		self.requests.clear();
		self.previews.clear();
		self.enlarged = None;
	}
	pub fn remove_runtime(&mut self, id: &str) {
		if self.result.as_ref().is_some_and(|result| result.id == id) {
			self.result = None;
		}
		self.requests.retain(
			|request| !matches!(request, ExtensionRequest::Invoke { id: plugin, .. } if plugin == id),
		);
	}
	pub fn present_output(
		&mut self,
		id: String,
		mut invocation: Invocation,
		context: ExtensionContext,
		mut output: Output,
		state: &State,
	) {
		if !context.is_current(state) {
			self.status = "Result discarded because the conversation or draft changed.".into();
			return;
		}
		invocation.storage = None;
		output.storage = None;
		self.result = Some(ResultPanel {
			id,
			invocation,
			context,
			output,
			values: BTreeMap::new(),
		});
	}
	fn queue(&mut self, ctx: &egui::Context, request: ExtensionRequest) {
		if self.requests.len() < 4
			&& request_bytes(&request)
				.saturating_add(self.requests.iter().map(request_bytes).sum::<usize>())
				<= 4 * extensions::MAX_IO_BYTES
		{
			self.requests.push(request);
			ctx.request_repaint();
		} else {
			self.status =
				"Extensions are busy. Try again after the current action finishes.".into();
		}
	}
	pub(crate) fn message_actions(&self) -> Arc<Vec<MenuAction>> {
		self.message_actions.clone()
	}
	pub(crate) fn invoke_message(
		&mut self,
		action: MenuAction,
		text: String,
		state: &State,
		ctx: &egui::Context,
	) {
		self.queue(
			ctx,
			ExtensionRequest::Invoke {
				id: action.plugin,
				invocation: Invocation {
					action: action.action,
					selected_message: Some(text),
					..Default::default()
				},
				context: ExtensionContext::capture(state, false),
			},
		);
	}
	pub(crate) fn composer_menu(&mut self, ui: &mut egui::Ui, state: &State) {
		if !self.entries.iter().any(|entry| {
			entry.enabled
				&& entry
					.manifest
					.actions
					.iter()
					.any(|action| action.surface == Surface::Composer)
		}) {
			return;
		}
		let mut selected = None;
		ui.menu_button("Tools", |ui| {
			for entry in self.entries.iter().filter(|entry| entry.enabled) {
				for action in entry
					.manifest
					.actions
					.iter()
					.filter(|action| action.surface == Surface::Composer)
				{
					if ui
						.add_enabled(
							!self.busy,
							egui::Button::new(format!(
								"{} · {}",
								entry.manifest.name, action.label
							)),
						)
						.clicked()
					{
						selected = Some((entry.manifest.id.clone(), action.id.clone()));
						ui.close();
					}
				}
			}
		});
		if let Some((id, action)) = selected {
			let context = ExtensionContext::capture(state, true);
			let invocation = Invocation {
				action,
				composer: context.draft.clone(),
				..Default::default()
			};
			self.queue(
				ui.ctx(),
				ExtensionRequest::Invoke {
					id,
					invocation,
					context,
				},
			);
		}
	}
	pub(crate) fn settings(&mut self, ui: &mut egui::Ui, state: &State) {
		self.preview_clock = self.preview_clock.saturating_add(1);

		let colors = design::palette(ui);
		if ui.available_width() >= 540.0 {
			ui.label(design::semibold(ui, "Make Serein yours", 26.0).color(colors.text_strong));
			ui.label(
				egui::RichText::new("A new look. A useful little tool. Made by the community.")
					.color(colors.muted),
			);
			ui.add_space(4.0);
		}
		ui.horizontal_wrapped(|ui| {
			for (themes, label) in [(false, "Plugins"), (true, "Themes")] {
				if ui
					.add(
						egui::Button::new(label)
							.selected(self.themes == themes)
							.min_size(egui::vec2(90.0, 34.0)),
					)
					.clicked()
				{
					self.themes = themes;
					self.query.clear();
				}
			}
		});
		ui.horizontal_wrapped(|ui| {
			ui.add(
				egui::TextEdit::singleline(&mut self.query)
					.hint_text(if self.themes {
						"Search themes"
					} else {
						"Search plugins"
					})
					.char_limit(128)
					.desired_width((ui.available_width() - 260.0).max(120.0)),
			);
			if ui
				.add_enabled(!self.busy, egui::Button::new("Refresh"))
				.clicked()
			{
				self.previews.clear();
				self.queue(ui.ctx(), ExtensionRequest::RefreshCatalog);
			}
			if ui
				.add_enabled(!self.busy, egui::Button::new("Import package..."))
				.clicked()
			{
				self.queue(ui.ctx(), ExtensionRequest::Import);
			}
		});
		if self.busy {
			ui.weak("Loading extensions...");
		}
		if !self.status.is_empty() {
			ui.label(&self.status);
		}
		if self.themes {
			self.reset_theme_button(ui);
		}
		let mut enable = None;
		let mut disable = None;
		let mut invoke = None;
		let query = self.query.trim().to_lowercase();
		let entries: Vec<_> = self
			.entries
			.iter()
			.enumerate()
			.filter(|(_, entry)| {
				(entry.manifest.kind == ExtensionKind::Theme) == self.themes
					&& (query.is_empty()
						|| [
							&entry.manifest.name,
							&entry.manifest.author,
							&entry.description,
						]
						.into_iter()
						.any(|text| text.to_lowercase().contains(&query)))
			})
			.map(|(index, _)| index)
			.collect();
		let columns = if ui.available_width() >= 540.0 { 2 } else { 1 };
		let gap = 16.0;
		let width = (ui.available_width() - gap * (columns - 1) as f32) / columns as f32;
		let mut visible = Vec::new();
		let row_height = (width - 30.0).max(1.0) * 9.0 / 16.0 + 232.0;
		for row in entries.chunks(columns) {
			let row_rect = egui::Rect::from_min_size(
				ui.cursor().min,
				egui::vec2(ui.available_width(), row_height),
			);
			if !ui.is_rect_visible(row_rect) {
				ui.allocate_space(row_rect.size());
				continue;
			}
			visible.extend_from_slice(row);
			ui.horizontal_top(|ui| {
				ui.spacing_mut().item_spacing.x = gap;
				for index in row {
					let entry = self.entries[*index].clone();
					ui.push_id(&entry.manifest.id, |ui| {
						ui.allocate_ui_with_layout(
							egui::vec2(width, 0.0),
							egui::Layout::top_down(egui::Align::Min),
							|ui| {
								egui::Frame::new()
									.fill(colors.raised)
									.corner_radius(10)
									.stroke(egui::Stroke::new(1.0, colors.border))
									.inner_margin(14)
									.show(ui, |ui| {
										ui.set_width((width - 30.0).max(1.0));
										ui.set_min_height(row_height - 30.0);
										ui.spacing_mut().item_spacing = egui::vec2(6.0, 8.0);
										self.preview_image(ui, &entry);
										ui.horizontal(|ui| {
											ui.label(design::eyebrow(
												ui,
												if self.themes { "Theme" } else { "Plugin" },
												colors.muted,
											));
											ui.with_layout(
												egui::Layout::right_to_left(egui::Align::Center),
												|ui| {
													ui.label(
														egui::RichText::new(if entry.enabled {
															"Enabled"
														} else {
															"Free"
														})
														.size(12.0)
														.color(if entry.enabled {
															colors.positive
														} else {
															colors.muted
														}),
													);
												},
											);
										});
										ui.add(
											egui::Label::new(
												design::semibold(ui, &entry.manifest.name, 18.0)
													.color(colors.text_strong),
											)
											.truncate(),
										)
										.on_hover_text(&entry.manifest.name);
										ui.add(
											egui::Label::new(
												egui::RichText::new(format!(
													"by {}",
													entry.manifest.author
												))
												.size(12.0)
												.color(colors.muted),
											)
											.truncate(),
										)
										.on_hover_text(&entry.manifest.author);
										let description = if entry.description.is_empty() {
											if self.themes {
												"Give your conversations a different look."
											} else {
												"Add a new tool to your conversations."
											}
										} else {
											&entry.description
										};
										ui.allocate_ui(
											egui::vec2(ui.available_width(), 38.0),
											|ui| {
												let mut text = egui::text::LayoutJob::simple(
													description.into(),
													egui::FontId::proportional(13.0),
													colors.text,
													ui.available_width(),
												);
												text.wrap.max_rows = 2;
												ui.label(text).on_hover_text(description);
											},
										);
										ui.add_space(
											(ui.min_rect().top() + row_height
												- 92.0 - ui.cursor().top())
											.max(0.0),
										);
										ui.horizontal_wrapped(|ui| {
											if entry.cleanup_pending {
												if ui.button("Retry cleanup").clicked() {
													disable = Some(entry.manifest.id.clone());
												}
											} else if entry.enabled {
												if ui.button("Disable & delete data").clicked() {
													disable = Some(entry.manifest.id.clone());
												}
												if entry.update_available
													&& ui
														.add_enabled(
															!self.busy,
															egui::Button::new("Review update"),
														)
														.clicked()
												{
													let mut update = entry.clone();
													if let Some(manifest) = &entry.update_manifest {
														update.manifest = manifest.clone();
														update.reviewed = true;
													}
													enable = Some(update);
												}
												if entry
													.manifest
													.actions
													.iter()
													.any(|action| action.surface == Surface::Panel)
												{
													ui.menu_button("Open tool", |ui| {
														for action in
															entry.manifest.actions.iter().filter(
																|action| {
																	action.surface == Surface::Panel
																},
															) {
															if ui
																.add_enabled(
																	!self.busy,
																	egui::Button::new(
																		&action.label,
																	),
																)
																.clicked()
															{
																invoke = Some((
																	entry.manifest.id.clone(),
																	action.id.clone(),
																));
																ui.close();
															}
														}
													});
												}
											} else if ui
												.add_enabled(
													!self.busy,
													egui::Button::new(
														egui::RichText::new("Review & enable")
															.color(colors.accent_text),
													)
													.fill(colors.accent)
													.min_size(egui::vec2(
														ui.available_width(),
														32.0,
													)),
												)
												.clicked()
											{
												enable = Some(entry.clone());
											}
										});
										if entry.cleanup_pending {
											ui.weak("Disabled - cleanup pending");
										}
									});
							},
						);
					});
				}
			});
		}
		if self
			.previews
			.values()
			.filter(|preview| preview.loading)
			.count() < 4
			&& self
				.previews
				.values()
				.any(|preview| preview.used < self.preview_clock)
			&& visible.iter().any(|index| {
				self.entries[*index].preview.is_some()
					&& !self
						.previews
						.contains_key(&self.entries[*index].manifest.id)
			}) {
			// One settling frame lets a newly visible card replace a previously visible thumbnail.
			ui.ctx().request_repaint();
		}
		if entries.is_empty() {
			ui.add_space(20.0);
			ui.label(design::semibold(
				ui,
				if query.is_empty() {
					"Find your next favorite"
				} else {
					"No matches"
				},
				18.0,
			));
			ui.weak(if query.is_empty() {
				"Refresh the catalog or import a creator's package to get started."
			} else {
				"Try a different name or creator."
			});
		}
		self.preview_modal(ui.ctx());
		if let Some(entry) = enable {
			self.offer_import(entry);
		}
		if let Some(id) = disable {
			self.remove_runtime(&id);
			self.queue(ui.ctx(), ExtensionRequest::Disable { id });
		}
		if let Some((id, action)) = invoke {
			self.queue(
				ui.ctx(),
				ExtensionRequest::Invoke {
					id,
					invocation: Invocation {
						action,
						..Default::default()
					},
					context: ExtensionContext::capture(state, false),
				},
			);
		}
		if let Some(mut consent) = self.consent.take() {
			let mut close = false;
			let ctx = ui.ctx().clone();
			let modal = egui::Modal::new(egui::Id::unique("extension-consent")).show(&ctx, |ui| {
				ui.set_width((ctx.content_rect().width() - 64.0).clamp(180.0, 440.0));
				egui::ScrollArea::vertical()
					.max_height((ctx.content_rect().height() - 80.0).max(160.0))
					.show(ui, |ui| {
						ui.label(design::semibold(ui, "Enable this extension?", 17.0));
						entry_details(ui, &consent.entry);
						if !consent.entry.reviewed {
							ui.colored_label(
								colors.warning,
								"Unreviewed package — its source has not been reviewed for the catalog.",
							);
						}
						for capability in &consent.entry.manifest.capabilities {
							let mut granted = consent.grants.contains(capability);
							if ui
								.checkbox(&mut granted, capability_label(*capability))
								.changed()
							{
								if granted {
									consent.grants.push(*capability);
								} else {
									consent.grants.retain(|grant| grant != capability);
								}
							}
						}
						if consent.entry.manifest.capabilities.is_empty() {
							ui.weak("No access to conversations or composer text.");
						}
						ui.weak(
							"Disable removes downloaded code and local extension data. Re-enabling starts fresh.",
						);
						let ready = consent
							.entry
							.manifest
							.capabilities
							.iter()
							.all(|capability| consent.grants.contains(capability));
						ui.horizontal(|ui| {
							if ui
								.add_enabled(ready && !self.busy, egui::Button::new("Enable"))
								.clicked()
							{
								self.queue(
									ui.ctx(),
									ExtensionRequest::Enable {
										id: consent.entry.manifest.id.clone(),
										grants: consent.grants.clone(),
										sha256: consent.entry.sha256.clone(),
										reviewed: consent.entry.reviewed,
									},
								);
								close = true;
							}
							if ui.button("Cancel").clicked() {
								close = true;
							}
						});
					});
			});
			if !close && !modal.should_close() {
				self.consent = Some(consent);
			}
		}
	}
	pub(crate) fn reset_theme_shortcut(&mut self, ctx: &egui::Context) {
		if ctx.input_mut(|input| {
			input.consume_key(
				egui::Modifiers::CTRL | egui::Modifiers::SHIFT,
				egui::Key::F12,
			)
		}) {
			self.reset_theme(ctx);
		}
	}
	fn reset_theme(&mut self, ctx: &egui::Context) {
		let ids: Vec<_> = self
			.entries
			.iter()
			.filter(|entry| {
				(entry.enabled || entry.cleanup_pending)
					&& entry.manifest.kind == ExtensionKind::Theme
			})
			.map(|entry| entry.manifest.id.clone())
			.collect();
		design::set_extension_theme(None);
		design::apply(ctx);
		for id in ids {
			self.queue(ctx, ExtensionRequest::Disable { id });
		}
	}
	pub(crate) fn reset_theme_button(&mut self, ui: &mut egui::Ui) {
		if !self.entries.iter().any(|entry| {
			(entry.enabled || entry.cleanup_pending) && entry.manifest.kind == ExtensionKind::Theme
		}) {
			return;
		}
		if ui
			.add(
				egui::Button::new(
					egui::RichText::new("Reset community theme").color(egui::Color32::WHITE),
				)
				.fill(egui::Color32::from_rgb(40, 40, 48)),
			)
			.on_hover_text("Reset anytime with Ctrl+Shift+F12, even if a theme is unreadable.")
			.clicked()
		{
			self.reset_theme(ui.ctx());
		}
	}

	pub(crate) fn show_result(
		&mut self,
		ctx: &egui::Context,
		state: &mut State,
		changes: &mut Vec<Id>,
		editing: bool,
	) {
		if let Some(message) = self.error.take() {
			let mut open = true;
			egui::Window::new("Extension error")
				.id(egui::Id::unique("extension-error"))
				.open(&mut open)
				.default_width(360.0)
				.show(ctx, |ui| {
					ui.label(&message);
				});
			if open {
				self.error = Some(message);
			}
		}
		let Some(mut result) = self.result.take() else {
			return;
		};
		if !result.context.is_current(state) {
			self.status = "Result discarded because the conversation or draft changed.".into();
			return;
		}
		let mut open = true;
		let mut applied = false;
		let mut action = None;
		egui::Window::new("Extension result")
			.id(egui::Id::unique("extension-result"))
			.open(&mut open)
			.default_width(360.0)
			.max_width(600.0)
			.show(ctx, |ui| {
				ui.weak(&result.id);
				egui::ScrollArea::vertical()
					.max_height(400.0)
					.show(ui, |ui| {
						if let Some(replacement) = &result.output.replacement {
							ui.label("Proposed composer text");
							ui.add(egui::Label::new(replacement).wrap());
							if ui
								.add_enabled(
									!editing && result.context.draft.is_some(),
									egui::Button::new("Apply to draft"),
								)
								.clicked()
							{
								if apply_proposal(state, &result.context, replacement) {
									if let Some(channel) = state.selected {
										changes.push(channel);
									}
									applied = true;
								} else {
									self.status = "The draft changed or the proposal exceeds the draft limit.".into();
								}
							}
						}
						render_elements(ui, &result.output.panel, &mut result.values, &mut action);
					});
			});
		if let Some(action) = action {
			let mut invocation = result.invocation.clone();
			invocation.action = action;
			invocation.composer = None;
			invocation.selected_message = None;
			invocation.values = result.values.clone();
			self.queue(
				ctx,
				ExtensionRequest::Invoke {
					id: result.id.clone(),
					invocation,
					context: result.context.clone(),
				},
			);
		}
		if open && !applied {
			self.result = Some(result);
		}
	}
}
fn request_bytes(request: &ExtensionRequest) -> usize {
	std::mem::size_of_val(request)
		+ match request {
			ExtensionRequest::RefreshCatalog | ExtensionRequest::Import => 0,
			ExtensionRequest::Enable {
				id, sha256, grants, ..
			} => id.len() + sha256.len() + grants.len() * std::mem::size_of::<Capability>(),
			ExtensionRequest::Disable { id } | ExtensionRequest::Preview { id } => id.len(),
			ExtensionRequest::Invoke {
				id,
				invocation,
				context,
			} => {
				id.len()
					+ invocation.action.len()
					+ [
						&invocation.selected_message,
						&invocation.composer,
						&invocation.storage,
						&context.draft,
					]
					.into_iter()
					.flatten()
					.map(String::len)
					.sum::<usize>() + invocation
					.values
					.iter()
					.map(|(key, value)| key.len() + value.len())
					.sum::<usize>()
			}
		}
}

/// Draw a small native conversation using the package's real palette, without image IO.
fn draw_native_preview(ui: &egui::Ui, rect: egui::Rect, entry: &ExtensionEntry) {
	let colors = entry.theme_preview.as_ref().map_or_else(
		|| design::palette(ui),
		|theme| design::theme_preview_palette(ui, theme),
	);
	let painter = ui.painter().with_clip_rect(rect);
	let at = |x: f32, y: f32| rect.min + egui::vec2(rect.width() * x, rect.height() * y);
	let panel = |x, y, w, h, color| {
		painter.rect_filled(
			egui::Rect::from_min_max(at(x, y), at(x + w, y + h)),
			3,
			color,
		);
	};
	panel(0.0, 0.0, 1.0, 1.0, colors.base);
	panel(0.08, 0.0, 0.22, 1.0, colors.sidebar);
	panel(0.30, 0.0, 0.70, 1.0, colors.chat);
	let font = egui::FontId::proportional((rect.width() / 29.0).clamp(9.0, 20.0));
	painter.text(
		at(0.35, 0.09),
		egui::Align2::LEFT_CENTER,
		"# general",
		font.clone(),
		colors.text_strong,
	);
	for i in 0..3 {
		let y = 0.25 + i as f32 * 0.20;
		let deleted = i == 1
			&& entry
				.manifest
				.capabilities
				.contains(&Capability::DeletedMessages);
		if deleted {
			panel(
				0.31,
				y - 0.045,
				0.68,
				0.17,
				colors.danger.gamma_multiply(0.12),
			);
		}
		painter.circle_filled(
			at(0.355, y),
			rect.height() * 0.035,
			if deleted {
				colors.danger
			} else {
				colors.accent
			},
		);
		painter.text(
			at(0.41, y),
			egui::Align2::LEFT_CENTER,
			if deleted {
				"Deleted message"
			} else {
				["Robin", "Alex", "You"][i]
			},
			font.clone(),
			if deleted {
				colors.danger
			} else {
				colors.text_strong
			},
		);
		panel(
			0.41,
			y + 0.06,
			if i == 1 { 0.42 } else { 0.31 },
			0.025,
			if deleted { colors.danger } else { colors.muted },
		);
		panel(0.025, y, 0.025, 0.05, colors.accent);
		panel(0.115, y, 0.14, 0.025, colors.muted);
	}
	panel(0.33, 0.86, 0.64, 0.09, colors.raised);
	panel(0.89, 0.88, 0.05, 0.05, colors.accent);
}

fn capability_label(capability: Capability) -> &'static str {
	match capability {
		Capability::SelectedMessage => "Read the message I choose for an action",
		Capability::Composer => "Read my draft and propose text changes",
		Capability::Storage => "Store up to 1 MiB of local data for this account",
		Capability::DeletedMessages => {
			"Keep already-loaded deleted messages in memory until disabled or evicted"
		}
	}
}
fn entry_details(ui: &mut egui::Ui, entry: &ExtensionEntry) {
	ui.weak(format!(
		"{} · {} · {} · {:.1} KiB",
		entry.manifest.author,
		entry.manifest.version,
		entry.manifest.license,
		entry.download_bytes as f64 / 1024.0
	));
	ui.horizontal_wrapped(|ui| {
		ui.label(if entry.reviewed {
			"Reviewed release"
		} else {
			"Unreviewed"
		});
		ui.hyperlink_to("View source", &entry.manifest.source);
		if entry.enabled {
			ui.label("Enabled");
		}
	});
}
fn render_elements(
	ui: &mut egui::Ui,
	elements: &[Element],
	values: &mut BTreeMap<String, String>,
	action: &mut Option<String>,
) {
	for element in elements {
		match element {
			Element::Text { text } => {
				ui.add(egui::Label::new(text).wrap());
			}
			Element::Row { children } => {
				ui.horizontal_wrapped(|ui| render_elements(ui, children, values, action));
			}
			Element::Button { id, label } => {
				if ui.button(label).clicked() {
					*action = Some(id.clone());
				}
			}
			Element::TextInput { id, label, value } => {
				let value = values.entry(id.clone()).or_insert_with(|| value.clone());
				let label = ui.label(label);
				ui.add(
					egui::TextEdit::singleline(value)
						.char_limit(1024)
						.desired_width(ui.available_width().min(320.0)),
				)
				.labelled_by(label.id);
			}
			Element::Checkbox { id, label, checked } => {
				let value = values
					.entry(id.clone())
					.or_insert_with(|| checked.to_string());
				let mut checked = value == "true";
				if ui.checkbox(&mut checked, label).changed() {
					*value = checked.to_string();
				}
			}
		}
	}
}
fn apply_proposal(state: &mut State, context: &ExtensionContext, replacement: &str) -> bool {
	let Some(channel) = state.selected else {
		return false;
	};
	let old_bytes = state.drafts.get(&channel).map_or(0, String::len);
	if context.draft.is_none()
		|| !context.is_current(state)
		|| replacement.chars().count() > MAX_CONTENT
		|| state
			.draft_bytes()
			.saturating_sub(old_bytes)
			.saturating_add(replacement.len())
			> MAX_DRAFT_BYTES
	{
		return false;
	}
	if replacement.is_empty() {
		state.drafts.remove(&channel);
	} else {
		state.drafts.insert(channel, replacement.to_owned());
	}
	true
}

#[cfg(test)]
mod tests {
	use super::*;
	fn entry() -> ExtensionEntry {
		ExtensionEntry {
			description: String::new(),
			theme_preview: None,
			preview: None,
			manifest: Manifest {
				api_version: 1,
				id: "synthetic.words".into(),
				name: "Synthetic word tools".into(),
				version: "1.0.0".into(),
				author: "Offline fixture".into(),
				license: "MIT".into(),
				source: "https://example.com/source".into(),
				kind: ExtensionKind::Plugin,
				capabilities: vec![Capability::Composer],
				actions: vec![],
			},
			reviewed: false,
			sha256: "a".repeat(64),
			download_bytes: 4096,
			enabled: false,
			cleanup_pending: false,
			update_available: false,
			update_manifest: None,
		}
	}
	fn frame(
		ctx: &egui::Context,
		extensions: &mut ExtensionUi,
		width: f32,
		events: Vec<egui::Event>,
	) -> Vec<(String, egui::Rect)> {
		fn collect(shape: &egui::Shape, labels: &mut Vec<(String, egui::Rect)>) {
			match shape {
				egui::Shape::Text(text) => {
					labels.push((text.galley.job.text.clone(), text.visual_bounding_rect()))
				}
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						collect(shape, labels);
					}
				}
				_ => {}
			}
		}
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(width, 1000.0),
				)),
				events,
				focused: true,
				..Default::default()
			},
			|ui| {
				extensions.settings(ui, &State::default());
			},
		);
		let mut labels = vec![];
		for shape in &output.shapes {
			collect(&shape.shape, &mut labels);
		}
		output.drop_without_applying_deltas();
		labels
	}
	fn click(
		ctx: &egui::Context,
		extensions: &mut ExtensionUi,
		width: f32,
		labels: &[(String, egui::Rect)],
		label: &str,
	) {
		let pos = labels
			.iter()
			.find(|(text, _)| text == label)
			.unwrap_or_else(|| panic!("Missing {label}"))
			.1
			.center();
		for pressed in [true, false] {
			frame(
				ctx,
				extensions,
				width,
				vec![
					egui::Event::PointerMoved(pos),
					egui::Event::PointerButton {
						pos,
						button: egui::PointerButton::Primary,
						pressed,
						modifiers: egui::Modifiers::NONE,
					},
				],
			);
		}
	}
	#[test]
	fn consent_requires_explicit_grant_and_cleanup_stays_available() {
		for width in [320.0, 900.0] {
			for theme in [egui::ThemePreference::Dark, egui::ThemePreference::Light] {
				let ctx = egui::Context::default();
				ctx.set_theme(theme);
				let mut extensions = ExtensionUi::default();
				extensions.set_entries(vec![entry()]);
				extensions.offer_import(entry());
				frame(&ctx, &mut extensions, width, vec![]);
				let labels = frame(&ctx, &mut extensions, width, vec![]);
				extensions.requests.clear();
				click(&ctx, &mut extensions, width, &labels, "Enable");
				assert!(
					extensions.requests.is_empty(),
					"Permission must be granted before enable"
				);
				click(
					&ctx,
					&mut extensions,
					width,
					&labels,
					capability_label(Capability::Composer),
				);
				let labels = frame(&ctx, &mut extensions, width, vec![]);
				click(&ctx, &mut extensions, width, &labels, "Enable");
				assert!(
					matches!(extensions.requests.as_slice(), [ExtensionRequest::Enable { grants, sha256, reviewed, .. }] if grants == &[Capability::Composer] && sha256 == &"a".repeat(64) && !reviewed)
				);

				extensions.requests.clear();
				extensions.offer_import(entry());
				let labels = frame(&ctx, &mut extensions, width, vec![]);
				click(&ctx, &mut extensions, width, &labels, "Cancel");
				let mut reviewed_entry = entry();
				reviewed_entry.reviewed = true;
				reviewed_entry.sha256 = "b".repeat(64);
				extensions.set_entries(vec![reviewed_entry]);
				let labels = frame(&ctx, &mut extensions, width, vec![]);
				click(&ctx, &mut extensions, width, &labels, "Review & enable");
				let labels = frame(&ctx, &mut extensions, width, vec![]);
				click(
					&ctx,
					&mut extensions,
					width,
					&labels,
					capability_label(Capability::Composer),
				);
				let labels = frame(&ctx, &mut extensions, width, vec![]);
				click(&ctx, &mut extensions, width, &labels, "Enable");
				assert!(
					matches!(extensions.requests.as_slice(), [ExtensionRequest::Enable { sha256, reviewed: true, .. }] if sha256 == &"b".repeat(64))
				);
				extensions.requests.clear();
				let mut pending = entry();
				pending.cleanup_pending = true;
				extensions.set_entries(vec![pending]);
				extensions.busy = true;
				frame(&ctx, &mut extensions, width, vec![]);
				let labels = frame(&ctx, &mut extensions, width, vec![]);
				click(&ctx, &mut extensions, width, &labels, "Retry cleanup");
				assert!(matches!(
					extensions.requests.as_slice(),
					[ExtensionRequest::Disable { .. }]
				));
			}
		}
	}

	#[test]
	fn shop_previews_load_visible_cards_once_and_evict_with_metadata() {
		for width in [320.0, 900.0] {
			let ctx = egui::Context::default();
			let mut shop = ExtensionUi::default();
			let entries: Vec<_> = (0..20)
				.map(|i| {
					let mut entry = entry();
					entry.manifest.id = format!("plugin-{i}");
					entry.preview = Some(extensions::Preview {
						url: "https://example.com/preview.png".into(),
						sha256: "a".repeat(64),
						download_bytes: 123,
					});
					entry
				})
				.collect();
			shop.set_entries(entries.clone());
			frame(&ctx, &mut shop, width, vec![]);
			shop.requests.clear();
			frame(&ctx, &mut shop, width, vec![]);
			assert!(
				shop.previews.len() < entries.len(),
				"offscreen cards must not fetch images"
			);
			shop.requests.clear();
			frame(&ctx, &mut shop, width, vec![]);
			assert!(
				shop.requests.is_empty(),
				"pending previews must be deduplicated"
			);
			for entry in &entries {
				shop.preview_clock += 2;
				shop.preview_fixture_image(
					entry.manifest.id.clone(),
					egui::ColorImage::filled([640, 360], egui::Color32::BLUE),
				);
			}
			assert_eq!(shop.previews.len(), MAX_PREVIEWS);
			assert_eq!(
				shop.previews
					.values()
					.filter_map(|preview| preview.image.as_ref())
					.map(|image| image.pixels.len() * 4)
					.sum::<usize>(),
				7_372_800
			);
			shop.receive_preview(
				"plugin-19".into(),
				Some(egui::ColorImage::filled([641, 1], egui::Color32::BLUE)),
			);
			assert!(shop.previews["plugin-19"].image.is_none());
			let mut changed = entries;
			for entry in &mut changed {
				entry.preview.as_mut().unwrap().sha256 = "b".repeat(64);
			}
			shop.set_entries(changed);
			assert!(shop.previews.is_empty());
			shop.query = "missing creator".into();
			let labels = frame(&ctx, &mut shop, width, vec![]);
			assert!(labels.iter().any(|(text, _)| text == "No matches"));
			assert!(shop.previews.is_empty());
		}
	}

	#[test]
	fn tall_shop_keeps_visible_previews_without_repeated_downloads() {
		let ctx = egui::Context::default();
		let mut shop = ExtensionUi::default();
		shop.set_entries(
			(0..20)
				.map(|i| {
					let mut entry = entry();
					entry.manifest.id = format!("plugin-{i}");
					entry.preview = Some(extensions::Preview {
						url: "https://example.com/image.png".into(),
						sha256: "a".repeat(64),
						download_bytes: 123,
					});
					entry
				})
				.collect(),
		);
		for _ in 0..5 {
			ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(900.0, 8000.0),
					)),
					..Default::default()
				},
				|ui| shop.settings(ui, &State::default()),
			)
			.drop_without_applying_deltas();
			for request in std::mem::take(&mut shop.requests) {
				if let ExtensionRequest::Preview { id } = request {
					shop.receive_preview(
						id,
						Some(egui::ColorImage::filled([1, 1], egui::Color32::BLUE)),
					);
				}
			}
		}
		let retained: Vec<_> = shop.previews.keys().cloned().collect();
		assert_eq!(retained.len(), MAX_PREVIEWS);
		ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(900.0, 8000.0),
				)),
				..Default::default()
			},
			|ui| shop.settings(ui, &State::default()),
		)
		.drop_without_applying_deltas();
		assert!(shop.requests.is_empty());
		shop.receive_preview(
			"plugin-19".into(),
			Some(egui::ColorImage::filled([1, 1], egui::Color32::BLUE)),
		);
		assert_eq!(
			shop.previews.keys().cloned().collect::<Vec<_>>(),
			retained,
			"late offscreen images cannot evict visible previews"
		);
	}

	#[test]
	fn queued_actions_and_closed_results_are_bounded() {
		let mut extensions = ExtensionUi::default();
		for _ in 0..10 {
			extensions.queue(&egui::Context::default(), ExtensionRequest::RefreshCatalog);
		}
		assert_eq!(extensions.requests.len(), 4);
		assert!(!extensions.status.is_empty());
		extensions.reset_runtime();
		assert!(extensions.requests.is_empty());
		extensions.queue(
			&egui::Context::default(),
			ExtensionRequest::Invoke {
				id: "test".into(),
				invocation: Invocation {
					composer: Some("x".repeat(4 * extensions::MAX_IO_BYTES)),
					..Default::default()
				},
				context: ExtensionContext::capture(&State::default(), false),
			},
		);
		assert!(extensions.requests.is_empty());
	}
	#[test]
	fn proposal_requires_unchanged_account_conversation_and_draft() {
		let mut state = State {
			selected: Some(Id(1)),
			..Default::default()
		};
		state.drafts.insert(Id(1), "original".into());
		let context = ExtensionContext::capture(&state, true);
		state.generation += 1;
		assert!(!apply_proposal(&mut state, &context, "changed"));
		state.generation -= 1;
		state.selected = Some(Id(2));
		assert!(!apply_proposal(&mut state, &context, "changed"));
		state.selected = Some(Id(1));
		state.drafts.insert(Id(1), "new draft".into());
		assert!(!apply_proposal(&mut state, &context, "changed"));
		state.drafts.insert(Id(1), "original".into());
		assert!(!apply_proposal(
			&mut state,
			&context,
			&"x".repeat(MAX_CONTENT + 1)
		));
		assert!(apply_proposal(&mut state, &context, "changed"));
		assert_eq!(state.drafts[&Id(1)], "changed");
	}
}
