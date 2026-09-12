use client_core::{Command, State};
use model::User;

#[derive(Default)]
pub(super) struct ContactEditor {
	user: Option<User>,
	nickname: bool,
	draft: String,
	loaded: bool,
	generation: u64,
}

impl ContactEditor {
	pub fn open(&mut self, user: User, nickname: bool, state: &mut State) -> Option<Command> {
		self.draft = if nickname {
			state.friend_nickname(user.id).unwrap_or("").to_owned()
		} else {
			String::new()
		};
		self.nickname = nickname;
		self.loaded = nickname;
		self.generation = state.generation;
		let command = if nickname {
			None
		} else {
			state.load_user_note(user.id)
		};
		self.user = Some(user);
		command
	}
	pub fn show(&mut self, ctx: &egui::Context, state: &mut State, commands: &mut Vec<Command>) {
		if self.generation != state.generation {
			*self = Self::default();
			return;
		}
		let Some(user) = &self.user else {
			return;
		};
		let busy = state.user_action_pending();
		if !self.loaded
			&& !busy && state.user_action_status().is_none()
			&& let Some(note) = state.user_note(user.id)
		{
			self.draft = note.to_owned();
			self.loaded = true;
		}
		let mut close = false;
		let response = egui::Modal::new(egui::Id::unique("contact-editor")).show(ctx, |ui| {
			let colors = crate::design::palette(ui);
			ui.set_width((ctx.content_rect().width() - 56.0).clamp(160.0, 400.0));
			ui.heading(if self.nickname {
				"Friend Nickname"
			} else {
				"Note"
			});
			ui.label(crate::design::semibold(ui, &user.name, 16.0));
			ui.colored_label(
				colors.muted,
				if self.nickname {
					"Only you can see this nickname. It does not change their server name."
				} else {
					"Only you can see this note. Saved to your Discord account."
				},
			);
			ui.add_space(12.0);
			if !self.loaded {
				ui.label(if busy {
					"Loading note…"
				} else {
					"Could not load the note. Your existing note has not been changed."
				});
				if ui.add_enabled(!busy, egui::Button::new("Retry")).clicked()
					&& let Some(command) = state.load_user_note(user.id)
				{
					commands.push(command);
				}
			} else {
				let label = ui.label(if self.nickname { "Nickname" } else { "Note" });
				let limit = if self.nickname { 32 } else { 256 };
				ui.add_enabled_ui(!busy, |ui| {
					let edit = if self.nickname {
						egui::TextEdit::singleline(&mut self.draft).align(egui::Align2::LEFT_CENTER)
					} else {
						egui::TextEdit::multiline(&mut self.draft).desired_rows(5)
					};
					ui.add_sized(
						[
							ui.available_width(),
							if self.nickname { 44.0 } else { 124.0 },
						],
						edit.char_limit(limit).hint_text(if self.nickname {
							"Enter a nickname"
						} else {
							"Add something to remember…"
						}),
					)
					.labelled_by(label.id);
				});
				ui.colored_label(
					colors.muted,
					format!(
						"{} / {limit} · Leave empty to remove",
						self.draft.chars().count()
					),
				);
			}
			if let Some(status) = state.user_action_status() {
				ui.label(status);
			}
			let ready = if self.nickname {
				state.friends().any(|friend| friend.id == user.id)
			} else {
				state.user_note(user.id).is_some()
			};
			if self.loaded && !ready {
				ui.label(if self.nickname {
					"This user is no longer a confirmed friend."
				} else {
					"Connection refreshed. Reload the saved note before saving; your draft is kept."
				});
				if !self.nickname
					&& ui
						.add_enabled(!busy, egui::Button::new("Reload saved note"))
						.clicked() && let Some(command) = state.load_user_note(user.id)
				{
					commands.push(command);
				}
			}
			ui.add_space(16.0);
			ui.horizontal(|ui| {
				close = ui.add_enabled(!busy, egui::Button::new("Cancel")).clicked();
				let valid =
					client_core::user_actions::valid_personal_text(&self.draft, self.nickname);
				let saved = if self.nickname {
					state.friend_nickname(user.id).unwrap_or("")
				} else {
					state.user_note(user.id).unwrap_or("")
				};
				if ui
					.add_enabled(
						self.loaded && ready && valid && !busy && saved != self.draft,
						egui::Button::new(
							egui::RichText::new(if busy { "Saving…" } else { "Save" })
								.color(colors.accent_text),
						)
						.fill(colors.accent),
					)
					.clicked()
				{
					let command = if self.nickname {
						state.set_friend_nickname(user.id, self.draft.clone())
					} else {
						state.set_user_note(user.id, self.draft.clone())
					};
					if let Some(command) = command {
						commands.push(command);
					}
				}
			});
		});
		if close || (!state.user_action_pending() && response.should_close()) {
			*self = Self::default();
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use client_core::{Envelope, Event, user_actions};
	#[test]
	fn editors_load_before_edit_keep_failed_drafts_and_clear_on_account_change() {
		for nickname in [false, true] {
			for theme in [egui::ThemePreference::Light, egui::ThemePreference::Dark] {
				let ctx = egui::Context::default();
				ctx.set_theme(theme);
				crate::design::apply(&ctx);
				let mut state = test_support::demo_state();
				let user = state
					.channels
					.iter()
					.find_map(|c| c.recipients.first())
					.unwrap()
					.clone();
				state.apply(Envelope {
					generation: state.generation,
					event: Event::UserAction(user_actions::Event::Friends(Some(vec![(
						user.clone(),
						"synthetic".into(),
					)]))),
				});
				let mut editor = ContactEditor::default();
				let command = editor.open(user.clone(), nickname, &mut state);
				if let Some(Command::UserAction { request, .. }) = command {
					assert!(!editor.loaded);
					state.apply(Envelope {
						generation: state.generation,
						event: Event::UserAction(user_actions::Event::NoteLoaded {
							user: user.id,
							request,
							result: Ok("Original note".into()),
						}),
					});
				}
				let render = |editor: &mut ContactEditor, state: &mut State| {
					let mut commands = vec![];
					ctx.run_ui(
						egui::RawInput {
							screen_rect: Some(egui::Rect::from_min_size(
								egui::Pos2::ZERO,
								egui::vec2(320.0, 600.0),
							)),
							..Default::default()
						},
						|_| editor.show(&ctx, state, &mut commands),
					)
					.drop_without_applying_deltas();
					assert!(commands.is_empty(), "Rendering never saves automatically");
				};
				render(&mut editor, &mut state);
				assert!(editor.loaded);
				editor.draft = "Private draft 🌙".into();
				let command = if nickname {
					state.set_friend_nickname(user.id, editor.draft.clone())
				} else {
					state.set_user_note(user.id, editor.draft.clone())
				}
				.unwrap();
				state.command_rejected(command);
				render(&mut editor, &mut state);
				assert_eq!(editor.draft, "Private draft 🌙");
				assert!(state.user_action_status().is_some());
				state.generation += 1;
				render(&mut editor, &mut state);
				assert!(editor.user.is_none() && editor.draft.is_empty());
			}
		}
	}
}
