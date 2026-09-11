//! Inline controls and one bounded texture. The desktop owns media decoding and playback.
use model::{Attachment, Id, Message};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VideoState {
	#[default]
	Idle,
	Loading,
	Playing,
	Paused,
	Ended,
	Failed(&'static str),
}
pub enum VideoCommand {
	Play(Attachment),
	Pause(bool),
	Seek(f64),
	Volume(f32),
	Stop,
}
pub struct VideoUi {
	pub active: Option<(Id, Id, Attachment)>,
	pub state: VideoState,
	pub position: f64,
	pub duration: f64,
	pub command: Option<VideoCommand>,
	pub seen: bool,
	pub volume: f32,
	texture: Option<egui::TextureHandle>,
}
impl Default for VideoUi {
	fn default() -> Self {
		Self {
			active: None,
			state: VideoState::Idle,
			position: 0.0,
			duration: 0.0,
			command: None,
			seen: false,
			volume: 1.0,
			texture: None,
		}
	}
}
impl VideoUi {
	pub fn stop(&mut self) {
		self.active = None;
		self.texture = None;
		self.state = VideoState::Idle;
		self.position = 0.0;
		self.duration = 0.0;
		self.seen = false;
		self.command = Some(VideoCommand::Stop);
	}
	/// The desktop rejects stale session/player frames before handing over decoded pixels.
	pub fn accept_frame(
		&mut self,
		ctx: &egui::Context,
		width: usize,
		height: usize,
		rgba: &[u8],
	) -> bool {
		if self.active.is_none()
			|| width == 0
			|| height == 0
			|| width > 1920
			|| height > 1920
			|| width * height > 1920 * 1080
			|| rgba.len() != width * height * 4
		{
			return false;
		}
		let image = egui::ColorImage::from_rgba_unmultiplied([width, height], rgba);
		if let Some(texture) = &mut self.texture {
			texture.set(image, egui::TextureOptions::LINEAR);
		} else {
			self.texture =
				Some(ctx.load_texture("inline-video", image, egui::TextureOptions::LINEAR));
		}
		true
	}
	fn toggle(&mut self, message: &Message, attachment: &Attachment, state: VideoState) {
		self.command = Some(match state {
			VideoState::Loading => {
				self.stop();
				VideoCommand::Stop
			}
			VideoState::Playing => VideoCommand::Pause(true),
			VideoState::Paused => VideoCommand::Pause(false),
			_ => {
				self.active = Some((message.channel, message.id, attachment.clone()));
				self.texture = None;
				self.state = VideoState::Loading;
				self.position = 0.0;
				self.duration = 0.0;
				VideoCommand::Play(attachment.clone())
			}
		});
	}
	pub fn show(&mut self, ui: &mut egui::Ui, message: &Message, attachment: &Attachment) {
		let colors = crate::design::palette(ui);
		let active = self.active.as_ref().is_some_and(|(channel, id, file)| {
			*channel == message.channel && *id == message.id && file == attachment
		});
		let state = if active { self.state } else { VideoState::Idle };
		let width = ui.available_width().clamp(1.0, 420.0);
		let card = egui::Frame::new()
			.fill(colors.raised)
			.stroke(egui::Stroke::new(1.0, colors.border))
			.corner_radius(8)
			.inner_margin(8)
			.show(ui, |ui| {
				let width = (width - 16.0).max(1.0);
				ui.set_width(width);
				ui.spacing_mut().item_spacing = egui::vec2(6.0, 4.0);
				ui.add(
					egui::Label::new(crate::design::semibold(ui, &attachment.filename, 13.0))
						.truncate(),
				)
				.on_hover_text(&attachment.filename);
				let (rect, _) =
					ui.allocate_exact_size(stage_size(attachment, width), egui::Sense::hover());
				ui.painter().rect_filled(rect, 4, egui::Color32::BLACK);
				if let Some(texture) = self.texture.as_ref().filter(|_| active) {
					let size = texture.size_vec2();
					let scale = (rect.width() / size.x).min(rect.height() / size.y);
					ui.painter().image(
						texture.id(),
						egui::Rect::from_center_size(rect.center(), size * scale),
						egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
						egui::Color32::WHITE,
					);
				} else {
					crate::icons::paint(
						ui.painter(),
						crate::icons::Icon::Video,
						egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(40.0)),
						colors.muted,
					);
				}
				ui.horizontal(|ui| {
					let label = match state {
						VideoState::Loading => "Cancel",
						VideoState::Playing => "Pause",
						VideoState::Ended => "Replay",
						VideoState::Failed(_) => "Retry",
						_ => "Play",
					};
					if ui
						.add_sized([62.0, 32.0], egui::Button::new(label))
						.clicked()
					{
						self.toggle(message, attachment, state);
					}
					let can_seek = active
						&& self.duration.is_finite()
						&& self.duration > 0.0
						&& matches!(state, VideoState::Playing | VideoState::Paused);
					let mut position = if active { self.position.max(0.0) } else { 0.0 };
					ui.spacing_mut().slider_width = ui.available_width().max(16.0);
					let seek = ui.add_enabled(
						can_seek,
						egui::Slider::new(
							&mut position,
							0.0..=if can_seek { self.duration } else { 1.0 },
						)
						.show_value(false)
						.trailing_fill(true),
					);
					seek.widget_info(|| egui::WidgetInfo::slider(can_seek, position, "Seek video"));
					if seek.on_hover_text("Seek video").changed() {
						self.command = Some(VideoCommand::Seek(position));
					}
				});
				ui.horizontal(|ui| {
					let (position, duration) = if active {
						(self.position, self.duration)
					} else {
						(0.0, 0.0)
					};
					ui.small(format!(
						"{} / {}",
						timestamp(position),
						if duration > 0.0 {
							timestamp(duration)
						} else {
							"--:--".into()
						}
					));
					ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
						ui.spacing_mut().slider_width = 52.0;
						let volume = ui
							.add(egui::Slider::new(&mut self.volume, 0.0..=1.0).show_value(false));
						volume.widget_info(|| {
							egui::WidgetInfo::slider(
								ui.is_enabled(),
								self.volume as f64,
								"Video volume",
							)
						});
						if volume.on_hover_text("Video volume").changed() {
							self.command = Some(VideoCommand::Volume(self.volume));
						}
						crate::icons::inline(ui, crate::icons::Icon::Speaker, 16.0, colors.muted);
					});
				});
				let status = match state {
					VideoState::Loading => "Loading video…",
					VideoState::Failed(error) => error,
					_ => "",
				};
				ui.add(
					egui::Label::new(egui::RichText::new(status).small().color(
						if matches!(state, VideoState::Failed(_)) {
							colors.danger
						} else {
							colors.muted
						},
					))
					.truncate(),
				)
				.on_hover_text(status);
			});
		if ui.is_rect_visible(card.response.rect)
			&& self.active.as_ref().is_some_and(|(channel, id, file)| {
				*channel == message.channel && *id == message.id && file == attachment
			}) {
			self.seen = true;
			if matches!(self.state, VideoState::Loading | VideoState::Playing) {
				ui.ctx()
					.request_repaint_after(std::time::Duration::from_millis(33));
			}
		}
	}
}
fn stage_size(attachment: &Attachment, width: f32) -> egui::Vec2 {
	let ratio = if attachment.media.width > 0 && attachment.media.height > 0 {
		attachment.media.width as f32 / attachment.media.height as f32
	} else {
		16.0 / 9.0
	};
	egui::vec2(width, (width / ratio.clamp(0.5, 3.0)).min(320.0))
}
pub(super) fn estimated_height(attachment: &Attachment, width: f32) -> f32 {
	stage_size(attachment, (width.min(420.0) - 16.0).max(1.0)).y + 150.0
}
fn timestamp(seconds: f64) -> String {
	let seconds = seconds.max(0.0) as u64;
	format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn video_controls_are_explicit_bounded_and_keyboard_operable() {
		let mut message = test_support::message(1, Id(2));
		let attachment = Attachment {
			id: Id(3),
			filename: "synthetic-clip.MOV".into(),
			description: None,
			content_type: Some("application/octet-stream".into()),
			size: 128,
			media: model::EmbedMedia {
				width: 1920,
				height: 1080,
				..Default::default()
			},
			spoiler: false,
			duration_ms: None,
			waveform: Vec::new(),
		};
		message.attachments.push(attachment.clone());
		for (width, theme) in [(220.0, egui::Theme::Dark), (420.0, egui::Theme::Light)] {
			let ctx = egui::Context::default();
			crate::design::apply(&ctx);
			ctx.set_theme(theme);
			let mut video = VideoUi::default();
			let frame = |video: &mut VideoUi, key: Option<egui::Key>| {
				ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(width + 16.0, 600.0),
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
						crate::attachments::show(
							ui,
							&message,
							&mut crate::avatars::Avatars::default(),
							&mut None,
							&mut None,
							&mut crate::attachments::DownloadUi::default(),
							&mut crate::AudioUi::default(),
							video,
							false,
						);
						assert!(ui.min_rect().width() <= width + 2.0);
					},
				)
				.drop_without_applying_deltas();
			};
			frame(&mut video, None);
			assert!(video.command.is_none() && video.active.is_none());
			assert!(!video.accept_frame(&ctx, 1, 1, &[0, 0, 0, 255]));
			for key in [egui::Key::Tab, egui::Key::Enter] {
				frame(&mut video, Some(key));
			}
			assert!(
				matches!(video.command.take(), Some(VideoCommand::Play(file)) if file == attachment)
			);
			assert!(video.seen);
			assert!(!video.accept_frame(&ctx, 1921, 1080, &[]));
			assert!(!video.accept_frame(&ctx, 1, 1921, &[]));
			assert!(!video.accept_frame(&ctx, 1920, 1920, &[]));
			assert!(!video.accept_frame(&ctx, usize::MAX, usize::MAX, &[]));
			assert!(!video.accept_frame(&ctx, 1, 1, &[0]));
			assert!(video.accept_frame(&ctx, 1, 1920, &vec![0; 1920 * 4]));
			assert!(video.accept_frame(&ctx, 1, 1, &[0, 0, 0, 255]));
			video.state = VideoState::Playing;
			video.duration = 12.0;
			frame(&mut video, Some(egui::Key::Enter));
			assert!(matches!(
				video.command.take(),
				Some(VideoCommand::Pause(true))
			));
			video.state = VideoState::Paused;
			frame(&mut video, Some(egui::Key::Enter));
			assert!(matches!(
				video.command.take(),
				Some(VideoCommand::Pause(false))
			));
			for key in [egui::Key::Tab, egui::Key::ArrowRight] {
				frame(&mut video, Some(key));
			}
			assert!(matches!(video.command.take(), Some(VideoCommand::Seek(value)) if value > 0.0));
			for key in [egui::Key::Tab, egui::Key::ArrowLeft] {
				frame(&mut video, Some(key));
			}
			assert!(
				matches!(video.command.take(), Some(VideoCommand::Volume(value)) if value < 1.0)
			);
			video.state = VideoState::Failed("Unsupported video codec");
			frame(&mut video, None);
			video.stop();
			assert!(video.active.is_none() && video.texture.is_none());
			assert!(matches!(video.command, Some(VideoCommand::Stop)));
		}
	}
}
