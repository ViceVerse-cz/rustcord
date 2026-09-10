//! Bounded suggestions from loaded people/channels and the local emoji catalog.
use client_core::State;
use model::{Channel, CustomEmoji, Id, User};
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
    Emoji,
}
#[derive(Clone)]
struct Candidate {
    token: String,
    name: String,
    kind: Kind,
}
pub fn known_emojis(state: &State, channel: Id) -> &[CustomEmoji] {
    state
        .channels
        .iter()
        .find(|c| c.id == channel)
        .and_then(|c| c.guild)
        .and_then(|id| state.guilds.iter().find(|g| g.id == id))
        .and_then(|g| g.emojis.as_deref())
        .unwrap_or_default()
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
    let (start, _) = prefix.rmatch_indices(['@', '#', ':']).find(|(start, _)| {
        prefix[..*start]
            .chars()
            .next_back()
            .is_none_or(|c| c.is_whitespace() || matches!(c, '(' | '[' | '{'))
    })?;
    let kind = match prefix.as_bytes()[start] {
        b'#' => Kind::Channel,
        b':' => Kind::Emoji,
        _ => Kind::User,
    };
    if in_code(&prefix[..start]) {
        return None;
    }
    let query = &prefix[start + 1..];
    if query.chars().count() > if kind == Kind::Emoji { 96 } else { 64 }
        || query.chars().any(|c| {
            c.is_whitespace()
                || matches!(c, '<' | '>' | '@' | '`')
                || (kind == Kind::Channel && c == '#')
                || (kind == Kind::Emoji
                    && !c.is_ascii_alphanumeric()
                    && !matches!(c, '_' | '+' | '-'))
        })
    {
        return None;
    }
    Some((start..end, query, kind))
}
// Keep incomplete code literal while typing, including unmatched backtick runs.
fn in_code(prefix: &str) -> bool {
    let mut delimiter = None;
    let mut chars = prefix.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && delimiter.is_none() {
            chars.next();
        } else if c == '`' || c == '~' {
            let mut count = 1;
            while chars.peek() == Some(&c) {
                chars.next();
                count += 1;
            }
            if delimiter == Some((c, count)) {
                delimiter = None;
            } else if delimiter.is_none() && (c == '`' || count >= 3) {
                delimiter = Some((c, count));
            }
        }
    }
    delimiter.is_some()
}

fn replace(
    draft: &mut String,
    range: Range<usize>,
    token: &str,
    remaining: usize,
) -> Option<usize> {
    if range.start > range.end
        || range.end > draft.len()
        || !draft.is_char_boundary(range.start)
        || !draft.is_char_boundary(range.end)
    {
        return None;
    }
    let selection = egui::text::CCursorRange::two(
        egui::text::CCursor::new(draft[..range.start].chars().count()),
        egui::text::CCursor::new(draft[..range.end].chars().count()),
    );
    crate::emoji_picker::insert(draft, token, Some(selection), remaining)
}

pub fn insert(draft: &mut String, pick: Pick, remaining: usize) -> Option<usize> {
    replace(
        draft,
        pick.range,
        &format!("{} ", pick.candidate.token),
        remaining,
    )
}

/// Convert the complete shortcode immediately before the scalar-index caret.
/// The caller excludes active IME composition and nonempty selections.
pub fn complete_shortcode(
    draft: &mut String,
    cursor: usize,
    emojis: &[CustomEmoji],
    remaining: usize,
) -> Option<usize> {
    let end = draft
        .char_indices()
        .nth(cursor)
        .map_or(draft.len(), |(i, _)| i);
    let prefix = draft[..end].strip_suffix(':')?;
    let (range, name, kind) = query(prefix, cursor.checked_sub(1)?)?;
    if kind != Kind::Emoji || name.is_empty() {
        return None;
    }
    let token = crate::emoji_picker::shortcodes()
        .iter()
        .find(|(_, alias)| alias.eq_ignore_ascii_case(name))
        .map(|(text, _)| (*text).to_owned())
        .or_else(|| {
            emojis
                .iter()
                .find(|emoji| emoji.usable() && emoji.name.eq_ignore_ascii_case(name))
                .map(CustomEmoji::markup)
        })?;
    replace(draft, range.start..end, &token, remaining)
}

