use crate::{MessagingUi, design};
use client_core::{
	Command, State,
	auth::AuthState,
	voice::{Participant, Phase, RosterEntry},
};
use egui::RichText;
use model::Id;

impl MessagingUi {
	fn is_speaking(&self, state: &State, channel: Id, participant: &Participant) -> bool {
		!participant.muted
			&& !participant.deafened
			&& !participant.server_muted
			&& !participant.server_deafened
			&& state.voice.active.as_ref().is_some_and(|call| {
				call.channel == channel
					&& call.phase == Phase::Connected
					&& !call.deafened
					&& !call.server_deafened
			}) && self.voice_speaking.contains(&participant.user)
	}

	pub(super) fn voice_channel_button(
		&mut self,
		ui: &mut egui::Ui,
		state: &State,
		channel: &model::Channel,
		selected: bool,
	) -> egui::Response {
		let colors = design::palette(ui);
		let call = state
			.voice
			.active
			.as_ref()
			.filter(|c| c.channel == channel.id);
		let connected = call.is_some_and(|c| matches!(c.phase, Phase::Waiting | Phase::Connected));
		let elapsed = call.and_then(elapsed_label);
		// A channel the account cannot view stays visible but inert, as before the restyle.
		let viewable = state.can_view(channel.id);
		let (rect, response) = ui
			.push_id(channel.id, |ui| {
				ui.allocate_exact_size(
					egui::vec2(ui.available_width(), 34.0),
					if viewable {
						egui::Sense::click()
					} else {
						egui::Sense::hover()
					},
				)
			})
			.inner;
		let row = rect.shrink2(egui::vec2(0.0, 1.0));
		let hovered = viewable && (response.hovered() || response.has_focus());
		if selected {
			ui.painter().rect_filled(row, 8, colors.selected);
		} else if hovered {
			ui.painter().rect_filled(row, 8, colors.hover);
		}
		let text_color = if !viewable {
			colors.muted.gamma_multiply(0.6)
		} else if connected {
			colors.accent
		} else if selected || hovered {
			colors.text_strong
		} else {
			colors.muted
		};
		crate::icons::paint(
			ui.painter(),
			crate::icons::Icon::Speaker,
			egui::Rect::from_center_size(
				row.left_center() + egui::vec2(18.0, 0.0),
				egui::Vec2::splat(20.0),
			),
			text_color,
		);
		let elapsed_width = if elapsed.is_some() { 64.0 } else { 0.0 };
		let name = ui.painter().layout(
			channel.name.clone(),
			egui::FontId::new(15.0, design::medium_family(ui.ctx())),
			text_color,
			(row.width() - 40.0 - elapsed_width).max(10.0),
		);
		let name_rect = egui::Rect::from_min_size(
			egui::pos2(row.left() + 34.0, row.center().y - name.size().y * 0.5),
			egui::vec2(row.width() - 40.0 - elapsed_width, name.size().y),
		);
		ui.painter()
			.with_clip_rect(name_rect)
			.galley(name_rect.min, name, text_color);
		if let Some(elapsed) = &elapsed {
			ui.painter().text(
				row.right_center() - egui::vec2(8.0, 0.0),
				egui::Align2::RIGHT_CENTER,
				elapsed,
				egui::FontId::monospace(11.0),
				text_color,
			);
		}
		if elapsed.is_some() && ui.is_rect_visible(response.rect) {
			ui.ctx()
				.request_repaint_after(std::time::Duration::from_secs(1));
		}
		response.widget_info(|| {
			egui::WidgetInfo::selected(
				egui::WidgetType::SelectableLabel,
				viewable,
				selected,
				format!(
					"{} voice channel{}",
					channel.name,
					if connected { ", connected" } else { "" }
				),
			)
		});
		response.on_hover_text(format!(
			"{} · View voice channel{}",
			channel.name,
			if connected { " · Connected" } else { "" }
		))
	}

