use client_core::State;
use model::{Channel, Id};

const QUERY_CHARS: usize = 128;
const QUERY_BYTES: usize = QUERY_CHARS * 4;
const RESULTS: usize = 20;

#[derive(Default)]
pub(super) struct Switcher {
    open: bool,
    query: String,
    selected: usize,
    focus: bool,
    previous_focus: Option<egui::Id>,
    composing: bool,
}

struct Candidate {
    id: Id,
    label: String,
}

fn bounded(value: &str) -> String {
    value.chars().take(QUERY_CHARS).collect()
}

fn candidates(state: &State, query: &str) -> Vec<Candidate> {
    let query = bounded(query).to_lowercase();
    let words: Vec<_> = query.split_whitespace().collect();
    let matches = |channel: &Channel| {
        if words.is_empty() {
            return true;
        }
        let guild = channel
            .guild
            .and_then(|id| state.guilds.iter().find(|g| g.id == id));
        // Normalize each bounded field once, without retaining account metadata.
        let mut labels = vec![bounded(&channel.name).to_lowercase()];
        if let Some(guild) = guild {
            labels.push(bounded(&guild.name).to_lowercase());
        }
        if channel.guild.is_none() {
            labels.extend(
                channel
                    .recipients
                    .iter()
                    .take(64)
                    .map(|user| bounded(&user.name).to_lowercase()),
            );
        }
        words
            .iter()
            .all(|word| labels.iter().any(|label| label.contains(word)))
    };
    let selected = state
        .channels
        .iter()
        .filter(|c| Some(c.id) == state.selected);
    selected
        .chain(
            state
                .channels
                .iter()
                .filter(|c| Some(c.id) != state.selected),
        )
        .filter(|c| (c.supports_text() || c.kind == 2) && state.can_view(c.id) && matches(c))
        .take(RESULTS)
        .map(|channel| {
            let name = if channel.name.is_empty() && channel.guild.is_none() {
                channel
                    .recipients
                    .first()
                    .map_or("Direct message", |u| u.name.as_str())
            } else {
                channel.name.as_str()
            };
            let scope = channel
                .guild
                .and_then(|id| state.guilds.iter().find(|g| g.id == id))
                .map_or(
                    if channel.guild.is_some() {
                        "Server"
                    } else if channel.kind == 3 {
                        "Group direct message"
                    } else {
                        "Direct message"
                    },
                    |g| g.name.as_str(),
                );
            let kind = if channel.kind == 2 {
                "Voice · roster"
            } else if channel.guild.is_some() {
                "#"
            } else {
                ""
            };
            Candidate {
                id: channel.id,
                label: format!("{kind} {} · {}", bounded(name), bounded(scope)),
            }
        })
        .collect()
}

impl Switcher {
    pub(super) fn is_open(&self) -> bool {
        self.open
    }

    pub(super) fn open(&mut self, ctx: &egui::Context) {
        if self.open {
            return;
        }
        self.open = true;
        self.query.clear();
        self.selected = 0;
        self.focus = true;
        self.composing = false;
        self.previous_focus = ctx.memory(|memory| memory.focused());
    }

    fn close(&mut self, ctx: &egui::Context, restore: bool) {
        self.open = false;
        self.query.clear();
        self.composing = false;
        if !restore {
            self.previous_focus = None;
        }
        ctx.request_repaint();
    }

