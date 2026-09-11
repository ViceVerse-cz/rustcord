use client_core::{Pending, State};
use egui::RichText;
use model::Delivery;

pub struct Upload {
	pub nonce: String,
	pub preview: Option<egui::TextureHandle>,
	pub bytes: u64,
	pub progress: Option<(u64, u64)>,
}

pub fn show(
	ui: &mut egui::Ui,
	pending: &Pending,
	compact: bool,
	state: &State,
	avatars: &mut crate::avatars::Avatars,
	upload: Option<&Upload>,
	(restore, cancel): (&mut Option<String>, &mut bool),
) {
	let colors = crate::design::palette(ui);
	let upload = upload.filter(|upload| upload.nonce == pending.nonce);
	egui::Frame::NONE
		.inner_margin(egui::Margin { left: 16, right: 16, top: if compact { 1 } else { 14 }, bottom: 1 })
		.show(ui, |ui| {
			ui.spacing_mut().item_spacing = egui::vec2(16.0, 4.0);
			ui.horizontal_top(|ui| {
				if compact {
					ui.allocate_exact_size(egui::vec2(40.0, 22.0), egui::Sense::hover());
				} else { ui.scope(|ui| {
					ui.set_opacity(0.55);
					if let Some(user) = &state.user {
						avatars.show(ui, user, 40.0, state.demo);
					} else {
						ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
					}
				}); }
				ui.vertical(|ui| {
					ui.set_width(ui.available_width());
					if !compact || matches!(pending.delivery, Delivery::Rejected | Delivery::Ambiguous) {
					ui.horizontal_wrapped(|ui| {
						ui.spacing_mut().item_spacing.x = 8.0;
						if !compact { ui.label(crate::design::medium(ui, state.user.as_ref().map_or("You", |u| &u.name), 15.5).color(colors.muted)); }
						ui.label(RichText::new(match pending.delivery {
							Delivery::Sending => "Sending…",
							Delivery::Ambiguous => "Delivery unknown",
							Delivery::Rejected => "Not sent",
							Delivery::Confirmed => "Sent",
						}).size(12.0).color(if pending.delivery == Delivery::Rejected { colors.danger } else { colors.muted }));
					}); }
					if !pending.content.is_empty() {
						ui.add(egui::Label::new(RichText::new(&pending.content).color(if pending.delivery == Delivery::Rejected { colors.danger } else { colors.muted })).wrap().selectable(true)).on_hover_text(match pending.delivery {
 Delivery::Sending => "Sending…", Delivery::Ambiguous => "Delivery unknown", Delivery::Rejected => "Not sent", Delivery::Confirmed => "Sent", });
					}
					if let Some(filename) = &pending.attachment {
						ui.scope(|ui| {
							ui.set_max_width(ui.available_width().min(360.0));
							if let Some(texture) = upload.and_then(|u| u.preview.as_ref()) {
								ui.add(egui::Image::new(texture).max_size(egui::vec2(ui.available_width(), 220.0)).tint(egui::Color32::from_gray(145)));
							}
							egui::Frame::NONE.fill(colors.raised).corner_radius(6.0).inner_margin(10.0).show(ui, |ui| {
								ui.set_width((ui.available_width()).min(340.0));
								ui.add(egui::Label::new(RichText::new(filename).color(colors.muted)).truncate()).on_hover_text(filename);
								if let Some(upload) = upload {
									ui.label(RichText::new(crate::attachments::format_size(upload.bytes)).small().color(colors.muted));
									}
								if pending.delivery == Delivery::Sending {
										let (fraction, label) = match upload.and_then(|u| u.progress) {
											Some((sent, total)) if total > 0 && sent < total => ((sent as f32 / total as f32).clamp(0.0, 1.0), format!("Uploading · {}%", sent.saturating_mul(100) / total)),
											Some((_, total)) if total > 0 => (1.0, "Sending… · 100%".into()),
											_ => (0.0, "Preparing upload…".into()),
										};
										ui.add(egui::ProgressBar::new(fraction).desired_width(ui.available_width()).text(label));
								}
							});
						});
					}
					if pending.delivery != Delivery::Sending && pending.delivery != Delivery::Confirmed {
						if pending.delivery == Delivery::Ambiguous {
							ui.label(RichText::new("Check the conversation before sending again.").small().color(colors.muted));
						}
						if ui.button("Restore to composer").clicked() { *restore = Some(pending.nonce.clone()); }
					} else if pending.delivery == Delivery::Sending && upload.is_some() && ui.small_button("Cancel upload").on_hover_text("The message may already have reached Discord. Check the conversation before sending again.").clicked() {
						*cancel = true;
					}
				});
			});
		});
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn pending_content_progress_and_failure_remain_visible() {
		fn text(shape: &egui::Shape, result: &mut String) {
			match shape {
				egui::Shape::Text(t) => result.push_str(&t.galley.job.text),
				egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| text(s, result)),
				_ => {}
			}
		}
		let ctx = egui::Context::default();
		let state = State {
			demo: true,
			..Default::default()
		};
		let mut pending = Pending {
			channel: model::Id(1),
			content: format!("{} FULL END", "Full message text ".repeat(10)),
			attachment: Some("notes.txt".into()),
			nonce: "local".into(),
			delivery: Delivery::Sending,
			confirmed: None,
		};
		let mut upload = Upload {
			nonce: pending.nonce.clone(),
			preview: None,
			bytes: 100,
			progress: Some((25, 100)),
		};
		for (delivery, progress, expected) in [
			(Delivery::Sending, Some((25, 100)), "Uploading · 25%"),
			(Delivery::Sending, Some((100, 100)), "Sending… · 100%"),
			(Delivery::Sending, None, "Preparing upload…"),
			(Delivery::Sending, Some((0, 0)), "Preparing upload…"),
			(Delivery::Rejected, None, "Not sent"),
			(Delivery::Ambiguous, None, "Delivery unknown"),
		] {
			pending.delivery = delivery;
			upload.progress = progress;
			let mut painted = String::new();
			for _ in 0..2 {
				let output = ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(260.0, 900.0),
						)),
						..Default::default()
					},
					|ui| {
						show(
							ui,
							&pending,
							true,
							&state,
							&mut crate::avatars::Avatars::default(),
							Some(&upload),
							(&mut None, &mut false),
						);
					},
				);
				painted.clear();
				for shape in &output.shapes {
					text(&shape.shape, &mut painted);
				}
				output.drop_without_applying_deltas();
			}
			assert!(painted.contains("FULL END"), "{painted}");
			assert!(painted.contains(expected), "{painted}");
			assert_eq!(
				painted.contains("Restore to composer"),
				delivery != Delivery::Sending
			);
		}
	}
}
