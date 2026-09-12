//! Explicit, session-bound invite lookup followed by a user-confirmed join.
use crate::{design, icons, invites::input_code};
use client_core::{Command, State};

fn invite_input(ui: &mut egui::Ui, text: &mut String, focus: bool) -> egui::Response {
	let colors = design::palette(ui);
	let response = ui.add_sized(
		[ui.available_width(), 48.0],
		egui::TextEdit::singleline(text)
			.hint_text("https://discord.gg/hTKzmak")
			.font(egui::FontId::proportional(16.0))
			.align(egui::Align2::LEFT_CENTER)
			.frame(
				egui::Frame::new()
					.fill(colors.base)
					.corner_radius(8)
					.inner_margin(egui::Margin::symmetric(12, 8)),
			)
			.char_limit(512),
	);
	if focus {
		response.request_focus();
	}
	let stroke = if response.has_focus() {
		egui::Stroke::new(2.0, colors.accent)
	} else {
		egui::Stroke::new(1.0, colors.border)
	};
	ui.painter()
		.rect_stroke(response.rect, 8, stroke, egui::StrokeKind::Inside);
	response
}

#[derive(Default)]
pub(super) struct JoinDialog {
	generation: Option<u64>,
	input: String,
	focus: bool,
	status: &'static str,
}

