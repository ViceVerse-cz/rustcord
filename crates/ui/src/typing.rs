//! Incoming typing uses existing conversation identities, never profile or directory requests.
use client_core::State;
use model::Id;
use std::time::Instant;

fn name(state: &State, channel: Id, user: Id) -> Option<&str> {
	state
		.channels
		.iter()
		.find(|entry| entry.id == channel)
		.and_then(|entry| entry.recipients.iter().find(|entry| entry.id == user))
		.map(|user| user.name.as_str())
		.or_else(|| {
			state
				.members
				.as_ref()
				.filter(|members| members.channel == channel)
				.and_then(|members| {
					members
						.rows
						.iter()
						.flatten()
						.find(|member| member.user.id == user)
				})
				.map(|member| member.nick.as_deref().unwrap_or(&member.user.name))
		})
		.or_else(|| {
			state
				.timeline
				.iter()
				.filter(|message| message.channel == channel)
				.find_map(|message| {
					if message.author.id == user {
						Some(message.author.name.as_str())
					} else {
						message
							.mentions
							.iter()
							.find(|entry| entry.id == user)
							.map(|user| user.name.as_str())
					}
				})
		})
}

fn label(state: &State, channel: Id, now: Instant) -> Option<String> {
	if state.selected != Some(channel) {
		return None;
	}
	let mut names = Vec::new();
	let mut count = 0;
	for user in state.typing_users(now) {
		count += 1;
		if names.len() < 3
			&& let Some(name) = name(state, channel, user)
		{
			let name: String = name
				.chars()
				.take(40)
				.map(|c| if c.is_control() { ' ' } else { c })
				.collect();
			if !name.trim().is_empty() {
				names.push(name);
			}
		}
	}
	if count == 0 {
		return None;
	}
	if names.is_empty() {
		return Some(if count == 1 {
			"Someone is typing".into()
		} else {
			format!("{count} people are typing")
		});
	}
	let others = count - names.len();
	if others > 0 {
		names.push(format!(
			"{others} other{}",
			if others == 1 { "" } else { "s" }
		));
	}
	let last = names.pop().unwrap();
	let subjects = if names.is_empty() {
		last
	} else {
		format!("{} and {last}", names.join(", "))
	};
	Some(format!(
		"{subjects} {} typing",
		if count == 1 { "is" } else { "are" }
	))
}

