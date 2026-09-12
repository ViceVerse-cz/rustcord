//! Session-scoped account menu; the host owns presence publication.
use crate::{MessagingUi, design, icons, profiles};
use client_core::{Command, State};
use egui::{RichText, vec2};
use model::PresenceStatus;

#[derive(Default)]
pub(super) struct AccountMenu {
	open: bool,
	generation: u64,
	draft: String,
	custom_open: bool,
}

impl AccountMenu {
	/// Fixture-only: show the popout on the first frame of a demo capture.
	#[cfg(any(test, feature = "demo"))]
	pub(super) fn preview(&mut self, generation: u64) {
		self.open = true;
		self.generation = generation;
	}
}

impl MessagingUi {
	pub(super) fn account_menu(
		&mut self,
		anchor: &egui::Response,
		state: &mut State,
		commands: &mut Vec<Command>,
	) {
		if self.account_menu.generation != state.generation {
			self.account_menu = AccountMenu {
				generation: state.generation,
				..Default::default()
			};
		}
		if anchor.clicked() {
			self.account_menu.open = !self.account_menu.open;
			if self.account_menu.open {
				self.account_menu.draft = self.own_presence.custom_status.clone();
				self.profile = None;
				if state.own_profile.data.is_none()
					&& !state.own_profile.loading
					&& let Some(command) = state.load_own_profile()
				{
					commands.push(command);
				}
			}
		}
		let colors = design::palette_for(&anchor.ctx);
		let mut open = self.account_menu.open;
		egui::Popup::from_response(anchor)
			.id(anchor.id.with("account-menu"))
			.open_bool(&mut open)
			.align(egui::RectAlign::TOP_START)
			.close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
			.width(300.0)
			.frame(
				egui::Frame::popup(&anchor.ctx.style_of(anchor.ctx.theme()))
					.fill(colors.raised)
					.inner_margin(0)
					.corner_radius(10),
			)
			.show(|ui| {
				ui.set_width(300.0);
				let height = (ui.ctx().content_rect().height() - 90.0).clamp(180.0, 620.0);
				egui::ScrollArea::vertical()
					.min_scrolled_height(height)
					.max_height(height)
					.show(ui, |ui| self.account_menu_contents(ui, state, commands));
			});
		self.account_menu.open = open;
		if self.account_menu.custom_open {
			let response = egui::Modal::new(egui::Id::unique("custom-status-editor")).show(
				&anchor.ctx,
				|ui| {
					ui.set_width((ui.ctx().content_rect().width() - 56.0).clamp(160.0, 360.0));
					ui.heading("Set a custom status");
					ui.add_space(12.0);
					self.custom_status_editor(ui, state);
					if ui.button("Done").clicked() {
						self.account_menu.custom_open = false;
					}
				},
			);
			if response.should_close() {
				self.account_menu.custom_open = false;
			}
		}
	}

	fn account_menu_contents(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		commands: &mut Vec<Command>,
	) {
		let colors = design::palette(ui);
		let (banner, _) =
			ui.allocate_exact_size(vec2(ui.available_width(), 64.0), egui::Sense::hover());
		let corner = egui::CornerRadius {
			nw: 10,
			ne: 10,
			sw: 0,
			se: 0,
		};
		if let Some(profile) = &state.own_profile.data {
			self.avatars
				.paint_banner(ui, profile, banner, corner, state.demo);
		} else {
			ui.painter().rect_filled(
				banner,
				corner,
				design::mix(colors.accent, colors.base, 0.55),
			);
		}
		if let Some(user) = &state.user {
			let avatar = egui::Rect::from_min_size(
				banner.left_bottom() + vec2(14.0, -38.0),
				vec2(72.0, 72.0),
			);
			// Thick popover-coloured ring, as Discord punches the avatar out of the banner.
			ui.painter()
				.circle_filled(avatar.center(), 42.0, colors.raised);
			ui.scope_builder(egui::UiBuilder::new().max_rect(avatar), |ui| {
				self.avatars.show(ui, user, 72.0, state.demo);
			});
			let dot = avatar.right_bottom() - egui::Vec2::splat(11.0);
			ui.painter().circle_filled(dot, 10.5, colors.raised);
			presence_dot(
				ui.painter(),
				dot,
				7.0,
				self.own_presence.status,
				colors.raised,
			);
			ui.add_space((avatar.bottom() + 12.0 - ui.cursor().top()).max(0.0));
		} else {
			ui.add_space(12.0);
		}
		egui::Frame::new()
			.inner_margin(egui::Margin {
				left: 12,
				right: 12,
				top: 0,
				bottom: 12,
			})
			.show(ui, |ui| {
				ui.set_width(ui.available_width());
				self.account_identity_card(ui, state, commands);
				ui.add_space(8.0);
				ui.spacing_mut().item_spacing.y = 2.0;
				self.account_status_row(ui);
				self.account_custom_status_row(ui);
			});
	}