impl JoinDialog {
	pub fn open(&mut self, generation: u64) {
		*self = Self {
			generation: Some(generation),
			focus: true,
			..Self::default()
		};
	}
	pub fn show(&mut self, ctx: &egui::Context, state: &mut State, commands: &mut Vec<Command>) {
		if self.generation != Some(state.generation) {
			*self = Self::default();
			return;
		}
		let colors = design::palette_for(ctx);
		let mut close = false;
		let mut ready = false;
		let mut member = false;
		let mut loading = false;
		let mut accepted = false;
		let modal = egui::Modal::new(egui::Id::unique("join-server-dialog"))
			.frame(
				egui::Frame::new()
					.fill(colors.chat)
					.stroke(egui::Stroke::new(1.0, colors.border))
					.corner_radius(14)
					.inner_margin(24),
			)
			.show(ctx, |ui| {
				ui.set_width((ctx.content_rect().width() - 80.0).clamp(180.0, 490.0));
				ui.horizontal(|ui| {
					ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
						close = icons::button(ui, icons::Icon::Close, 24.0, "Close dialog").clicked();
					});
				});
				let body_height = (ctx.content_rect().height() - 220.0).clamp(100.0, 450.0);
				egui::ScrollArea::vertical().max_height(body_height).min_scrolled_height(body_height).show(ui, |ui| {
					ui.vertical_centered(|ui| {
						ui.label(design::semibold(ui, "Join a Server", 28.0).color(colors.text_strong));
						ui.add_space(8.0);
						ui.label("Enter an invite below to join an existing server");
					});
					ui.add_space(18.0);
					let label = ui.label(design::semibold(ui, "Invite link *", 16.0));
					let input = invite_input(ui, &mut self.input, std::mem::take(&mut self.focus));
					let input = input.labelled_by(label.id);
					if input.changed() {
						self.status = "";
					}
					ui.add_space(16.0);
					ui.colored_label(colors.muted, "Invites should look like");
					ui.horizontal_wrapped(|ui| {
						ui.code("hTKzmak");
						ui.code("https://discord.gg/hTKzmak");
						ui.code("https://discord.gg/wumpus-friends");
					});
					ui.add_space(16.0);
					egui::Frame::new().fill(colors.base).corner_radius(10).inner_margin(14).show(ui, |ui| {
						ui.label(design::semibold(ui, "Don't have an invite?", 17.0));
						ui.hyperlink_to("Explore discoverable communities in Discord ↗", "https://discord.com/servers");
					});
					ui.add_space(16.0);
					let parsed = input_code(&self.input);
					let preview = parsed.as_ref()
						.and_then(|code| state.invites.get(code))
						.filter(|(at, _)| at.elapsed().as_secs() < 300);
					loading = preview.is_some_and(|(_, value)| value.is_none());
					ready = if let Some((_, Some(Ok(preview)))) = preview {
						ui.label(design::semibold(ui, preview.embed.title.as_deref().unwrap_or("Server invite"), 20.0));
						member = state.guild(preview.guild).is_some();
						ui.label(if member { "You are already a member of this server." } else { "Review this server, then choose Join Server to confirm." });
						true
					} else {
						if let Some((_, Some(Err(error)))) = preview { ui.colored_label(colors.danger, error.label()); }
						false
					};
					accepted = parsed.as_deref() == Some(&state.invite_join.code) && matches!(state.invite_join.result, Some(Ok(_)));
					if parsed.as_deref() == Some(&state.invite_join.code) {
						if accepted { ui.label("Invite accepted. Waiting for server access; complete any server rules in Discord."); }
						if let Some(Err(error)) = state.invite_join.result { ui.colored_label(colors.danger, error.label()); }
					}
					if state.demo { ui.colored_label(colors.muted, "Offline demo — joining servers is disabled."); }
					ui.colored_label(colors.muted, self.status);
				});
				let parsed = input_code(&self.input);
					ui.add_space(16.0);
					ui.horizontal(|ui| {
						if ui.button("Back").clicked() { close = true; }
						ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
							let busy = loading || state.invite_join.pending;
							let enabled = !state.demo && !busy && !member && !accepted;
							let text = if busy { "Please wait…" } else if ready { "Join Server" } else { "Check Invite" };
							let clicked = ui.add_enabled(enabled, egui::Button::new(egui::RichText::new(text).color(colors.accent_text)).fill(colors.accent).min_size(egui::vec2(132.0, 44.0))).clicked();
							if clicked {
								if let Some(code) = parsed {
									let command = if ready {
										state.join_invite(code)
									} else {
										// Only an explicit retry may discard a completed failed lookup.
										state.invites.remove(&code);
										state.request_join_preview(code)
										};
									if let Some(command) = command {
										commands.push(command);
										self.status = "";
									} else {
										self.status = "Unable to proceed. Check your connection or try again after the current request.";
									}
								} else {
									self.status = "Enter a valid Discord invite link or invite code.";
								}
							}
						});
					});
			});
		if close || modal.should_close() {
			*self = Self::default();
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn invite_field_centers_hint_and_text_with_padding_and_focus_outline() {
		for light in [false, true] {
			for width in [240.0, 490.0] {
				for initial in ["", "synthetic-invite"] {
					let ctx = egui::Context::default();
					design::apply(&ctx);
					ctx.set_visuals(if light {
						egui::Visuals::light()
					} else {
						egui::Visuals::dark()
					});
					let mut text = initial.to_owned();
					let mut rect = egui::Rect::NOTHING;
					for _ in 0..2 {
						let output = ctx.run_ui(egui::RawInput::default(), |ui| {
							ui.set_width(width);
							let response = invite_input(ui, &mut text, true);
							assert!(response.has_focus());
							rect = response.rect;
						});
						let mut shapes = Vec::new();
						fn flatten<'a>(shape: &'a egui::Shape, out: &mut Vec<&'a egui::Shape>) {
							if let egui::Shape::Vec(children) = shape {
								for child in children {
									flatten(child, out);
								}
							} else {
								out.push(shape);
							}
						}
						for shape in &output.shapes {
							flatten(&shape.shape, &mut shapes);
						}
						assert!((rect.height() - 48.0).abs() < 1.0, "{rect:?}");
						let label = shapes
							.iter()
							.find_map(|shape| match shape {
								egui::Shape::Text(t)
									if t.galley.job.text
										== if initial.is_empty() {
											"https://discord.gg/hTKzmak"
										} else {
											initial
										} =>
								{
									Some(t.galley.rect.translate(t.pos.to_vec2()))
								}
								_ => None,
							})
							.expect("input text is rendered");
						assert!(
							(label.center().y - rect.center().y).abs() <= 1.0,
							"text {label:?}, field {rect:?}"
						);
						assert!(label.left() >= rect.left() + 11.0);
						assert!(shapes.iter().any(|shape| matches!(shape, egui::Shape::Rect(r) if r.rect == rect && r.stroke.width == 2.0 && r.stroke.color == design::palette_for(&ctx).accent)));
						output.drop_without_applying_deltas();
					}
				}
			}
		}
	}
	fn frame(
		ctx: &egui::Context,
		dialog: &mut JoinDialog,
		state: &mut State,
		commands: &mut Vec<Command>,
		size: egui::Vec2,
		events: Vec<egui::Event>,
	) -> Vec<(String, egui::Rect)> {
		fn labels(shape: &egui::Shape, output: &mut Vec<(String, egui::Rect)>) {
			match shape {
				egui::Shape::Text(text) => output.push((
					text.galley.job.text.clone(),
					text.galley.rect.translate(text.pos.to_vec2()),
				)),
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						labels(shape, output);
					}
				}
				_ => {}
			}
		}
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
				events,
				..Default::default()
			},
			|ui| dialog.show(ui.ctx(), state, commands),
		);
		let mut texts = vec![];
		for shape in &output.shapes {
			labels(&shape.shape, &mut texts);
		}
		output.drop_without_applying_deltas();
		texts
	}
	fn click(
		ctx: &egui::Context,
		dialog: &mut JoinDialog,
		state: &mut State,
		commands: &mut Vec<Command>,
		size: egui::Vec2,
		label: &str,
	) {
		let texts = frame(ctx, dialog, state, commands, size, vec![]);
		let rect = texts
			.iter()
			.find(|(text, _)| text == label)
			.unwrap_or_else(|| panic!("Missing {label}: {texts:?}"))
			.1;
		assert!(egui::Rect::from_min_size(egui::Pos2::ZERO, size).contains_rect(rect));
		for pressed in [true, false] {
			frame(
				ctx,
				dialog,
				state,
				commands,
				size,
				vec![
					egui::Event::PointerMoved(rect.center()),
					egui::Event::PointerButton {
						pos: rect.center(),
						button: egui::PointerButton::Primary,
						pressed,
						modifiers: egui::Modifiers::NONE,
					},
				],
			);
		}
	}
	#[test]
	fn join_dialog_checks_then_confirms_once_and_clears_on_session_change() {
		for (light, size) in [
			(false, egui::vec2(960.0, 760.0)),
			(true, egui::vec2(320.0, 760.0)),
		] {
			let ctx = egui::Context::default();
			design::apply(&ctx);
			ctx.set_visuals(if light {
				egui::Visuals::light()
			} else {
				egui::Visuals::dark()
			});
			let mut state = State {
				auth: client_core::auth::AuthState::Authenticated,
				gateway_connected: true,
				..State::default()
			};
			let mut dialog = JoinDialog::default();
			dialog.open(state.generation);
			dialog.input = "https://discord.gg/synthetic".into();
			let mut commands = vec![];
			for _ in 0..3 {
				frame(&ctx, &mut dialog, &mut state, &mut commands, size, vec![]);
			}
			assert!(commands.is_empty());
			click(
				&ctx,
				&mut dialog,
				&mut state,
				&mut commands,
				size,
				"Check Invite",
			);
			assert!(matches!(commands.pop(), Some(Command::Invite { .. })));
			frame(&ctx, &mut dialog, &mut state, &mut commands, size, vec![]);
			assert!(commands.is_empty());
			state.apply_invite(
				"synthetic".into(),
				Err(client_core::auth::Failure::Forbidden),
			);
			let texts = frame(&ctx, &mut dialog, &mut state, &mut commands, size, vec![]);
			assert!(texts.iter().any(|(text, _)| text == "Permission denied"));
			assert!(commands.is_empty(), "no automatic retries");
			click(
				&ctx,
				&mut dialog,
				&mut state,
				&mut commands,
				size,
				"Check Invite",
			);
			assert!(matches!(commands.pop(), Some(Command::Invite { .. })));
			state.apply_invite(
				"synthetic".into(),
				Ok(model::InvitePreview {
					guild: model::Id(12),
					embed: model::Embed {
						title: Some("Synthetic server".into()),
						..Default::default()
					},
				}),
			);
			for _ in 0..2 {
				frame(&ctx, &mut dialog, &mut state, &mut commands, size, vec![]);
			}
			assert!(commands.is_empty(), "preview never joins automatically");
			click(
				&ctx,
				&mut dialog,
				&mut state,
				&mut commands,
				size,
				"Join Server",
			);
			let Some(Command::JoinInvite { request, .. }) = commands.pop() else {
				panic!("join missing")
			};
			click(
				&ctx,
				&mut dialog,
				&mut state,
				&mut commands,
				size,
				"Please wait…",
			);
			assert!(commands.is_empty());
			state.apply(client_core::Envelope {
				generation: state.generation,
				event: client_core::Event::JoinInvite {
					request,
					result: Ok(model::Id(12)),
				},
			});
			assert!(state.guild(model::Id(12)).is_none());
			click(&ctx, &mut dialog, &mut state, &mut commands, size, "Back");
			assert!(dialog.generation.is_none());
			dialog.open(state.generation);
			state.generation += 1;
			frame(&ctx, &mut dialog, &mut state, &mut commands, size, vec![]);
			assert!(dialog.generation.is_none());
		}
	}
}