	pub(super) fn voice_participant(
		&mut self,
		ui: &mut egui::Ui,
		state: &State,
		entry: &RosterEntry,
	) {
		if !state.can_view(entry.channel) {
			return;
		}
		let colors = design::palette(ui);
		let (user, name) = resolve_member(state, entry);
		ui.push_id(
			("voice-participant", entry.channel, entry.participant.user),
			|ui| {
				ui.horizontal(|ui| {
					ui.set_min_height(34.0);
					ui.spacing_mut().item_spacing.x = 6.0;
					let avatar = if let Some(user) = user {
						self.avatars.show(ui, user, 28.0, state.demo)
					} else {
						design::avatar(ui, name, 28.0)
					};
					if self.is_speaking(state, entry.channel, &entry.participant) {
						speaking_avatar(ui, &avatar, name);
					}
					if let Some(user) = user {
						crate::user_menu::show(
							&avatar,
							state,
							user,
							&mut self.profile,
							&mut self.user_action,
						);
					}
					if avatar.clicked()
						&& let Some(user) = user
					{
						self.profile = Some(user.clone());
					}
					ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
						if entry.participant.deafened {
							status_icon(
								ui,
								true,
								if entry.participant.server_deafened {
									"Deafened by server"
								} else {
									"Deafened"
								},
							);
						}
						if entry.participant.muted {
							status_icon(
								ui,
								false,
								if entry.participant.server_muted {
									"Muted by server"
								} else {
									"Microphone muted"
								},
							);
						}
						let response = ui
							.allocate_ui_with_layout(
								egui::vec2(ui.available_width(), 28.0),
								egui::Layout::left_to_right(egui::Align::Center),
								|ui| {
									ui.add(
										egui::Label::new(RichText::new(name).color(colors.muted))
											.truncate()
											.sense(egui::Sense::click()),
									)
								},
							)
							.inner
							.on_hover_text(name);
						if let Some(user) = user {
							crate::user_menu::show(
								&response,
								state,
								user,
								&mut self.profile,
								&mut self.user_action,
							);
						}
						if response.clicked()
							&& let Some(user) = user
						{
							self.profile = Some(user.clone());
						}
					});
				});
			},
		);
	}

	/// Guild voice channel: Discord-style black stage with participant tiles and, when
	/// connected, the call control bar; otherwise a Join Voice button.
	pub(super) fn voice_channel(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		channel: Id,
		commands: &mut Vec<Command>,
	) {
		let connected = state
			.voice
			.active
			.as_ref()
			.is_some_and(|call| call.channel == channel);
		let stage = ui.available_rect_before_wrap();
		ui.painter().rect_filled(stage, 0, STAGE_FILL);
		let (rect, _) = ui.allocate_exact_size(stage.size(), egui::Sense::hover());
		let notices = self.stage_notices(state, channel, connected);
		let bottom = 44.0 + 2.0 * STAGE_MARGIN;
		let body = egui::Rect::from_min_max(
			rect.left_top() + egui::vec2(STAGE_MARGIN, STAGE_MARGIN),
			egui::pos2(rect.right() - STAGE_MARGIN, rect.bottom() - bottom),
		);
		let mut body_ui = ui.new_child(
			egui::UiBuilder::new()
				.max_rect(body)
				.layout(egui::Layout::top_down(egui::Align::Min)),
		);
		stage_notices(&mut body_ui, &notices);
		if !state.can_view(channel) {
			body_ui.label(
				RichText::new("Participant list unavailable with the current access.")
					.color(STAGE_MUTED),
			);
		} else {
			let entries: Vec<RosterEntry> = state
				.voice
				.roster
				.iter()
				.filter(|e| e.channel == channel)
				.cloned()
				.collect();
			if entries.is_empty() {
				body_ui.add_space((body_ui.available_height() * 0.4).max(0.0));
				body_ui.vertical_centered(|ui| {
					ui.label(
						design::semibold(
							ui,
							if !state.demo && !state.gateway_connected {
								"Participant list unavailable while disconnected"
							} else {
								"No one's here yet"
							},
							18.0,
						)
						.color(STAGE_TEXT),
					);
				});
			} else {
				if !state.demo && !state.gateway_connected {
					body_ui.label(
						RichText::new("Last known participants · reconnect to refresh")
							.small()
							.color(STAGE_MUTED),
					);
				}
				self.participant_tiles(&mut body_ui, state, channel, &entries);
			}
		}
		let bar = egui::Rect::from_min_max(
			egui::pos2(rect.left(), rect.bottom() - bottom),
			rect.right_bottom(),
		);
		let mut bar_ui = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(
			egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
		));
		if connected {
			self.call_controls(&mut bar_ui, state, channel, commands);
		} else {
			bar_ui.horizontal_centered(|ui| {
				ui.add_space((ui.available_width() - 160.0).max(0.0) * 0.5);
				self.call_button(ui, state, channel, commands);
			});
		}
	}

	/// Participant tiles in a virtualized grid; only visible rows request avatars.
	fn participant_tiles(
		&mut self,
		ui: &mut egui::Ui,
		state: &State,
		channel: Id,
		entries: &[RosterEntry],
	) {
		let width = ui.available_width();
		let max_columns = ((width + TILE_GAP) / (180.0 + TILE_GAP)).floor().max(1.0) as usize;
		let columns = ((entries.len() as f32).sqrt().ceil() as usize)
			.clamp(1, max_columns)
			.min(entries.len().max(1));
		let tile_width = ((width - TILE_GAP * (columns as f32 - 1.0)) / columns as f32).min(360.0);
		let tile_height = (tile_width * 9.0 / 16.0).max(120.0);
		let rows = entries.len().div_ceil(columns);
		egui::ScrollArea::vertical()
			.id_salt(("voice-tiles", channel))
			.show_rows(ui, tile_height + TILE_GAP, rows, |ui, range| {
				for row in range {
					let in_row = entries.len().saturating_sub(row * columns).min(columns);
					let row_width =
						tile_width * in_row as f32 + TILE_GAP * (in_row as f32 - 1.0).max(0.0);
					ui.horizontal(|ui| {
						ui.spacing_mut().item_spacing.x = TILE_GAP;
						ui.add_space(((ui.available_width() - row_width) * 0.5).max(0.0));
						for entry in entries.iter().skip(row * columns).take(columns) {
							self.participant_tile(
								ui,
								state,
								entry,
								egui::vec2(tile_width, tile_height),
							);
						}
					});
				}
			});
	}

	fn participant_tile(
		&mut self,
		ui: &mut egui::Ui,
		state: &State,
		entry: &RosterEntry,
		size: egui::Vec2,
	) {
		let (user, name) = resolve_member(state, entry);
		let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
		if !ui.is_rect_visible(rect) {
			return;
		}
		ui.painter().rect_filled(rect, 8, TILE_FILL);
		let avatar_size = (size.y * 0.45).clamp(48.0, 80.0);
		let avatar_rect = egui::Rect::from_center_size(
			rect.center() - egui::vec2(0.0, 10.0),
			egui::Vec2::splat(avatar_size),
		);
		let mut avatar_ui = ui.new_child(egui::UiBuilder::new().max_rect(avatar_rect));
		let avatar = if let Some(user) = user {
			self.avatars
				.show(&mut avatar_ui, user, avatar_size, state.demo)
		} else {
			design::avatar(&mut avatar_ui, name, avatar_size)
		};
		if self.is_speaking(state, entry.channel, &entry.participant) {
			speaking_avatar(ui, &avatar, name);
			ui.painter().rect_stroke(
				rect.shrink(1.0),
				8,
				egui::Stroke::new(2.0, design::palette(ui).positive),
				egui::StrokeKind::Inside,
			);
		}
		if let Some(user) = user {
			crate::user_menu::show(
				&avatar,
				state,
				user,
				&mut self.profile,
				&mut self.user_action,
			);
		}
		if avatar.clicked()
			&& let Some(user) = user
		{
			self.profile = Some(user.clone());
		}
		// Name badge, bottom-left, with mute/deafen glyphs like Discord's tiles.
		let font = egui::FontId::new(13.0, design::medium_family(ui.ctx()));
		let icons = usize::from(entry.participant.muted) + usize::from(entry.participant.deafened);
		let max_text = size.x - 24.0 - icons as f32 * 20.0;
		let galley = ui
			.painter()
			.layout(name.to_owned(), font, STAGE_TEXT, max_text.max(20.0));
		let badge = egui::Rect::from_min_size(
			rect.left_bottom() + egui::vec2(8.0, -8.0 - 24.0),
			egui::vec2(galley.size().x + 16.0 + icons as f32 * 20.0, 24.0),
		);
		ui.painter()
			.rect_filled(badge, 6, egui::Color32::from_black_alpha(160));
		let mut x = badge.left() + 8.0;
		for (show, icon) in [
			(entry.participant.muted, crate::icons::Icon::MicrophoneSlash),
			(
				entry.participant.deafened,
				crate::icons::Icon::HeadphonesSlash,
			),
		] {
			if show {
				crate::icons::paint(
					ui.painter(),
					icon,
					egui::Rect::from_center_size(
						egui::pos2(x + 8.0, badge.center().y),
						egui::Vec2::splat(16.0),
					),
					design::palette(ui).danger,
				);
				x += 20.0;
			}
		}
		ui.painter().galley(
			egui::pos2(x, badge.center().y - galley.size().y * 0.5),
			galley,
			STAGE_TEXT,
		);
		response.on_hover_text(name);
	}

	fn stage_notices(&self, state: &State, channel: Id, connected: bool) -> Vec<(String, bool)> {
		let mut notices = Vec::new();
		if let Some(call) = state.voice.active.as_ref().filter(|c| c.channel == channel) {
			let mut status = if state.demo {
				"Voice preview".to_owned()
			} else {
				call.phase.label().to_owned()
			};
			if let Some(elapsed) = elapsed_label(call) {
				status = format!("{status} · {elapsed}");
			}
			notices.push((status, true));
			if !state.demo
				&& !self.screen.status.is_empty()
				&& self.screen.context == Some((state.generation, channel, call.request))
			{
				notices.push((self.screen.status.to_owned(), false));
			}
			if let Some(error) = call.error {
				notices.push((error.to_owned(), false));
			}
			if call.server_deafened {
				notices.push(("Deafened by the server".into(), false));
			} else if call.server_muted {
				notices.push(("Muted by the server".into(), false));
			}
			if self.voice_push_to_talk {
				notices.push((
					"Push to talk · hold V while focused and not typing".into(),
					false,
				));
			}
			if !state.demo && !state.can_speak(channel) {
				notices.push((
					"Speaking is unavailable in this channel. You can still listen.".into(),
					false,
				));
			} else if !state.demo
				&& state.permission(channel, model::permissions::USE_VAD) != Some(true)
			{
				notices.push((
					"Push-to-talk is required to speak here. Enable it in Voice settings.".into(),
					false,
				));
			}
		} else if !connected {
			if state.demo {
				notices.push((
					"Synthetic participants · microphone and speakers are off.".into(),
					false,
				));
			} else if let Some(reason) = self.call_unavailable(state, channel) {
				notices.push((reason.to_owned(), false));
			}
		}
		notices
	}

	fn call_unavailable(&self, state: &State, channel: Id) -> Option<&'static str> {
		if state.demo {
			Some("Calls are unavailable in the offline preview. No microphone is accessed.")
		} else if !self.voice_available {
			Some(
				"This is the text-only build. Install a voice-enabled build to join voice channels and make calls.",
			)
		} else if state.auth != AuthState::Authenticated || !state.gateway_connected {
			Some("Reconnect to Discord before calling.")
		} else if state.voice.active.is_some() {
			Some("Leave your current call before starting another.")
		} else if !state.can_call(channel) {
			Some("Joining this channel is unavailable with current permission information.")
		} else {
			None
		}
	}

	pub(super) fn call_button(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		channel: Id,
		commands: &mut Vec<Command>,
	) -> egui::Response {
		let unavailable = self.call_unavailable(state, channel);
		let incoming = state.voice.incoming == Some(channel);
		let guild = state
			.channels
			.iter()
			.any(|c| c.id == channel && c.kind == 2);
		let label = if guild {
			"Join Voice"
		} else if incoming {
			"Answer call"
		} else {
			"Start voice call"
		};
		let hint = unavailable.unwrap_or(if state.can_speak(channel) {
			"Join audio. Your microphone starts after the call is secured."
		} else {
			"Join to listen. Speaking is unavailable in this channel."
		});
		// Guild channels keep Discord's green Join Voice button; DM headers use an icon.
		let response = if guild {
			let colors = design::palette(ui);
			ui.add_enabled(
				unavailable.is_none(),
				egui::Button::new(design::medium(ui, label, 15.0).color(egui::Color32::WHITE))
					.fill(colors.positive)
					.stroke(egui::Stroke::NONE)
					.corner_radius(8)
					.min_size(egui::vec2(160.0, 40.0)),
			)
		} else {
			ui.add_enabled_ui(unavailable.is_none(), |ui| {
				crate::icons::button(ui, crate::icons::Icon::Phone, 32.0, label)
			})
			.inner
		}
		.on_hover_text(hint)
		.on_disabled_hover_text(hint);
		if response.clicked()
			&& let Some(command) = state.start_call(channel, !guild && !incoming)
		{
			commands.push(command);
		}
		response
	}

	pub(super) fn voice_settings(&mut self, ui: &mut egui::Ui, demo: bool, active: bool) {
		let trigger =
			crate::icons::button(ui, crate::icons::Icon::Headphones, 32.0, "Voice settings");
		egui::Popup::menu(&trigger).show(|ui| self.voice_settings_menu(ui, demo, active));
	}

	pub(super) fn voice_settings_menu(&mut self, ui: &mut egui::Ui, demo: bool, active: bool) {
		ui.set_max_width(300.0);
		ui.strong("Voice settings");
		if demo || !self.voice_available {
			ui.label(if demo {
				"Microphone and speakers are unavailable in the offline preview."
			} else {
				"Install a voice-enabled build to join voice channels and make calls."
			});
			return;
		}
		if active && let Some(code) = &self.voice_privacy_code {
			ui.label("Voice privacy code");
			ui.add(egui::Label::new(code).selectable(true).wrap());
			ui.label(
				RichText::new(
					"Compare with the other participants; this code changes with the encrypted call group.",
				)
				.small(),
			);
			ui.separator();
		}
		ui.label("Microphone");
		device_combo(ui, "voice-input", &self.voice_inputs, &mut self.voice_input);
		ui.label("Speakers");
		device_combo(
			ui,
			"voice-output",
			&self.voice_outputs,
			&mut self.voice_output,
		);
		gain_controls(ui, &mut self.voice_gain);
		if ui.button("Refresh audio devices").clicked() {
			self.voice_refresh_devices = true;
		}
		if !self.voice_device_status.is_empty() {
			ui.label(self.voice_device_status);
		}
		ui.separator();
		ui.label("Echo cancellation · Always on");
		ui.checkbox(&mut self.voice_noise_suppression, "Noise suppression")
			.on_hover_text(
				"Reduces background sounds locally while keeping your voice. Echo cancellation stays on.",
			);
		ui.label(RichText::new("Reduces keyboard noise, breathing and fans. Strong wind or distorted audio may still get through.").small());
		ui.separator();
		ui.checkbox(&mut self.voice_push_to_talk, "Push to talk");
		ui.label(RichText::new("Hold V while this window is focused and you are not typing. Mute and deafen always take priority.").small());
		ui.label(RichText::new("Voice settings apply to this session. Microphone capture begins only after you join a secured call.").small());
	}

	/// Whether the local mute/deafen controls may emit commands for the active call.
	fn controls_enabled(&self, state: &State) -> bool {
		self.voice_available
			&& !state.demo
			&& state
				.voice
				.active
				.as_ref()
				.is_some_and(|call| call.phase != Phase::Failed)
	}

	/// Mute or deafen toggle: red slashed glyph while active, like Discord's user area.
	pub(super) fn mute_toggle(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		commands: &mut Vec<Command>,
		deafen: bool,
		size: f32,
	) -> egui::Response {
		let colors = design::palette(ui);
		let Some(call) = state.voice.active.as_ref() else {
			let (rect, response) =
				ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::hover());
			crate::icons::paint(
				ui.painter(),
				if deafen {
					crate::icons::Icon::Headphones
				} else {
					crate::icons::Icon::Microphone
				},
				rect.shrink(size * 0.2),
				colors.muted.gamma_multiply(0.5),
			);
			let label = if deafen { "Deafen" } else { "Mute" };
			response
				.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, false, label));
			return response.on_hover_text("Join a voice channel or call first.");
		};
		let channel = call.channel;
		let can_speak = state.can_speak(channel);
		let (mut muted, mut deafened) = (call.muted || !can_speak, call.deafened);
		let active = if deafen { deafened } else { muted };
		let enabled = self.controls_enabled(state) && (deafen || can_speak);
		let label = match (deafen, active) {
			(true, true) => "Undeafen",
			(true, false) => "Deafen",
			(false, true) => "Unmute",
			(false, false) => "Mute",
		};
		let response = ui
			.add_enabled_ui(enabled, |ui| {
				let (rect, response) =
					ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::click());
				if response.hovered() || response.has_focus() {
					ui.painter().rect_filled(rect, 6, colors.hover);
				}
				let icon = match (deafen, active) {
					(true, true) => crate::icons::Icon::HeadphonesSlash,
					(true, false) => crate::icons::Icon::Headphones,
					(false, true) => crate::icons::Icon::MicrophoneSlash,
					(false, false) => crate::icons::Icon::Microphone,
				};
				let color = if !enabled {
					colors.muted.gamma_multiply(0.5)
				} else if active {
					colors.danger
				} else if response.hovered() || response.has_focus() {
					colors.text_strong
				} else {
					colors.muted
				};
				crate::icons::paint(ui.painter(), icon, rect.shrink(size * 0.2), color);
				response.widget_info(|| {
					egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, active, label)
				});
				response
			})
			.inner
			.on_hover_text(if enabled {
				label
			} else if !can_speak && !deafen {
				"Speaking is unavailable in this channel."
			} else {
				"Controls are unavailable in this build or preview."
			});
		if response.clicked() {
			if deafen {
				deafened = !deafened;
			} else {
				muted = !muted;
			}
			if let Some(command) = state.set_call_mute(muted, deafened) {
				commands.push(command);
			}
		}
		response
	}

	/// Discord's call control bar: mic and camera pills, tools, and the red hang-up button.
	fn call_controls(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		channel: Id,
		commands: &mut Vec<Command>,
	) {
		let colors = design::palette(ui);
		let Some(call) = state.voice.active.as_ref() else {
			return;
		};
		let phase = call.phase;
		let can_speak = state.can_speak(channel);
		let (mut muted, mut deafened) = (call.muted || !can_speak, call.deafened);
		let controls = self.controls_enabled(state);
		let compact = ui.available_width() < 480.0;
		let width = if compact {
			76.0 + 12.0 + 48.0 + 12.0 + 64.0
		} else {
			152.0 + 12.0 + 192.0 + 12.0 + 64.0
		};
		let mut mute_clicked = false;
		let mut deafen_changed = false;
		let mut leave = false;
		ui.horizontal(|ui| {
			ui.spacing_mut().item_spacing.x = 12.0;
			ui.add_space(((ui.available_width() - width) * 0.5).max(0.0));
			pill(ui, |ui| {
				let mic = control(
					ui,
					if muted {
						crate::icons::Icon::MicrophoneSlash
					} else {
						crate::icons::Icon::Microphone
					},
					48.0,
					controls && can_speak,
					if muted { colors.danger } else { STAGE_TEXT },
					if muted { "Unmute" } else { "Mute" },
					if !can_speak {
						"Speaking is unavailable in this channel."
					} else if muted {
						"Turn on microphone"
					} else {
						"Turn off microphone"
					},
				);
				mute_clicked = mic.clicked();
				let settings = control(
					ui,
					crate::icons::Icon::ChevronDown,
					28.0,
					true,
					STAGE_TEXT,
					"Voice settings",
					"Microphone and speaker settings",
				);
				egui::Popup::menu(&settings)
					.show(|ui| self.voice_settings_menu(ui, state.demo, true));
				if !compact {
					control(
						ui,
						crate::icons::Icon::VideoSlash,
						48.0,
						false,
						STAGE_TEXT,
						"Camera",
						"Camera is not available in Serein.",
					);
					control(
						ui,
						crate::icons::Icon::ChevronDown,
						28.0,
						false,
						STAGE_TEXT,
						"Camera settings",
						"Camera is not available in Serein.",
					);
				}
			});
			if !compact {
				pill(ui, |ui| {
					self.screen_share_control(ui, state);
					for (icon, label) in [
						(crate::icons::Icon::Activities, "Activities"),
						(crate::icons::Icon::Soundboard, "Soundboard"),
					] {
						control(
							ui,
							icon,
							48.0,
							false,
							STAGE_TEXT,
							label,
							"Not available in Serein.",
						);
					}
					let more = control(
						ui,
						crate::icons::Icon::More,
						48.0,
						true,
						STAGE_TEXT,
						"More options",
						"Deafen, push to talk and call details",
					);
					egui::Popup::menu(&more).show(|ui| {
						ui.set_min_width(220.0);
						if ui
							.add_enabled(controls, egui::Checkbox::new(&mut deafened, "Deafen"))
							.changed()
						{
							deafen_changed = true;
						}
						ui.add_enabled(
							!state.demo && self.voice_available,
							egui::Checkbox::new(&mut self.voice_push_to_talk, "Push to talk"),
						);
						if let Some(code) = &self.voice_privacy_code {
							ui.separator();
							ui.label(RichText::new("Voice privacy code").small());
							ui.add(egui::Label::new(code).selectable(true).wrap());
						}
					});
				});
			}
			if compact {
				pill(ui, |ui| self.screen_share_control(ui, state));
			}
			let hang_up = {
				let (rect, response) =
					ui.allocate_exact_size(egui::vec2(64.0, 44.0), egui::Sense::click());
				let enabled = !state.demo;
				let fill = if !enabled {
					colors.danger.gamma_multiply(0.45)
				} else if response.hovered() || response.has_focus() {
					colors.danger.gamma_multiply(0.85)
				} else {
					colors.danger
				};
				ui.painter().rect_filled(rect, 12, fill);
				crate::icons::paint(
					ui.painter(),
					crate::icons::Icon::HangUp,
					egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(22.0)),
					egui::Color32::WHITE,
				);
				let label = if phase == Phase::Failed {
					"Dismiss call"
				} else {
					"Disconnect"
				};
				response.widget_info(|| {
					egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label)
				});
				response.on_hover_text(if enabled {
					label
				} else {
					"Leaving is unavailable in the offline preview."
				})
			};
			leave = hang_up.clicked() && !state.demo;
		});
		if mute_clicked {
			muted = !muted;
		}
		if (mute_clicked || deafen_changed)
			&& let Some(command) = state.set_call_mute(muted, deafened)
		{
			commands.push(command);
		}
		if leave && let Some(command) = state.leave_call() {
			commands.push(command);
		}
	}

	fn screen_share_control(&mut self, ui: &mut egui::Ui, state: &State) {
		let enabled = self.screen.busy
			|| state.demo
			|| (self.screen.supported
				&& state.voice.active.as_ref().is_some_and(|call| {
					call.phase == Phase::Connected && state.can_stream(call.channel)
				}));
		let label = if self.screen.busy {
			"Stop sharing"
		} else {
			"Share your screen"
		};
		let color = if self.screen.busy {
			design::palette(ui).accent
		} else {
			STAGE_TEXT
		};
		if control(
			ui,
			crate::icons::Icon::ScreenShare,
			48.0,
			enabled,
			color,
			label,
			if enabled {
				label
			} else {
				"Screen sharing requires a connected call and video permission on macOS or Windows."
			},
		)
		.clicked()
		{
			self.screen.launch(state);
		}
	}

	/// DM call stage above the conversation, plus the incoming-call banner.
	pub(super) fn call_bar(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		commands: &mut Vec<Command>,
	) {
		let colors = design::palette(ui);
		let selected = state.selected;
		if let Some(channel) = state
			.voice
			.active
			.as_ref()
			.filter(|call| call.guild.is_none() && Some(call.channel) == selected)
			.map(|call| call.channel)
		{
			let height = (ui.available_height() * 0.42).clamp(240.0, 340.0);
			egui::Panel::top("dm-call")
				.exact_size(height)
				.show_separator_line(false)
				.frame(egui::Frame::new().fill(STAGE_FILL))
				.show(ui, |ui| {
					let rect = ui.max_rect();
					let notices = self.stage_notices(state, channel, true);
					let mut notice_ui = ui.new_child(
						egui::UiBuilder::new()
							.max_rect(rect.shrink(STAGE_MARGIN))
							.layout(egui::Layout::top_down(egui::Align::Min)),
					);
					stage_notices(&mut notice_ui, &notices);
					let participants: Vec<(Option<model::User>, Participant)> = state
						.voice
						.active
						.as_ref()
						.map(|call| {
							call.participants
								.iter()
								.map(|p| (participant_user(state, channel, p.user).cloned(), *p))
								.collect()
						})
						.unwrap_or_default();
					let visible = ((rect.width() - 2.0 * STAGE_MARGIN + 16.0) / 96.0)
						.floor()
						.max(1.0) as usize;
					let shown = participants.len().min(visible);
					let extra = participants.len() - shown;
					let count = shown + usize::from(extra > 0);
					let row_width = count as f32 * 80.0 + (count as f32 - 1.0).max(0.0) * 16.0;
					let row = egui::Rect::from_center_size(
						egui::pos2(rect.center().x, rect.center().y - 24.0),
						egui::vec2(row_width, 80.0),
					);
					let mut row_ui = ui.new_child(
						egui::UiBuilder::new()
							.max_rect(row)
							.layout(egui::Layout::left_to_right(egui::Align::Center)),
					);
					row_ui.spacing_mut().item_spacing.x = 16.0;
					for (user, participant) in participants.iter().take(shown) {
						let name = user.as_ref().map_or("Participant", |u| u.name.as_str());
						let avatar = match user {
							Some(user) => self.avatars.show(&mut row_ui, user, 80.0, state.demo),
							None => design::avatar(&mut row_ui, name, 80.0),
						};
						if self.is_speaking(state, channel, participant) {
							speaking_avatar(&row_ui, &avatar, name);
						}
						let badge_icon = if participant.deafened {
							Some(crate::icons::Icon::HeadphonesSlash)
						} else if participant.muted {
							Some(crate::icons::Icon::MicrophoneSlash)
						} else {
							None
						};
						if let Some(icon) = badge_icon {
							let center = avatar.rect.right_bottom() - egui::vec2(12.0, 12.0);
							row_ui.painter().circle_filled(center, 14.0, TILE_FILL);
							crate::icons::paint(
								row_ui.painter(),
								icon,
								egui::Rect::from_center_size(center, egui::Vec2::splat(16.0)),
								colors.danger,
							);
						}
						if let Some(user) = user {
							crate::user_menu::show(
								&avatar,
								state,
								user,
								&mut self.profile,
								&mut self.user_action,
							);
						}
						if avatar
							.on_hover_text(if participant.deafened {
								format!("{name} · Deafened")
							} else if participant.muted {
								format!("{name} · Muted")
							} else {
								name.to_owned()
							})
							.clicked() && let Some(user) = user
						{
							self.profile = Some(user.clone());
						}
					}
					if extra > 0 {
						let (more, _) = row_ui
							.allocate_exact_size(egui::Vec2::splat(80.0), egui::Sense::hover());
						row_ui
							.painter()
							.circle_filled(more.center(), 40.0, TILE_FILL);
						row_ui.painter().text(
							more.center(),
							egui::Align2::CENTER_CENTER,
							format!("+{extra}"),
							egui::FontId::new(20.0, design::semibold_family(ui.ctx())),
							STAGE_TEXT,
						);
					}
					let bar = egui::Rect::from_min_max(
						egui::pos2(rect.left(), rect.bottom() - 44.0 - STAGE_MARGIN),
						egui::pos2(rect.right(), rect.bottom() - STAGE_MARGIN),
					);
					let mut bar_ui = ui.new_child(
						egui::UiBuilder::new()
							.max_rect(bar)
							.layout(egui::Layout::left_to_right(egui::Align::Center)),
					);
					self.call_controls(&mut bar_ui, state, channel, commands);
				});
		}
		if let Some(channel) = state.voice.incoming {
			let caller = state
				.channels
				.iter()
				.find(|c| c.id == channel)
				.and_then(|c| c.recipients.first())
				.cloned();
			let name = state
				.channels
				.iter()
				.find(|c| c.id == channel)
				.map_or("Direct message", |c| c.name.as_str())
				.to_owned();
			let unavailable = self.call_unavailable(state, channel);
			egui::Panel::top("dm-incoming")
				.show_separator_line(false)
				.frame(
					egui::Frame::new()
						.fill(colors.raised)
						.inner_margin(egui::Margin::symmetric(16, 10)),
				)
				.show(ui, |ui| {
					ui.horizontal(|ui| {
						ui.spacing_mut().item_spacing.x = 12.0;
						match &caller {
							Some(user) => {
								self.avatars.show(ui, user, 40.0, state.demo);
							}
							None => {
								design::avatar(ui, &name, 40.0);
							}
						}
						ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
							ui.spacing_mut().item_spacing.x = 8.0;
							let decline = round_action(
								ui,
								crate::icons::Icon::HangUp,
								colors.danger,
								!state.demo,
								"Decline",
							);
							if decline.clicked()
								&& let Some(command) = state.decline_call()
							{
								commands.push(command);
							}
							let answer = round_action(
								ui,
								crate::icons::Icon::Phone,
								colors.positive,
								unavailable.is_none(),
								"Answer",
							)
							.on_disabled_hover_text(unavailable.unwrap_or(""));
							if answer.clicked()
								&& let Some(command) = state.start_call(channel, false)
							{
								commands.push(command);
							}
							ui.with_layout(
								egui::Layout::left_to_right(egui::Align::Center),
								|ui| {
									ui.vertical(|ui| {
										ui.spacing_mut().item_spacing.y = 2.0;
										ui.add(
											egui::Label::new(
												design::semibold(ui, &name, 15.0)
													.color(colors.text_strong),
											)
											.truncate(),
										);
										ui.label(
											RichText::new(unavailable.unwrap_or("Incoming call…"))
												.size(13.0)
												.color(colors.muted),
										);
									});
								},
							);
						});
					});
				});
		}
	}

	/// Sidebar panel above the account card while connected: Discord's "Voice Connected" area.
	pub(super) fn voice_connection_panel(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		commands: &mut Vec<Command>,
	) {
		let Some(call) = state.voice.active.as_ref() else {
			return;
		};
		let colors = design::palette(ui);
		let phase = call.phase;
		let connected = matches!(phase, Phase::Connected | Phase::Waiting);
		let channel = state
			.channels
			.iter()
			.find(|c| c.id == call.channel)
			.map_or("Direct message", |c| c.name.as_str())
			.to_owned();
		let guild = call
			.guild
			.and_then(|id| state.guilds.iter().find(|g| g.id == id))
			.map(|g| g.name.clone());
		let detail = match guild {
			Some(guild) => format!("{channel} / {guild}"),
			None => channel,
		};
		let title = if state.demo {
			"Voice preview"
		} else if phase == Phase::Failed {
			"Call failed"
		} else if connected {
			"Voice Connected"
		} else {
			"Connecting…"
		};
		let color = if phase == Phase::Failed {
			colors.danger
		} else if connected || state.demo {
			colors.positive
		} else {
			colors.warning
		};
		egui::Panel::bottom("voice-connection")
			.show_separator_line(false)
			.frame(egui::Frame::new().inner_margin(egui::Margin {
				left: 8,
				right: 8,
				top: 0,
				bottom: 0,
			}))
			.show(ui, |ui| {
				egui::Frame::new()
					.fill(colors.raised)
					.corner_radius(8)
					.inner_margin(egui::Margin::symmetric(8, 6))
					.show(ui, |ui| {
						ui.set_width(ui.available_width());
						ui.horizontal(|ui| {
							ui.spacing_mut().item_spacing.x = 8.0;
							ui.with_layout(
								egui::Layout::right_to_left(egui::Align::Center),
								|ui| {
									let leave = ui
										.add_enabled_ui(!state.demo, |ui| {
											crate::icons::button(
												ui,
												crate::icons::Icon::HangUp,
												32.0,
												if phase == Phase::Failed {
													"Dismiss call"
												} else {
													"Disconnect"
												},
											)
										})
										.inner;
									if leave.clicked()
										&& let Some(command) = state.leave_call()
									{
										commands.push(command);
									}
									ui.with_layout(
										egui::Layout::left_to_right(egui::Align::Center),
										|ui| {
											crate::icons::inline(
												ui,
												crate::icons::Icon::InCall,
												18.0,
												color,
											);
											ui.vertical(|ui| {
												ui.spacing_mut().item_spacing.y = 0.0;
												ui.add(
													egui::Label::new(
														design::semibold(ui, title, 13.0)
															.color(color),
													)
													.truncate()
													.selectable(false),
												);
												ui.add(
													egui::Label::new(
														RichText::new(detail)
															.size(12.0)
															.color(colors.muted),
													)
													.truncate()
													.selectable(false),
												);
											});
										},
									);
								},
							);
						});
					});
			});
		if connected && ui.is_rect_visible(ui.max_rect()) {
			ui.ctx()
				.request_repaint_after(std::time::Duration::from_secs(1));
		}
	}
}

