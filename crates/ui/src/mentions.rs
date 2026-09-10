//! Suggestions use only the bounded people already present in this conversation.
use client_core::State;
use model::{Id, User};
use std::ops::Range;

#[derive(Default)]
pub struct Menu {
    channel: Option<Id>,
    range: Range<usize>,
    query: String,
    candidates: Vec<User>,
    selected: usize,
    dismissed: bool,
}
pub struct Pick {
    range: Range<usize>,
    user: User,
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
fn query(draft: &str, cursor: usize) -> Option<(Range<usize>, &str)> {
    let end = draft
        .char_indices()
        .nth(cursor)
        .map_or(draft.len(), |(i, _)| i);
    let prefix = &draft[..end];
    let start = prefix.rfind('@')?;
    if prefix[..start]
        .chars()
        .next_back()
        .is_some_and(|c| !c.is_whitespace() && !matches!(c, '(' | '[' | '{'))
    {
        return None;
    }
    let query = &prefix[start + 1..];
    if query.chars().count() > 64
        || query
            .chars()
            .any(|c| c.is_whitespace() || matches!(c, '<' | '>' | '@' | '`'))
    {
        return None;
    }
    Some((start..end, query))
}
pub fn insert(draft: &mut String, pick: Pick) -> Option<usize> {
    if pick.range.end > draft.len()
        || !draft.is_char_boundary(pick.range.start)
        || !draft.is_char_boundary(pick.range.end)
    {
        return None;
    }
    let token = format!("<@{}> ", pick.user.id);
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
    pub fn refresh(&mut self, channel: Id, draft: &str, cursor: Option<usize>, users: &[User]) {
        let Some((range, query)) = cursor.and_then(|cursor| query(draft, cursor)) else {
            *self = Self::default();
            return;
        };
        if self.channel != Some(channel) || self.range != range || self.query != query {
            self.dismissed = false;
            self.selected = 0;
        }
        self.channel = Some(channel);
        self.range = range;
        self.query = query.into();
        let query = query.to_lowercase();
        self.candidates = users
            .iter()
            .filter(|u| {
                query.is_empty()
                    || u.name.to_lowercase().contains(&query)
                    || u.id.to_string().starts_with(&query)
            })
            .take(8)
            .cloned()
            .collect();
        self.selected = self.selected.min(self.candidates.len().saturating_sub(1));
    }
    fn pick(&self, index: usize) -> Option<Pick> {
        self.candidates.get(index).cloned().map(|user| Pick {
            range: self.range.clone(),
            user,
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
        ui.small("Mention a person · ↑↓ choose · Tab/Enter insert · Esc dismiss");
        let mut picked = None;
        for (index, user) in self.candidates.iter().enumerate() {
            if ui
                .selectable_label(
                    self.selected == index,
                    format!("@{} · {}", user.name, user.id),
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
            guild: None,
            parent_id: None,
            position: 0,
            name: "Synthetic DM".into(),
            kind: 1,
            recipients: vec![user(42, "Zoe")],
            member_list_id: None,
        });
        state.drafts.insert(Id(1), "@Zo".into());
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
        assert_eq!(state.drafts[&Id(1)], "<@42> ");
        assert!(view.draft_changes.contains(&Id(1)));
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
        assert_eq!(query("čau @Zo", 7), Some((5..8, "Zo")));
        let mut menu = Menu::default();
        let users = vec![user(1, "Zoe"), user(2, "Zoë")];
        menu.refresh(Id(1), "čau @Zo", Some(7), &users);
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
        menu.refresh(Id(1), "@", Some(1), &users);
        assert_eq!(menu.candidates.len(), 8);
        menu.refresh(Id(1), "no query", Some(8), &users);
        assert!(menu.candidates.is_empty());
    }
}
