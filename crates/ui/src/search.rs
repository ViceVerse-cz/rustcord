use crate::{design, icons};
use client_core::{Command, State};
use egui::RichText;
use model::Id;

/// Width of the results pane when the window is wide enough to keep the timeline readable.
pub const PANE_WIDTH: f32 = 420.0;

#[derive(Default)]
pub struct SearchUi {
	pub open: bool,
	pins: bool,
	query: String,
	channel: Option<Id>,
	focus: bool,
	composing: bool,
	ime_frame: bool,
	pending_submit: bool,
}

impl SearchUi {
	pub fn toggle(&mut self, pins: bool) -> bool {
		self.open = !self.open || self.pins != pins;
		self.pins = pins;
		self.focus = self.open;
		self.open
	}
	/// Fixture-only: open a text search for `query` and submit it on the next frame.
	pub fn preview(&mut self, query: &str) {
		self.open = true;
		self.pins = false;
		self.query = query.to_owned();
		self.pending_submit = true;
	}
	/// True while the pane shows pinned messages rather than query results.
	pub fn pins(&self) -> bool {
		self.pins
	}
	/// Per-frame bookkeeping: Escape closes, navigation resets and closed views cancel requests.
	pub fn sync(&mut self, ctx: &egui::Context, state: &mut State, commands: &mut Vec<Command>) {
		if self.open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
			self.open = false;
		}
		if self.channel != state.selected {
			self.channel = state.selected;
			// A fixture preview adopts the initial selection instead of closing.
			if !self.pending_submit {
				self.query.clear();
				self.open = false;
			}
		}
		if !self.open {
			if state.search.is_some() {
				commands.push(state.clear_search());
			}
			return;
		}
		if state
			.search
			.as_ref()
			.is_some_and(|view| view.pins != self.pins)
			|| (!state.can_search() && state.search.is_some())
		{
			commands.push(state.clear_search());
		}
		self.ime_frame = self.composing;
		ctx.input(|i| {
			for event in &i.events {
				if let egui::Event::Ime(event) = event {
					self.ime_frame = true;
					self.composing =
						matches!(event, egui::ImeEvent::Preedit { text,.. } if !text.is_empty());
				}
			}
		});
	}
	/// Query field shown in the conversation header while a text search is open.
	pub fn header_input(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		commands: &mut Vec<Command>,
	) {
		let colors = design::palette(ui);
		let allowed = state.can_search();
		let mut submit = false;
		egui::Frame::new()
			.fill(colors.raised)
			.corner_radius(6)
			.stroke(egui::Stroke::new(1.0, colors.accent))
			.inner_margin(egui::Margin::symmetric(8, 0))
			.show(ui, |ui| {
				ui.set_height(28.0);
				ui.horizontal_centered(|ui| {
					ui.spacing_mut().item_spacing.x = 6.0;
					let clear = icons::button(ui, icons::Icon::Close, 22.0, "Close search");
					let input = ui.add(
						egui::TextEdit::singleline(&mut self.query)
							.char_limit(256)
							.frame(egui::Frame::NONE)
							.hint_text("Search")
							.desired_width(ui.available_width().max(60.0)),
					);
					input.widget_info(|| {
						egui::WidgetInfo::labeled(
							egui::WidgetType::TextEdit,
							true,
							"Search messages in this conversation",
						)
					});
					if self.focus {
						input.request_focus();
						self.focus = false;
					}
					let valid = allowed && model::valid_search_query(&self.query);
					submit = valid
						&& input.lost_focus()
						&& ui.input(|i| i.key_pressed(egui::Key::Enter))
						&& !self.ime_frame;
					if clear.clicked() {
						self.open = false;
					}
				});
			});
		if submit && let Some(command) = state.request_search(self.query.trim().into(), None) {
			commands.push(command);
		}
	}
	/// Results pane rendered where the member list normally lives.
	pub fn pane(&mut self, ui: &mut egui::Ui, state: &mut State, commands: &mut Vec<Command>) {
		let colors = design::palette(ui);
		let allowed = state.can_search();
		let mut submit = std::mem::take(&mut self.pending_submit) && !self.pins;
		let mut older = None;
		let mut older_pins = false;
		let mut target = None;
		ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
		ui.horizontal(|ui| {
			let title = if self.pins {
				"Pinned Messages".to_owned()
			} else {
				match state.search.as_ref().and_then(|view| view.page.as_ref()) {
					Some(page) if !state.search.as_ref().is_some_and(|v| v.loading) => {
						format!("{} Results", page.total)
					}
					_ => "Search".to_owned(),
				}
			};
			ui.label(design::semibold(ui, title, 16.0).color(colors.text_strong));
			ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
				if icons::button(ui, icons::Icon::Close, 28.0, "Close").clicked() {
					self.open = false;
				}
				if self.pins {
					let reload = ui.add_enabled(allowed, egui::Button::new("Reload pins"));
					if self.focus {
						reload.request_focus();
						self.focus = false;
					}
					submit = reload.clicked();
				} else if let Some(view) = &state.search
					&& let Some(page) = &view.page
				{
					if let Some(last) = page.hits.last()
						&& (page.total > page.hits.len() as u64 || page.partial)
						&& ui
							.add_enabled(allowed && !view.loading, egui::Button::new("Older"))
							.clicked()
					{
						older = Some((view.query.clone(), Some(last.id)));
					}
					if view.before.is_some()
						&& ui
							.add_enabled(allowed && !view.loading, egui::Button::new("Newest"))
							.clicked()
					{
						older = Some((view.query.clone(), None));
					}
				}
			});
		});
		if self.pins
			&& let Some(view) = &state.search
		{
			let retry = view.error.is_some() && view.pin_before.is_some();
			if retry
				|| view
					.page
					.as_ref()
					.is_some_and(|page| page.pin_cursor.is_some())
			{
				older_pins = ui
					.push_id("older-pins", |ui| {
						ui.add_enabled(
							allowed && !view.loading,
							egui::Button::new(if retry {
								"Retry older pins"
							} else {
								"Older pins"
							}),
						)
					})
					.inner
					.clicked();
			}
		}
		if !allowed {
			ui.label(
				RichText::new(
					"Messages are unavailable while disconnected or without channel access.",
				)
				.small()
				.color(colors.muted),
			);
		}
		if state.search.is_none() && !self.pins {
			ui.label(
				RichText::new("Type a query above and press Enter.")
					.small()
					.color(colors.muted),
			);
		}
		if let Some(view) = &state.search {
			if view.loading {
				ui.label(
					RichText::new(if view.pins {
						if view.pin_before.is_some() {
							"Loading older pins…"
						} else {
							"Loading newest pins…"
						}
					} else {
						"Searching…"
					})
					.small()
					.color(colors.muted),
				);
			}
			if let Some(error) = view.error {
				ui.label(RichText::new(error).color(colors.danger));
			}
			if let Some(page) = &view.page {
				let note = if view.pins {
					if page.pin_cursor.is_none() && !view.loading {
						Some(if page.partial {
							"More pins may exist, but this page has no usable continuation."
						} else {
							"Showing one page of up to 25 pins."
						})
					} else {
						None
					}
				} else if page.partial {
					Some("Indexing is incomplete; results may be missing.")
				} else {
					None
				};
				if let Some(note) = note {
					ui.label(RichText::new(note).small().color(colors.muted));
				}
				if page.hits.is_empty() {
					ui.label(
						RichText::new(if view.pins {
							"No pinned messages returned; history access may be unavailable."
						} else {
							"No matching messages in this page."
						})
						.color(colors.muted),
					);
				}
				egui::ScrollArea::vertical()
					.id_salt(("search-results", view.request))
					.auto_shrink([false, false])
					.show(ui, |ui| {
						ui.spacing_mut().item_spacing.y = 8.0;
						for hit in &page.hits {
							ui.push_id(hit.id, |ui| {
								egui::Frame::new()
									.fill(colors.raised)
									.stroke(egui::Stroke::new(1.0, colors.border))
									.corner_radius(8)
									.inner_margin(egui::Margin::same(10))
									.show(ui, |ui| {
										ui.set_width(ui.available_width());
										ui.horizontal(|ui| {
											ui.label(
												design::semibold(ui, &hit.author, 14.0)
													.color(colors.text_strong),
											);
											ui.label(
												RichText::new(format!("#{}", hit.id))
													.size(11.0)
													.color(colors.muted),
											);
											ui.with_layout(
												egui::Layout::right_to_left(egui::Align::Center),
												|ui| {
													if ui
														.add_enabled(
															allowed && hit.id.0 < u64::MAX,
															egui::Button::new(
																RichText::new("Jump").size(12.0),
															),
														)
														.on_hover_text(
															"Open this message in the timeline",
														)
														.clicked()
													{
														target = Some(hit.id);
													}
												},
											);
										});
										ui.add(
											egui::Label::new(
												RichText::new(&hit.excerpt).color(colors.text),
											)
											.wrap()
											.selectable(true),
										);
									});
							});
						}
					});
			}
		}
		if submit
			&& let Some(command) = if self.pins {
				state.request_pins()
			} else {
				state.request_search(self.query.trim().into(), None)
			} {
			commands.push(command);
		}
		if let Some((query, before)) = older
			&& let Some(command) = state.request_search(query, before)
		{
			commands.push(command);
		}
		if older_pins && let Some(command) = state.request_older_pins() {
			commands.push(command);
		}
		if let Some(target) = target
			&& let Some(command) = state.open_search_hit(target)
		{
			commands.push(command);
			self.open = false;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	fn run(ui: &mut egui::Ui, view: &mut SearchUi, state: &mut State, commands: &mut Vec<Command>) {
		view.sync(ui.ctx(), state, commands);
		if view.open {
			if !view.pins {
				view.header_input(ui, state, commands);
			}
			view.pane(ui, state, commands);
		}
	}
	#[test]
	fn pins_reload_is_keyboard_operable_and_close_cancels_at_narrow_width() {
		for dark in [false, true] {
			let mut state = State {
				auth: client_core::auth::AuthState::Authenticated,
				gateway_connected: true,
				selected: Some(Id(1)),
				channels: vec![model::Channel {
					id: Id(1),
					guild: None,
					parent_id: None,
					position: 0,
					name: "Synthetic".into(),
					kind: 3,
					recipients: vec![],
					member_list_id: None,
					last_message: None,
				}],
				..State::default()
			};
			let ctx = egui::Context::default();
			ctx.set_visuals(if dark {
				egui::Visuals::dark()
			} else {
				egui::Visuals::light()
			});
			let mut view = SearchUi {
				channel: Some(Id(1)),
				..SearchUi::default()
			};
			assert!(view.toggle(true));
			let mut commands = Vec::new();
			for frame in 0..3 {
				let events = if frame == 2 {
					vec![egui::Event::Key {
						key: egui::Key::Enter,
						physical_key: None,
						pressed: true,
						repeat: false,
						modifiers: egui::Modifiers::NONE,
					}]
				} else {
					vec![]
				};
				let mut output = ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(420.0, 480.0),
						)),
						events,
						..Default::default()
					},
					|ui| run(ui, &mut view, &mut state, &mut commands),
				);
				assert!(output.platform_output.commands.is_empty());
				output.textures_delta.clear();
				if frame < 2 {
					assert!(commands.is_empty());
				}
			}
			assert_eq!(
				commands
					.iter()
					.filter(|c| matches!(c, Command::Pins { .. }))
					.count(),
				1
			);
			assert!(state.search.as_ref().unwrap().pins);
			let mut output = ctx.run_ui(
				egui::RawInput {
					events: vec![egui::Event::Key {
						key: egui::Key::Escape,
						physical_key: None,
						pressed: true,
						repeat: false,
						modifiers: egui::Modifiers::NONE,
					}],
					..Default::default()
				},
				|ui| run(ui, &mut view, &mut state, &mut commands),
			);
			output.textures_delta.clear();
			assert!(!view.open);
			assert!(state.search.is_none());
			assert!(matches!(commands.last(), Some(Command::CancelSearch)));
			let cursor = 1_700_000_000_000_000_000i128;
			for (loading, continuation, available, retry, expected) in [
				(false, Some(cursor), true, false, true),
				(false, None, true, true, true),
				(true, Some(cursor), true, false, false),
				(false, None, true, false, false),
				(false, Some(cursor), false, false, false),
			] {
				state.gateway_connected = true;
				state.request_pins().unwrap();
				let page = state.search.as_mut().unwrap();
				page.loading = loading;
				page.pin_before = retry.then_some(cursor);
				page.error = retry.then_some("Synthetic pin request failed");
				page.page = (!retry).then(|| model::SearchPage {
					hits: vec![model::SearchHit {
						id: Id(10),
						channel: Id(1),
						author: "Synthetic".into(),
						excerpt: "Synthetic pinned message".into(),
					}],
					total: 1,
					partial: continuation.is_some(),
					pin_cursor: continuation,
				});
				state.gateway_connected = available;
				let ctx = egui::Context::default();
				ctx.set_visuals(if dark {
					egui::Visuals::dark()
				} else {
					egui::Visuals::light()
				});
				let mut view = SearchUi {
					channel: Some(Id(1)),
					pins: true,
					open: true,
					focus: true,
					..SearchUi::default()
				};
				let mut commands = Vec::new();
				// Focus starts on Reload; Tab reaches Older (or Retry older) when enabled.
				// Disabled/exhausted states must not submit another pin-page request.
				for key in [None, None, Some(egui::Key::Tab), Some(egui::Key::Enter)] {
					let output = ctx.run_ui(
						egui::RawInput {
							screen_rect: Some(egui::Rect::from_min_size(
								egui::Pos2::ZERO,
								egui::vec2(420.0, 480.0),
							)),
							events: key
								.into_iter()
								.map(|key| egui::Event::Key {
									key,
									physical_key: None,
									pressed: true,
									repeat: false,
									modifiers: egui::Modifiers::NONE,
								})
								.collect(),
							..Default::default()
						},
						|ui| run(ui, &mut view, &mut state, &mut commands),
					);
					assert!(output.platform_output.commands.is_empty());
					output.drop_without_applying_deltas();
				}
				assert_eq!(
					commands
						.iter()
						.filter(|c| matches!(c, Command::Pins { .. }))
						.count(),
					usize::from(expected)
				);
				if expected {
					assert!(
						matches!(commands.last(), Some(Command::Pins { before: Some(value), .. }) if *value == cursor)
					);
					let page = state.search.as_ref().unwrap();
					assert_eq!(page.pin_before, Some(cursor));
					assert!(page.loading && page.page.is_none());
				}
			}
		}
	}
	#[test]
	fn keyboard_search_is_explicit_and_ime_commit_does_not_submit() {
		for ime in [false, true] {
			let mut state = State {
				auth: client_core::auth::AuthState::Authenticated,
				gateway_connected: true,
				selected: Some(Id(1)),
				..State::default()
			};
			state.channels.push(model::Channel {
				id: Id(1),
				guild: None,
				parent_id: None,
				position: 0,
				name: "Synthetic".into(),
				kind: 1,
				recipients: vec![],
				member_list_id: None,
				last_message: None,
			});
			let ctx = egui::Context::default();
			let mut view = SearchUi {
				channel: Some(Id(1)),
				open: true,
				focus: true,
				query: "synthetic".into(),
				..SearchUi::default()
			};
			let mut commands = Vec::new();
			for frame in 0..3 {
				let mut events = Vec::new();
				if frame == 2 {
					if ime {
						events.push(egui::Event::Ime(egui::ImeEvent::Commit("語".into())));
					}
					events.push(egui::Event::Key {
						key: egui::Key::Enter,
						physical_key: None,
						pressed: true,
						repeat: false,
						modifiers: egui::Modifiers::NONE,
					});
				}
				let mut output = ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(640.0, 480.0),
						)),
						events,
						..Default::default()
					},
					|ui| run(ui, &mut view, &mut state, &mut commands),
				);
				assert!(output.platform_output.commands.is_empty());
				output.textures_delta.clear();
				if frame < 2 {
					assert!(commands.is_empty());
				}
			}
			assert_eq!(
				commands
					.iter()
					.filter(|c| matches!(c, Command::Search { .. }))
					.count(),
				usize::from(!ime)
			);
			view.open = false;
			let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
				run(ui, &mut view, &mut state, &mut commands)
			});
			output.textures_delta.clear();
			assert!(state.search.is_none());
		}
	}
}