/// Discord's call stage is black in every appearance; pills and tiles sit on it in fixed greys.
const STAGE_FILL: egui::Color32 = egui::Color32::BLACK;
const TILE_FILL: egui::Color32 = egui::Color32::from_rgb(0x2b, 0x2d, 0x31);
const PILL_FILL: egui::Color32 = egui::Color32::from_rgb(0x1e, 0x1f, 0x22);
const STAGE_TEXT: egui::Color32 = egui::Color32::from_rgb(0xdb, 0xde, 0xe1);
const STAGE_MUTED: egui::Color32 = egui::Color32::from_rgb(0x9a, 0x9b, 0xa1);
const STAGE_MARGIN: f32 = 16.0;
const TILE_GAP: f32 = 8.0;

/// Ring plus sound glyph keeps activity legible without relying on color alone.
fn speaking_avatar(ui: &egui::Ui, avatar: &egui::Response, name: &str) {
	let colors = design::palette(ui);
	ui.painter().circle_stroke(
		avatar.rect.center(),
		avatar.rect.width() * 0.5 + 2.0,
		egui::Stroke::new(2.0, colors.positive),
	);
	let size = (avatar.rect.width() * 0.3).clamp(10.0, 18.0);
	let badge = egui::Rect::from_center_size(
		avatar.rect.right_bottom() - egui::Vec2::splat(size * 0.4),
		egui::Vec2::splat(size),
	);
	ui.painter()
		.circle_filled(badge.center(), size * 0.65, colors.raised);
	crate::icons::paint(
		ui.painter(),
		crate::icons::Icon::Speaker,
		badge,
		colors.positive,
	);
	let label = format!("{name} · Speaking");
	avatar.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Image, true, &label));
	avatar.clone().on_hover_text(label);
}

