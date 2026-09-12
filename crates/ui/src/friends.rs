//! Friends overview using the retained relationship/presence state and existing user actions.
use crate::{
	MessagingUi, design,
	icons::{self, Icon},
	profiles, user_menu,
};
use client_core::{Command, State};
use egui::{RichText, vec2};

#[derive(Default)]
pub(super) struct Friends {
	tab: Tab,
	query: String,
	username: String,
	outgoing: bool,
}
#[derive(Default, PartialEq, Eq, Clone, Copy)]
enum Tab {
	#[default]
	Online,
	All,
	Pending,
	Add,
}
impl Friends {
	fn matches(&self, state: &State, user: &model::User, query: &str) -> bool {
		let (status, _, _) = profiles::presence(state, user.id, None);
		(self.tab == Tab::All || matches!(status, Some("online" | "idle" | "dnd")))
			&& (user.name.to_lowercase().contains(query)
				|| state.user_display_name(user).to_lowercase().contains(query)
				|| state
					.friend_username(user.id)
					.is_some_and(|name| name.to_lowercase().contains(query)))
	}
}
impl MessagingUi {
	fn add_friend_page(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		commands: &mut Vec<Command>,
	) {
		let colors = design::palette(ui);
		egui::Frame::new().inner_margin(24).show(ui, |ui| {
			ui.label(design::semibold(ui, "Add Friend", 24.0));
			ui.label("You can add friends with their Discord username.");
			ui.add_space(18.0);
			let label = ui.label("Username");
			let input = ui
				.add_sized(
					[ui.available_width(), 48.0],
					egui::TextEdit::singleline(&mut self.friends.username)
						.hint_text("Enter a username")
						.char_limit(33)
						.align(egui::Align2::LEFT_CENTER)
						.frame(
							egui::Frame::new()
								.fill(colors.base)
								.stroke(egui::Stroke::new(1.0, colors.border))
								.corner_radius(8)
								.inner_margin(egui::Margin::symmetric(12, 8)),
						),
				)
				.labelled_by(label.id);
			if input.has_focus() {
				ui.painter().rect_stroke(
					input.rect,
					8,
					egui::Stroke::new(2.0, colors.accent),
					egui::StrokeKind::Inside,
				);
			}
			ui.add_space(12.0);
			let busy = state.user_action_pending();
			let enabled = !busy
				&& !self.friends.username.trim().is_empty()
				&& (state.demo
					|| (state.gateway_connected
						&& state.auth == client_core::auth::AuthState::Authenticated));
			if ui
				.add_enabled(
					enabled,
					egui::Button::new(
						RichText::new(if busy {
							"Sending…"
						} else {
							"Send Friend Request"
						})
						.color(colors.accent_text),
					)
					.fill(colors.accent)
					.min_size(vec2(180.0, 40.0)),
				)
				.clicked() && let Some(command) = state.add_friend(&self.friends.username)
			{
				commands.push(command);
			}
			if let Some(status) = state.user_action_status() {
				ui.label(status);
			}
			if state.demo {
				ui.colored_label(colors.muted, "Offline demo · actions are simulated.");
			} else if !state.gateway_connected {
				ui.label("Reconnect before sending a friend request.");
			}
			ui.add_space(8.0);
			ui.colored_label(
				colors.muted,
				"Personalized request notes are not supported yet.",
			);
			ui.add_space(24.0);
			ui.separator();
			ui.add_space(16.0);
			ui.label(design::semibold(ui, "Other Places to Make Friends", 20.0));
			ui.label("Don't have a username? Discover public communities in Discord.");
			ui.hyperlink_to(
				"Explore Discoverable Servers ↗",
				"https://discord.com/servers",
			);
		});
	}
	fn friend_requests_page(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		commands: &mut Vec<Command>,
	) {
		let colors = design::palette(ui);
		let mut resolve = None;
		egui::Frame::new().inner_margin(24).show(ui, |ui| {
			ui.horizontal_wrapped(|ui| {
				for outgoing in [false, true] {
					let count = state
						.pending_friends()
						.filter(|(_, _, incoming)| *incoming != outgoing)
						.count();
					if ui
						.selectable_label(
							self.friends.outgoing == outgoing,
							format!(
								"{} — {count}",
								if outgoing { "Outgoing" } else { "Incoming" }
							),
						)
						.clicked()
					{
						self.friends.outgoing = outgoing;
					}
				}
			});
			ui.add_space(12.0);
			ui.add_sized(
				[ui.available_width(), 40.0],
				egui::TextEdit::singleline(&mut self.friends.query)
					.hint_text("Search requests")
					.char_limit(128)
					.align(egui::Align2::LEFT_CENTER),
			);
			if let Some(status) = state.user_action_status() {
				ui.label(status);
			}
			ui.add_space(16.0);
			let query = self.friends.query.trim().to_lowercase();
			let mut rows: Vec<_> = state
				.pending_friends()
				.filter(|(user, name, incoming)| {
					*incoming != self.friends.outgoing
						&& (user.name.to_lowercase().contains(&query)
							|| name.to_lowercase().contains(&query))
				})
				.collect();
			rows.sort_unstable_by(|a, b| a.0.name.cmp(&b.0.name).then(a.0.id.cmp(&b.0.id)));
			if rows.is_empty() {
				ui.label(if !state.friend_requests_known() {
					"Friend requests are not available yet."
				} else if !query.is_empty() {
					"No requests match your search."
				} else if self.friends.outgoing {
					"No outgoing friend requests."
				} else {
					"No incoming friend requests."
				});
			}
			ui.spacing_mut().item_spacing.y = 0.0;
			egui::ScrollArea::vertical()
				.id_salt("friend-requests")
				.auto_shrink([false, false])
				.show_rows(ui, 72.0, rows.len(), |ui, range| {
					for (user, name, incoming) in &rows[range] {
						ui.push_id(user.id.0, |ui| {
							let (rect, _) = ui.allocate_exact_size(
								vec2(ui.available_width(), 72.0),
								egui::Sense::hover(),
							);
							ui.painter().hline(
								rect.x_range(),
								rect.top(),
								egui::Stroke::new(1.0, colors.border),
							);
							let mut row = ui.new_child(
								egui::UiBuilder::new()
									.max_rect(rect.shrink2(vec2(0.0, 12.0)))
									.layout(egui::Layout::left_to_right(egui::Align::Center)),
							);
							if self
								.avatars
								.show(&mut row, user, 40.0, state.demo)
								.clicked()
							{
								self.profile = Some(user.clone());
							}
							let width = (row.available_width() - 88.0).max(1.0);
							row.allocate_ui_with_layout(
								vec2(width, 44.0),
								egui::Layout::top_down(egui::Align::Min),
								|ui| {
									ui.add(
										egui::Label::new(design::semibold(
											ui,
											state.user_display_name(user),
											16.0,
										))
										.truncate(),
									);
									ui.add(
										egui::Label::new(RichText::new(name).color(colors.muted))
											.truncate(),
									);
								},
							);
							row.add_enabled_ui(
								!state.user_action_pending()
									&& (state.demo || state.gateway_connected),
								|ui| {
									if *incoming
										&& icons::button(ui, Icon::Check, 32.0, "Accept request")
											.clicked()
									{
										resolve = Some((user.id, true));
									}
									if icons::button(
										ui,
										Icon::Close,
										32.0,
										if *incoming {
											"Decline request"
										} else {
											"Cancel request"
										},
									)
									.clicked()
									{
										resolve = Some((user.id, false));
									}
								},
							);
						});
					}
				});
		});
		if let Some((user, accept)) = resolve
			&& let Some(command) = state.resolve_friend_request(user, accept)
		{
			commands.push(command);
		}
	}
	pub(super) fn friends_page(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		commands: &mut Vec<Command>,
	) {
		let colors = design::palette(ui);
		egui::Frame::new()
			.inner_margin(egui::Margin::symmetric(24, 8))
			.show(ui, |ui| {
				ui.horizontal_wrapped(|ui| {
					ui.set_min_height(32.0);
					ui.spacing_mut().item_spacing.x = 16.0;
					icons::inline(ui, Icon::People, 22.0, colors.muted);
					ui.label(design::semibold(ui, "Friends", 16.0));
					ui.separator();
					for (tab, title) in [
						(Tab::Online, "Online"),
						(Tab::All, "All"),
						(Tab::Pending, "Pending"),
						(Tab::Add, "Add Friend"),
					] {
						if ui
							.add(
								egui::Button::new(RichText::new(title).color(if tab == Tab::Add {
									colors.accent_text
								} else {
									colors.text
								}))
								.selected(self.friends.tab == tab)
								.fill(if tab == Tab::Add {
									colors.accent
								} else if self.friends.tab == tab {
									colors.raised
								} else {
									egui::Color32::TRANSPARENT
								}),
							)
							.clicked()
						{
							self.friends.tab = tab;
							self.friends.query.clear();
						}
					}
				});
			});
		ui.separator();
		if self.friends.tab == Tab::Add {
			self.add_friend_page(ui, state, commands);
			return;
		}
		if self.friends.tab == Tab::Pending {
			self.friend_requests_page(ui, state, commands);
			return;
		}
		let mut selected = None;
		egui::Frame::new()
			.inner_margin(egui::Margin::symmetric(24, 24))
			.show(ui, |ui| {
				egui::Frame::new()
					.stroke(egui::Stroke::new(1.0, colors.border))
					.corner_radius(8)
					.inner_margin(egui::Margin::symmetric(12, 8))
					.show(ui, |ui| {
						ui.horizontal(|ui| {
							icons::inline(ui, Icon::Search, 18.0, colors.muted);
							ui.add(
								egui::TextEdit::singleline(&mut self.friends.query)
									.hint_text("Search")
									.char_limit(128)
									.frame(egui::Frame::NONE)
									.desired_width(ui.available_width()),
							);
						});
					});
				ui.add_space(20.0);
				let query = self.friends.query.trim().to_lowercase();
				let mut friends: Vec<_> = state
					.friends()
					.filter(|user| self.friends.matches(state, user, &query))
					.collect();
				friends.sort_unstable_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
				ui.label(
					RichText::new(format!(
						"{} \u{2014} {}",
						if self.friends.tab == Tab::All {
							"All friends"
						} else {
							"Online"
						},
						friends.len()
					))
					.size(13.0)
					.color(colors.muted),
				);
				ui.add_space(12.0);
				ui.spacing_mut().item_spacing.y = 0.0;
				if friends.is_empty() {
					ui.add_space(20.0);
					ui.label(
						RichText::new(if !state.friends_known() {
							"Friends are not available yet."
						} else if !query.is_empty() {
							"No friends match your search."
						} else if self.friends.tab == Tab::All {
							"No friends yet."
						} else {
							"No friends are currently online."
						})
						.color(colors.muted),
					);
					return;
				}
				egui::ScrollArea::vertical()
					.id_salt("friends-list")
					.auto_shrink([false, false])
					.show_rows(ui, 64.0, friends.len(), |ui, range| {
						for user in &friends[range] {
							ui.push_id(user.id.0, |ui| {
								let (status, custom, activities) =
									profiles::presence(state, user.id, None);
								let (rect, response) = ui.allocate_exact_size(
									vec2(ui.available_width(), 64.0),
									egui::Sense::click(),
								);
								ui.painter().hline(
									rect.x_range(),
									rect.top(),
									egui::Stroke::new(1.0, colors.border),
								);
								if response.hovered() || response.has_focus() {
									ui.painter().rect_filled(rect.shrink(1.0), 6, colors.hover);
								}
								response.widget_info(|| {
									egui::WidgetInfo::labeled(
										egui::WidgetType::Button,
										true,
										format!("Profile: {}", user.name),
									)
								});
								if response.clicked() {
									self.profile = Some((*user).clone());
								}
								user_menu::show(
									&response,
									state,
									user,
									&mut self.profile,
									&mut self.user_action,
								);
								let mut avatar_ui = ui.new_child(egui::UiBuilder::new().max_rect(
									egui::Rect::from_min_size(
										rect.min + vec2(0.0, 12.0),
										vec2(40.0, 40.0),
									),
								));
								let avatar =
									self.avatars.show(&mut avatar_ui, user, 40.0, state.demo);
								if avatar.clicked() {
									self.profile = Some((*user).clone());
								}
								if let Some(status) = status {
									design::presence_dot(
										ui,
										avatar.rect,
										profiles::presence_color(status),
										colors.chat,
									);
								}
								let mut text = ui.new_child(egui::UiBuilder::new().max_rect(
									egui::Rect::from_min_max(
										rect.min + vec2(52.0, 12.0),
										rect.max - vec2(100.0, 8.0),
									),
								));
								text.spacing_mut().item_spacing.y = 1.0;
								text.add(
									egui::Label::new(design::semibold(
										ui,
										state.user_display_name(user),
										16.0,
									))
									.truncate(),
								);
								let subtitle = profiles::subtitle(custom, activities)
									.unwrap_or_else(|| {
										status
											.map_or(
												"Presence unavailable",
												profiles::presence_label,
											)
											.into()
									});
								text.horizontal(|ui| {
									if let Some(activity) = activities.first() {
										icons::inline(
											ui,
											if activity.kind == 2 {
												Icon::Spotify
											} else {
												Icon::GameController
											},
											14.0,
											colors.positive,
										);
									}
									ui.add(
										egui::Label::new(
											RichText::new(&subtitle).size(13.0).color(colors.muted),
										)
										.truncate(),
									)
									.on_hover_text(&subtitle);
								});
								let dm = state.channels.iter().find(|c| {
									c.guild.is_none()
										&& c.kind == 1 && c.recipients.iter().any(|u| u.id == user.id)
								});
								let mut actions = ui.new_child(
									egui::UiBuilder::new()
										.max_rect(egui::Rect::from_min_size(
											rect.right_center() - vec2(88.0, 18.0),
											vec2(88.0, 36.0),
										))
										.layout(egui::Layout::left_to_right(egui::Align::Center)),
								);
								actions.spacing_mut().item_spacing.x = 8.0;
								let message = actions
									.add_enabled_ui(dm.is_some(), |ui| {
										icons::button(ui, Icon::Threads, 36.0, "Message")
									})
									.inner;
								if message
									.on_disabled_hover_text(
										"No open direct message with this friend",
									)
									.clicked()
								{
									selected = dm.map(|c| c.id);
								}
								let more = icons::button(&mut actions, Icon::More, 36.0, "More");
								egui::Popup::menu(&more).show(|ui| {
									user_menu::contents(
										ui,
										state,
										user,
										&mut self.profile,
										&mut self.user_action,
									)
								});
							});
						}
					});
			});
		if let Some(channel) = selected
			&& let Some(command) = state.select(channel)
		{
			commands.push(command);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	fn labels(shape: &egui::Shape, out: &mut Vec<String>) {
		match shape {
			egui::Shape::Text(text) => out.push(text.galley.job.text.clone()),
			egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| labels(shape, out)),
			_ => {}
		}
	}
	#[test]
	fn friends_filters_search_and_rows_fit_both_themes() {
		for (width, theme) in [(320., egui::Theme::Dark), (1000., egui::Theme::Light)] {
			let ctx = egui::Context::default();
			design::apply(&ctx);
			ctx.set_theme(theme);
			let mut state = test_support::friends_demo_state();
			let mut view = MessagingUi::default();
			let render = |view: &mut MessagingUi, state: &mut State| {
				let mut commands = Vec::new();
				let output = ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							vec2(width, 760.),
						)),
						..Default::default()
					},
					|ui| {
						view.friends_page(ui, state, &mut commands);
						assert!(ui.min_rect().width() <= width, "friends overflow");
					},
				);
				assert!(
					commands.is_empty(),
					"rendering must not send friend actions"
				);
				let mut text = Vec::new();
				for shape in &output.shapes {
					labels(&shape.shape, &mut text);
				}
				output.drop_without_applying_deltas();
				text
			};
			let text = render(&mut view, &mut state);
			assert!(
				text.iter()
					.any(|s| s.starts_with("Online") && s.ends_with('7')),
				"{text:?}"
			);
			assert!(text.iter().any(|s| s == "Robin"));
			assert!(!text.iter().any(|s| s == "Parker"));
			view.friends.tab = Tab::All;
			let text = render(&mut view, &mut state);
			assert!(
				text.iter()
					.any(|s| s.starts_with("All friends") && s.ends_with("16"))
			);
			view.friends.query = "ROBIN.SYNTHETIC".into();
			let text = render(&mut view, &mut state);
			assert!(text.iter().any(|s| s == "Robin"));
			assert!(!text.iter().any(|s| s == "Casey"));
			view.friends.query = "no-match".into();
			assert!(
				render(&mut view, &mut state)
					.iter()
					.any(|s| s == "No friends match your search.")
			);
			view.friends.tab = Tab::Pending;
			view.friends.query.clear();
			let text = render(&mut view, &mut state);
			assert!(text.iter().any(|s| s == "Avery"));
			assert!(!text.iter().any(|s| s == "Morgan"));
			view.friends.outgoing = true;
			let text = render(&mut view, &mut state);
			assert!(text.iter().any(|s| s == "Morgan"));
			assert!(!text.iter().any(|s| s == "Avery"));
			view.friends.query = "no-match".into();
			assert!(
				render(&mut view, &mut state)
					.iter()
					.any(|s| s == "No requests match your search.")
			);
			view.friends.tab = Tab::Add;
			assert!(
				render(&mut view, &mut state)
					.iter()
					.any(|s| s == "Send Friend Request")
			);
		}
	}
}
