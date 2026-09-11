//! Inline service attachments; decoded pixels reuse the existing bounded media cache.
use crate::{
	avatars::Avatars,
	design,
	icons::{self, Icon},
	markdown::external_url,
};
use egui::{Color32, Rect, RichText, Sense, Stroke, StrokeKind};
use model::{Attachment, Id, Message};

/// Rough content family of a file, chosen from its name and reported type only.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FileKind {
	Image,
	Pdf,
	Archive,
	Text,
	Code,
	Audio,
	Video,
	Other,
}
pub fn file_kind(filename: &str, content_type: Option<&str>) -> FileKind {
	let extension = filename
		.rsplit_once('.')
		.map(|(_, extension)| extension.to_ascii_lowercase())
		.unwrap_or_default();
	let mime = content_type
		.map(|kind| {
			kind.split(';')
				.next()
				.unwrap_or(kind)
				.trim()
				.to_ascii_lowercase()
		})
		.unwrap_or_default();
	match extension.as_str() {
		"png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "bmp" | "tiff" | "heic" | "svg" => {
			return FileKind::Image;
		}
		"pdf" => return FileKind::Pdf,
		"zip" | "7z" | "rar" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "zst" => {
			return FileKind::Archive;
		}
		"txt" | "md" | "rtf" | "log" | "csv" | "doc" | "docx" | "odt" => return FileKind::Text,
		"rs" | "py" | "js" | "ts" | "tsx" | "jsx" | "json" | "toml" | "yaml" | "yml" | "html"
		| "css" | "c" | "h" | "cpp" | "hpp" | "java" | "kt" | "swift" | "go" | "rb" | "sh"
		| "xml" | "sql" => return FileKind::Code,
		"mp3" | "wav" | "ogg" | "flac" | "m4a" | "aac" | "opus" => return FileKind::Audio,
		"mp4" | "mov" | "webm" | "mkv" | "avi" | "m4v" => return FileKind::Video,
		_ => {}
	}
	if mime.starts_with("image/") {
		FileKind::Image
	} else if mime == "application/pdf" {
		FileKind::Pdf
	} else if mime.starts_with("audio/") {
		FileKind::Audio
	} else if mime.starts_with("video/") {
		FileKind::Video
	} else if mime.starts_with("text/") {
		FileKind::Text
	} else if mime.contains("zip") || mime.contains("compressed") || mime.contains("tar") {
		FileKind::Archive
	} else {
		FileKind::Other
	}
}
impl FileKind {
	pub fn icon(self) -> Icon {
		match self {
			FileKind::Image => Icon::FileImage,
			FileKind::Pdf => Icon::FilePdf,
			FileKind::Archive => Icon::FileZip,
			FileKind::Text => Icon::FileText,
			FileKind::Code => Icon::FileCode,
			FileKind::Audio => Icon::FileAudio,
			FileKind::Video => Icon::FileVideo,
			FileKind::Other => Icon::File,
		}
	}
	/// Discord-style glyph tint: red documents, amber archives, blurple media, muted text.
	pub fn tint(self, colors: &design::Palette) -> Color32 {
		match self {
			FileKind::Pdf => colors.danger,
			FileKind::Archive => colors.warning,
			FileKind::Image | FileKind::Audio | FileKind::Video => colors.accent,
			FileKind::Code => colors.link,
			FileKind::Text | FileKind::Other => colors.muted,
		}
	}
}
/// Human file size in the units Discord shows next to attachments.
pub fn format_size(bytes: u64) -> String {
	const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
	if bytes < 1024 {
		return format!("{bytes} bytes");
	}
	let mut value = bytes as f64 / 1024.0;
	let mut unit = 0;
	while value >= 1024.0 && unit + 1 < UNITS.len() {
		value /= 1024.0;
		unit += 1;
	}
	if value >= 100.0 {
		format!("{value:.0} {}", UNITS[unit])
	} else {
		format!("{value:.2} {}", UNITS[unit])
	}
}