fn stage_notices(ui: &mut egui::Ui, notices: &[(String, bool)]) {
	ui.spacing_mut().item_spacing.y = 2.0;
	for (text, strong) in notices {
		ui.add(
			egui::Label::new(
				RichText::new(text)
					.size(if *strong { 13.0 } else { 12.0 })
					.color(if *strong { STAGE_TEXT } else { STAGE_MUTED }),
			)
			.truncate()
			.selectable(false),
		);
	}
	if !notices.is_empty() {
		ui.add_space(6.0);
	}
	ui.spacing_mut().item_spacing.y = 8.0;
}

/// Rounded dark group holding several call controls.
fn pill<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
	let frame = egui::Frame::new()
		.fill(PILL_FILL)
		.corner_radius(12)
		.show(ui, |ui| {
			ui.horizontal(|ui| {
				ui.spacing_mut().item_spacing.x = 0.0;
				add(ui)
			})
			.inner
		});
	ui.painter().rect_stroke(
		frame.response.rect,
		12,
		egui::Stroke::new(1.0, egui::Color32::from_white_alpha(18)),
		egui::StrokeKind::Inside,
	);
	frame.inner
}

/// One control inside a pill; disabled controls stay visible but inert, like Discord's.
fn control(
	ui: &mut egui::Ui,
	icon: crate::icons::Icon,
	width: f32,
	enabled: bool,
	color: egui::Color32,
	label: &str,
	hint: &str,
) -> egui::Response {
	let (rect, response) = ui.allocate_exact_size(
		egui::vec2(width, 44.0),
		if enabled {
			egui::Sense::click()
		} else {
			egui::Sense::hover()
		},
	);
	if enabled && (response.hovered() || response.has_focus()) {
		ui.painter()
			.rect_filled(rect.shrink(3.0), 8, egui::Color32::from_white_alpha(28));
	}
	let size = if width < 40.0 { 14.0 } else { 22.0 };
	crate::icons::paint(
		ui.painter(),
		icon,
		egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(size)),
		if enabled {
			color
		} else {
			STAGE_MUTED.gamma_multiply(0.45)
		},
	);
	response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
	response.on_hover_text(hint)
}