	/// Name, handle and current custom status, grouped on the sunken card Discord uses.
	fn account_identity_card(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		commands: &mut Vec<Command>,
	) {
		let colors = design::palette(ui);
		egui::Frame::new()
			.fill(colors.base)
			.corner_radius(8)
			.inner_margin(12)
			.show(ui, |ui| {
				ui.set_width(ui.available_width());
				ui.spacing_mut().item_spacing.y = 2.0;
				let profile = state.own_profile.data.as_ref();
				let name = profile
					.and_then(|p| p.global_name.as_deref())
					.or_else(|| state.user.as_ref().map(|u| u.name.as_str()))
					.unwrap_or("Your account");
				ui.add(
					egui::Label::new(design::semibold(ui, name, 20.0).color(colors.text_strong))
						.wrap(),
				);
				if let Some(profile) = profile {
					ui.add(
						egui::Label::new(
							design::medium(ui, &profile.username, 14.0).color(colors.muted),
						)
						.wrap(),
					);
					if !profile.pronouns.is_empty() {
						ui.add(
							egui::Label::new(
								RichText::new(&profile.pronouns)
									.size(12.0)
									.color(colors.muted),
							)
							.wrap(),
						);
					}
				}
				if !self.own_presence.custom_status.is_empty() {
					ui.add_space(10.0);
					let (line, _) = ui
						.allocate_exact_size(vec2(ui.available_width(), 1.0), egui::Sense::hover());
					ui.painter().rect_filled(line, 0, colors.border);
					ui.add_space(10.0);
					ui.add(
						egui::Label::new(
							RichText::new(&self.own_presence.custom_status)
								.size(14.0)
								.color(colors.text),
						)
						.wrap(),
					);
				}
				if state.own_profile.loading {
					ui.add_space(6.0);
					ui.label(
						RichText::new("Loading profile…")
							.small()
							.color(colors.muted),
					);
				}
				if let Some(error) = state.own_profile.error {
					ui.add_space(6.0);
					ui.add(
						egui::Label::new(RichText::new(error).size(12.0).color(colors.danger))
							.wrap(),
					);
					if ui.button("Reload profile").clicked()
						&& let Some(command) = state.load_own_profile()
					{
						commands.push(command);
					}
				}
			});
	}

	/// Presence row: status dot, label and a chevron opening the status menu.
	fn account_status_row(&mut self, ui: &mut egui::Ui) {
		let colors = design::palette(ui);
		let status = self.own_presence.status;
		let label = design::medium(ui, status.label(), 14.0).color(colors.text_strong);
		let response = ui
			.scope(|ui| {
				let width = ui.available_width();
				ui.spacing_mut().button_padding = vec2(34.0, 10.0);
				egui::containers::menu::MenuButton::from_button(
					egui::Button::new(())
						.left_text(label)
						.frame_when_inactive(false)
						.corner_radius(6)
						.min_size(vec2(width, 40.0)),
				)
				.ui(ui, |ui| self.presence_menu(ui))
				.0
			})
			.inner;
		let rect = response.rect;
		let background = if response.hovered() || response.has_focus() {
			ui.visuals().widgets.hovered.weak_bg_fill
		} else {
			colors.raised
		};
		presence_dot(
			ui.painter(),
			egui::pos2(rect.left() + 17.0, rect.center().y),
			5.0,
			status,
			background,
		);
		icons::paint(
			ui.painter(),
			icons::Icon::ChevronRight,
			egui::Rect::from_center_size(
				egui::pos2(rect.right() - 18.0, rect.center().y),
				egui::Vec2::splat(16.0),
			),
			colors.muted,
		);
	}

