//! Guild channel actions share one context menu and a session-scoped editor.
use crate::{design, user_menu};
use client_core::{
	Command, State,
	channel_actions::{Action, Edit, Mute},
};
use model::{Channel, ChannelPreferences, Id};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
	Edit,
	Duplicate,
	Create,
	Delete,
}

enum Intent {
	Read,
	Dialog(Kind),
	Write(Action),
}

struct Dialog {
	channel: Id,
	guild: Id,
	kind: Kind,
	draft: Edit,
	before: Edit,
	loaded: bool,
	submitted: bool,
}

#[derive(Default)]
pub(super) struct ChannelMenu {
	pub invite_requested: Option<(Id, Id)>,
	requested: Option<(Id, Intent)>,
	dialog: Option<Dialog>,
	feedback: Option<Id>,
	preference_error: bool,
	generation: u64,
}

impl ChannelMenu {
	pub fn context(
		&mut self,
		response: &egui::Response,
		state: &State,
		channel: &Channel,
		prefs: &mut ChannelPreferences,
		prefs_changed: &mut bool,
		preferences_available: bool,
	) {
		let Some(guild) = channel.guild else { return };
		let colors = design::palette_for(&response.ctx);
		user_menu::popup(
			response,
			response.id.with(("channel-menu", state.generation)),
		)
		.frame(
			egui::Frame::popup(&response.ctx.style_of(response.ctx.theme()))
				.fill(colors.chat)
				.inner_margin(8)
				.corner_radius(8),
		)
		.show(|ui| {
			ui.set_width(232.0);
			ui.spacing_mut().button_padding = egui::vec2(12.0, 8.0);
			let available = (state.demo || state.gateway_connected)
				&& !state.channel_action_pending()
				&& state.can_view(channel.id);
			let mut intent = None;
			if row(
				ui,
				"Mark As Read",
				state.can_mark_channel_read(channel.id),
				false,
			)
			.clicked()
			{
				intent = Some(Intent::Read);
			}
			ui.separator();
			let favorite = prefs.is_favorite(channel.id);
			if channel.kind != 4
				&& row(
					ui,
					if favorite {
						"Remove From Favorites"
					} else {
						"Add To Favorites"
					},
					preferences_available,
					false,
				)
				.on_hover_text("Favorites are saved on this device.")
				.clicked()
			{
				let changed = prefs.toggle_favorite(channel.id);
				*prefs_changed |= changed;
				self.preference_error = !changed;
				self.generation = state.generation;
				ui.close();
			}
			ui.separator();
			if state.can_create_server_invite(guild, channel.id)
				&& row(
					ui,
					"Invite to Channel",
					available && !state.server_invite_pending() && !state.server_action_pending(),
					false,
				)
				.clicked()
			{
				self.invite_requested = Some((guild, channel.id));
				self.generation = state.generation;
				ui.close();
			}
			let pinned = prefs.is_pinned(channel.id);
			if channel.kind != 4
				&& row(
					ui,
					if pinned {
						"Unpin Channel From Top"
					} else {
						"Pin Channel to Top"
					},
					preferences_available,
					false,
				)
				.on_hover_text("Pinned channels are saved on this device.")
				.clicked()
			{
				let changed = prefs.toggle_pinned(channel.id);
				*prefs_changed |= changed;
				self.preference_error = !changed;
				self.generation = state.generation;
				ui.close();
			}
			if row(ui, "Copy Link", true, false).clicked() {
				ui.ctx().copy_text(format!(
					"https://discord.com/channels/{guild}/{}",
					channel.id
				));
				ui.close();
			}
			ui.separator();
			ui.add_enabled_ui(available, |ui| {
				if state.guild_channel_muted(channel.id) == Some(true)
					&& row(ui, "Unmute Channel", true, false).clicked()
				{
					intent = Some(Intent::Write(Action::Mute(Mute::Unmute)));
				}
				ui.menu_button("Mute Channel", |ui| {
					for (label, seconds) in [
						("For 15 Minutes", 900),
						("For 1 Hour", 3600),
						("For 3 Hours", 10800),
						("For 8 Hours", 28800),
						("For 24 Hours", 86400),
					] {
						if row(ui, label, true, false).clicked() {
							intent = Some(Intent::Write(Action::Mute(Mute::For(seconds))));
						}
					}
					if row(ui, "Until I Turn It Back On", true, false).clicked() {
						intent = Some(Intent::Write(Action::Mute(Mute::Forever)));
					}
				});
				ui.menu_button("Notification Settings", |ui| {
					let level = state.channel_notification_level(channel.id);
					for (value, label) in [
						(0, "All Messages"),
						(1, "Only @mentions"),
						(2, "Nothing"),
						(3, "Use Server Default"),
					] {
						if ui.selectable_label(level == Some(value), label).clicked() {
							intent = Some(Intent::Write(Action::Notifications(value)));
						}
					}
				});
			});
			if state.can_manage_channel(channel.id) {
				ui.separator();
				if matches!(channel.kind, 0 | 5)
					&& row(ui, "Edit Channel", available, false).clicked()
				{
					intent = Some(Intent::Dialog(Kind::Edit));
				}
				for (label, kind) in [
					("Duplicate Channel", Kind::Duplicate),
					("Create Text Channel", Kind::Create),
					("Delete Channel", Kind::Delete),
				] {
					if row(ui, label, available, kind == Kind::Delete).clicked() {
						intent = Some(Intent::Dialog(kind));
					}
				}
			}
			ui.separator();
			if row(ui, "Copy Channel ID", true, false).clicked() {
				ui.ctx().copy_text(channel.id.to_string());
				ui.close();
			}
			if let Some(intent) = intent {
				self.requested = Some((channel.id, intent));
				self.generation = state.generation;
				ui.close();
			}
		});
	}