/// Circular filled action (answer/decline) used by the incoming-call banner.
fn round_action(
	ui: &mut egui::Ui,
	icon: crate::icons::Icon,
	fill: egui::Color32,
	enabled: bool,
	label: &str,
) -> egui::Response {
	let (rect, response) = ui.allocate_exact_size(
		egui::Vec2::splat(40.0),
		if enabled {
			egui::Sense::click()
		} else {
			egui::Sense::hover()
		},
	);
	let fill = if !enabled {
		fill.gamma_multiply(0.45)
	} else if response.hovered() || response.has_focus() {
		fill.gamma_multiply(0.85)
	} else {
		fill
	};
	ui.painter().circle_filled(rect.center(), 20.0, fill);
	crate::icons::paint(
		ui.painter(),
		icon,
		egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(20.0)),
		egui::Color32::WHITE,
	);
	response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
	response.on_hover_text(label)
}

/// Resolve the display name and user for a roster entry from the entry, member list or self.
fn resolve_member<'a>(
	state: &'a State,
	entry: &'a RosterEntry,
) -> (Option<&'a model::User>, &'a str) {
	let member = entry.member.as_ref().or_else(|| {
		state
			.members
			.as_ref()
			.filter(|list| list.guild == Some(entry.guild))
			.and_then(|list| {
				list.rows
					.iter()
					.flatten()
					.find(|m| m.user.id == entry.participant.user)
			})
	});
	let user = member.map(|m| &m.user).or_else(|| {
		state
			.user
			.as_ref()
			.filter(|u| u.id == entry.participant.user)
	});
	let name = member
		.and_then(|m| m.nick.as_deref())
		.or_else(|| user.map(|u| u.name.as_str()))
		.unwrap_or("Participant");
	(user, name)
}

