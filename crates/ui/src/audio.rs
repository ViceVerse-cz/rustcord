//! Inline controls only. The desktop owns downloading, decoding and the output device.
use model::{Attachment, Id, Message};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum AudioState {
	#[default]
	Idle,
	Loading,
	Playing,
	Paused,
	Ended,
	Failed(&'static str),
}
pub enum AudioCommand {
	Play(Attachment),
	Pause(bool),
	Seek(f64),
	Volume(f32),
	Stop,
}
pub struct AudioUi {
	pub active: Option<(Id, Id, Attachment)>,
	pub state: AudioState,
	pub position: f64,
	pub duration: f64,
	pub command: Option<AudioCommand>,
	pub seen: bool,
	pub volume: f32,
}
impl Default for AudioUi {
	fn default() -> Self {
		Self {
			active: None,
			state: AudioState::Idle,
			position: 0.0,
			duration: 0.0,
			command: None,
			seen: false,
			volume: 1.0,
		}
	}
}
impl AudioUi {
	pub fn stop(&mut self) {
		self.active = None;
		self.state = AudioState::Idle;
		self.command = Some(AudioCommand::Stop);
	}
	pub fn show(&mut self, ui: &mut egui::Ui, message: &Message, attachment: &Attachment) {
		let colors = crate::design::palette(ui);
		let active = self.active.as_ref().is_some_and(|(channel, id, file)| {
			*channel == message.channel && *id == message.id && file == attachment
		});
		let state = if active { self.state } else { AudioState::Idle };
		let width = ui.available_width().min(380.0);
		let card = egui::Frame::new()
			.fill(colors.raised)
			.stroke(egui::Stroke::new(1.0, colors.border))
			.corner_radius(8)
			.inner_margin(10)
			.show(ui, |ui| {
				ui.set_width((width - 20.0).max(1.0));
				ui.add(
					egui::Label::new(crate::design::semibold(ui, &attachment.filename, 13.0))
						.truncate(),
				)
				.on_hover_text(&attachment.filename);
				ui.horizontal(|ui| {
					let label = match state {
						AudioState::Loading => "Cancel",
						AudioState::Playing => "Pause",
						AudioState::Ended => "Replay",
						AudioState::Failed(_) => "Retry",
						_ => "Play",
					};
					if ui.button(label).clicked() {
						self.command = Some(match state {
							AudioState::Loading => {
								self.active = None;
								AudioCommand::Stop
							}
							AudioState::Playing => AudioCommand::Pause(true),
							AudioState::Paused => AudioCommand::Pause(false),
							_ => {
								self.active =
									Some((message.channel, message.id, attachment.clone()));
								self.state = AudioState::Loading;
								self.position = 0.0;
								self.duration = 0.0;
								AudioCommand::Play(attachment.clone())
							}
						});
					}
					let duration = if active { self.duration } else { 0.0 };
					let mut position = if active { self.position } else { 0.0 };
					ui.spacing_mut().slider_width =
						(ui.available_width() - ui.spacing().item_spacing.x - 36.0).max(24.0);
					let seek = ui.add_enabled(
						duration > 0.0 && matches!(state, AudioState::Playing | AudioState::Paused),
						egui::Slider::new(&mut position, 0.0..=duration.max(1.0))
							.show_value(false)
							.text("Seek"),
					);
					if seek.changed() {
						self.command = Some(AudioCommand::Seek(position));
					}
				});
				ui.horizontal_wrapped(|ui| {
					let position = if active { self.position } else { 0.0 };
					let duration = if active && self.duration > 0.0 {
						timestamp(self.duration)
					} else {
						"--:--".into()
					};
					ui.small(format!("{} / {duration}", timestamp(position)));
					ui.spacing_mut().slider_width = 60.0;
					if ui
						.add(
							egui::Slider::new(&mut self.volume, 0.0..=1.0)
								.show_value(false)
								.text("Volume"),
						)
						.changed()
					{
						self.command = Some(AudioCommand::Volume(self.volume));
					}
				});
				match state {
					AudioState::Loading => {
						ui.small("Loading audio…");
					}
					AudioState::Failed(error) => {
						ui.colored_label(colors.danger, error);
					}
					_ => {}
				}
			});
		if ui.is_rect_visible(card.response.rect)
			&& self.active.as_ref().is_some_and(|(channel, id, file)| {
				*channel == message.channel && *id == message.id && file == attachment
			}) {
			self.seen = true;
		}
	}
}
fn timestamp(seconds: f64) -> String {
	let seconds = seconds.max(0.0) as u64;
	format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn audio_is_explicit_keyboard_operable_and_fits_narrow_cards() {
		let state = test_support::audio_demo_state();
		let message = state.timeline.iter().last().unwrap();
		let file = &message.attachments[0];
		for width in [220.0, 380.0] {
			let ctx = egui::Context::default();
			let mut audio = AudioUi::default();
			let frame = |audio: &mut AudioUi, key: Option<egui::Key>| {
				ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(width + 16.0, 300.0),
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
					|ui| {
						ui.set_width(width);
						ui.spacing_mut().item_spacing.x = 16.0;
						audio.show(ui, message, file);
						assert!(ui.min_rect().width() <= width + 2.0);
					},
				)
				.drop_without_applying_deltas();
			};
			frame(&mut audio, None);
			assert!(audio.active.is_none() && audio.command.is_none());
			for key in [egui::Key::Tab, egui::Key::Enter] {
				frame(&mut audio, Some(key));
			}
			assert!(matches!(audio.command.take(), Some(AudioCommand::Play(a)) if a == *file));
			assert!(audio.seen);
			audio.state = AudioState::Playing;
			audio.duration = 12.0;
			frame(&mut audio, Some(egui::Key::Enter));
			assert!(matches!(
				audio.command.take(),
				Some(AudioCommand::Pause(true))
			));
			audio.state = AudioState::Paused;
			frame(&mut audio, Some(egui::Key::Enter));
			assert!(matches!(
				audio.command.take(),
				Some(AudioCommand::Pause(false))
			));
			for key in [egui::Key::Tab, egui::Key::ArrowRight] {
				frame(&mut audio, Some(key));
			}
			assert!(matches!(audio.command.take(), Some(AudioCommand::Seek(value)) if value > 0.0));
			for key in [egui::Key::Tab, egui::Key::ArrowLeft] {
				frame(&mut audio, Some(key));
			}
			assert!(
				matches!(audio.command.take(), Some(AudioCommand::Volume(value)) if value < 1.0)
			);
			audio.stop();
			assert!(audio.active.is_none());
			assert!(matches!(audio.command, Some(AudioCommand::Stop)));
		}
		assert_eq!(timestamp(125.4), "2:05");
	}
}
