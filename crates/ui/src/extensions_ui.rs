//! Native community extensions UI. The desktop owns all package IO and execution.
use crate::design;
use client_core::{MAX_CONTENT, MAX_DRAFT_BYTES, State};
use extensions::{Capability, Element, ExtensionKind, Invocation, Manifest, Output, Surface};
use model::Id;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
pub struct ExtensionEntry {
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
	opened: bool,
	themes: bool,
}
impl ExtensionUi {
	pub fn set_entries(&mut self, entries: Vec<ExtensionEntry>) {
		self.consent = None;
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
	pub fn offer_import(&mut self, entry: ExtensionEntry) {
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
		if !self.opened {
			self.opened = true;
			self.queue(ui.ctx(), ExtensionRequest::RefreshCatalog);
		}
		let colors = design::palette(ui);
		ui.horizontal_wrapped(|ui| {
			ui.selectable_value(&mut self.themes, false, "Plugins");
			ui.selectable_value(&mut self.themes, true, "Themes");
			if ui
				.add_enabled(!self.busy, egui::Button::new("Refresh catalog"))
				.clicked()
			{
				self.queue(ui.ctx(), ExtensionRequest::RefreshCatalog);
			}
			if ui
				.add_enabled(!self.busy, egui::Button::new("Import package…"))
				.clicked()
			{
				self.queue(ui.ctx(), ExtensionRequest::Import);
			}
		});
		ui.weak("Free community extensions. Review the source and permissions before enabling.");
		if self.busy {
			ui.label("Working…");
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
		let mut shown = false;
		for entry in &self.entries {
			if (entry.manifest.kind == ExtensionKind::Theme) != self.themes {
				continue;
			}
			shown = true;
			ui.push_id(&entry.manifest.id, |ui| {
				design::card(ui, |ui| {
					ui.label(
						design::semibold(ui, &entry.manifest.name, 17.0).color(colors.text_strong),
					);
					entry_details(ui, entry);
					ui.horizontal_wrapped(|ui| {
						if entry.cleanup_pending {
							ui.weak("Disabled. Package cleanup is pending.");
							if ui.button("Retry cleanup").clicked() {
								disable = Some(entry.manifest.id.clone());
							}
						} else if entry.enabled {
							if ui.button("Disable & delete data").clicked() {
								disable = Some(entry.manifest.id.clone());
							}
							if entry.update_available
								&& ui
									.add_enabled(!self.busy, egui::Button::new("Review update"))
									.clicked()
							{
								let mut update = entry.clone();
								if let Some(manifest) = &entry.update_manifest {
									update.manifest = manifest.clone();
									update.reviewed = true;
								}
								enable = Some(update);
							}
							for action in entry
								.manifest
								.actions
								.iter()
								.filter(|action| action.surface == Surface::Panel)
							{
								if ui
									.add_enabled(!self.busy, egui::Button::new(&action.label))
									.clicked()
								{
									invoke = Some((entry.manifest.id.clone(), action.id.clone()));
								}
							}
						} else if ui
							.add_enabled(!self.busy, egui::Button::new("Review & enable"))
							.clicked()
						{
							enable = Some(entry.clone());
						}
					});
				})
			});
		}
		if !shown {
			ui.label(if self.themes {
				"No themes in this catalog. Import a creator’s package to try one."
			} else {
				"No plugins in this catalog. Import a creator’s package to try one."
			});
		}
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
			ui.separator();
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
			let mut close = false;
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
			if !close {
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
			ExtensionRequest::Disable { id } => id.len(),
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

fn capability_label(capability: Capability) -> &'static str {
	match capability {
		Capability::SelectedMessage => "Read the message I choose for an action",
		Capability::Composer => "Read my draft and propose text changes",
		Capability::Storage => "Store up to 1 MiB of local data for this account",
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