/// Find a call participant's user from self, DM recipients or the roster.
fn participant_user(state: &State, channel: Id, user: Id) -> Option<&model::User> {
	state
		.user
		.as_ref()
		.filter(|u| u.id == user)
		.or_else(|| {
			state
				.channels
				.iter()
				.find(|c| c.id == channel)
				.and_then(|c| c.recipients.iter().find(|u| u.id == user))
		})
		.or_else(|| {
			state
				.voice
				.roster
				.iter()
				.find(|e| e.channel == channel && e.participant.user == user)
				.and_then(|e| e.member.as_ref())
				.map(|m| &m.user)
		})
}

fn gain_controls(ui: &mut egui::Ui, gain: &mut crate::VoiceGain) -> [egui::Response; 2] {
	let label = ui.label("Microphone gain");
	let input = ui
		.add(
			egui::Slider::new(&mut gain.input_percent, 0..=200)
				.suffix("%")
				.step_by(1.0),
		)
		.labelled_by(label.id);
	let label = ui.label("Speaker volume");
	let output = ui
		.add(
			egui::Slider::new(&mut gain.output_percent, 0..=200)
				.suffix("%")
				.step_by(1.0),
		)
		.labelled_by(label.id);
	ui.label(RichText::new("100% keeps the original level. Boosting above 100% can clip.").small());
	if ui.small_button("Reset levels").clicked() {
		*gain = crate::VoiceGain::default();
	}
	[input, output]
}