	pub fn show(
		&mut self,
		ctx: &egui::Context,
		state: &mut State,
		active: Option<Id>,
		commands: &mut Vec<Command>,
	) {
		if self.generation != state.generation {
			*self = Self::default();
			return;
		}
		if let Some((id, intent)) = self.requested.take()
			&& let Some(channel) = state.channel(id)
			&& let Some(guild) = channel.guild.filter(|g| Some(*g) == active)
		{
			match intent {
				Intent::Read => {
					if let Some(command) = state.prepare_mark_channel_read(id) {
						commands.push(command);
					}
				}
				Intent::Write(action) => {
					self.feedback = Some(id);
					if let Some(command) = state.request_channel_action(id, action) {
						commands.push(command);
					}
				}
				Intent::Dialog(kind) => {
					self.dialog = Some(Dialog {
						channel: id,
						guild,
						kind,
						draft: Edit {
							name: if kind == Kind::Create {
								String::new()
							} else {
								channel.name.chars().take(100).collect()
							},
							topic: String::new(),
							slowmode: 0,
							nsfw: false,
						},
						loaded: kind != Kind::Edit,
						before: Edit::default(),
						submitted: false,
					});
					state.clear_channel_action_result(id);
					if kind == Kind::Edit
						&& let Some(command) = state.request_channel_action(id, Action::Load)
					{
						commands.push(command);
					}
				}
			}
		}
		self.show_feedback(ctx, state, active);
		let Some(dialog) = &mut self.dialog else {
			return;
		};
		if active != Some(dialog.guild)
			|| state.channel(dialog.channel).is_none()
			|| (dialog.submitted && state.channel_action_succeeded(dialog.channel))
		{
			self.dialog = None;
			return;
		}
		let pending = state.channel_action_pending();
		if !dialog.loaded
			&& !pending
			&& state.channel_action_status(dialog.channel).is_none()
			&& let Some(details) = state.channel_details(dialog.channel)
		{
			dialog.draft = details.clone();
			dialog.before = details.clone();
			dialog.loaded = true;
		}
		let colors = design::palette_for(ctx);
		let mut close = false;
		let modal = egui::Modal::new(egui::Id::unique(("channel-dialog", self.generation)))
			.frame(
				egui::Frame::new()
					.fill(colors.chat)
					.stroke(egui::Stroke::new(1.0, colors.border))
					.corner_radius(12)
					.inner_margin(24),
			)
			.show(ctx, |ui| {
				ui.set_width((ctx.content_rect().width() - 64.0).clamp(160.0, 440.0));
				ui.heading(match dialog.kind {
					Kind::Edit => "Edit Channel",
					Kind::Duplicate => "Duplicate Channel",
					Kind::Create => "Create Text Channel",
					Kind::Delete => "Delete Channel?",
				});
				ui.add_space(12.0);
				let allowed = state.can_manage_channel(dialog.channel);
				let current =
					dialog.kind != Kind::Edit || state.channel_details(dialog.channel).is_some();
				if !allowed {
					ui.colored_label(
						colors.warning,
						"You no longer have permission to manage this channel.",
					);
				}
				if !dialog.loaded {
					ui.label(if pending {
						"Loading channel settings…"
					} else {
						"Channel settings could not be loaded."
					});
					if ui
						.add_enabled(allowed && !pending, egui::Button::new("Retry"))
						.clicked() && let Some(command) =
						state.request_channel_action(dialog.channel, Action::Load)
					{
						commands.push(command);
					}
				} else if dialog.kind == Kind::Delete {
					ui.label(format!(
						"Delete #{}? Its messages will be permanently deleted. This cannot be undone.",
						dialog.draft.name
					));
				} else {
					ui.add_enabled_ui(allowed && !pending, |ui| {
						let label = ui.label("Channel name");
						let name = ui
							.add(
								egui::TextEdit::singleline(&mut dialog.draft.name)
									.char_limit(100)
									.desired_width(f32::INFINITY),
							)
							.labelled_by(label.id);
						if name.changed() {
							dialog.draft.name.shrink_to_fit();
						}
						if dialog.kind == Kind::Edit {
							let label = ui.label("Topic");
							let topic = ui
								.add(
									egui::TextEdit::multiline(&mut dialog.draft.topic)
										.char_limit(1024)
										.desired_rows(3)
										.desired_width(f32::INFINITY),
								)
								.labelled_by(label.id);
							if topic.changed() {
								dialog.draft.topic.shrink_to_fit();
							}
							ui.horizontal(|ui| {
								ui.label("Slowmode");
								ui.add(
									egui::DragValue::new(&mut dialog.draft.slowmode)
										.range(0..=21600)
										.suffix(" seconds"),
								);
							});
							ui.checkbox(&mut dialog.draft.nsfw, "Age-restricted channel");
						} else if dialog.kind == Kind::Duplicate {
							ui.label(
								"Copies this channel's settings and permissions. Messages are not copied.",
							);
						}
					});
				}
				if let Some(status) = state.channel_action_status(dialog.channel) {
					ui.colored_label(colors.warning, status);
				}
				if dialog.loaded && !current {
					ui.label(
						"Channel settings need to be refreshed before saving. Reloading replaces this draft.",
					);
					if ui
						.add_enabled(allowed && !pending, egui::Button::new("Reload Channel"))
						.clicked() && let Some(command) =
						state.request_channel_action(dialog.channel, Action::Load)
					{
						commands.push(command);
						dialog.loaded = false;
					}
				}
				ui.add_space(16.0);
				ui.horizontal(|ui| {
					close = ui
						.button(if pending { "Close" } else { "Cancel" })
						.clicked();
					let valid = dialog.kind == Kind::Delete
						|| if dialog.kind == Kind::Edit {
							dialog.draft.valid()
						} else {
							client_core::channel_actions::valid_name(&dialog.draft.name)
						};
					let label = if pending {
						"Working…"
					} else {
						match dialog.kind {
							Kind::Edit => "Save Changes",
							Kind::Duplicate => "Duplicate Channel",
							Kind::Create => "Create Channel",
							Kind::Delete => "Delete Channel",
						}
					};
					if ui
						.add_enabled(
							allowed
								&& dialog.loaded && current
								&& valid && !pending && (state.demo || state.gateway_connected),
							egui::Button::new(egui::RichText::new(label).color(
								if dialog.kind == Kind::Delete {
									colors.danger
								} else {
									colors.text
								},
							)),
						)
						.clicked()
					{
						let action = match dialog.kind {
							Kind::Edit => Action::Edit {
								before: dialog.before.clone(),
								after: dialog.draft.clone(),
							},
							Kind::Duplicate => Action::Duplicate {
								name: dialog.draft.name.clone(),
							},
							Kind::Create => Action::CreateText {
								name: dialog.draft.name.clone(),
							},
							Kind::Delete => Action::Delete,
						};
						if let Some(command) = state.request_channel_action(dialog.channel, action)
						{
							commands.push(command);
							dialog.submitted = true;
						}
					}
				});
				if state.demo {
					ui.small("Offline preview · no server changes");
				}
			});
		if close || modal.should_close() {
			if pending {
				self.feedback = Some(dialog.channel);
			}
			self.dialog = None;
		}
	}

