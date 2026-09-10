use client_core::{Command, State};
use model::Id;

#[derive(Default)]
pub struct SearchUi {
    pub open: bool,
    query: String,
    channel: Option<Id>,
    focus: bool,
    composing: bool,
}

impl SearchUi {
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.focus = self.open;
    }
    pub fn show(&mut self, ctx: &egui::Context, state: &mut State, commands: &mut Vec<Command>) {
        if self.channel != state.selected {
            self.channel = state.selected;
            self.query.clear();
            self.open = false;
            if state.search.is_some() {
                commands.push(state.clear_search());
            }
        }
        if !self.open {
            if state.search.is_some() {
                commands.push(state.clear_search());
            }
            return;
        }
        let mut open = true;
        if !state.can_search() && state.search.is_some() {
            commands.push(state.clear_search());
        }
        let mut submit = false;
        let mut older = None;
        let mut target = None;
        let allowed = state.can_search();
        let mut ime_frame = self.composing;
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Ime(event) = event {
                    ime_frame = true;
                    self.composing =
                        matches!(event, egui::ImeEvent::Preedit { text,.. } if !text.is_empty());
                }
            }
        });
        egui::Window::new("Search this conversation")
            .open(&mut open)
            .collapsible(false)
            .default_width(420.0)
            .max_height((ctx.content_rect().height() * 0.75).max(180.0))
            .show(ctx, |ui| {
                let input = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .char_limit(256)
                        .hint_text("Search messages")
                        .desired_width(f32::INFINITY),
                );
                input.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::TextEdit,
                        true,
                        "Search messages in this conversation",
                    )
                });
                if self.focus {
                    input.request_focus();
                    self.focus = false;
                }
                let valid = allowed && model::valid_search_query(&self.query);
                let enter = input.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && !ime_frame;
                ui.horizontal(|ui| {
                    submit = ui.add_enabled(valid, egui::Button::new("Search")).clicked()
                        || (valid && enter);
                    if ui.button("Close").clicked() {
                        self.open = false;
                    }
                });
                if !allowed {
                    ui.weak("Search is unavailable while disconnected or without channel access.");
                }
                if let Some(view) = &state.search {
                    ui.separator();
                    ui.label(format!("Results for: {}", view.query));
                    if view.loading {
                        ui.weak("Searching...");
                    }
                    if let Some(error) = view.error {
                        ui.label(error);
                    }
                    if let Some(page) = &view.page {
                        ui.weak(format!("{} results reported by the service", page.total));
                        if page.partial {
                            ui.label("Indexing is incomplete; results may be missing.");
                        }
                        if page.hits.is_empty() {
                            ui.label("No matching messages in this page.");
                        }
                        ui.weak(
                            "Search results are snapshots; opening a message reloads its history.",
                        );
                        ui.horizontal(|ui| {
                            if view.before.is_some()
                                && ui
                                    .add_enabled(allowed, egui::Button::new("Newest results"))
                                    .clicked()
                            {
                                older = Some((view.query.clone(), None));
                            }
                            if let Some(last) = page.hits.last()
                                && (page.total > page.hits.len() as u64 || page.partial)
                                && ui
                                    .add_enabled(allowed, egui::Button::new("Older results"))
                                    .clicked()
                            {
                                older = Some((view.query.clone(), Some(last.id)));
                            }
                        });
                        egui::ScrollArea::vertical()
                            .id_salt(("search-results", view.request))
                            .show_rows(ui, 94.0, page.hits.len(), |ui, range| {
                                for hit in &page.hits[range] {
                                    ui.push_id(hit.id, |ui| {
                                        ui.set_height(90.0);
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new(&hit.author).strong());
                                            if ui
                                                .add_enabled(
                                                    allowed && hit.id.0 < u64::MAX,
                                                    egui::Button::new("Open message"),
                                                )
                                                .clicked()
                                            {
                                                target = Some(hit.id);
                                            }
                                        });
                                        ui.add(
                                            egui::Label::new(&hit.excerpt)
                                                .truncate()
                                                .selectable(true),
                                        );
                                        ui.weak(format!("Message {}", hit.id));
                                        ui.separator();
                                    });
                                }
                            });
                    }
                }
            });
        self.open &= open;
        if submit && let Some(command) = state.request_search(self.query.trim().into(), None) {
            commands.push(command);
        }
        if let Some((query, before)) = older
            && let Some(command) = state.request_search(query, before)
        {
            commands.push(command);
        }
        if let Some(target) = target
            && let Some(command) = state.open_search_hit(target)
        {
            commands.push(command);
            self.open = false;
        }
        if !self.open {
            commands.push(state.clear_search());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keyboard_search_is_explicit_and_ime_commit_does_not_submit() {
        for ime in [false, true] {
            let mut state = State {
                auth: client_core::auth::AuthState::Authenticated,
                gateway_connected: true,
                selected: Some(Id(1)),
                ..State::default()
            };
            state.channels.push(model::Channel {
                id: Id(1),
                guild: None,
                parent_id: None,
                position: 0,
                name: "Synthetic".into(),
                kind: 1,
                recipients: vec![],
                member_list_id: None,
                last_message: None,
            });
            let ctx = egui::Context::default();
            let mut view = SearchUi {
                channel: Some(Id(1)),
                open: true,
                focus: true,
                query: "synthetic".into(),
                ..SearchUi::default()
            };
            let mut commands = Vec::new();
            for frame in 0..3 {
                let mut events = Vec::new();
                if frame == 2 {
                    if ime {
                        events.push(egui::Event::Ime(egui::ImeEvent::Commit("語".into())));
                    }
                    events.push(egui::Event::Key {
                        key: egui::Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(640.0, 480.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |_| view.show(&ctx, &mut state, &mut commands),
                );
                assert!(output.platform_output.commands.is_empty());
                output.textures_delta.clear();
                if frame < 2 {
                    assert!(commands.is_empty());
                }
            }
            assert_eq!(
                commands
                    .iter()
                    .filter(|c| matches!(c, Command::Search { .. }))
                    .count(),
                usize::from(!ime)
            );
            view.open = false;
            let mut output = ctx.run_ui(egui::RawInput::default(), |_| {
                view.show(&ctx, &mut state, &mut commands)
            });
            output.textures_delta.clear();
            assert!(state.search.is_none());
        }
    }
}