    pub(super) fn show(&mut self, ctx: &egui::Context, state: &State) -> Option<Id> {
        if !self.open {
            let modal = ctx.memory(|memory| memory.top_modal_layer());
            if modal.is_none() {
                if let Some(id) = self.previous_focus.take() {
                    ctx.memory_mut(|memory| memory.request_focus(id));
                }
            } else if modal
                == Some(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::unique("conversation-switcher"),
                ))
            {
                // Finish the modal's closing pass before restoring or changing focus.
                ctx.request_repaint();
            }
            return None;
        }
        let ime_frame = ctx.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Ime(_)))
        });
        ctx.input(|input| {
            for event in &input.events {
                match event {
                    egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                        self.composing = !text.is_empty()
                    }
                    egui::Event::Ime(egui::ImeEvent::Commit(_)) => self.composing = false,
                    _ => {}
                }
            }
        });
        let blocked = self.composing || ime_frame;
        let query_focused = ctx.memory(|memory| memory.focused())
            == Some(egui::Id::unique("conversation-switcher-query"));
        let (up, down, enter, escape) = ctx.input_mut(|input| {
            if blocked {
                input.consume_key(egui::Modifiers::NONE, egui::Key::Enter);
                return (false, false, false, false);
            }
            (
                query_focused && input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                query_focused && input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                query_focused && input.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
            )
        });
        // egui schedules directional focus before widgets consume this frame's keys.
        // Cancel that first-frame movement too, before the field's focus filter is installed.
        if up || down {
            ctx.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
        let mut target = None;
        let mut cancel = escape;
        let modal = egui::Modal::new(egui::Id::unique("conversation-switcher"));
        let response = modal.show(ctx, |ui| {
            ui.set_width((ctx.content_rect().width() - 48.0).clamp(180.0, 480.0));
            ui.heading("Find conversation");
            ui.weak("Loaded conversations · ↑↓ choose · Enter open · Esc close");
            if blocked {
                ui.weak("Finish composing text before opening or closing.");
            }
            let input = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .id(egui::Id::unique("conversation-switcher-query"))
                    .event_filter(egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                        ..Default::default()
                    })
                    .hint_text("Channel, server or person")
                    .char_limit(QUERY_CHARS)
                    .desired_width(f32::INFINITY),
            );
            if self.focus {
                input.request_focus();
                self.focus = false;
            }
            if self.query.chars().count() > QUERY_CHARS {
                self.query = bounded(&self.query);
            }
            if self.query.capacity() > QUERY_BYTES {
                self.query = std::mem::take(&mut self.query)
                    .into_boxed_str()
                    .into_string();
            }
            if input.changed() {
                self.selected = 0;
            }
            let choices = candidates(state, &self.query);
            self.selected = self.selected.min(choices.len().saturating_sub(1));
            if !choices.is_empty() {
                if down {
                    self.selected = (self.selected + 1) % choices.len();
                }
                if up {
                    self.selected = (self.selected + choices.len() - 1) % choices.len();
                }
                if enter {
                    target = Some(choices[self.selected].id);
                }
            } else {
                ui.label("No loaded conversations match");
            }
            egui::ScrollArea::vertical()
                .max_height((ctx.content_rect().height() - 210.0).clamp(72.0, 360.0))
                .show(ui, |ui| {
                    for (index, choice) in choices.iter().enumerate() {
                        let selected = self.selected == index;
                        let row = ui
                            .push_id(choice.id, |ui| {
                                ui.add_enabled(
                                    !blocked,
                                    egui::Button::selectable(selected, &choice.label).truncate(),
                                )
                            })
                            .inner;
                        if selected && (up || down || input.changed()) {
                            row.scroll_to_me(Some(egui::Align::Center));
                        }
                        if row.has_focus() {
                            self.selected = index;
                            if row.gained_focus() {
                                row.scroll_to_me(Some(egui::Align::Center));
                            }
                        }
                        if row.clicked() {
                            target = Some(choice.id);
                        }
                    }
                });
            if ui
                .add_enabled(!blocked, egui::Button::new("Close"))
                .clicked()
            {
                cancel = true;
            }
        });
        if !blocked && response.backdrop_response.clicked() {
            cancel = true;
        }
        if cancel || target.is_some() {
            self.close(ctx, cancel);
        }
        if cancel { None } else { target }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> State {
        let mut state = test_support::demo_state();
        state.channels = (1..=30)
            .map(|id| Channel {
                id: Id(id),
                guild: None,
                parent_id: None,
                position: 0,
                name: format!("Room {id}"),
                kind: 1,
                recipients: vec![model::User {
                    id: Id(90),
                    name: "Žofie Example".into(),
                    avatar: None,
                    discriminator: 0,
                }],
                member_list_id: None,
                last_message: None,
            })
            .collect();
        state.selected = Some(Id(25));
        state
    }

    #[test]
    fn search_is_bounded_scoped_and_matches_words_across_labels() {
        let mut state = state();
        assert_eq!(candidates(&state, "").len(), RESULTS);
        assert_eq!(candidates(&state, "")[0].id, Id(25));
        assert_eq!(candidates(&state, "ROOM 17 ŽOFIE")[0].id, Id(17));
        state.channels[16].kind = 4;
        assert!(candidates(&state, "room 17").is_empty());
        state.channels[16].kind = 0;
        state.channels[16].guild = Some(Id(999));
        assert!(candidates(&state, "room 17").is_empty());
        let guild = state.guilds[0].id;
        state.guilds[0].name = "Synthetic Server".into();
        state.channels[16].guild = Some(guild);
        state.channels[16].kind = 2;
        state
            .permissions
            .replace(test_support::permission_snapshot(&state))
            .unwrap();
        let voice = candidates(&state, "SERVER ROOM 17");
        assert_eq!(voice.len(), 1);
        assert_eq!(voice[0].id, Id(17));
        assert!(voice[0].label.contains("Voice · roster"));
        state.channels[0].name = "🦀".repeat(1000);
        assert!(bounded(&state.channels[0].name).len() <= 512);
        assert!(candidates(&state, "").iter().all(|c| c.label.len() <= 1100));
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn keyboard_selection_composition_and_cancel_restore_focus() {
        for dark in [false, true] {
            let ctx = egui::Context::default();
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            let state = state();
            let mut switcher = Switcher::default();
            let prior = egui::Id::unique("previous-input");
            ctx.memory_mut(|memory| memory.request_focus(prior));
            switcher.open(&ctx);
            let mut previous_text = String::from("Unsent draft");
            let mut frame = |switcher: &mut Switcher, events| {
                let mut result = None;
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(340.0, 480.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        result = switcher.show(&ctx, &state);
                        ui.add(egui::TextEdit::singleline(&mut previous_text).id(prior));
                    },
                );
                output.drop_without_applying_deltas();
                result
            };
            assert_eq!(frame(&mut switcher, vec![]), None);
            assert_eq!(frame(&mut switcher, vec![key(egui::Key::ArrowDown)]), None);
            assert_eq!(
                ctx.memory(|m| m.focused()),
                Some(egui::Id::unique("conversation-switcher-query"))
            );
            assert_eq!(
                frame(&mut switcher, vec![key(egui::Key::Enter)]),
                Some(Id(1))
            );
            assert!(!switcher.is_open());
            ctx.memory_mut(|memory| memory.request_focus(prior));
            switcher.open(&ctx);
            frame(&mut switcher, vec![]);
            assert_eq!(
                frame(
                    &mut switcher,
                    vec![
                        egui::Event::Ime(egui::ImeEvent::Preedit {
                            text: "Ž".into(),
                            active_range_chars: None
                        }),
                        key(egui::Key::Enter)
                    ]
                ),
                None
            );
            assert!(switcher.is_open());
            assert_eq!(frame(&mut switcher, vec![key(egui::Key::Enter)]), None);
            assert!(switcher.is_open());
            let outside = egui::pos2(2.0, 2.0);
            frame(
                &mut switcher,
                vec![
                    egui::Event::PointerMoved(outside),
                    egui::Event::PointerButton {
                        pos: outside,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: outside,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            assert!(
                switcher.is_open(),
                "Backdrop must not send a later IME commit to the composer"
            );
            assert_eq!(
                frame(
                    &mut switcher,
                    vec![
                        egui::Event::Ime(egui::ImeEvent::Commit("Ž".into())),
                        key(egui::Key::Enter)
                    ]
                ),
                None
            );
            assert!(switcher.is_open());
            assert!(!candidates(&state, &switcher.query).is_empty());
            frame(&mut switcher, vec![key(egui::Key::Escape)]);
            assert!(!switcher.is_open());
            for _ in 0..3 {
                if switcher.previous_focus.is_none() {
                    break;
                }
                frame(&mut switcher, vec![]);
            }
            assert!(
                switcher.previous_focus.is_none(),
                "Closed modal must release pending focus restoration"
            );
            assert_eq!(ctx.memory(|m| m.focused()), Some(prior));
            switcher.open(&ctx);
            frame(&mut switcher, vec![]);
            frame(&mut switcher, vec![key(egui::Key::Tab)]);
            frame(&mut switcher, vec![key(egui::Key::Tab)]);
            assert_eq!(
                frame(&mut switcher, vec![key(egui::Key::Enter)]),
                Some(Id(1)),
                "Enter must activate the focused result"
            );
            switcher.open(&ctx);
            switcher.query = "Room 17".into();
            frame(&mut switcher, vec![]);
            frame(&mut switcher, vec![key(egui::Key::Tab)]);
            frame(&mut switcher, vec![key(egui::Key::Tab)]);
            assert_eq!(
                frame(&mut switcher, vec![key(egui::Key::Enter)]),
                None,
                "Focused Close must never open a result"
            );
            assert!(!switcher.is_open());
            switcher.open(&ctx);
            frame(&mut switcher, vec![]);
            frame(&mut switcher, vec![egui::Event::Paste("🦀".repeat(10_000))]);
            assert_eq!(switcher.query.chars().count(), QUERY_CHARS);
            assert!(switcher.query.len() <= QUERY_BYTES);
            assert!(switcher.query.capacity() <= QUERY_BYTES);
            frame(
                &mut switcher,
                vec![egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: "Ž".into(),
                    active_range_chars: None,
                })],
            );
            assert!(switcher.composing);
            let dismissed = egui::Event::Ime(egui::ImeEvent::Preedit {
                text: String::new(),
                active_range_chars: None,
            });
            assert_eq!(
                frame(&mut switcher, vec![dismissed, key(egui::Key::Enter)]),
                None
            );
            assert!(!switcher.composing);
            assert!(switcher.is_open());
            frame(&mut switcher, vec![key(egui::Key::Escape)]);
            assert!(!switcher.is_open());
        }
    }
}