fn elapsed_label(call: &client_core::voice::Call) -> Option<String> {
	if !matches!(call.phase, Phase::Waiting | Phase::Connected) {
		return None;
	}
	let seconds = call.connected_at?.elapsed().as_secs();
	Some(format!(
		"{:02}:{:02}:{:02}",
		seconds / 3600,
		seconds / 60 % 60,
		seconds % 60
	))
}

fn status_icon(ui: &mut egui::Ui, deafened: bool, label: &str) {
	let (rect, response) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
	crate::icons::paint(
		ui.painter(),
		if deafened {
			crate::icons::Icon::HeadphonesSlash
		} else {
			crate::icons::Icon::MicrophoneSlash
		},
		rect.shrink(1.0),
		design::palette(ui).muted,
	);
	response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label));
	response.on_hover_text(label);
}

fn device_combo(
	ui: &mut egui::Ui,
	id: &str,
	devices: &[(String, String)],
	selected: &mut Option<String>,
) {
	let label = match selected.as_ref() {
		None => "System default",
		Some(id) => devices
			.iter()
			.find(|(key, _)| key == id)
			.map_or("Device unavailable", |(_, label)| label.as_str()),
	};
	egui::ComboBox::from_id_salt(id)
		.selected_text(label)
		.width(256.0)
		.height(220.0)
		.show_ui(ui, |ui| {
			ui.selectable_value(selected, None, "System default");
			for (id, label) in devices.iter().take(32) {
				ui.selectable_value(selected, Some(id.clone()), label);
			}
		});
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn gain_sliders_accept_keyboard_input_and_reset_with_the_session() {
		let mut messaging = MessagingUi::default();
		assert_eq!(messaging.voice_gain.input_percent, 100);
		assert_eq!(messaging.voice_gain.output_percent, 100);
		let ctx = egui::Context::default();
		let raw = || egui::RawInput {
			screen_rect: Some(egui::Rect::from_min_size(
				egui::Pos2::ZERO,
				egui::vec2(240.0, 260.0),
			)),
			..Default::default()
		};
		ctx.run_ui(raw(), |ui| {
			gain_controls(ui, &mut messaging.voice_gain)[0].request_focus();
		})
		.drop_without_applying_deltas();
		let mut input = raw();
		input.events.push(egui::Event::Key {
			key: egui::Key::ArrowRight,
			physical_key: None,
			pressed: true,
			repeat: false,
			modifiers: egui::Modifiers::NONE,
		});
		ctx.run_ui(input, |ui| {
			let controls = gain_controls(ui, &mut messaging.voice_gain);
			assert!(
				controls
					.iter()
					.all(|r| r.rect.right() <= ui.max_rect().right() + 1.0)
			);
		})
		.drop_without_applying_deltas();
		assert_eq!(messaging.voice_gain.input_percent, 101);
		assert_eq!(messaging.voice_gain.output_percent, 100);
		assert!(
			!messaging.voice_refresh_devices,
			"Gain does not enumerate devices"
		);
		messaging.voice_gain.input_percent = u16::MAX;
		messaging.voice_gain.output_percent = 0;
		ctx.run_ui(raw(), |ui| {
			gain_controls(ui, &mut messaging.voice_gain);
		})
		.drop_without_applying_deltas();
		assert_eq!(messaging.voice_gain.input_percent, 200);
		assert_eq!(messaging.voice_gain.output_percent, 0);
		messaging.clear();
		assert_eq!(messaging.voice_gain.input_percent, 100);
		assert_eq!(messaging.voice_gain.output_percent, 100);
	}

	#[test]
	fn guild_voice_requires_explicit_keyboard_join_and_demo_never_emits_media() {
		let mut state = test_support::demo_state();
		state.demo = false;
		assert!(
			state.select(Id(25)).is_none(),
			"Voice selection must not fetch history"
		);
		assert_eq!(state.selected, Some(Id(25)));
		let mut messaging = MessagingUi {
			voice_available: true,
			..Default::default()
		};
		let context = egui::Context::default();
		let mut commands = vec![];
		context
			.run_ui(Default::default(), |ui| {
				messaging
					.call_button(ui, &mut state, Id(25), &mut commands)
					.request_focus();
			})
			.drop_without_applying_deltas();
		assert!(commands.is_empty());
		assert!(state.voice.active.is_none());
		let enter = egui::RawInput {
			events: vec![egui::Event::Key {
				key: egui::Key::Enter,
				physical_key: None,
				pressed: true,
				repeat: false,
				modifiers: egui::Modifiers::NONE,
			}],
			..Default::default()
		};
		context
			.run_ui(enter, |ui| {
				messaging.call_button(ui, &mut state, Id(25), &mut commands);
			})
			.drop_without_applying_deltas();
		assert!(matches!(
			commands.as_slice(),
			[Command::Voice(client_core::voice::Command::Join {
				channel: Id(25),
				ring: false,
				..
			})]
		));
		let call = state.voice.active.as_mut().unwrap();
		assert!(elapsed_label(call).is_none());
		call.connected_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(3663));
		call.phase = Phase::Waiting;
		assert_eq!(elapsed_label(call).as_deref(), Some("01:01:03"));
		call.phase = Phase::Failed;
		assert!(elapsed_label(call).is_none());
		state.demo = true;
		for width in [640.0, 1120.0] {
			let mut output = context.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(width, 480.0),
					)),
					..Default::default()
				},
				|ui| {
					assert!(
						messaging
							.show(ui, &mut state)
							.iter()
							.all(|c| !matches!(c, Command::Voice(_) | Command::History { .. }))
					);
				},
			);
			output.textures_delta.clear();
		}
		assert!(messaging.take_avatar_requests().is_empty());
		state.voice.active = None;
		assert!(messaging.call_unavailable(&state, Id(25)).is_some());
		state.demo = false;
		messaging.voice_available = false;
		assert!(
			messaging
				.call_unavailable(&state, Id(25))
				.unwrap()
				.contains("text-only")
		);
	}

	#[test]
	fn voice_roster_preserves_status_space_with_long_names_and_virtualizes() {
		let mut state = State {
			demo: true,
			selected: Some(Id(25)),
			..Default::default()
		};
		state.voice.roster = (1..=64)
			.map(|id| RosterEntry {
				guild: Id(10),
				channel: Id(25),
				participant: client_core::voice::Participant {
					user: Id(id),
					muted: true,
					deafened: true,
					server_muted: false,
					server_deafened: false,
				},
				member: Some(model::Member {
					user: model::User {
						id: Id(id),
						name: "Long synthetic participant name ".repeat(5),
						avatar: None,
						discriminator: 0,
					},
					nick: None,
					roles: vec![],
					status: None,
					custom_status: None,
					activities: vec![],
				}),
			})
			.collect();
		let mut messaging = MessagingUi::default();
		let ctx = egui::Context::default();
		for theme in [egui::Theme::Light, egui::Theme::Dark] {
			ctx.set_theme(theme);
			let mut output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(190.0, 320.0),
					)),
					..Default::default()
				},
				|ui| {
					let width = ui.available_width();
					let row = ui.scope(|ui| {
						messaging.voice_participant(ui, &state, &state.voice.roster[0])
					});
					assert!(
						row.response.rect.width() <= width + 1.0,
						"Long names must not displace the mute/deafen icons"
					);
					messaging.voice_channel(ui, &mut state, Id(25), &mut vec![]);
				},
			);
			assert!(
				output.textures_delta.set.len() < 20,
				"Only visible avatars should be loaded"
			);
			output.textures_delta.clear();
		}
		assert!(messaging.take_avatar_requests().is_empty());
	}

	#[test]
	fn viewing_an_incoming_call_never_answers_and_preview_cannot_call() {
		let mut state = State {
			demo: true,
			selected: Some(Id(1)),
			auth: AuthState::Authenticated,
			gateway_connected: true,
			channels: vec![model::Channel {
				last_message: None,
				id: Id(1),
				guild: None,
				parent_id: None,
				position: 0,
				name: "Synthetic DM".into(),
				kind: 1,
				recipients: vec![model::User {
					id: Id(2),
					name: "Synthetic peer".into(),
					avatar: None,
					discriminator: 0,
				}],
				member_list_id: None,
				message_count: None,
			}],
			..Default::default()
		};
		state.voice.incoming = Some(Id(1));
		let mut messaging = MessagingUi {
			voice_available: true,
			..Default::default()
		};
		let context = egui::Context::default();
		let output = context.run_ui(Default::default(), |ui| {
			let commands = messaging.show(ui, &mut state);
			assert!(
				commands
					.iter()
					.all(|command| !matches!(command, Command::Voice(_)))
			);
		});
		output.drop_without_applying_deltas();
		assert!(state.voice.active.is_none());
		assert_eq!(state.voice.incoming, Some(Id(1)));
		assert!(messaging.call_unavailable(&state, Id(1)).is_some());
		state.demo = false;
		let output = context.run_ui(Default::default(), |ui| {
			let commands = messaging.show(ui, &mut state);
			assert!(
				commands
					.iter()
					.all(|command| !matches!(command, Command::Voice(_)))
			);
		});
		output.drop_without_applying_deltas();
		assert!(
			state.voice.active.is_none(),
			"Incoming calls require an explicit answer"
		);
		messaging.voice_available = false;
		assert!(
			messaging
				.call_unavailable(&state, Id(1))
				.unwrap()
				.contains("text-only")
		);
		messaging.voice_available = true;
		assert!(messaging.call_unavailable(&state, Id(1)).is_none());
	}
}