/// Card for a file that will be uploaded with the next message. Returns `true` when the user
/// asks to remove it.
pub fn pending_card(
	ui: &mut egui::Ui,
	filename: &str,
	bytes: u64,
	preview: Option<&egui::TextureHandle>,
	removable: bool,
) -> bool {
	const WIDTH: f32 = 176.0;
	const HEIGHT: f32 = 168.0;
	// Discord floats the action pill over the card's top edge; reserve that overhang.
	const OVERHANG: f32 = 10.0;
	let colors = design::palette(ui);
	let kind = file_kind(filename, None);
	let (allocated, _) =
		ui.allocate_exact_size(egui::vec2(WIDTH + 12.0, HEIGHT + OVERHANG), Sense::hover());
	let card = Rect::from_min_size(
		allocated.left_bottom() - egui::vec2(0.0, HEIGHT),
		egui::vec2(WIDTH, HEIGHT),
	);
	ui.painter().rect(
		card,
		8,
		colors.sidebar,
		Stroke::new(1.0, colors.border),
		StrokeKind::Inside,
	);
	let preview_rect =
		Rect::from_min_size(card.min + egui::vec2(8.0, 8.0), egui::vec2(160.0, 108.0));
	ui.painter().rect_filled(preview_rect, 4, colors.base);
	match preview {
		Some(texture) => {
			let size = texture.size_vec2();
			let scale = (preview_rect.width() / size.x)
				.min(preview_rect.height() / size.y)
				.clamp(f32::EPSILON, 1.0);
			let fitted = Rect::from_center_size(preview_rect.center(), size * scale);
			ui.put(
				fitted,
				egui::Image::from_texture(texture)
					.fit_to_exact_size(fitted.size())
					.corner_radius(4),
			);
		}
		None => {
			icons::paint(
				ui.painter(),
				kind.icon(),
				Rect::from_center_size(preview_rect.center(), egui::Vec2::splat(56.0)),
				kind.tint(&colors),
			);
		}
	}
	let name = Rect::from_min_size(
		preview_rect.left_bottom() + egui::vec2(0.0, 6.0),
		egui::vec2(preview_rect.width(), 18.0),
	);
	ui.put(
		name,
		egui::Label::new(design::semibold(ui, filename, 13.0).color(colors.text_strong))
			.truncate()
			.selectable(false),
	)
	.on_hover_text(filename);
	let size = Rect::from_min_size(name.left_bottom(), egui::vec2(name.width(), 16.0));
	ui.put(
		size,
		egui::Label::new(
			RichText::new(format_size(bytes))
				.size(12.0)
				.color(colors.muted),
		)
		.truncate()
		.selectable(false),
	);
	let pill = Rect::from_min_size(
		egui::pos2(card.right() - 28.0, card.top() - OVERHANG),
		egui::vec2(36.0, 36.0),
	);
	ui.painter().rect(
		pill,
		6,
		colors.raised,
		Stroke::new(1.0, colors.border),
		StrokeKind::Inside,
	);
	ui.scope_builder(
		egui::UiBuilder::new().max_rect(pill.shrink(2.0)).layout(
			egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
		),
		|ui| {
			ui.add_enabled_ui(removable, |ui| {
				icons::button(ui, Icon::Trash, 32.0, "Remove attachment").clicked()
			})
			.inner
		},
	)
	.inner
}

