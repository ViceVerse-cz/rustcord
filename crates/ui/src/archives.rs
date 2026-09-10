use client_core::{Command, State};
use model::archives::Kind;

#[derive(Default)]
pub struct ArchivesUi {
    pub focus: bool,
}

impl ArchivesUi {
    pub fn show(&mut self, ctx: &egui::Context, state: &mut State, commands: &mut Vec<Command>) {
        let Some(view) = &state.archives else {
            return;
        };
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            commands.push(state.clear_archives());
            return;
        }
        let parent = view.parent;
        let allowed = state.can_archive(parent, view.kind);
        let parent_channel = state.channels.iter().find(|c| c.id == parent);
        let private = parent_channel.is_some_and(|c| c.kind == 0);
        let mut open = true;
        let mut close = false;
        let mut request = None;
        let mut target = None;
        egui::Window::new("Archived threads")
            .open(&mut open).collapsible(false).default_width(420.0)
            .max_height((ctx.content_rect().height() * 0.75).max(180.0))
            .show(ctx, |ui| {
                ui.add(egui::Label::new(parent_channel.map_or("Unavailable channel", |c| c.name.as_str())).truncate());
                ui.weak("One page of up to 25 archived threads. Opening loads messages; it does not join or reopen a thread.");
                ui.horizontal_wrapped(|ui| {
                    for (kind, name) in [(Kind::Public, "Public"), (Kind::JoinedPrivate, "Joined private"), (Kind::Private, "Private")] {
                        if (kind == Kind::Public || private)
                            && ui.add_enabled(allowed && !view.loading, egui::Button::selectable(view.kind == kind, name)).clicked()
                            && kind != view.kind
                        { request = Some((kind, None)); }
                    }
                });
                if view.kind == Kind::Private { ui.weak("Private archives require permission from the service."); }
                ui.horizontal_wrapped(|ui| {
                    let reload = ui.add_enabled(allowed, egui::Button::new("Reload"));
                    if self.focus { reload.request_focus(); self.focus = false; }
                    if reload.clicked() { request = Some((view.kind, None)); }
                    if view.error.is_some() {
                        if ui.add_enabled(allowed && !view.loading, egui::Button::new("Retry")).clicked() {
                            request = Some((view.kind, view.before));
                        }
                    } else if let Some(before) = view.page.as_ref().and_then(|page| page.next)
                        && ui.add_enabled(allowed && !view.loading, egui::Button::new("Older")).clicked()
                    { request = Some((view.kind, Some(before))); }
                    if ui.button("Close").clicked() { close = true; }
                });
                if !allowed { ui.weak("Archives are unavailable while disconnected or without channel access."); }
                if view.loading { ui.weak("Loading archived threads..."); }
                if let Some(error) = view.error { ui.label(error); }
                if let Some(page) = &view.page {
                    ui.weak(format!("{} threads · {} page", page.threads.len(), if view.before.is_some() { "older" } else { "newest" }));
                    if page.threads.is_empty() { ui.label("No archived threads returned."); }
                    if page.next.is_none() && !view.loading { ui.weak("No older threads reported by the service."); }
                    egui::ScrollArea::vertical().id_salt(("archive-page", view.request))
                        .show_rows(ui, 42.0, page.threads.len(), |ui, range| {
                            for thread in &page.threads[range] {
                                ui.push_id(thread.id, |ui| {
                                    ui.set_height(42.0);
                                    ui.horizontal(|ui| {
                                        if ui.add_enabled(allowed && !view.loading, egui::Button::new("Open thread")).clicked() {
                                            target = Some(thread.id);
                                        }
                                        ui.add(egui::Label::new(&thread.name).truncate());
                                    });
                                });
                            }
                        });
                }
            });
        if !open || close {
            commands.push(state.clear_archives());
        } else if let Some((kind, before)) = request {
            if let Some(command) = state.request_archives(parent, kind, before) {
                commands.push(command);
            }
        } else if let Some(target) = target
            && let Some(command) = state.open_archived_thread(target)
        {
            commands.push(command);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::{
        Channel, Id,
        archives::{Cursor, Page},
    };

    fn channel(id: u64, kind: u8, parent_id: Option<Id>) -> Channel {
        Channel {
            id: Id(id),
            guild: Some(Id(100)),
            parent_id,
            kind,
            position: 0,
            name: format!("Synthetic thread {id}"),
            recipients: vec![],
            last_message: None,
            member_list_id: None,
        }
    }
    fn frame(ctx: &egui::Context, key: Option<egui::Key>, draw: impl FnMut(&mut egui::Ui)) {
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(420.0, 480.0),
                )),
                events: key
                    .into_iter()
                    .map(|key| egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    })
                    .collect(),
                ..Default::default()
            },
            draw,
        );
        assert!(output.platform_output.commands.is_empty());
        output.drop_without_applying_deltas();
    }
    #[test]
    fn forum_archive_is_keyboard_accessible_paginates_and_opens_only_history() {
        for dark in [false, true] {
            let ctx = egui::Context::default();
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            let mut state = State {
                auth: client_core::auth::AuthState::Authenticated,
                gateway_connected: true,
                guilds: vec![model::Guild {
                    id: Id(100),
                    name: "Synthetic".into(),
                    icon: None,
                }],
                channels: vec![channel(7, 15, None)],
                ..State::default()
            };
            let mut ui = crate::MessagingUi {
                guild: Some(Id(100)),
                ..Default::default()
            };
            frame(&ctx, None, |root| {
                assert!(ui.channel_list(root, &state).is_none());
            });
            assert!(ui.archive_parent.is_none());
            for key in [egui::Key::Tab, egui::Key::Enter] {
                frame(&ctx, Some(key), |root| {
                    assert!(ui.channel_list(root, &state).is_none());
                });
            }
            assert_eq!(ui.archive_parent.take(), Some(Id(7)));
            assert!(state.selected.is_none());
            let mut commands = vec![state.request_archives(Id(7), Kind::Public, None).unwrap()];
            ui.archives.focus = true;
            for _ in 0..2 {
                frame(&ctx, None, |_| {
                    ui.archives.show(&ctx, &mut state, &mut commands)
                });
            }
            assert_eq!(commands.len(), 1); // Opening/loading never submits another request.
            let request = state.archives.as_ref().unwrap().request;
            let before = Cursor::Time(1_700_000_000_000_000_000);
            state.apply_archives(
                Id(7),
                request,
                Ok(Page {
                    threads: vec![channel(8, 11, Some(Id(7)))],
                    next: Some(before),
                }),
            );
            for key in [None, Some(egui::Key::Tab), Some(egui::Key::Enter)] {
                frame(&ctx, key, |_| {
                    ui.archives.show(&ctx, &mut state, &mut commands)
                });
            }
            assert!(
                matches!(commands.last(), Some(Command::Archives { before: Some(cursor), .. }) if *cursor == before)
            );
            assert!(state.archives.as_ref().unwrap().page.is_none());
            let request = state.archives.as_ref().unwrap().request;
            state.apply_archives(
                Id(7),
                request,
                Ok(Page {
                    threads: vec![channel(9, 11, Some(Id(7)))],
                    next: None,
                }),
            );
            ui.archives.focus = true;
            // Reload, Close, Open thread: exhausted pages have no Older control.
            for key in [
                None,
                None,
                Some(egui::Key::Tab),
                Some(egui::Key::Tab),
                Some(egui::Key::Enter),
            ] {
                frame(&ctx, key, |_| {
                    ui.archives.show(&ctx, &mut state, &mut commands)
                });
            }
            assert!(matches!(
                commands.last(),
                Some(Command::History { channel: Id(9), .. })
            ));
            assert_eq!(state.selected, Some(Id(9)));
            assert!(state.archives.is_none());
            state.request_archives(Id(7), Kind::Public, None).unwrap();
            let request = state.archives.as_ref().unwrap().request;
            state.apply_archives(
                Id(7),
                request,
                Ok(Page {
                    threads: vec![],
                    next: None,
                }),
            );
            let count = commands.len();
            frame(&ctx, None, |_| {
                ui.archives.show(&ctx, &mut state, &mut commands)
            });
            assert_eq!(commands.len(), count);
            frame(&ctx, Some(egui::Key::Escape), |_| {
                ui.archives.show(&ctx, &mut state, &mut commands)
            });
            assert!(state.archives.is_none());
            assert!(matches!(commands.last(), Some(Command::CancelSearch)));
        }
    }
}
