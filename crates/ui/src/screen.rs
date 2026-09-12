//! Screen selection is a local intent; the desktop owns capture and stream credentials.
use client_core::{
	State,
	screen::{Settings, Source, SourceId},
	voice::Phase,
};
use model::Id;

pub enum Request {
	Start(Settings),
	Stop,
}

pub struct ScreenUi {
	pub context: Option<(u64, Id, u64)>,
	pub open: bool,
	pub sources: Vec<Source>,
	pub selected: Option<SourceId>,
	pub refresh_requested: bool,
	pub request: Option<Request>,
	pub busy: bool,
	pub status: &'static str,
	pub supported: bool,
	pub preview: Option<egui::TextureHandle>,
	height: u32,
	fps: u32,
	cursor: bool,
}
impl Default for ScreenUi {
	fn default() -> Self {
		Self {
			context: None,
			open: false,
			sources: Vec::new(),
			selected: None,
			refresh_requested: false,
			request: None,
			busy: false,
			status: "",
			supported: false,
			preview: None,
			height: 1080,
			fps: 30,
			cursor: true,
		}
	}
}
impl ScreenUi {
	pub(super) fn launch(&mut self, state: &State) {
		let Some(call) = &state.voice.active else {
			return;
		};
		if self.busy {
			self.request = Some(Request::Stop);
			return;
		}
		self.context = Some((state.generation, call.channel, call.request));
		self.open = true;
		self.selected = None;
		self.sources.clear();
		if state.demo {
			self.sources = vec![
				Source {
					id: SourceId::Display(1),
					name: "Display 1 · Synthetic preview".into(),
				},
				Source {
					id: SourceId::Window(2),
					name: "Project notes · Synthetic window".into(),
				},
			];
			self.selected = Some(SourceId::Display(1));
			self.status = "Offline preview · no screen is captured";
		} else {
			self.refresh_requested = true;
			self.status = "Looking for screens and windows…";
		}
	}
	fn settings(&self) -> Option<Settings> {
		let source = self
			.selected
			.filter(|id| self.sources.iter().any(|s| s.id == *id))?;
		let settings = Settings {
			source,
			width: if self.height == 1080 { 1920 } else { 1280 },
			height: self.height,
			fps: self.fps,
			cursor: self.cursor,
		};
		settings.valid().then_some(settings)
	}
	pub(super) fn show(&mut self, ctx: &egui::Context, state: &State) {
		let current = state
			.voice
			.active
			.as_ref()
			.filter(|c| c.phase != Phase::Failed)
			.map(|c| (state.generation, c.channel, c.request));
		if self.context.is_some() && current != self.context {
			self.preview = None;
			self.open = false;
			self.sources.clear();
			self.selected = None;
			self.request = None;
			self.refresh_requested = false;
		}
		if !self.open {
			return;
		}
		let mut cancel = false;
		let mut share = false;
		let modal = egui::Modal::new(egui::Id::unique("screen-share-settings")).show(ctx, |ui| {
			ui.set_width((ctx.content_rect().width() - 48.0).clamp(180.0, 460.0));
			let colors = crate::design::palette(ui);
			ui.heading("Share your screen");
			ui.label(
				egui::RichText::new("Choose what people in this call can see.").color(colors.muted),
			);
			ui.add_space(12.0);
			egui::ScrollArea::vertical()
				.max_height((ctx.content_rect().height() - 220.0).max(100.0))
				.show(ui, |ui| {
					ui.label(egui::RichText::new("Screen or window").strong());
					let name = self
						.sources
						.iter()
						.find(|s| Some(s.id) == self.selected)
						.map_or("Select a screen or window", |s| s.name.as_str());
					egui::ComboBox::from_id_salt("screen-source")
						.selected_text(name)
						.width(ui.available_width().min(420.0))
						.height(220.0)
						.show_ui(ui, |ui| {
							for source in &self.sources {
								ui.selectable_value(
									&mut self.selected,
									Some(source.id),
									&source.name,
								);
							}
						});
					if ui
						.add_enabled(!state.demo, egui::Button::new("Refresh sources"))
						.clicked()
					{
						self.selected = None;
						self.sources.clear();
						self.refresh_requested = true;
						self.status = "Looking for screens and windows…";
					}
					ui.add_space(12.0);
					ui.label(egui::RichText::new("Quality").strong());
					ui.horizontal_wrapped(|ui| {
						ui.selectable_value(&mut self.height, 720, "720p");
						ui.selectable_value(&mut self.height, 1080, "1080p");
					});
					ui.label("Frame rate");
					ui.horizontal_wrapped(|ui| {
						for fps in [15, 30, 60] {
							ui.selectable_value(&mut self.fps, fps, format!("{fps} fps"));
						}
					});
					ui.label(
						egui::RichText::new("Quality selection does not require Nitro.")
							.small()
							.color(colors.muted),
					);
					ui.add_space(8.0);
					ui.checkbox(&mut self.cursor, "Show cursor");
					ui.label(
						egui::RichText::new(
							"Screen video only. Your call microphone keeps its current settings.",
						)
						.small()
						.color(colors.muted),
					);
					ui.add_space(8.0);
					ui.label(self.status);
				});
			ui.add_space(12.0);
			ui.horizontal_wrapped(|ui| {
				cancel = ui.button("Cancel").clicked();
				let allowed = !state.demo
					&& self.supported
					&& !self.busy && self.settings().is_some()
					&& state.voice.active.as_ref().is_some_and(|call| {
						matches!(call.phase, Phase::Connected | Phase::Waiting)
							&& state.can_stream(call.channel)
					});
				share = ui
					.add_enabled(
						allowed,
						egui::Button::new("Share screen").fill(colors.accent),
					)
					.clicked();
			});
		});
		cancel |= modal.should_close();
		if share && !cancel {
			self.request = self.settings().map(Request::Start);
		}
		if cancel || share {
			self.open = false;
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn settings_require_a_current_source_and_offer_high_quality_without_entitlements() {
		let mut picker = ScreenUi {
			selected: Some(SourceId::Window(7)),
			..Default::default()
		};
		assert!(picker.settings().is_none());
		picker.sources.push(Source {
			id: SourceId::Window(7),
			name: "Notes".into(),
		});
		picker.fps = 60;
		let settings = picker.settings().unwrap();
		assert_eq!(
			(settings.width, settings.height, settings.fps),
			(1920, 1080, 60)
		);
		assert_eq!(settings.bit_rate(), 16_000_000);
		picker.sources.clear();
		assert!(picker.settings().is_none());
	}
}
