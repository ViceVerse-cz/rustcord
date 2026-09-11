//! Server actions start from explicit menu choices, with one session-scoped dialog.
use crate::{design, icons};
use client_core::{Command, State};
use model::Id;

#[derive(Clone, Copy)]
enum Dialog {
	Invite { guild: Id, channel: Option<Id> },
	Leave(Id),
}

impl Dialog {
	fn guild(self) -> Id {
		match self {
			Self::Invite { guild, .. } | Self::Leave(guild) => guild,
		}
	}
}

#[derive(Default)]
pub(super) struct ServerMenu {
	dialog: Option<Dialog>,
	generation: u64,
	copied: bool,
}

impl ServerMenu {
	pub fn header(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		guild: Id,
		title: &str,
	) -> egui::Response {
		ui.push_id(("server-menu", guild), |ui| {
			let colors = design::palette(ui);
			ui.spacing_mut().button_padding.x = 24.0;
			let button = ui.add_sized(
				[ui.available_width(), 40.0],
				egui::Button::new(design::semibold(ui, title, 15.0).color(colors.text_strong))
					.truncate()
					.frame(false),
			);
			button.widget_info(|| {
				egui::WidgetInfo::labeled(
					egui::WidgetType::Button,
					true,
					format!("Server menu, {title}"),
				)
			});
			icons::paint(
				ui.painter(),
				icons::Icon::ChevronDown,
				egui::Rect::from_center_size(
					egui::pos2(button.rect.right() - 10.0, button.rect.center().y),
					egui::Vec2::splat(14.0),
				),
				colors.muted,
			);
			egui::Popup::menu(&button).show(|ui| {
				ui.set_min_width(210.0);
				ui.spacing_mut().button_padding = egui::vec2(8.0, 6.0);
				let available =
					!state.server_action_pending() && (state.demo || state.gateway_connected);
				if ui
					.add_enabled(available, egui::Button::new("Create invite"))
					.clicked()
				{
					state.clear_server_action_result(guild);
					self.dialog = Some(Dialog::Invite {
						guild,
						channel: state.invite_channel(guild),
					});
					self.generation = state.generation;
					self.copied = false;
					ui.close();
				}
				ui.separator();
				if ui
					.add_enabled(
						available,
						egui::Button::new(egui::RichText::new("Leave server").color(colors.danger)),
					)
					.clicked()
				{
					state.clear_server_action_result(guild);
					self.dialog = Some(Dialog::Leave(guild));
					self.generation = state.generation;
					ui.close();
				}
				if !available {
					ui.small(if state.server_action_pending() {
						"A server action is in progress."
					} else {
						"Reconnect to manage this server."
					});
				}
			});
			button
		})
		.inner
	}

