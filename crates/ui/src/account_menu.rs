//! Session-scoped account menu; the host owns presence publication.
use crate::{MessagingUi, design, profiles};
use client_core::{Command, State};
use egui::{RichText, vec2};
use model::PresenceStatus;

#[derive(Default)]
pub(super) struct AccountMenu {
	open: bool,
	generation: u64,
	draft: String,
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
	}

	fn account_menu_contents(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		commands: &mut Vec<Command>,
	) {
		let colors = design::palette(ui);
		let (banner, _) =
			ui.allocate_exact_size(vec2(ui.available_width(), 66.0), egui::Sense::hover());
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
				banner.left_bottom() + vec2(16.0, -30.0),
				vec2(64.0, 64.0),
			);
			ui.painter()
				.circle_filled(avatar.center(), 37.0, colors.raised);
			ui.scope_builder(egui::UiBuilder::new().max_rect(avatar), |ui| {
				self.avatars.show(ui, user, 64.0, state.demo);
			});
			design::presence_dot(
				ui,
				avatar,
				profiles::presence_color(self.own_presence.status.wire()),
				colors.raised,
			);
			ui.add_space((avatar.bottom() + 10.0 - ui.cursor().top()).max(0.0));
		}
		egui::Frame::new().inner_margin(16).show(ui, |ui| {
			ui.set_width(ui.available_width());
			let profile = state.own_profile.data.as_ref();
			let name = profile
				.and_then(|p| p.global_name.as_deref())
				.or_else(|| state.user.as_ref().map(|u| u.name.as_str()))
				.unwrap_or("Your account");
			ui.add(
				egui::Label::new(design::semibold(ui, name, 21.0).color(colors.text_strong)).wrap(),
			);
			if let Some(profile) = profile {
				ui.add(
					egui::Label::new(RichText::new(&profile.username).color(colors.muted)).wrap(),
				);
				if !profile.pronouns.is_empty() {
					ui.add(
						egui::Label::new(
							RichText::new(&profile.pronouns).small().color(colors.muted),
						)
						.wrap(),
					);
				}
			}
			if state.own_profile.loading {
				ui.small("Loading profile…");
			}
			if let Some(error) = state.own_profile.error {
				ui.colored_label(colors.danger, error);
				if ui.button("Reload profile").clicked()
					&& let Some(command) = state.load_own_profile()
				{
					commands.push(command);
				}
			}
			ui.add_space(8.0);
			ui.separator();
			ui.label(design::eyebrow(ui, "Status", colors.muted));
			for status in PresenceStatus::ALL {
				let selected = self.own_presence.status == status;
				let response = ui.add_sized(
					[ui.available_width(), 30.0],
					egui::Button::new(status.label()).selected(selected),
				);
				ui.painter().circle_filled(
					egui::pos2(response.rect.left() + 16.0, response.rect.center().y),
					4.0,
					profiles::presence_color(status.wire()),
				);
				if response.clicked() && !selected {
					self.own_presence.status = status;
					self.own_presence_changed = true;
				}
			}
			ui.add_space(10.0);
			let label = ui.label(design::eyebrow(ui, "Custom status", colors.muted));
			ui.add(
				egui::TextEdit::singleline(&mut self.account_menu.draft)
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
					.add_enabled(valid && changed, egui::Button::new("Apply"))
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
		});
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
				assert!(!view.account_menu.open);
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