impl Menu {
    pub fn refresh(
        &mut self,
        channel: Id,
        draft: &str,
        cursor: Option<usize>,
        (users, channels, emojis): (&[User], &[Channel], &[CustomEmoji]),
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
                    token: format!("<@{}>", user.id),
                    name: user.name.chars().take(120).collect(),
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
                            && c.guild == guild
                            && c.supports_text()
                            && !matches!(c.kind, 1 | 3)
                            && matches(c.id, &c.name)
                    })
                    .take(8)
                    .map(|c| Candidate {
                        token: format!("<#{}>", c.id),
                        name: c.name.chars().take(120).collect(),
                        kind,
                    })
                    .collect()
            }
            Kind::Emoji => {
                let mut candidates: Vec<Candidate> = Vec::new();
                if !query.is_empty() {
                    for prefix_only in [true, false] {
                        for (text, name) in crate::emoji_picker::shortcodes() {
                            if candidates.len() == 8 {
                                break;
                            }
                            if (if prefix_only {
                                name.starts_with(&query)
                            } else {
                                name.contains(&query)
                            }) && !candidates.iter().any(|c| c.token == *text)
                            {
                                candidates.push(Candidate {
                                    token: (*text).into(),
                                    name: name.clone(),
                                    kind,
                                });
                            }
                        }
                        for emoji in emojis.iter().filter(|e| e.usable()) {
                            if candidates.len() == 8 {
                                break;
                            }
                            let name = emoji.name.to_lowercase();
                            if (if prefix_only {
                                name.starts_with(&query)
                            } else {
                                name.contains(&query)
                            }) && !candidates.iter().any(|c| c.token == emoji.markup())
                            {
                                candidates.push(Candidate {
                                    token: emoji.markup(),
                                    name: emoji.name.chars().take(64).collect(),
                                    kind,
                                });
                            }
                        }
                    }
                }
                candidates
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
        ui.small(match self.kind {
            Some(Kind::Channel) => "Link a channel · ↑↓ choose · Tab/Enter insert · Esc dismiss",
            Some(Kind::Emoji) => "Insert emoji · ↑↓ choose · Tab/Enter insert · Esc dismiss",
            _ => "Mention a person · ↑↓ choose · Tab/Enter insert · Esc dismiss",
        });
        let mut picked = None;
        for (index, candidate) in self.candidates.iter().enumerate() {
            let label = match candidate.kind {
                Kind::Channel => format!("#{}", candidate.name),
                Kind::User => format!(
                    "@{} · {}",
                    candidate.name,
                    candidate
                        .token
                        .trim_start_matches("<@")
                        .trim_end_matches('>')
                ),
                Kind::Emoji => format!(":{}:", candidate.name),
            };
            let button = if candidate.kind == Kind::Emoji
                && let Some(image) = crate::emoji::image(ui.ctx(), &candidate.token, 20.0)
            {
                egui::Button::image_and_text(image, label).image_tint_follows_text_color(false)
            } else {
                egui::Button::new(label)
            };
            if ui
                .add(button.small().truncate().selected(self.selected == index))
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
            (":hea", "❤️ ", None, 1),
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
                    egui::text::CCursor::new(draft.chars().count()),
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
    #[test]
    fn composer_closing_colon_converts_with_undo_and_ime_stays_literal() {
        for ime in [false, true] {
            let ctx = egui::Context::default();
            let mut state = State {
                selected: Some(Id(1)),
                demo: true,
                ..State::default()
            };
            state.channels.push(channel(1, None, 1, "Synthetic DM"));
            state.drafts.insert(Id(1), ":heart".into());
            let mut view = crate::MessagingUi::default();
            let mut editor = egui::Id::NULL;
            let mut frame = |time, events| {
                let mut commands = Vec::new();
                ctx.run_ui(
                    egui::RawInput {
                        time: Some(time),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        editor = ui.make_persistent_id("message-input");
                        view.composer(ui, &mut state, Id(1), &ctx, &mut commands);
                    },
                )
                .drop_without_applying_deltas();
                assert!(commands.is_empty());
                if time == 0.0 {
                    ctx.memory_mut(|m| m.request_focus(editor));
                    let mut edit_state =
                        egui::text_edit::TextEditState::load(&ctx, editor).unwrap();
                    edit_state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(state.drafts[&Id(1)].chars().count()),
                        )));
                    edit_state.store(&ctx, editor);
                }
                state.drafts[&Id(1)].clone()
            };
            frame(0.0, vec![]);
            if ime {
                assert_eq!(
                    frame(
                        1.0,
                        vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                            text: ":".into(),
                            active_range_chars: None
                        })]
                    ),
                    ":heart:"
                );
                assert_eq!(
                    frame(
                        2.0,
                        vec![egui::Event::Ime(egui::ImeEvent::Commit(":".into()))]
                    ),
                    ":heart:"
                );
                assert_eq!(frame(3.0, vec![]), ":heart:");
            } else {
                assert_eq!(frame(1.0, vec![egui::Event::Text(":".into())]), "❤️");
                let undone = frame(
                    2.0,
                    vec![egui::Event::Key {
                        key: egui::Key::Z,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND,
                    }],
                );
                assert!(
                    undone.starts_with(":heart"),
                    "Undo must restore shortcode source: {undone}"
                );
            }
        }
    }

    #[test]
    fn emoji_shortcodes_preserve_literal_text_unicode_and_budgets() {
        for (source, expected) in [
            (":heart:", "❤️"),
            ("čau :+1:", "čau 👍"),
            (":woman_technologist:", "👩‍💻"),
            (":HEART:", "❤️"),
        ] {
            let mut draft = source.to_owned();
            let end = draft.chars().count();
            assert_eq!(
                complete_shortcode(&mut draft, end, &[], 100),
                Some(expected.chars().count())
            );
            assert_eq!(draft, expected);
        }
        for source in [
            r"\:heart:",
            "`:heart:",
            "``code ` :heart:",
            "```rust\n:heart:",
            "~~~\n:heart:",
            ":unknown:",
            "https://host:heart:",
        ] {
            let mut draft = source.to_owned();
            let end = draft.chars().count();
            assert_eq!(
                complete_shortcode(&mut draft, end, &[], 100),
                None,
                "{source}"
            );
            assert_eq!(draft, source);
        }
        let mut draft = "`literal` :heart: suffix".to_owned();
        assert_eq!(complete_shortcode(&mut draft, 17, &[], 0), Some(12));
        assert_eq!(draft, "`literal` ❤️ suffix");
        let mut menu = Menu::default();
        menu.refresh(Id(1), ":hea", Some(4), (&[], &[], &[]));
        assert_eq!(menu.candidates[0].token, "❤️");
        assert!(menu.candidates.len() <= 8);
        menu.refresh(
            Id(1),
            &format!(":{}", "a".repeat(97)),
            Some(98),
            (&[], &[], &[]),
        );
        assert!(menu.candidates.is_empty());
        let custom = CustomEmoji {
            id: Id(99),
            name: "party_blob".into(),
            animated: true,
            available: true,
            managed: false,
            roles: Some(vec![]),
        };
        let mut emojis = vec![custom];
        menu.refresh(Id(1), ":party_bl", Some(9), (&[], &[], &emojis));
        assert_eq!(menu.candidates[0].token, "<a:party_blob:99>");
        let mut draft = ":party_blob:".to_owned();
        assert_eq!(complete_shortcode(&mut draft, 12, &emojis, 0), None);
        assert_eq!(complete_shortcode(&mut draft, 12, &emojis, 100), Some(17));
        assert_eq!(draft, "<a:party_blob:99>");
        emojis[0].available = false;
        menu.refresh(Id(1), ":party_bl", Some(9), (&[], &[], &emojis));
        assert!(menu.candidates.is_empty());
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
        menu.refresh(Id(1), "čau @Zo", Some(7), (&users, &[], &[]));
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
        assert_eq!(insert(&mut draft, chosen.unwrap(), 100), Some(9));
        assert_eq!(draft, "čau <@2> ");
        let users = (1..=1000).map(|id| user(id, "User")).collect::<Vec<_>>();
        menu.refresh(Id(1), "@", Some(1), (&users, &[], &[]));
        assert_eq!(menu.candidates.len(), 8);
        menu.refresh(Id(1), "no query", Some(8), (&users, &[], &[]));
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
        menu.refresh(Id(1), "čau #Žl", Some(7), (&[], &channels, &[]));
        assert_eq!(
            menu.candidates
                .iter()
                .map(|c| c.token.as_str())
                .collect::<Vec<_>>(),
            ["<#6>", "<#7>"]
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
        assert_eq!(insert(&mut draft, pick.unwrap(), 100), Some(9));
        assert_eq!(draft, "čau <#6> ");
        menu.refresh(Id(3), "#", Some(1), (&[], &channels, &[]));
        assert!(menu.candidates.is_empty());
        channels.extend((20..40).map(|id| channel(id, Some(Id(9)), 0, &"é".repeat(300))));
        menu.refresh(Id(1), "#é", Some(2), (&[], &channels, &[]));
        assert_eq!(menu.candidates.len(), 8);
        assert!(menu.candidates.iter().all(|c| c.name.len() <= 480));
        let mut full = format!("{} #", "x".repeat(client_core::MAX_CONTENT - 2));
        menu.refresh(
            Id(1),
            &full,
            Some(client_core::MAX_CONTENT),
            (&[], &channels, &[]),
        );
        assert!(insert(&mut full, menu.pick(0).unwrap(), 100).is_none());
    }
}