	pub fn show(
		&mut self,
		ctx: &egui::Context,
		state: &mut State,
		active: Option<Id>,
		commands: &mut Vec<Command>,
	) {
		let Some(mut dialog) = self.dialog else {
			return;
		};
		let guild = dialog.guild();
		if self.generation != state.generation || active != Some(guild) {
			self.dialog = None;
			return;
		}
		let Some(name) = state
			.guilds
			.iter()
			.find(|g| g.id == guild)
			.map(|g| g.name.clone())
		else {
			self.dialog = None;
			return;
		};
		let mut close = false;
		let modal = egui::Modal::new(egui::Id::unique("server-action-dialog")).show(ctx, |ui| {
			ui.set_width((ctx.content_rect().width() - 48.0).clamp(180.0, 400.0));
			let colors = design::palette(ui);
			let pending = state.server_action_pending();
			if state.demo {
				ui.small("Offline preview · no server changes");
			}
			match &mut dialog {
				Dialog::Invite { channel, .. } => {
					ui.heading("Create invite");
					ui.label(&name);
					ui.add_space(12.0);
					if channel.is_none_or(|c| !state.can_create_server_invite(guild, c)) {
						*channel = state.invite_channel(guild);
					}
					let before = *channel;
					ui.label("Invite people to");
					ui.add_enabled_ui(!pending, |ui| {
						let label = state
							.channels
							.iter()
							.find(|c| Some(c.id) == *channel)
							.map_or("No eligible channel", |c| c.name.as_str());
						egui::ComboBox::from_id_salt("invite-channel")
							.selected_text(label)
							.width(ui.available_width())
							.height(220.0)
							.show_ui(ui, |ui| {
								for item in state
									.channels
									.iter()
									.filter(|c| state.can_create_server_invite(guild, c.id))
								{
									ui.selectable_value(channel, Some(item.id), &item.name);
								}
							});
					});
					if before != *channel {
						state.clear_server_action_result(guild);
						self.copied = false;
					}
					ui.label(
						egui::RichText::new("Expires in 24 hours. No limit on uses.")
							.small()
							.color(colors.muted),
					);
					if channel.is_none() {
						ui.colored_label(
							colors.warning,
							"You need Create Invite permission in a channel to create an invite.",
						);
					}
					ui.add_space(12.0);
					if let Some(link) = state.created_invite(guild) {
						ui.add(egui::Label::new(link).wrap().selectable(true));
						if ui
							.button(if self.copied {
								"Copied"
							} else {
								"Copy invite link"
							})
							.clicked()
						{
							ui.ctx().copy_text(link.to_owned());
							self.copied = true;
						}
					} else if ui
						.add_enabled(
							!pending && channel.is_some(),
							egui::Button::new(if pending {
								"Creating invite…"
							} else {
								"Create invite"
							}),
						)
						.clicked() && let Some(channel) = *channel
						&& let Some(command) = state.create_server_invite(guild, channel)
					{
						commands.push(command);
					}
				}
				Dialog::Leave(_) => {
					ui.heading("Leave server?");
					ui.label(format!(
						"Leave {name}? You will need another invite to rejoin."
					));
					ui.add_space(12.0);
					let reason = state.leave_server_reason(guild);
					if let Some(reason) = reason {
						ui.colored_label(colors.warning, reason);
					}
					if ui
						.add_enabled(
							!pending && reason.is_none(),
							egui::Button::new(
								egui::RichText::new(if pending {
									"Leaving…"
								} else {
									"Leave server"
								})
								.color(colors.danger),
							),
						)
						.clicked() && let Some(command) = state.leave_server(guild)
					{
						commands.push(command);
					}
				}
			}
			if let Some(status) = state.server_action_status(guild) {
				ui.colored_label(colors.warning, status);
			}
			ui.add_space(8.0);
			if ui
				.button(if pending { "Close" } else { "Cancel" })
				.clicked()
			{
				close = true;
			}
		});
		self.dialog = if close || modal.should_close() {
			None
		} else {
			Some(dialog)
		};
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use egui::{Event, Pos2, Rect};

	fn labels(shape: &egui::Shape, text: &mut Vec<(String, Rect)>) {
		match shape {
			egui::Shape::Text(t) => text.push((
				t.galley.job.text.clone(),
				t.galley.rect.translate(t.pos.to_vec2()),
			)),
			egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| labels(s, text)),
			_ => {}
		}
	}
	fn pointer(pos: Pos2, pressed: bool) -> Vec<Event> {
		vec![
			Event::PointerMoved(pos),
			Event::PointerButton {
				pos,
				button: egui::PointerButton::Primary,
				pressed,
				modifiers: egui::Modifiers::NONE,
			},
		]
	}
	fn frame(
		ctx: &egui::Context,
		menu: &mut ServerMenu,
		state: &mut State,
		events: Vec<Event>,
		commands: &mut Vec<Command>,
	) -> (egui::Response, Vec<(String, Rect)>) {
		let mut response = None;
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(320.0, 550.0))),
				events,
				..Default::default()
			},
			|ui| {
				response = Some(menu.header(
					ui,
					state,
					Id(10),
					"A long synthetic server name for the narrow sidebar",
				));
				menu.show(ui.ctx(), state, Some(Id(10)), commands);
			},
		);
		let mut text = vec![];
		for shape in &output.shapes {
			labels(&shape.shape, &mut text);
		}
		output.drop_without_applying_deltas();
		(response.unwrap(), text)
	}
	#[test]
	fn server_menu_requires_explicit_invite_and_leave_confirmation() {
		for light in [false, true] {
			for action in ["Create invite", "Leave server"] {
				let ctx = egui::Context::default();
				ctx.set_visuals(if light {
					egui::Visuals::light()
				} else {
					egui::Visuals::dark()
				});
				let mut state = test_support::chat_demo_state();
				let mut menu = ServerMenu::default();
				let mut commands = vec![];
				let (button, _) = frame(&ctx, &mut menu, &mut state, vec![], &mut commands);
				if light {
					button.request_focus();
					frame(
						&ctx,
						&mut menu,
						&mut state,
						vec![Event::Key {
							key: egui::Key::Enter,
							physical_key: None,
							pressed: true,
							repeat: false,
							modifiers: egui::Modifiers::NONE,
						}],
						&mut commands,
					);
				} else {
					for pressed in [true, false] {
						frame(
							&ctx,
							&mut menu,
							&mut state,
							pointer(button.rect.center(), pressed),
							&mut commands,
						);
					}
				}
				let (_, text) = frame(&ctx, &mut menu, &mut state, vec![], &mut commands);
				assert!(commands.is_empty());
				let position = text
					.iter()
					.find(|(t, _)| t == action)
					.unwrap_or_else(|| panic!("Missing {action}: {text:?}"))
					.1
					.center();
				for pressed in [true, false] {
					frame(
						&ctx,
						&mut menu,
						&mut state,
						pointer(position, pressed),
						&mut commands,
					);
				}
				let (_, text) = frame(&ctx, &mut menu, &mut state, vec![], &mut commands);
				assert!(
					commands.is_empty(),
					"opening a dialog must not write to Discord"
				);
				let position = text
					.iter()
					.rev()
					.find(|(t, _)| t == action)
					.unwrap()
					.1
					.center();
				for pressed in [true, false] {
					frame(
						&ctx,
						&mut menu,
						&mut state,
						pointer(position, pressed),
						&mut commands,
					);
				}
				assert_eq!(
					commands.len(),
					1,
					"one explicit confirmation sends one command"
				);
				assert!(matches!(&commands[0], Command::ServerAction { .. }));
				frame(&ctx, &mut menu, &mut state, vec![], &mut commands);
				assert_eq!(commands.len(), 1);
				state.generation += 1;
				frame(&ctx, &mut menu, &mut state, vec![], &mut commands);
				assert!(menu.dialog.is_none(), "old account dialogs close");
			}
		}
	}
}