	fn show_feedback(&mut self, ctx: &egui::Context, state: &State, active: Option<Id>) {
		if self.feedback.is_some_and(|id| {
			state.channel_action_succeeded(id)
				|| state.channel(id).is_none_or(|c| c.guild != active)
		}) {
			self.feedback = None;
		}
		if self.feedback.is_none() && !self.preference_error {
			return;
		}
		let mut open = true;
		egui::Window::new("Channel action")
			.id(egui::Id::unique("channel-feedback"))
			.collapsible(false)
			.resizable(false)
			.open(&mut open)
			.show(ctx, |ui| {
				ui.set_max_width((ctx.content_rect().width() - 64.0).clamp(160.0, 360.0));
				if self.preference_error {
					ui.label("Your favorites and pins are full. Remove one before adding another.");
				}
				if let Some(id) = self.feedback {
					ui.label(if state.channel_action_pending() {
						"Updating channel settings…"
					} else {
						state
							.channel_action_status(id)
							.unwrap_or("The channel action could not be started.")
					});
				}
			});
		if !open {
			self.feedback = None;
			self.preference_error = false;
		}
	}
}

fn row(ui: &mut egui::Ui, label: &str, enabled: bool, danger: bool) -> egui::Response {
	let colors = design::palette(ui);
	ui.add_enabled(
		enabled,
		egui::Button::new(())
			.left_text(design::medium(ui, label, 14.0).color(if danger {
				colors.danger
			} else {
				colors.text
			}))
			.min_size(egui::vec2(ui.available_width(), 34.0))
			.frame_when_inactive(false)
			.corner_radius(4),
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use egui::{Event, Modifiers, PointerButton, Pos2, Rect};

	fn labels(shape: &egui::Shape, output: &mut Vec<(String, Rect)>) {
		match shape {
			egui::Shape::Text(text) => output.push((
				text.galley.job.text.clone(),
				text.galley.rect.translate(text.pos.to_vec2()),
			)),
			egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| labels(s, output)),
			_ => {}
		}
	}
	fn pointer(pos: Pos2, button: PointerButton, pressed: bool) -> Vec<Event> {
		vec![
			Event::PointerMoved(pos),
			Event::PointerButton {
				pos,
				button,
				pressed,
				modifiers: Modifiers::NONE,
			},
		]
	}
	struct Harness {
		state: State,
		menu: ChannelMenu,
		prefs: ChannelPreferences,
		changed: bool,
		commands: Vec<Command>,
		copied: Vec<String>,
	}
	impl Harness {
		fn frame(
			&mut self,
			ctx: &egui::Context,
			events: Vec<Event>,
		) -> (egui::Response, Vec<(String, Rect)>) {
			let mut row = None;
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(320.0, 760.0))),
					events,
					..Default::default()
				},
				|ui| {
					let response = ui.button("#getting-started");
					self.menu.context(
						&response,
						&self.state,
						self.state.channel(Id(20)).unwrap(),
						&mut self.prefs,
						&mut self.changed,
						true,
					);
					row = Some(response);
					self.menu
						.show(ui.ctx(), &mut self.state, Some(Id(10)), &mut self.commands);
				},
			);
			let mut text = vec![];
			for shape in &output.shapes {
				labels(&shape.shape, &mut text);
			}
			for command in &output.platform_output.commands {
				if let egui::OutputCommand::CopyText(value) = command {
					self.copied.push(value.clone());
				}
			}
			output.drop_without_applying_deltas();
			(row.unwrap(), text)
		}
		fn click(&mut self, ctx: &egui::Context, pos: Pos2, button: PointerButton) {
			for pressed in [true, false] {
				self.frame(ctx, pointer(pos, button, pressed));
			}
		}
	}
	#[test]
	fn context_menu_keyboard_mouse_permissions_and_delete_confirmation() {
		for light in [false, true] {
			for action in [
				"Add To Favorites",
				"Copy Channel ID",
				"Invite to Channel",
				"Delete Channel",
			] {
				let ctx = egui::Context::default();
				design::apply(&ctx);
				ctx.set_visuals(if light {
					egui::Visuals::light()
				} else {
					egui::Visuals::dark()
				});
				let mut state = test_support::chat_demo_state();
				let mut permissions = test_support::permission_snapshot(&state);
				for guild in &mut permissions.guilds {
					guild.owner = state.user.as_ref().map(|u| u.id);
				}
				state.permissions.replace(permissions).unwrap();
				let mut h = Harness {
					state,
					menu: ChannelMenu::default(),
					prefs: ChannelPreferences::default(),
					changed: false,
					commands: vec![],
					copied: vec![],
				};
				let (row, _) = h.frame(&ctx, vec![]);
				if light {
					row.request_focus();
					h.frame(
						&ctx,
						vec![Event::Key {
							key: egui::Key::F10,
							physical_key: None,
							pressed: true,
							repeat: false,
							modifiers: Modifiers::SHIFT,
						}],
					);
				} else {
					h.click(&ctx, row.rect.center(), PointerButton::Secondary);
				}
				let (_, text) = h.frame(&ctx, vec![]);
				for expected in [
					"Mark As Read",
					"Add To Favorites",
					"Invite to Channel",
					"Pin Channel to Top",
					"Copy Link",
					"Mute Channel",
					"Notification Settings",
					"Edit Channel",
					"Duplicate Channel",
					"Create Text Channel",
					"Delete Channel",
					"Copy Channel ID",
				] {
					let rect = text
						.iter()
						.find(|(label, _)| label == expected)
						.unwrap_or_else(|| panic!("Missing {expected}: {text:?}"))
						.1;
					assert!(
						Rect::from_min_size(Pos2::ZERO, egui::vec2(320.0, 760.0))
							.contains_rect(rect),
						"{expected} stays inside the viewport: {rect:?}"
					);
				}
				assert!(h.commands.is_empty());
				h.click(
					&ctx,
					text.iter()
						.find(|(label, _)| label == action)
						.unwrap()
						.1
						.center(),
					PointerButton::Primary,
				);
				assert!(
					h.commands.is_empty(),
					"Opening a dialog does not send a destructive action"
				);
				match action {
					"Add To Favorites" => assert!(h.prefs.is_favorite(Id(20)) && h.changed),
					"Copy Channel ID" => assert_eq!(h.copied, ["20"]),
					"Invite to Channel" => {
						assert_eq!(h.menu.invite_requested, Some((Id(10), Id(20))))
					}
					_ => {
						let (_, text) = h.frame(&ctx, vec![]);
						h.click(
							&ctx,
							text.iter()
								.find(|(label, _)| label == "Delete Channel")
								.unwrap()
								.1
								.center(),
							PointerButton::Primary,
						);
						assert_eq!(h.commands.len(), 1);
						assert!(matches!(
							&h.commands[0],
							Command::ChannelAction {
								channel: Id(20),
								action: Action::Delete,
								..
							}
						));
						h.state.generation += 1;
						h.frame(&ctx, vec![]);
						assert!(h.menu.dialog.is_none());
					}
				}
				// A fresh menu with unknown permissions never exposes administrative actions.
				egui::Popup::close_all(&ctx);
				h.state.permissions = Default::default();
				let (row, _) = h.frame(&ctx, vec![]);
				h.click(&ctx, row.rect.center(), PointerButton::Secondary);
				let (_, text) = h.frame(&ctx, vec![]);
				for hidden in [
					"Invite to Channel",
					"Edit Channel",
					"Duplicate Channel",
					"Create Text Channel",
					"Delete Channel",
				] {
					assert!(!text.iter().any(|(label, _)| label == hidden));
				}
			}
		}
	}
}