/// Discord-style row for a received file without an inline preview.
fn file_card(
	ui: &mut egui::Ui,
	attachment: &Attachment,
	download: &mut DownloadUi,
	opening: &mut Option<String>,
	demo: bool,
) {
	let colors = design::palette(ui);
	let kind = file_kind(&attachment.filename, attachment.content_type.as_deref());
	egui::Frame::new()
		.fill(colors.raised)
		.stroke(Stroke::new(1.0, colors.border))
		.corner_radius(8)
		.inner_margin(egui::Margin::symmetric(12, 10))
		.show(ui, |ui| {
			ui.set_width(ui.available_width().min(432.0));
			ui.horizontal(|ui| {
				ui.spacing_mut().item_spacing.x = 10.0;
				icons::inline(ui, kind.icon(), 32.0, kind.tint(&colors));
				ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
					ui.spacing_mut().item_spacing.x = 6.0;
					download_button(ui, attachment, download, demo);
					open_original(ui, attachment, opening);
					ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
						ui.vertical(|ui| {
							ui.spacing_mut().item_spacing.y = 0.0;
							ui.add(
								egui::Label::new(
									design::medium(ui, &attachment.filename, 14.0)
										.color(colors.link),
								)
								.truncate()
								.selectable(false),
							)
							.on_hover_text(&attachment.filename);
							ui.add(
								egui::Label::new(
									RichText::new(format_size(attachment.size))
										.size(12.0)
										.color(colors.muted),
								)
								.selectable(false),
							);
						});
					});
				});
			});
		});
}

#[allow(clippy::too_many_arguments)]
pub fn show(
	ui: &mut egui::Ui,
	message: &Message,
	images: &mut Avatars,
	viewing: &mut Option<(Id, Id)>,
	opening: &mut Option<String>,
	download: &mut DownloadUi,
	audio: &mut crate::audio::AudioUi,
	demo: bool,
) {
	for group in message
		.attachments
		.chunk_by(|a, b| a.is_image() == b.is_image())
	{
		if group[0].is_image() {
			let (columns, size) = image_layout(group.len(), ui.available_width());
			ui.scope(|ui| {
				ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
				for row in group.chunks(columns) {
					ui.horizontal_top(|ui| {
						for attachment in row {
							ui.push_id(("attachment", attachment.id), |ui| {
								let response = images
									.show_embed(ui, &attachment.media, size, demo)
									.interact(egui::Sense::click());
								response.widget_info(|| {
									egui::WidgetInfo::labeled(
										egui::WidgetType::Button,
										ui.is_enabled(),
										format!("View image {}", attachment.filename),
									)
								});
								if response
									.on_hover_text(
										attachment
											.description
											.as_deref()
											.unwrap_or("Enlarge image"),
									)
									.clicked()
								{
									*viewing = Some((message.id, attachment.id));
								}
							});
						}
					});
				}
			});
		} else {
			for attachment in group {
				ui.push_id(("attachment", attachment.id), |ui| {
					if attachment.is_audio() {
						audio.show(ui, message, attachment);
						ui.horizontal_wrapped(|ui| {
							download_button(ui, attachment, download, demo);
							open_original(ui, attachment, opening);
						});
					} else {
						file_card(ui, attachment, download, opening, demo);
					}
					ui.add_space(6.0);
				});
			}
		}
	}
}
fn image_layout(count: usize, width: f32) -> (usize, egui::Vec2) {
	let columns = if count > 1 && width >= 280.0 { 2 } else { 1 };
	let width = ((width.min(420.0) - (columns - 1) as f32 * 6.0) / columns as f32).max(1.0);
	(
		columns,
		egui::vec2(width, if count > 1 { 180.0 } else { 280.0 }),
	)
}

fn open_original(ui: &mut egui::Ui, attachment: &Attachment, opening: &mut Option<String>) {
	if let Some(target) = attachment.media.url.as_deref().and_then(external_url)
		&& ui.small_button("Open original…").clicked()
	{
		*opening = Some(target);
	}
}
#[derive(Default)]
pub struct DownloadUi {
	pub request: Option<Attachment>,
	pub cancel_requested: bool,
	pub active: bool,
	pub status: String,
}
impl DownloadUi {
	pub fn show_status(&mut self, ui: &mut egui::Ui) {
		if self.active || !self.status.is_empty() {
			ui.horizontal_wrapped(|ui| {
				ui.small(&self.status);
				if self.active && ui.small_button("Cancel download").clicked() {
					self.cancel_requested = true;
				}
			});
		}
	}
}
fn download_button(
	ui: &mut egui::Ui,
	attachment: &Attachment,
	download: &mut DownloadUi,
	demo: bool,
) {
	if ui
		.add_enabled(
			!demo && !download.active && download.request.is_none(),
			egui::Button::new("Download"),
		)
		.on_hover_text("Choose where to save this file · up to 100 MiB")
		.on_disabled_hover_text(if demo {
			"Downloads are disabled for synthetic attachments"
		} else {
			"A download is already active"
		})
		.clicked()
	{
		download.request = Some(attachment.clone());
	}
}
/// Resolve against the current message every frame: deleted or hidden media cannot linger.
fn gallery_step(attachments: &[Attachment], current: Id, previous: bool) -> Option<Id> {
	let images: Vec<_> = attachments.iter().filter(|a| a.is_image()).collect();
	let index = images.iter().position(|a| a.id == current)?;
	let index = if previous {
		(index + images.len() - 1) % images.len()
	} else {
		(index + 1) % images.len()
	};
	Some(images[index].id)
}

