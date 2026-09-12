//! Server actions start from explicit menu choices, with one session-scoped dialog.
use crate::{avatars::Avatars, design, icons, server_invite::InviteDialog};
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
	pub settings_requested: Option<Id>,
	dialog: Option<Dialog>,
	generation: u64,
	invite: InviteDialog,
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
			let button = ui.add_sized(
				[ui.available_width(), 40.0],
				egui::Button::new(()).frame(false),
			);
			egui::ContainerAtom::new((
				design::semibold(ui, title, 15.0),
				egui::Atom::paint(egui::Vec2::splat(14.0), |ui, args| {
					icons::paint(
						ui.painter(),
						icons::Icon::ChevronDown,
						args.rect,
						colors.muted,
					);
				}),
			))
			.gap(5.0)
			.align2(egui::Align2::LEFT_CENTER)
			.wrap_mode(egui::TextWrapMode::Truncate)
			.fallback_text_color(colors.text_strong)
			.measure(ui, button.rect.size())
			.paint_at(ui, button.rect);
			button.widget_info(|| {
				egui::WidgetInfo::labeled(
					egui::WidgetType::Button,
					ui.is_enabled(),
					format!("Server menu, {title}"),
				)
			});
			egui::Popup::menu(&button)
				.frame(
					egui::Frame::popup(ui.style())
						.fill(colors.chat)
						.inner_margin(8)
						.corner_radius(8),
				)
				.show(|ui| {
					ui.set_width(232.0);
					let available = !state.server_action_pending()
						&& !state.server_invite_pending()
						&& (state.demo || state.gateway_connected);
					if state.can_manage_guild(guild)
						&& menu_row(ui, icons::Icon::Gear, "Server Settings", colors.text).clicked()
					{
						self.settings_requested = Some(guild);
						ui.close();
					}
					if ui
						.add_enabled_ui(available, |ui| {
							menu_row(ui, icons::Icon::AddPeople, "Create invite", colors.text)
						})
						.inner
						.clicked()
					{
						state.clear_server_action_result(guild);
						self.dialog = Some(Dialog::Invite {
							guild,
							channel: state.invite_channel(guild),
						});
						self.generation = state.generation;
						self.invite.open();
						ui.close();
					}
					ui.separator();
					if ui
						.add_enabled_ui(available, |ui| {
							menu_row(ui, icons::Icon::ArrowRight, "Leave server", colors.danger)
						})
						.inner
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
		avatars: &mut Avatars,
	) {
		let Some(mut dialog) = self.dialog else {
			return;
		};
		let guild = dialog.guild();
		if self.generation != state.generation || active != Some(guild) {
			self.dialog = None;
			self.invite = InviteDialog::default();
			return;
		}
		let Some(name) = state
			.guilds
			.iter()
			.find(|g| g.id == guild)
			.map(|g| g.name.clone())
		else {
			self.dialog = None;
			self.invite = InviteDialog::default();
			return;
		};
		if let Dialog::Invite { channel, .. } = &mut dialog {
			let close = self
				.invite
				.show(ctx, state, (guild, &name), channel, avatars, commands);
			self.dialog = if close { None } else { Some(dialog) };
			if close {
				self.invite = InviteDialog::default();
			}
			return;
		}
		let mut close = false;
		let colors = design::palette_for(ctx);
		let modal = egui::Modal::new(egui::Id::unique("server-action-dialog"))
			.frame(
				egui::Frame::new()
					.fill(colors.chat)
					.stroke(egui::Stroke::new(1.0, colors.border))
					.corner_radius(12)
					.inner_margin(24),
			)
			.show(ctx, |ui| {
				ui.set_width((ctx.content_rect().width() - 80.0).clamp(180.0, 536.0));
				let pending = state.server_action_pending();
				ui.horizontal(|ui| {
					let heading = match dialog {
						Dialog::Invite { .. } => format!("Invite friends to {name}"),
						Dialog::Leave(_) => "Leave server?".to_owned(),
					};
					let heading_width = ui.available_width() - 36.0;
					ui.allocate_ui_with_layout(
						egui::vec2(heading_width, 28.0),
						egui::Layout::top_down(egui::Align::Min),
						|ui| {
							ui.set_width(heading_width);
							ui.add(
								egui::Label::new(
									design::semibold(ui, heading, 20.0).color(colors.text_strong),
								)
								.wrap(),
							);
						},
					);
					close = icons::button(ui, icons::Icon::Close, 28.0, "Close dialog").clicked();
				});
				ui.add_space(8.0);
				match &mut dialog {
					Dialog::Invite { .. } => unreachable!(),
					Dialog::Leave(_) => {
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
				if matches!(dialog, Dialog::Leave(_))
					&& ui
						.button(if pending { "Close" } else { "Cancel" })
						.clicked()
				{
					close = true;
				}
				if state.demo {
					ui.label(
						egui::RichText::new("Offline preview · no server changes")
							.small()
							.color(colors.muted),
					);
				}
			});
		self.dialog = if close || modal.should_close() {
			None
		} else {
			Some(dialog)
		};
	}
}

fn menu_row(
	ui: &mut egui::Ui,
	icon: icons::Icon,
	label: &str,
	color: egui::Color32,
) -> egui::Response {
	ui.scope(|ui| {
		ui.spacing_mut().button_padding = egui::vec2(36.0, 8.0);
		let response = ui.add_sized(
			[ui.available_width(), 36.0],
			egui::Button::new(())
				.left_text(design::medium(ui, label, 14.0).color(color))
				.frame_when_inactive(false)
				.corner_radius(4),
		);
		icons::paint(
			ui.painter(),
			icon,
			egui::Rect::from_center_size(
				egui::pos2(response.rect.left() + 18.0, response.rect.center().y),
				egui::Vec2::splat(20.0),
			),
			if ui.is_enabled() {
				color
			} else {
				color.gamma_multiply(0.5)
			},
		);
		response
	})
	.inner
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
		width: f32,
		menu: &mut ServerMenu,
		state: &mut State,
		events: Vec<Event>,
		commands: &mut Vec<Command>,
	) -> (egui::Response, Vec<(String, Rect)>) {
		let copied_before = menu.invite.copied;
		let mut response = None;
		let output = ctx.run_ui(
			egui::RawInput {
				screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(width, 550.0))),
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
				menu.show(
					ui.ctx(),
					state,
					Some(Id(10)),
					commands,
					&mut Avatars::default(),
				);
			},
		);
		let mut text = vec![];
		for shape in &output.shapes {
			labels(&shape.shape, &mut text);
		}
		if menu.invite.copied && !copied_before {
			assert!(output.platform_output.commands.iter().any(|command| {
				matches!(command, egui::OutputCommand::CopyText(link) if link == "https://discord.gg/synthetic-invite")
			}));
		}
		output.drop_without_applying_deltas();
		(response.unwrap(), text)
	}
	#[test]
	fn server_menu_requires_explicit_invite_and_leave_confirmation() {
		for (light, width) in [(false, 320.0), (true, 320.0), (false, 960.0), (true, 960.0)] {
			for action in ["Create invite", "Leave server"] {
				let ctx = egui::Context::default();
				design::apply(&ctx);
				ctx.set_visuals(if light {
					egui::Visuals::light()
				} else {
					egui::Visuals::dark()
				});
				let mut state = test_support::chat_demo_state();
				let mut menu = ServerMenu::default();
				let mut commands = vec![];
				let (button, _) = frame(&ctx, width, &mut menu, &mut state, vec![], &mut commands);
				if light {
					button.request_focus();
					frame(
						&ctx,
						width,
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
							width,
							&mut menu,
							&mut state,
							pointer(button.rect.center(), pressed),
							&mut commands,
						);
					}
				}
				let (_, text) = frame(&ctx, width, &mut menu, &mut state, vec![], &mut commands);
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
						width,
						&mut menu,
						&mut state,
						pointer(position, pressed),
						&mut commands,
					);
				}
				let (_, text) = frame(&ctx, width, &mut menu, &mut state, vec![], &mut commands);
				assert_eq!(
					commands.len(),
					usize::from(action == "Create invite"),
					"Create invite is deliberate; Leave still requires confirmation"
				);
				if action == "Create invite" {
					let heading = text
						.iter()
						.find(|(t, _)| t.starts_with("Invite friends to "))
						.unwrap()
						.1;
					let footer = text
						.iter()
						.find(|(t, _)| t == "Or, send a server invite link to a friend")
						.unwrap()
						.1;
					assert!(
						(heading.left() - footer.left()).abs() < 1.0,
						"heading and footer align left: {heading:?} {footer:?}"
					);
				}
				if action == "Leave server" {
					let position = text
						.iter()
						.rev()
						.find(|(t, _)| {
							t == if action == "Create invite" {
								"Create link"
							} else {
								action
							}
						})
						.unwrap()
						.1
						.center();
					for pressed in [true, false] {
						frame(
							&ctx,
							width,
							&mut menu,
							&mut state,
							pointer(position, pressed),
							&mut commands,
						);
					}
				}
				assert_eq!(
					commands.len(),
					1,
					"one explicit confirmation sends one command"
				);
				assert!(matches!(&commands[0], Command::ServerAction { .. }));
				frame(&ctx, width, &mut menu, &mut state, vec![], &mut commands);
				assert_eq!(commands.len(), 1);
				if action == "Create invite" {
					let Command::ServerAction { action, request } = commands[0] else {
						unreachable!()
					};
					state.apply(client_core::Envelope {
						generation: state.generation,
						event: client_core::Event::ServerAction(
							client_core::server_actions::Event::Written {
								action,
								request,
								result: Ok(Some("synthetic-invite".into())),
							},
						),
					});
					let (_, text) =
						frame(&ctx, width, &mut menu, &mut state, vec![], &mut commands);
					let copy = text.iter().find(|(t, _)| t == "Copy").unwrap().1;
					assert!(
						copy.left() >= 0.0 && copy.right() <= width,
						"copy stays inside the viewport"
					);
					for pressed in [true, false] {
						frame(
							&ctx,
							width,
							&mut menu,
							&mut state,
							pointer(copy.center(), pressed),
							&mut commands,
						);
					}
					assert!(menu.invite.copied);
					assert_eq!(commands.len(), 1, "copying does not create another invite");
				}
				state.generation += 1;
				frame(&ctx, width, &mut menu, &mut state, vec![], &mut commands);
				assert!(menu.dialog.is_none(), "old account dialogs close");
			}
		}
	}
}