	fn presence_menu(&mut self, ui: &mut egui::Ui) {
		let colors = design::palette(ui);
		ui.set_width(260.0_f32.min(ui.ctx().content_rect().width() - 48.0));
		for status in PresenceStatus::ALL {
			let description = match status {
				PresenceStatus::DoNotDisturb => "You will not receive desktop notifications",
				PresenceStatus::Invisible => "You will appear offline",
				_ => "",
			};
			let height = if description.is_empty() { 40.0 } else { 62.0 };
			let response = ui.add_sized(
				[ui.available_width(), height],
				egui::Button::new("")
					.frame_when_inactive(false)
					.corner_radius(6),
			);
			response.widget_info(|| {
				egui::WidgetInfo::labeled(egui::WidgetType::Button, true, status.label())
			});
			let x = response.rect.left() + 34.0;
			let y = response.rect.top() + if description.is_empty() { 11.0 } else { 10.0 };
			let background = if response.hovered() || response.has_focus() {
				ui.visuals().widgets.hovered.weak_bg_fill
			} else {
				ui.visuals().window_fill
			};
			presence_dot(
				ui.painter(),
				egui::pos2(response.rect.left() + 17.0, response.rect.center().y),
				5.0,
				status,
				background,
			);
			ui.painter().text(
				egui::pos2(x, y),
				egui::Align2::LEFT_TOP,
				status.label(),
				egui::FontId::new(14.0, design::medium_family(ui.ctx())),
				colors.text_strong,
			);
			if !description.is_empty() {
				let galley = ui.painter().layout(
					description.into(),
					egui::FontId::proportional(12.0),
					colors.muted,
					(response.rect.width() - 46.0).max(80.0),
				);
				ui.painter()
					.galley(egui::pos2(x, y + 22.0), galley, colors.muted);
			}
			if response.clicked() {
				if self.own_presence.status != status {
					self.own_presence.status = status;
					self.own_presence_changed = true;
				}
				ui.close();
			}
			if status == PresenceStatus::Online {
				ui.separator();
			}
		}
	}

	/// Custom status row: smiley to write one, pencil once a status is set.
	fn account_custom_status_row(&mut self, ui: &mut egui::Ui) {
		let colors = design::palette(ui);
		let set = !self.own_presence.custom_status.is_empty();
		let label = if set {
			"Edit custom status"
		} else {
			"Set a custom status"
		};
		let text = design::medium(ui, label, 14.0).color(colors.text_strong);
		let response = ui
			.scope(|ui| {
				let width = ui.available_width();
				ui.spacing_mut().button_padding = vec2(34.0, 10.0);
				ui.add(
					egui::Button::new(())
						.left_text(text)
						.frame_when_inactive(false)
						.corner_radius(6)
						.min_size(vec2(width, 40.0)),
				)
			})
			.inner;
		icons::paint(
			ui.painter(),
			if set {
				icons::Icon::Pencil
			} else {
				icons::Icon::Smile
			},
			egui::Rect::from_center_size(
				egui::pos2(response.rect.left() + 17.0, response.rect.center().y),
				egui::Vec2::splat(17.0),
			),
			colors.muted,
		);
		if response.clicked() {
			self.account_menu
				.draft
				.clone_from(&self.own_presence.custom_status);
			self.account_menu.custom_open = true;
		}
	}
	fn custom_status_editor(&mut self, ui: &mut egui::Ui, state: &State) {
		let colors = design::palette(ui);
		let label = ui.label(design::eyebrow(ui, "Custom status", colors.muted));
		ui.add_sized(
			[ui.available_width(), 44.0],
			egui::TextEdit::singleline(&mut self.account_menu.draft)
				.align(egui::Align2::LEFT_CENTER)
				.id_salt(("account-custom-status", state.generation))
				.hint_text("What's on your mind?")
				.char_limit(128)
				.desired_width(f32::INFINITY),
		)
		.labelled_by(label.id);
		let valid = model::OwnPresence {
			status: self.own_presence.status,
			custom_status: self.account_menu.draft.trim().to_owned(),
		}
		.valid();
		let changed = self.account_menu.draft.trim() != self.own_presence.custom_status;
		ui.horizontal(|ui| {
			if ui
				.add_enabled(
					valid && changed,
					egui::Button::new(RichText::new("Apply").color(colors.accent_text))
						.fill(colors.accent),
				)
				.clicked()
			{
				self.own_presence.custom_status = self.account_menu.draft.trim().to_owned();
				self.account_menu
					.draft
					.clone_from(&self.own_presence.custom_status);
				self.own_presence_changed = true;
			}
			if ui
				.add_enabled(
					!self.own_presence.custom_status.is_empty()
						|| !self.account_menu.draft.is_empty(),
					egui::Button::new("Clear"),
				)
				.clicked()
			{
				self.account_menu.draft.clear();
				if !self.own_presence.custom_status.is_empty() {
					self.own_presence.custom_status.clear();
					self.own_presence_changed = true;
				}
			}
		});
		if !valid {
			ui.colored_label(
				colors.danger,
				"Use up to 128 characters without control characters.",
			);
		}
		ui.add_space(6.0);
		ui.label(
			RichText::new(if state.demo {
				"Offline preview · this session only"
			} else {
				"This session only"
			})
			.small()
			.color(colors.muted),
		);
		if !self.own_presence_status.is_empty() {
			ui.add(
				egui::Label::new(
					RichText::new(self.own_presence_status)
						.small()
						.color(colors.muted),
				)
				.wrap(),
			);
		}
	}
}

