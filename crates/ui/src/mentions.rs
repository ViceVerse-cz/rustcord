//! Suggestions use only bounded people and guild text channels already loaded in this session.
use client_core::State;
use model::{Channel, Id, User};
use std::ops::Range;

#[derive(Default)]
pub struct Menu {
	channel: Option<Id>,
	range: Range<usize>,
	query: String,
	kind: Option<Kind>,
	candidates: Vec<Candidate>,
	selected: usize,
	dismissed: bool,
}
pub struct Pick {
	range: Range<usize>,
	candidate: Candidate,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
	User,
	Channel,
}
#[derive(Clone)]
struct Candidate {
	id: Id,
	name: String,
	kind: Kind,
}
pub fn known_users(state: &State, channel: Id) -> Vec<User> {
	let mut users = Vec::new();
	let mut add = |user: &User| {
		if users.len() < 256 && !users.iter().any(|u: &User| u.id == user.id) {
			users.push(user.clone());
		}
	};
	if let Some(owner) = &state.user {
		add(owner);
	}
	if let Some(channel) = state.channels.iter().find(|c| c.id == channel) {
		for user in &channel.recipients {
			add(user);
		}
	}
	if let Some(members) = state.members.as_ref().filter(|m| m.channel == channel) {
		for member in members.rows.iter().flatten() {
			add(&member.user);
		}
	}
	for message in state.timeline.iter() {
		if message.channel == channel {
			add(&message.author);
			for user in &message.mentions {
				add(user);
			}
		}
	}
	users
}
fn query(draft: &str, cursor: usize) -> Option<(Range<usize>, &str, Kind)> {
	let end = draft
		.char_indices()
		.nth(cursor)
		.map_or(draft.len(), |(i, _)| i);
	let prefix = &draft[..end];
	let (start, _) = prefix.rmatch_indices(['@', '#']).find(|(start, _)| {
		prefix[..*start]
			.chars()
			.next_back()
			.is_none_or(|c| c.is_whitespace() || matches!(c, '(' | '[' | '{'))
	})?;
	let kind = if prefix.as_bytes()[start] == b'#' {
		Kind::Channel
	} else {
		Kind::User
	};
	let query = &prefix[start + 1..];
	if query.chars().count() > 64
		|| query.chars().any(|c| {
			c.is_whitespace()
				|| matches!(c, '<' | '>' | '@' | '`')
				|| (kind == Kind::Channel && c == '#')
		}) {
		return None;
	}
	Some((start..end, query, kind))
}
pub fn insert(draft: &mut String, pick: Pick) -> Option<usize> {
	if pick.range.end > draft.len()
		|| !draft.is_char_boundary(pick.range.start)
		|| !draft.is_char_boundary(pick.range.end)
	{
		return None;
	}
	let marker = if pick.candidate.kind == Kind::Channel {
		'#'
	} else {
		'@'
	};
	let token = format!("<{marker}{}> ", pick.candidate.id);
	if draft.chars().count() - draft[pick.range.clone()].chars().count() + token.chars().count()
		> client_core::MAX_CONTENT
	{
		return None;
	}
	let cursor = draft[..pick.range.start].chars().count() + token.chars().count();
	draft.replace_range(pick.range, &token);
	Some(cursor)
}
impl Menu {
	pub fn refresh(
		&mut self,
		channel: Id,
		draft: &str,
		cursor: Option<usize>,
		users: &[User],
		channels: &[Channel],
	) {
		let Some((range, query, kind)) = cursor.and_then(|cursor| query(draft, cursor)) else {
			*self = Self::default();
			return;
		};
		if self.channel != Some(channel)
			|| self.range != range
			|| self.query != query
			|| self.kind != Some(kind)
		{
			self.dismissed = false;
			self.selected = 0;
		}
		self.channel = Some(channel);
		self.range = range;
		self.query = query.into();
		self.kind = Some(kind);
		let query = query.to_lowercase();
		let matches = |id: Id, name: &str| {
			query.is_empty()
				|| name.to_lowercase().contains(&query)
				|| id.to_string().starts_with(&query)
		};
		self.candidates = match kind {
			Kind::User => users
				.iter()
				.filter(|user| matches(user.id, &user.name))
				.take(8)
				.map(|user| Candidate {
					id: user.id,
					name: user.name.clone(),
					kind,
				})
				.collect(),
			Kind::Channel => {
				let guild = channels
					.iter()
					.find(|c| c.id == channel)
					.and_then(|c| c.guild);
				channels
					.iter()
					.filter(|c| {
						guild.is_some()
							&& c.guild == guild && c.supports_text()
							&& !matches!(c.kind, 1 | 3)
							&& matches(c.id, &c.name)
					})
					.take(8)
					.map(|c| Candidate {
						id: c.id,
						name: c.name.chars().take(120).collect(),
						kind,
					})
					.collect()
			}
		};
		self.selected = self.selected.min(self.candidates.len().saturating_sub(1));
	}
	fn pick(&self, index: usize) -> Option<Pick> {
		self.candidates.get(index).cloned().map(|candidate| Pick {
			range: self.range.clone(),
			candidate,
		})
	}
	pub fn keys(&mut self, ctx: &egui::Context) -> Option<Pick> {
		if self.dismissed || self.candidates.is_empty() {
			return None;
		}
		ctx.input_mut(|input| {
			if input.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
				self.dismissed = true;
				return None;
			}
			if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
				self.selected = (self.selected + 1) % self.candidates.len();
			}
			if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
				self.selected = (self.selected + self.candidates.len() - 1) % self.candidates.len();
			}
			if input.consume_key(egui::Modifiers::NONE, egui::Key::Tab)
				|| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
			{
				return self.pick(self.selected);
			}
			None
		})
	}
	pub fn show(&mut self, ui: &mut egui::Ui) -> Option<Pick> {
		if self.dismissed || self.candidates.is_empty() {
			return None;
		}
		ui.small(if self.kind == Some(Kind::Channel) {
			"Link a channel · ↑↓ choose · Tab/Enter insert · Esc dismiss"
		} else {
			"Mention a person · ↑↓ choose · Tab/Enter insert · Esc dismiss"
		});
		let mut picked = None;
		for (index, candidate) in self.candidates.iter().enumerate() {
			let marker = if candidate.kind == Kind::Channel {
				'#'
			} else {
				'@'
			};
			if ui
				.selectable_label(
					self.selected == index,
					format!("{marker}{} · {}", candidate.name, candidate.id),
				)
				.clicked()
			{
				picked = Some(index);
			}
		}
		picked.and_then(|index| self.pick(index))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn composer_enter_accepts_suggestion_without_sending_message() {
		for (draft, expected, guild, kind) in [
			("@Zo", "<@42> ", None, 1),
			("#Zo", "<#42> ", Some(Id(9)), 0),
		] {
			let ctx = egui::Context::default();
			let mut state = State {
				selected: Some(Id(1)),
				freshness: model::Freshness::Fresh,
				gateway_connected: true,
				auth: client_core::auth::AuthState::Authenticated,
				..State::default()
			};
			state.channels.push(model::Channel {
				last_message: None,
				id: Id(1),
				guild,
				parent_id: None,
				position: 0,
				name: "Synthetic DM".into(),
				kind,
				recipients: vec![user(42, "Zoe")],
				member_list_id: None,
			});
			if let Some(guild) = guild {
				state.channels.push(channel(42, Some(guild), 0, "Zoe"));
			}
			state.drafts.insert(Id(1), draft.into());
			let mut view = crate::MessagingUi::default();
			let mut commands = Vec::new();
			let mut editor = egui::Id::NULL;
			let mut output = ctx.run_ui(Default::default(), |ui| {
				editor = ui.make_persistent_id("message-input");
				view.composer(ui, &mut state, Id(1), &ctx, &mut commands);
			});
			output.textures_delta.clear();
			ctx.memory_mut(|m| m.request_focus(editor));
			let mut edit_state = egui::text_edit::TextEditState::load(&ctx, editor).unwrap();
			edit_state
				.cursor
				.set_char_range(Some(egui::text::CCursorRange::one(
					egui::text::CCursor::new(3),
				)));
			edit_state.store(&ctx, editor);
			let mut output = ctx.run_ui(
				egui::RawInput {
					events: vec![egui::Event::Key {
						key: egui::Key::Enter,
						physical_key: None,
						pressed: true,
						repeat: false,
						modifiers: egui::Modifiers::NONE,
					}],
					..Default::default()
				},
				|ui| view.composer(ui, &mut state, Id(1), &ctx, &mut commands),
			);
			output.textures_delta.clear();
			assert!(commands.is_empty());
			assert_eq!(state.drafts[&Id(1)], expected);
			assert!(view.draft_changes.contains(&Id(1)));
		}
	}
	fn user(id: u64, name: &str) -> User {
		User {
			id: Id(id),
			name: name.into(),
			avatar: None,
			discriminator: 0,
		}
	}
	#[test]
	fn unicode_cursor_exact_insertion_bounded_choices_and_keyboard() {
		assert!(query("email@example", 13).is_none());
		assert!(query("<@42>", 5).is_none());
		assert_eq!(
			query("@name#1234", 10),
			Some((0..10, "name#1234", Kind::User))
		);
		assert_eq!(query("čau @Zo", 7), Some((5..8, "Zo", Kind::User)));
		let mut menu = Menu::default();
		let users = vec![user(1, "Zoe"), user(2, "Zoë")];
		menu.refresh(Id(1), "čau @Zo", Some(7), &users, &[]);
		let ctx = egui::Context::default();
		let mut chosen = None;
		let mut output = ctx.run_ui(
			egui::RawInput {
				events: vec![
					egui::Event::Key {
						key: egui::Key::ArrowDown,
						physical_key: None,
						pressed: true,
						repeat: false,
						modifiers: egui::Modifiers::NONE,
					},
					egui::Event::Key {
						key: egui::Key::Enter,
						physical_key: None,
						pressed: true,
						repeat: false,
						modifiers: egui::Modifiers::NONE,
					},
				],
				..Default::default()
			},
			|_| {
				chosen = menu.keys(&ctx);
				assert!(!ctx.input(|i| i.key_pressed(egui::Key::Enter)));
			},
		);
		output.textures_delta.clear();
		let mut draft = "čau @Zo".into();
		assert_eq!(insert(&mut draft, chosen.unwrap()), Some(9));
		assert_eq!(draft, "čau <@2> ");
		let users = (1..=1000).map(|id| user(id, "User")).collect::<Vec<_>>();
		menu.refresh(Id(1), "@", Some(1), &users, &[]);
		assert_eq!(menu.candidates.len(), 8);
		menu.refresh(Id(1), "no query", Some(8), &users, &[]);
		assert!(menu.candidates.is_empty());
	}

	fn channel(id: u64, guild: Option<Id>, kind: u8, name: &str) -> Channel {
		Channel {
			id: Id(id),
			guild,
			kind,
			name: name.into(),
			last_message: None,
			parent_id: None,
			position: 0,
			recipients: vec![],
			member_list_id: None,
		}
	}
	#[test]
	fn channel_references_scope_bound_and_insert_unicode_without_user_mentions() {
		let mut menu = Menu::default();
		let mut channels = vec![
			channel(1, Some(Id(9)), 0, "Home"),
			channel(2, Some(Id(8)), 0, "Žlutá other guild"),
			channel(3, None, 1, "Žlutá DM"),
			channel(4, Some(Id(9)), 2, "Žlutá voice"),
			channel(5, Some(Id(9)), 4, "Žlutá category"),
			channel(6, Some(Id(9)), 5, "Žlutá announcements"),
			channel(7, Some(Id(9)), 11, "Žlutá thread"),
			channel(8, Some(Id(9)), 15, "Žlutá forum container"),
		];
		assert_eq!(query("čau #Žl", 7), Some((5..9, "Žl", Kind::Channel)));
		for text in ["https://host/#name", "abc#name", "<#6>"] {
			assert!(query(text, text.chars().count()).is_none());
		}
		menu.refresh(Id(1), "čau #Žl", Some(7), &[], &channels);
		assert_eq!(
			menu.candidates.iter().map(|c| c.id).collect::<Vec<_>>(),
			[Id(6), Id(7)]
		);
		let ctx = egui::Context::default();
		let mut pick = None;
		ctx.run_ui(
			egui::RawInput {
				events: vec![egui::Event::Key {
					key: egui::Key::Enter,
					physical_key: None,
					pressed: true,
					repeat: false,
					modifiers: egui::Modifiers::NONE,
				}],
				..Default::default()
			},
			|_| {
				pick = menu.keys(&ctx);
				assert!(!ctx.input(|input| input.key_pressed(egui::Key::Enter)));
			},
		)
		.drop_without_applying_deltas();
		let mut draft = "čau #Žl".into();
		assert_eq!(insert(&mut draft, pick.unwrap()), Some(9));
		assert_eq!(draft, "čau <#6> ");
		menu.refresh(Id(3), "#", Some(1), &[], &channels);
		assert!(menu.candidates.is_empty());
		channels.extend((20..40).map(|id| channel(id, Some(Id(9)), 0, &"é".repeat(300))));
		menu.refresh(Id(1), "#é", Some(2), &[], &channels);
		assert_eq!(menu.candidates.len(), 8);
		assert!(menu.candidates.iter().all(|c| c.name.len() <= 480));
		let mut full = format!("{} #", "x".repeat(client_core::MAX_CONTENT - 2));
		menu.refresh(Id(1), &full, Some(client_core::MAX_CONTENT), &[], &channels);
		assert!(insert(&mut full, menu.pick(0).unwrap()).is_none());
	}
}