pub(super) fn show(ui: &mut egui::Ui, state: &State, channel: Id, now: Instant) {
	// Keep one text row even when idle so typing never resizes the conversation.
	let label = label(state, channel, now).unwrap_or_else(|| " ".into());
	let response = ui.add(
		egui::Label::new(
			egui::RichText::new(label)
				.small()
				.color(crate::design::palette(ui).muted),
		)
		.truncate(),
	);
	if ui.is_rect_visible(response.rect)
		&& let Some(deadline) = state.typing_deadline(now)
	{
		// One deadline, no dots animation and no periodic idle repaint.
		ui.ctx()
			.request_repaint_after(deadline.saturating_duration_since(now));
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use client_core::{auth::AuthState, typing::Signal};
	use model::{Channel, Freshness, User};
	use std::time::{Duration, SystemTime};

	fn state() -> State {
		State {
			auth: AuthState::Authenticated,
			gateway_connected: true,
			freshness: Freshness::Fresh,
			selected: Some(Id(10)),
			user: Some(User {
				id: Id(99),
				name: "You".into(),
				avatar: None,
				discriminator: 0,
			}),
			channels: vec![Channel {
				id: Id(10),
				guild: None,
				parent_id: None,
				kind: 1,
				position: 0,
				name: "Synthetic typing conversation".into(),
				last_message: None,
				member_list_id: None,
				message_count: None,
				recipients: vec![
					User {
						id: Id(1),
						name: "Alex".into(),
						avatar: None,
						discriminator: 0,
					},
					User {
						id: Id(2),
						name: "Robin".into(),
						avatar: None,
						discriminator: 0,
					},
					User {
						id: Id(3),
						name: "Long name\n".repeat(100),
						avatar: None,
						discriminator: 0,
					},
				],
			}],
			..Default::default()
		}
	}

	#[test]
	fn bounded_names_and_expiry_render_without_idle_repaints_or_scope_leaks() {
		let now = Instant::now();
		let wall = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
		let mut state = state();
		for user in 1..=8 {
			state.observe_typing_at(
				Signal {
					channel: Id(10),
					user: Id(user),
					timestamp: 1_000,
				},
				wall,
				now,
			);
		}
		let text = label(&state, Id(10), now).unwrap();
		assert!(text.starts_with("Alex, Robin, Long name"));
		assert!(text.ends_with("and 5 others are typing"));
		assert!(!text.contains('\n'));
		assert!(text.chars().count() < 160);
		assert!(label(&state, Id(11), now).is_none());
		for width in [160.0, 760.0] {
			let ctx = egui::Context::default();
			let mut row_height = None;
			for expired in [false, true] {
				let instant = if expired {
					now + Duration::from_secs(11)
				} else {
					now
				};
				for pass in 0..5 {
					let output = ctx.run_ui(
						egui::RawInput {
							screen_rect: Some(egui::Rect::from_min_size(
								egui::Pos2::ZERO,
								egui::vec2(width, 100.0),
							)),
							..Default::default()
						},
						|ui| {
							show(ui, &state, Id(10), instant);
							assert!(ui.min_rect().width() <= width);
							let height = ui.min_rect().height();
							assert!(height > 0.0);
							assert_eq!(*row_height.get_or_insert(height), height);
						},
					);
					assert!(output.platform_output.commands.is_empty());
					let rendered = output.shapes.iter().any(|shape| {
						matches!(&shape.shape,
                        egui::Shape::Text(text) if text.galley.job.text.contains("typing"))
					});
					assert_eq!(rendered, !expired);
					if pass == 4 {
						let delay = output.viewport_output[&egui::ViewportId::ROOT].repaint_delay;
						if expired {
							assert_eq!(delay, Duration::MAX);
						} else {
							assert!(delay > Duration::ZERO && delay <= Duration::from_secs(10));
						}
					}
					output.drop_without_applying_deltas();
				}
			}
		}
		state.gateway_connected = false;
		assert!(label(&state, Id(10), now).is_none());
		state.gateway_connected = true;
		state.selected = Some(Id(11));
		assert!(label(&state, Id(10), now).is_none());
		state.selected = Some(Id(10));
		state.freshness = Freshness::Stale;
		assert!(label(&state, Id(10), now).is_none());
	}

	#[test]
	fn unknown_typist_uses_generic_label_and_composer_keeps_draft_without_commands() {
		let now = Instant::now();
		let wall = SystemTime::now();
		let mut state = state();
		let timestamp = wall
			.duration_since(SystemTime::UNIX_EPOCH)
			.unwrap()
			.as_secs();
		state.observe_typing_at(
			Signal {
				channel: Id(10),
				user: Id(8),
				timestamp,
			},
			wall,
			now,
		);
		assert_eq!(
			label(&state, Id(10), now).as_deref(),
			Some("Someone is typing")
		);
		state.drafts.insert(Id(10), "My unsent draft".into());
		let mut messaging = crate::MessagingUi::default();
		let ctx = egui::Context::default();
		for _ in 0..3 {
			let mut commands = vec![];
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(360.0, 500.0),
					)),
					..Default::default()
				},
				|ui| messaging.composer(ui, &mut state, Id(10), &ctx, &mut commands),
			);
			assert!(commands.is_empty());
			assert!(output.platform_output.commands.is_empty());
			assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
                egui::Shape::Text(text) if text.galley.job.text == "Someone is typing")));
			assert_eq!(state.drafts[&Id(10)], "My unsent draft");
			assert!(messaging.draft_changes.is_empty());
			output.drop_without_applying_deltas();
		}
	}
}