/// Presence dot with Discord's status glyphs punched out in the surface colour.
fn presence_dot(
	painter: &egui::Painter,
	center: egui::Pos2,
	radius: f32,
	status: PresenceStatus,
	background: egui::Color32,
) {
	painter.circle_filled(center, radius, profiles::presence_color(status.wire()));
	match status {
		PresenceStatus::Idle => {
			painter.circle_filled(
				center - egui::Vec2::splat(radius * 0.5),
				radius * 0.8,
				background,
			);
		}
		PresenceStatus::Invisible => {
			painter.circle_filled(center, radius * 0.56, background);
		}
		PresenceStatus::DoNotDisturb => {
			painter.line_segment(
				[
					center - vec2(radius * 0.62, 0.0),
					center + vec2(radius * 0.62, 0.0),
				],
				egui::Stroke::new(radius * 0.42, background),
			);
		}
		_ => {}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use egui::{Event, Pos2, Rect};

	fn frame(
		ctx: &egui::Context,
		view: &mut MessagingUi,
		state: &mut State,
		size: egui::Vec2,
		events: Vec<Event>,
	) -> Vec<(String, Rect)> {
		let mut text = vec![];
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
				events,
				..Default::default()
			},
			|ui| {
				egui::Panel::bottom("test-account-footer").show(ui, |ui| {
					let mut commands = vec![];
					view.account_card(ui, state, &mut commands);
					assert!(commands.is_empty());
				});
			},
		);
		fn collect(shape: &egui::Shape, labels: &mut Vec<(String, Rect)>) {
			match shape {
				egui::Shape::Text(t) => labels.push((
					t.galley.job.text.clone(),
					Rect::from_min_size(t.pos, t.galley.size()),
				)),
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						collect(shape, labels);
					}
				}
				_ => {}
			}
		}
		for shape in &output.shapes {
			collect(&shape.shape, &mut text);
		}
		assert!(output.platform_output.commands.is_empty());
		output.drop_without_applying_deltas();
		text
	}
	fn click(
		ctx: &egui::Context,
		view: &mut MessagingUi,
		state: &mut State,
		size: egui::Vec2,
		position: Pos2,
	) {
		for pressed in [true, false] {
			frame(
				ctx,
				view,
				state,
				size,
				vec![
					Event::PointerMoved(position),
					Event::PointerButton {
						pos: position,
						button: egui::PointerButton::Primary,
						pressed,
						modifiers: egui::Modifiers::NONE,
					},
				],
			);
		}
	}
	fn locate(text: &[(String, Rect)], label: &str) -> Pos2 {
		text.iter()
			.find(|(text, _)| text == label)
			.unwrap_or_else(|| panic!("Missing {label}: {text:?}"))
			.1
			.center()
	}
	#[test]
	fn account_menu_opens_applies_clears_and_closes_across_themes_and_sizes() {
		for light in [false, true] {
			for size in [vec2(340.0, 760.0), vec2(760.0, 520.0)] {
				let ctx = egui::Context::default();
				ctx.set_theme(if light {
					egui::ThemePreference::Light
				} else {
					egui::ThemePreference::Dark
				});
				design::apply(&ctx);
				let mut state = test_support::demo_state();
				let own = state.user.as_ref().unwrap().clone();
				state.own_profile.data = Some(profiles::synthetic(&own, None));
				let mut view = MessagingUi::default();
				frame(&ctx, &mut view, &mut state, size, vec![]);
				let text = frame(&ctx, &mut view, &mut state, size, vec![]);
				if light {
					for key in [egui::Key::Tab, egui::Key::Enter] {
						frame(
							&ctx,
							&mut view,
							&mut state,
							size,
							vec![Event::Key {
								key,
								physical_key: None,
								pressed: true,
								repeat: false,
								modifiers: egui::Modifiers::NONE,
							}],
						);
					}
				} else {
					click(&ctx, &mut view, &mut state, size, locate(&text, &own.name));
				}
				for _ in 0..3 {
					frame(&ctx, &mut view, &mut state, size, vec![]);
				}
				assert!(view.account_menu.open);
				let text = frame(&ctx, &mut view, &mut state, size, vec![]);
				click(&ctx, &mut view, &mut state, size, locate(&text, "Online"));
				for _ in 0..3 {
					frame(&ctx, &mut view, &mut state, size, vec![]);
				}
				let text = frame(&ctx, &mut view, &mut state, size, vec![]);
				for status in PresenceStatus::ALL {
					let p = locate(&text, status.label());
					assert!(Rect::from_min_size(Pos2::ZERO, size).contains(p));
				}
				click(
					&ctx,
					&mut view,
					&mut state,
					size,
					locate(&text, "Invisible"),
				);
				assert_eq!(view.own_presence.status, PresenceStatus::Invisible);
				assert!(std::mem::take(&mut view.own_presence_changed));
				view.account_menu.open = true;
				for _ in 0..3 {
					frame(&ctx, &mut view, &mut state, size, vec![]);
				}
				let text = frame(&ctx, &mut view, &mut state, size, vec![]);
				click(
					&ctx,
					&mut view,
					&mut state,
					size,
					locate(&text, "Set a custom status"),
				);
				assert!(view.account_menu.custom_open);
				// A bounded draft is separate from the value published by the host.
				view.account_menu.draft = "  Synthetic status 🌙  ".into();
				assert!(view.own_presence.custom_status.is_empty());
				// Tall viewport keeps the editor in view; narrow/short mode additionally checks scrolling above.
				let size = vec2(size.x, 900.0);
				for _ in 0..3 {
					frame(&ctx, &mut view, &mut state, size, vec![]);
				}
				let text = frame(&ctx, &mut view, &mut state, size, vec![]);
				click(&ctx, &mut view, &mut state, size, locate(&text, "Apply"));
				assert_eq!(view.own_presence.custom_status, "Synthetic status 🌙");
				assert!(std::mem::take(&mut view.own_presence_changed));
				let text = frame(&ctx, &mut view, &mut state, size, vec![]);
				click(&ctx, &mut view, &mut state, size, locate(&text, "Clear"));
				assert!(view.own_presence.custom_status.is_empty());
				assert!(view.own_presence_changed);
				frame(
					&ctx,
					&mut view,
					&mut state,
					size,
					vec![Event::Key {
						key: egui::Key::Escape,
						physical_key: None,
						pressed: true,
						repeat: false,
						modifiers: egui::Modifiers::NONE,
					}],
				);
				assert!(!view.account_menu.custom_open);
				view.account_menu.open = true;
				view.account_menu.draft = "Old account draft".into();
				state.generation += 1;
				frame(&ctx, &mut view, &mut state, size, vec![]);
				assert!(!view.account_menu.open && view.account_menu.draft.is_empty());
				assert!(view.take_avatar_requests().is_empty());
			}
		}
	}
}