pub fn viewer(
	ui: &mut egui::Ui,
	attachments: &[Attachment],
	current: Id,
	images: &mut Avatars,
	download: &mut DownloadUi,
	demo: bool,
) -> Option<Id> {
	let mut current = current;
	if ui
		.ctx()
		.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft))
	{
		current = gallery_step(attachments, current, true)?;
	}
	if ui
		.ctx()
		.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight))
	{
		current = gallery_step(attachments, current, false)?;
	}
	let attachment = attachments
		.iter()
		.find(|a| a.id == current && a.is_image())?;
	let count = attachments.iter().filter(|a| a.is_image()).count();
	let index = attachments
		.iter()
		.filter(|a| a.is_image())
		.position(|a| a.id == current)?;
	let size = (ui.ctx().content_rect().size() - egui::vec2(32.0, 32.0)).max(egui::vec2(1.0, 1.0));
	let mut close = false;
	// Modal input capture prevents clicks and keys reaching the conversation. No dialog frame.
	let overlay = egui::Modal::new(egui::Id::unique("attachment-viewer"))
		.backdrop_color(Color32::from_black_alpha(240))
		.frame(egui::Frame::NONE)
		.show(ui.ctx(), |ui| {
			ui.set_min_size(size);
			ui.set_max_size(size);
			*ui.visuals_mut() = egui::Visuals::dark();
			ui.visuals_mut().override_text_color = Some(Color32::WHITE);
			ui.horizontal(|ui| {
				if count > 1 {
					ui.label(format!("{} / {count}", index + 1));
				}
				ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
					close = icons::button(ui, Icon::Close, 32.0, "Close image (Esc)").clicked();
					download_button(ui, attachment, download, demo);
				});
			});
			let available = egui::vec2(size.x, (size.y - 80.0).max(1.0));
			let (rect, _) = ui.allocate_exact_size(available, Sense::hover());
			let image_rect = rect.shrink2(egui::vec2(if count > 1 { 44.0 } else { 0.0 }, 0.0));
			ui.scope_builder(
				egui::UiBuilder::new().max_rect(image_rect).layout(
					egui::Layout::centered_and_justified(egui::Direction::TopDown),
				),
				|ui| {
					images
						.show_large(ui, &attachment.media, image_rect.size(), demo)
						.on_hover_text(
							attachment
								.description
								.as_deref()
								.unwrap_or(&attachment.filename),
						);
				},
			);
			if count > 1 {
				for (previous, center, label) in [
					(
						true,
						rect.left_center() + egui::vec2(18.0, 0.0),
						"Previous image (←)",
					),
					(
						false,
						rect.right_center() - egui::vec2(18.0, 0.0),
						"Next image (→)",
					),
				] {
					ui.scope_builder(
						egui::UiBuilder::new()
							.max_rect(Rect::from_center_size(center, egui::vec2(36.0, 44.0))),
						|ui| {
							if ui
								.add_sized(
									[36.0, 44.0],
									egui::Button::new(if previous { "←" } else { "→" }),
								)
								.on_hover_text(label)
								.clicked()
							{
								current =
									gallery_step(attachments, current, previous).unwrap_or(current);
							}
						},
					);
				}
			}
			download.show_status(ui);
		});
	(!close && !overlay.should_close()).then_some(current)
}
pub fn estimated_height(attachments: &[Attachment], width: f32) -> f32 {
	attachments
		.chunk_by(|a, b| a.is_image() == b.is_image())
		.map(|group| {
			if group[0].is_image() {
				let (columns, size) = image_layout(group.len(), width);
				group.len().div_ceil(columns) as f32 * (size.y + 6.0)
			} else {
				group
					.iter()
					.map(|attachment| if attachment.is_audio() { 138.0 } else { 62.0 })
					.sum()
			}
		})
		.sum()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn image_gallery_wraps_without_filenames_and_opens_each_attachment() {
		let mut message = test_support::message(1, Id(2));
		message.attachments = (0..3)
			.map(|id| Attachment {
				id: Id(id + 10),
				filename: format!("synthetic-image-{id}.png"),
				description: None,
				content_type: Some("image/png".into()),
				size: 512,
				spoiler: false,
				media: model::EmbedMedia {
					width: 640,
					height: 360,
					..Default::default()
				},
			})
			.collect();
		let mut file = message.attachments[0].clone();
		file.id = Id(20);
		file.filename = "synthetic-report.pdf".into();
		file.content_type = Some("application/pdf".into());
		message.attachments.push(file);
		assert_eq!(
			gallery_step(&message.attachments, Id(10), true),
			Some(Id(12))
		);
		assert_eq!(
			gallery_step(&message.attachments, Id(12), false),
			Some(Id(10))
		);
		assert_eq!(
			gallery_step(&message.attachments, Id(10), false),
			Some(Id(11))
		);
		assert_eq!(gallery_step(&message.attachments, Id(20), false), None);
		assert_eq!(gallery_step(&[], Id(10), false), None);
		assert_eq!(
			gallery_step(&message.attachments[..1], Id(10), true),
			Some(Id(10))
		);

		for width in [420.0, 240.0] {
			let ctx = egui::Context::default();
			let mut images = Avatars::default();
			let mut viewing = None;
			let mut opening = None;
			let mut download = DownloadUi::default();
			let mut frame = |events| {
				ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(width + 16.0, 800.0),
						)),
						events,
						..Default::default()
					},
					|ui| {
						ui.set_width(width);
						show(
							ui,
							&message,
							&mut images,
							&mut viewing,
							&mut opening,
							&mut download,
							&mut crate::audio::AudioUi::default(),
							true,
						);
					},
				)
			};
			frame(vec![]).drop_without_applying_deltas();
			let output = frame(vec![]);
			let rects: Vec<_> = output
				.shapes
				.iter()
				.filter_map(|shape| match &shape.shape {
					egui::Shape::Rect(shape)
						if shape.corner_radius == egui::CornerRadius::same(5) =>
					{
						Some(shape.rect)
					}
					_ => None,
				})
				.collect();
			let labels: Vec<_> = output
				.shapes
				.iter()
				.filter_map(|shape| match &shape.shape {
					egui::Shape::Text(shape) => Some(shape.galley.job.text.as_str()),
					_ => None,
				})
				.collect();
			assert_eq!(rects.len(), 3);
			assert!(labels.contains(&"synthetic-report.pdf"));
			assert!(
				labels
					.iter()
					.all(|label| !label.contains("synthetic-image-"))
			);
			for rect in &rects {
				assert!(rect.width() <= width);
				assert!((rect.width() / rect.height() - 16.0 / 9.0).abs() < 0.01);
			}
			if width >= 280.0 {
				assert_eq!(rects[0].top(), rects[1].top());
				assert!(rects[1].left() > rects[0].right());
			} else {
				assert!(rects[1].top() > rects[0].bottom());
			}
			assert!(rects[2].top() > rects[0].bottom());
			let pos = rects[1].center();
			output.drop_without_applying_deltas();
			for pressed in [true, false] {
				frame(vec![
					egui::Event::PointerMoved(pos),
					egui::Event::PointerButton {
						pos,
						button: egui::PointerButton::Primary,
						pressed,
						modifiers: egui::Modifiers::NONE,
					},
				])
				.drop_without_applying_deltas();
			}
			assert_eq!(viewing, Some((message.id, message.attachments[1].id)));
			assert!(opening.is_none() && download.request.is_none());
			assert!(images.take_requests().is_empty());
		}
		assert!(
			estimated_height(&message.attachments, 420.0)
				< estimated_height(&message.attachments, 240.0)
		);
	}

	#[test]
	fn nonimage_download_requires_keyboard_action_and_respects_single_job_controls() {
		fn frame(ctx: &egui::Context, key: Option<egui::Key>, draw: impl FnMut(&mut egui::Ui)) {
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(640.0, 480.0),
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
				draw,
			);
			assert!(output.platform_output.commands.is_empty());
			output.drop_without_applying_deltas();
		}
		let attachment = Attachment {
			id: Id(3),
			filename: "synthetic-report.pdf".into(),
			description: None,
			content_type: Some("application/pdf".into()),
			size: 512,
			spoiler: false,
			media: model::EmbedMedia {
				url: Some("https://cdn.discordapp.com/attachments/2/3/synthetic-report.pdf".into()),
				..Default::default()
			},
		};
		let message = Message {
			id: Id(1),
			channel: Id(2),
			author: model::User {
				id: Id(4),
				name: "Synthetic".into(),
				avatar: None,
				discriminator: 0,
			},
			content: String::new(),
			mention_roles: vec![],
			mention_everyone: false,
			suppress_notifications: false,
			mentions: vec![],
			edited: false,
			edited_at: None,
			revision: 0,
			nonce: None,
			reply_to: None,
			kind: 0,
			reply_deleted: false,
			unsupported: false,
			extra_content: Default::default(),
			embeds: vec![],
			embeds_suppressed: false,
			reactions: None,
			attachments: vec![attachment.clone()],
		};
		let ctx = egui::Context::default();
		let mut images = Avatars::default();
		let mut viewing = None;
		let mut opening = None;
		let mut download = DownloadUi::default();
		frame(&ctx, None, |ui| {
			show(
				ui,
				&message,
				&mut images,
				&mut viewing,
				&mut opening,
				&mut download,
				&mut crate::audio::AudioUi::default(),
				false,
			)
		});
		assert!(images.take_requests().is_empty());
		assert!(viewing.is_none() && opening.is_none() && download.request.is_none());
		for key in [egui::Key::Tab, egui::Key::Enter] {
			frame(&ctx, Some(key), |ui| {
				show(
					ui,
					&message,
					&mut images,
					&mut viewing,
					&mut opening,
					&mut download,
					&mut crate::audio::AudioUi::default(),
					false,
				)
			});
		}
		assert_eq!(download.request.as_ref(), Some(&attachment));
		assert!(download.request.as_ref().unwrap().bytes() <= model::MAX_ATTACHMENT_BYTES);
		assert!(images.take_requests().is_empty());
		assert!(viewing.is_none() && opening.is_none());

		for (demo, active, pending) in [
			(true, false, false),
			(false, true, false),
			(false, false, true),
		] {
			let ctx = egui::Context::default();
			let mut previous = attachment.clone();
			previous.id = Id(9);
			let mut download = DownloadUi {
				active,
				request: pending.then(|| previous.clone()),
				..Default::default()
			};
			for key in [None, Some(egui::Key::Tab), Some(egui::Key::Enter)] {
				frame(&ctx, key, |ui| {
					download_button(ui, &attachment, &mut download, demo)
				});
			}
			assert_eq!(download.request, pending.then_some(previous));
		}
		let ctx = egui::Context::default();
		let mut download = DownloadUi {
			active: true,
			status: "Downloading: 256 / 512 bytes".into(),
			..Default::default()
		};
		frame(&ctx, None, |ui| download.show_status(ui));
		assert!(!download.cancel_requested);
		for key in [egui::Key::Tab, egui::Key::Enter] {
			frame(&ctx, Some(key), |ui| download.show_status(ui));
		}
		assert!(download.cancel_requested);
	}
}
