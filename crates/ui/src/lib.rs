//! Native egui views; emits commands without owning transports or session credentials.
mod attachments;
pub use attachments::DownloadUi;
mod avatars;
mod categories;
pub mod design;
mod embeds;
pub mod emoji;
pub mod fonts;
mod markdown;
mod mentions;
mod profiles;
mod reactions;
mod search;
mod timeline;
mod voice;
use client_core::{Command, MAX_CONTENT, MAX_DRAFT_BYTES, State};
use egui::{RichText, TextEdit};
use model::{Delivery, Freshness, Id};

#[derive(Default)]
pub struct MessagingUi {
    search: search::SearchUi,
    timeline: timeline::TimelineView,
    avatars: avatars::Avatars,
    profile: Option<model::User>,
    profile_link: Option<String>,
    members_hidden: bool,
    members_narrow_open: bool,
    member_reload_requested: bool,
    guild: Option<Id>,
    collapsed_categories: std::collections::BTreeSet<Id>,
    navigation_channel: Option<Id>,
    pub logout_requested: bool,
    pub reconnect_requested: bool,
    pub draft_changes: Vec<Id>,
    pub draft_restore_pending: bool,
    pub clear_cache_requested: bool,
    pub voice_available: bool,
    pub voice_inputs: Vec<(String, String)>,
    pub voice_outputs: Vec<(String, String)>,
    pub voice_input: Option<String>,
    pub voice_output: Option<String>,
    pub voice_refresh_devices: bool,
    pub voice_device_status: &'static str,
    pub voice_push_to_talk: bool,
    pub voice_ptt_active: bool,
    pub voice_privacy_code: Option<String>,
    pub storage_status: &'static str,
    editing: Option<(Id, Id, String)>,
    deleting: Option<(Id, Id)>,
    ime_active: bool,
    mention_menu: mentions::Menu,
}

impl MessagingUi {
    pub fn downloads(&mut self) -> &mut DownloadUi {
        &mut self.timeline.download
    }
    pub fn clear_avatars(&mut self) {
        self.avatars = avatars::Avatars::default();
    }
    pub fn take_avatar_requests(&mut self) -> Vec<String> {
        self.avatars.take_requests()
    }
    pub fn accept_avatar(
        &mut self,
        ctx: &egui::Context,
        key: String,
        image: Option<egui::ColorImage>,
    ) {
        self.avatars.accept(ctx, key, image);
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn has_edit(&self) -> bool {
        self.editing.is_some()
    }
    fn member_rows(&mut self, ui: &mut egui::Ui, state: &State) {
        let colors = design::palette(ui);
        ui.label(
            RichText::new("PEOPLE")
                .size(10.0)
                .strong()
                .color(colors.muted),
        );
        ui.add_space(6.0);
        let Some(list) = state
            .members
            .as_ref()
            .filter(|list| Some(list.channel) == state.selected)
        else {
            ui.label(RichText::new("Choose a conversation to see its people.").color(colors.muted));
            return;
        };
        ui.label(
            RichText::new(match list.freshness {
                Freshness::Loading => "Loading people…",
                Freshness::Stale => "Awaiting member sync",
                Freshness::Unavailable => "Member list unavailable",
                Freshness::Fresh => {
                    if list.guild.is_some() {
                        "Showing up to 100 members"
                    } else {
                        "Conversation participants"
                    }
                }
            })
            .small()
            .color(colors.muted),
        );
        if matches!(list.freshness, Freshness::Stale | Freshness::Unavailable)
            && ui.small_button("Reload people").clicked()
        {
            self.member_reload_requested = true;
        }
        let members: Vec<_> = list.rows.iter().flatten().collect();
        if list.guild.is_some() && list.total > 0 {
            ui.label(
                RichText::new(format!("{} loaded · {} total", members.len(), list.total))
                    .size(10.0)
                    .color(colors.muted),
            );
        }
        ui.add_space(12.0);
        if members.is_empty() && list.freshness == Freshness::Fresh {
            ui.label(
                RichText::new("No people returned for this view.")
                    .small()
                    .color(colors.muted),
            );
        }
        egui::ScrollArea::vertical()
            .id_salt(("people", list.channel))
            .auto_shrink([false, false])
            .show_rows(ui, 40.0, members.len(), |ui, range| {
                for index in range {
                    let member = members[index];
                    ui.push_id(member.user.id.0, |ui| {
                        ui.horizontal(|ui| {
                            if self
                                .avatars
                                .show(ui, &member.user, 32.0, state.demo)
                                .clicked()
                            {
                                self.profile = Some(member.user.clone());
                            }
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                let name = member.nick.as_deref().unwrap_or(&member.user.name);
                                if ui
                                    .add(
                                        egui::Label::new(RichText::new(name).color(colors.text))
                                            .truncate()
                                            .sense(egui::Sense::click()),
                                    )
                                    .clicked()
                                {
                                    self.profile = Some(member.user.clone());
                                }
                                if let Some(status) = member.status.as_deref() {
                                    ui.label(
                                        RichText::new(match status {
                                            "online" => "Online",
                                            "idle" => "Away",
                                            "dnd" => "Do not disturb",
                                            "offline" => "Offline",
                                            _ => "Presence unavailable",
                                        })
                                        .size(10.0)
                                        .color(colors.muted),
                                    );
                                }
                            });
                        });
                    });
                }
            });
    }
    fn clear_draft(&mut self, state: &mut State, channel: Id) {
        if self.draft_restore_pending {
            // Preserve an explicit clear until the outstanding disk snapshot is merged.
            state.drafts.insert(channel, String::new());
        } else {
            state.drafts.remove(&channel);
        }
        self.draft_changes.push(channel);
    }
    fn composer(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut State,
        channel: Id,
        ctx: &egui::Context,
        commands: &mut Vec<Command>,
    ) {
        let colors = crate::design::palette(ui);
        let mut discard = None;
        for (index, pending) in state
            .pending
            .iter()
            .enumerate()
            .filter(|(_, p)| p.channel == channel)
        {
            ui.horizontal_wrapped(|ui| {
                ui.strong(match pending.delivery {
                    Delivery::Sending => "Sending…",
                    Delivery::Confirmed => "Delivered",
                    Delivery::Rejected => "Not sent",
                    Delivery::Ambiguous => "Delivery unknown",
                });
                let preview: String = pending.content.chars().take(80).collect();
                ui.label(preview);
                if pending.delivery != Delivery::Sending && ui.small_button("Restore to draft").on_hover_text("For an unknown outcome, check the official client first; sending again can duplicate it.").clicked() { discard = Some(index); }
            });
        }
        if let Some(index) = discard {
            if !state.drafts.contains_key(&channel) && state.drafts.len() >= 64 {
                state.status =
                    "Draft budget full. Clear an existing draft before restoring pending text";
            } else if state.drafts.get(&channel).is_none_or(String::is_empty) {
                let pending = state.pending.remove(index);
                state.drafts.insert(channel, pending.content);
                self.draft_changes.push(channel);
            } else {
                state.status = "Keep or clear the existing draft before restoring pending text";
            }
        }
        if let Some(reply) = state.reply {
            let author = state
                .timeline
                .get(reply)
                .map_or("an earlier message", |message| message.author.name.as_str());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Replying to {author}"))
                        .small()
                        .color(colors.accent),
                );
                if ui.small_button("Cancel reply").clicked() {
                    state.reply = None;
                }
            });
            ui.add_space(6.0);
        }
        let full = state.draft_bytes() >= MAX_DRAFT_BYTES
            || (!state.drafts.contains_key(&channel) && state.drafts.len() >= 64);
        if full {
            ui.label("Draft budget full. Clear an existing draft to continue.");
            if state.drafts.contains_key(&channel) && ui.button("Clear this draft").clicked() {
                self.clear_draft(state, channel);
            }
            return;
        }
        let ime_this_frame =
            ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(_))));
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Ime(event) = event {
                    match event {
                        egui::ImeEvent::Preedit { text, .. } => self.ime_active = !text.is_empty(),
                        egui::ImeEvent::Commit(_) => self.ime_active = false,
                        _ => {}
                    }
                }
            }
        });
        let composer_id = ui.make_persistent_id("message-input");
        let mention_enabled =
            !self.ime_active && !ime_this_frame && ctx.memory(|m| m.has_focus(composer_id));
        let mention_users = if mention_enabled {
            mentions::known_users(state, channel)
        } else {
            Vec::new()
        };
        let cursor = egui::text_edit::TextEditState::load(ctx, composer_id)
            .and_then(|s| s.cursor.char_range())
            .filter(|r| r.is_empty())
            .map(|r| r.primary.index.0);
        self.mention_menu.refresh(
            channel,
            state.drafts.get(&channel).map_or("", String::as_str),
            cursor.filter(|_| mention_enabled),
            &mention_users,
        );
        let mention_pick = if mention_enabled {
            self.mention_menu.keys(ctx)
        } else {
            None
        };
        let enter = !self.ime_active
            && !ime_this_frame
            && ctx.memory(|m| m.has_focus(composer_id))
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        egui::Frame::new()
            .fill(colors.raised)
            .stroke(egui::Stroke::new(1.0, colors.border))
            .corner_radius(12)
            .inner_margin(14)
            .show(ui, |ui| {
                let mut new_draft = String::new();
                let draft = state.drafts.get_mut(&channel).unwrap_or(&mut new_draft);
                let mut mention_changed = false;
                if let Some(pick) = mention_pick
                    && let Some(cursor) = mentions::insert(draft, pick)
                {
                    let mut edit_state =
                        egui::text_edit::TextEditState::load(ctx, composer_id).unwrap_or_default();
                    edit_state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(cursor),
                        )));
                    edit_state.store(ctx, composer_id);
                    mention_changed = true;
                }
                let mut output = TextEdit::multiline(draft)
                    .id(composer_id)
                    .char_limit(MAX_CONTENT)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .frame(egui::Frame::NONE)
                    .hint_text("Write a message… @ to mention")
                    .show(ui);
                let mention_cursor = output
                    .cursor_range
                    .filter(|r| r.is_empty())
                    .map(|r| r.primary.index.0)
                    .filter(|_| mention_enabled);
                self.mention_menu
                    .refresh(channel, draft, mention_cursor, &mention_users);
                if let Some(pick) = self.mention_menu.show(ui)
                    && let Some(cursor) = mentions::insert(draft, pick)
                {
                    output
                        .state
                        .cursor
                        .set_char_range(Some(egui::text::CCursorRange::one(
                            egui::text::CCursor::new(cursor),
                        )));
                    output.state.store(ctx, composer_id);
                    output.response.request_focus();
                    mention_changed = true;
                }
                let edit = output.response;
                let count = draft.chars().count();
                let cleared = draft.is_empty();
                if edit.changed() || mention_changed {
                    if cleared {
                        self.clear_draft(state, channel);
                    } else {
                        if !new_draft.is_empty() {
                            state.drafts.insert(channel, new_draft);
                        }
                        self.draft_changes.push(channel);
                    }
                }
                ui.add_space(8.0);
                let mut send = enter;
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_enabled(
                                state.freshness == Freshness::Fresh && count > 0,
                                egui::Button::new(
                                    RichText::new("Send").strong().color(colors.accent_text),
                                )
                                .fill(colors.accent)
                                .corner_radius(7)
                                .min_size(egui::vec2(74.0, 32.0)),
                            )
                            .clicked()
                        {
                            send = true;
                        }
                        ui.label(
                            RichText::new(format!("{count}/{MAX_CONTENT}"))
                                .size(10.0)
                                .color(colors.muted),
                        );
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new("Enter to send · Shift+Enter for a new line")
                                        .size(10.0)
                                        .color(colors.muted),
                                )
                                .truncate(),
                            );
                        });
                    });
                });
                if send && let Some(command) = state.prepare_send() {
                    commands.push(command);
                    edit.request_focus();
                }
            });
        ui.add_space(6.0);
        ui.label(
            RichText::new(if state.demo {
                "Preview only · drafts stay in memory"
            } else {
                self.storage_status
            })
            .size(10.0)
            .color(colors.muted),
        );
    }
    pub fn show(&mut self, ui: &mut egui::Ui, state: &mut State) -> Vec<Command> {
        let mut commands = Vec::new();
        let ctx = ui.ctx().clone();
        let colors = crate::design::palette(ui);
        if self.navigation_channel != state.selected {
            self.navigation_channel = state.selected;
            self.guild = state.selected.and_then(|id| {
                state
                    .channels
                    .iter()
                    .find(|channel| channel.id == id)
                    .and_then(|channel| channel.guild)
            });
        }
        egui::Panel::left("guilds")
            .resizable(false)
            .exact_size(64.0)
            .frame(egui::Frame::new().fill(colors.canvas).inner_margin(8))
            .show(ui, |ui| {
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        [48.0, 44.0],
                        egui::Button::selectable(
                            self.guild.is_none(),
                            RichText::new("S").size(22.0).strong(),
                        )
                        .corner_radius(15),
                    )
                    .on_hover_text("Direct messages")
                    .clicked()
                {
                    self.guild = None;
                }
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .id_salt("guild-list")
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 10.0;
                        for guild in &state.guilds {
                            if self
                                .avatars
                                .show_guild(ui, guild, self.guild == Some(guild.id), state.demo)
                                .clicked()
                            {
                                self.guild = Some(guild.id);
                            }
                        }
                    });
            });
        egui::Panel::left("channels")
            .resizable(true)
            .default_size(236.0)
            .size_range(190.0..=360.0)
            .frame(egui::Frame::new().fill(colors.surface).inner_margin(12))
            .show(ui, |ui| {
                ui.add_space(7.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            self.guild
                                .and_then(|id| state.guilds.iter().find(|g| g.id == id))
                                .map_or("Direct messages", |g| g.name.as_str()),
                        )
                        .size(17.0)
                        .strong(),
                    )
                    .truncate(),
                );
                ui.add_space(4.0);
                ui.label(
                    RichText::new(if self.voice_available {
                        "SEREIN  /  NATIVE CLIENT"
                    } else {
                        "SEREIN  /  TEXT CLIENT"
                    })
                    .size(10.0)
                    .color(colors.muted),
                );
                ui.add_space(14.0);
                egui::Panel::bottom("account-footer")
                    .frame(
                        egui::Frame::new()
                            .fill(colors.surface)
                            .inner_margin(egui::Margin {
                                left: 0,
                                right: 0,
                                top: 12,
                                bottom: 0,
                            }),
                    )
                    .show(ui, |ui| {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            if let Some(user) = &state.user {
                                if self.avatars.show(ui, user, 32.0, state.demo).clicked() {
                                    self.profile = Some(user.clone());
                                }
                                ui.add(
                                    egui::Label::new(RichText::new(&user.name).strong()).truncate(),
                                );
                            } else {
                                ui.strong("Your account");
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(if state.demo {
                                    "Offline preview"
                                } else {
                                    "Unofficial client"
                                })
                                .small()
                                .color(colors.muted),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.menu_button("Settings", |ui| {
                                        ui.set_min_width(240.0);
                                        ui.strong("Appearance");
                                        egui::widgets::global_theme_preference_buttons(ui);
                                        ui.add_space(8.0);
                                        ui.separator();
                                        ui.strong("Local storage");
                                        ui.label(
                                            RichText::new(if state.demo {
                                                "Preview uses session memory only."
                                            } else {
                                                self.storage_status
                                            })
                                            .small()
                                            .color(colors.muted),
                                        );
                                        if ui
                                            .add_enabled(
                                                !state.demo,
                                                egui::Button::new("Clear cache"),
                                            )
                                            .clicked()
                                        {
                                            self.clear_cache_requested = true;
                                            ui.close();
                                        }
                                        ui.separator();
                                        if ui
                                            .button(if state.demo {
                                                "Exit preview"
                                            } else {
                                                "Log out"
                                            })
                                            .clicked()
                                        {
                                            self.logout_requested = true;
                                            ui.close();
                                        }
                                        ui.label(
                                            RichText::new("Unofficial · not endorsed by Discord")
                                                .small()
                                                .color(colors.muted),
                                        );
                                    });
                                },
                            );
                        });
                    });
                ui.label(
                    RichText::new(if self.guild.is_some() {
                        "CHANNELS"
                    } else {
                        "CONVERSATIONS"
                    })
                    .size(10.0)
                    .strong()
                    .color(colors.muted),
                );
                ui.add_space(6.0);
                let select = self.channel_list(ui, state);
                if let Some(id) = select
                    && let Some(command) = state.select(id)
                {
                    commands.push(command);
                }
            });
        let wide_members = ui.available_width() >= 720.0;
        let show_members = state.selected.is_some()
            && if wide_members {
                !self.members_hidden
            } else {
                self.members_narrow_open
            };
        if show_members {
            if state
                .members
                .as_ref()
                .is_none_or(|list| Some(list.channel) != state.selected)
                && let Some(command) = state.request_members()
            {
                commands.push(command);
            }
            if wide_members {
                egui::Panel::right("people-pane")
                    .resizable(false)
                    .exact_size(216.0)
                    .frame(egui::Frame::new().fill(colors.surface).inner_margin(16))
                    .show(ui, |ui| {
                        self.member_rows(ui, state);
                    });
            } else {
                let mut open = self.members_narrow_open;
                egui::Window::new("People")
                    .open(&mut open)
                    .collapsible(false)
                    .resizable(false)
                    .default_width(248.0)
                    .default_height(350.0)
                    .show(&ctx, |ui| {
                        self.member_rows(ui, state);
                    });
                self.members_narrow_open = open;
            }
        } else if state.members.is_some() {
            commands.push(state.close_members());
        }
        if std::mem::take(&mut self.member_reload_requested)
            && let Some(command) = state.request_members()
        {
            commands.push(command);
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(colors.canvas).inner_margin(0))
            .show(ui, |ui| {
                egui::Panel::top("channel-header")
                    .frame(
                        egui::Frame::new()
                            .fill(colors.canvas)
                            .inner_margin(egui::Margin::symmetric(24, 12)),
                    )
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let channel = state
                                .selected
                                .and_then(|id| state.channels.iter().find(|c| c.id == id));
                            ui.label(
                                RichText::new(if channel.is_some_and(|c| c.guild.is_some()) {
                                    "#"
                                } else {
                                    "@"
                                })
                                .size(23.0)
                                .color(colors.muted),
                            );
                            ui.allocate_ui_with_layout(
                                egui::vec2(
                                    (ui.available_width()
                                        - if channel
                                            .is_some_and(|c| c.kind == 1 && c.guild.is_none())
                                        {
                                            206.0
                                        } else {
                                            82.0
                                        })
                                    .max(40.0),
                                    28.0,
                                ),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(
                                                channel.map_or("Your conversations", |c| {
                                                    c.name.as_str()
                                                }),
                                            )
                                            .size(18.0)
                                            .strong(),
                                        )
                                        .truncate(),
                                    );
                                },
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_enabled(
                                            state.selected.is_some(),
                                            egui::Button::selectable(show_members, "People"),
                                        )
                                        .on_hover_text("Show conversation members")
                                        .clicked()
                                    {
                                        if wide_members {
                                            self.members_hidden = !self.members_hidden;
                                        } else {
                                            self.members_narrow_open = !self.members_narrow_open;
                                        }
                                    }
                                    if let Some(channel) = state.selected.filter(|id| {
                                        state.channels.iter().any(|c| {
                                            c.id == *id && c.kind == 1 && c.guild.is_none()
                                        })
                                    }) {
                                        self.voice_settings(
                                            ui,
                                            state.demo,
                                            state.voice.active.is_some(),
                                        );
                                        self.call_button(ui, state, channel, &mut commands);
                                    }
                                },
                            );
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(if state.demo {
                                    "OFFLINE PREVIEW"
                                } else {
                                    "EXPERIMENTAL"
                                })
                                .size(10.0)
                                .strong()
                                .color(colors.accent),
                            )
                            .on_hover_text(if state.demo {
                                "Synthetic data · no network or local storage"
                            } else {
                                "Unofficial Discord client · live compatibility is unverified"
                            });
                            ui.label(RichText::new(state.status).size(11.0).color(colors.muted));
                            if !state.demo
                                && state.auth != client_core::auth::AuthState::Authenticated
                                && ui.small_button("Sign in again").clicked()
                            {
                                self.reconnect_requested = true;
                            }
                        });
                    });
                self.call_bar(ui, state, &mut commands);
                let Some(channel) = state.selected else {
                    ui.add_space((ui.available_height() * 0.28).max(24.0));
                    ui.vertical_centered(|ui| {
                        ui.heading("A little room to talk.");
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Choose a channel or direct message to get started.")
                                .color(colors.muted),
                        );
                    });
                    return;
                };
                egui::Panel::bottom("composer")
                    .frame(
                        egui::Frame::new()
                            .fill(colors.canvas)
                            .inner_margin(egui::Margin {
                                left: 24,
                                right: 24,
                                top: 12,
                                bottom: 16,
                            }),
                    )
                    .show(ui, |ui| {
                        self.composer(ui, state, channel, &ctx, &mut commands);
                    });
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(24, 4))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(
                                RichText::new(match state.freshness {
                                    Freshness::Fresh => {
                                        if state.demo {
                                            "Synthetic history"
                                        } else {
                                            "History up to date"
                                        }
                                    }
                                    Freshness::Loading => "Loading history…",
                                    Freshness::Stale => "Cached history · awaiting sync",
                                    Freshness::Unavailable => "Conversation unavailable",
                                })
                                .size(11.0)
                                .color(colors.muted),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_enabled(
                                            state.can_search(),
                                            egui::Button::new("Search").small().frame(false),
                                        )
                                        .clicked()
                                    {
                                        self.search.toggle();
                                    }
                                    if ui
                                        .add_enabled(
                                            state.freshness != Freshness::Loading,
                                            egui::Button::new("Reload").small().frame(false),
                                        )
                                        .clicked()
                                    {
                                        self.timeline.follow_latest();
                                        commands.push(state.history(None));
                                    }
                                    if ui
                                        .add_enabled(
                                            state.can_load_older(),
                                            egui::Button::new("Earlier messages")
                                                .small()
                                                .frame(false),
                                        )
                                        .clicked()
                                        && let Some(command) = state.older_history()
                                    {
                                        commands.push(command);
                                    }
                                },
                            );
                        });
                        if let Some(status) = state.read_state.status {
                            ui.label(RichText::new(status).small().color(colors.muted));
                        }
                        if state.history_before.is_some() {
                            ui.label(
                                RichText::new(
                                    "Browsing earlier messages · Jump to latest to return",
                                )
                                .size(11.0)
                                .color(colors.muted),
                            );
                        }
                    });
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(24, 8))
                    .show(ui, |ui| {
                        self.timeline.show(
                            ui,
                            state,
                            &mut self.editing,
                            &mut self.deleting,
                            &mut self.avatars,
                            &mut self.profile,
                        );
                        if std::mem::take(&mut self.timeline.latest) {
                            commands.push(state.history(None));
                        } else if std::mem::take(&mut self.timeline.load_older)
                            && let Some(command) = state.older_history()
                        {
                            commands.push(command);
                        }
                    });
            });
        self.search.show(&ctx, state, &mut commands);
        if let Some(message) = self.timeline.mark_read.take()
            && let Some(command) = state.prepare_mark_read(message)
        {
            commands.push(command);
        }
        if let Some((message, emoji)) = self.timeline.reaction.take() {
            if let Some(emoji) = emoji {
                if let Some(command) = state.prepare_reaction(message, emoji) {
                    commands.push(command);
                }
            } else {
                state.refresh_reactions(message);
            }
        }
        if let Some(user) = &self.profile {
            let profile_guild = state
                .channels
                .iter()
                .find(|channel| Some(channel.id) == state.selected)
                .and_then(|channel| channel.guild);
            if state
                .profile
                .as_ref()
                .is_none_or(|p| p.user != user.id || p.guild != profile_guild)
            {
                self.profile_link = None;
                if state.demo {
                    state.profile = Some(client_core::profile::ProfileView {
                        user: user.id,
                        guild: profile_guild,
                        request: 0,
                        loading: false,
                        error: None,
                        data: Some(profiles::synthetic(user, profile_guild)),
                    });
                } else if let Some(command) = state.request_profile(user.id, profile_guild) {
                    commands.push(command);
                }
            }
            match profiles::show(
                ui,
                user,
                state.profile.as_ref(),
                state,
                &mut self.avatars,
                &mut self.profile_link,
            ) {
                Some(profiles::Action::Profile(user)) => {
                    self.profile = Some(user);
                    self.profile_link = None;
                    commands.push(state.clear_profile());
                }
                Some(profiles::Action::Close) => {
                    self.profile = None;
                    self.profile_link = None;
                    commands.push(state.clear_profile());
                }
                Some(profiles::Action::Retry) => {
                    if let Some(command) = state.request_profile(user.id, profile_guild) {
                        commands.push(command);
                    }
                }
                Some(profiles::Action::Message(channel)) => {
                    self.profile = None;
                    self.profile_link = None;
                    commands.push(state.clear_profile());
                    if let Some(command) = state.select(channel) {
                        commands.push(command);
                    }
                }
                None => {}
            }
        }
        if let Some((channel, message, content)) = &mut self.editing {
            let mut save = false;
            let mut cancel = false;
            egui::Window::new("Edit your message")
                .collapsible(false)
                .show(&ctx, |ui| {
                    ui.label(
                        state
                            .channels
                            .iter()
                            .find(|c| c.id == *channel)
                            .map_or("Original conversation unavailable", |c| c.name.as_str()),
                    );
                    ui.add(
                        TextEdit::multiline(content)
                            .char_limit(MAX_CONTENT)
                            .desired_width(440.0),
                    );
                    ui.horizontal(|ui| {
                        save = ui.button("Save edit").clicked();
                        cancel = ui.button("Cancel").clicked();
                    });
                });
            if save {
                commands.push(Command::Edit {
                    channel: *channel,
                    message: *message,
                    content: content.clone(),
                });
            }
            if save || cancel {
                self.editing = None;
            }
        }
        if let Some((channel, message)) = self.deleting {
            egui::Window::new("Delete message from Discord?")
                .collapsible(false)
                .show(&ctx, |ui| {
                    ui.label(
                        state
                            .channels
                            .iter()
                            .find(|c| c.id == channel)
                            .map_or("Original conversation unavailable", |c| c.name.as_str()),
                    );
                    ui.label("This removes the selected message from the conversation.");
                    if ui.button("Delete message").clicked() {
                        commands.push(Command::Delete { channel, message });
                        self.deleting = None;
                    }
                    if ui.button("Keep message").clicked() {
                        self.deleting = None;
                    }
                });
        }
        self.voice_ptt_active = self.voice_push_to_talk
            && state.voice.active.is_some()
            && !state.demo
            && !ctx.egui_wants_keyboard_input()
            && ctx.input(|input| input.focused && input.key_down(egui::Key::V));
        commands
    }
}

#[cfg(test)]
mod composer_tests {
    use super::*;

    #[test]
    fn member_pane_virtualizes_and_preview_never_requests_network() {
        let mut state = State {
            demo: true,
            selected: Some(Id(1)),
            channels: vec![model::Channel {
                last_message: None,
                id: Id(1),
                guild: Some(Id(2)),
                parent_id: None,
                position: 0,
                name: "Synthetic".into(),
                kind: 0,
                recipients: Vec::new(),
                member_list_id: Some("everyone".into()),
            }],
            members: Some(model::MemberList {
                channel: Id(1),
                guild: Some(Id(2)),
                request: 1,
                total: 100,
                freshness: Freshness::Fresh,
                rows: (1..=100)
                    .map(|id| {
                        Some(model::Member {
                            user: model::User {
                                id: Id(id),
                                name: format!("Synthetic {id}"),
                                avatar: None,
                                discriminator: 0,
                            },
                            nick: None,
                            status: None,
                        })
                    })
                    .collect(),
            }),
            ..Default::default()
        };
        let mut messaging = MessagingUi::default();
        let context = egui::Context::default();
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1120.0, 760.0),
                )),
                ..Default::default()
            },
            |ui| {
                assert!(messaging.show(ui, &mut state).is_empty());
            },
        );
        assert!(messaging.take_avatar_requests().is_empty());
        assert!(
            !output.textures_delta.set.is_empty(),
            "Preview must exercise actual image uploads"
        );
        assert!(
            output.textures_delta.set.len() < 30,
            "Offscreen members must not upload textures"
        );
        output.drop_without_applying_deltas();
    }

    #[test]
    fn profile_uses_open_conversation_not_browsed_sidebar_server() {
        for guild in [None, Some(Id(10))] {
            let user = model::User {
                id: Id(2),
                name: "Synthetic".into(),
                avatar: None,
                discriminator: 0,
            };
            let mut state = State {
                demo: true,
                selected: Some(Id(1)),
                channels: vec![model::Channel {
                    last_message: None,
                    id: Id(1),
                    guild,
                    parent_id: None,
                    position: 0,
                    name: "Open conversation".into(),
                    kind: if guild.is_some() { 0 } else { 1 },
                    recipients: vec![user.clone()],
                    member_list_id: None,
                }],
                ..Default::default()
            };
            let mut messaging = MessagingUi {
                navigation_channel: state.selected,
                guild: Some(Id(99)),
                profile: Some(user),
                ..Default::default()
            };
            let output = egui::Context::default().run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1120.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    messaging.show(ui, &mut state);
                },
            );
            assert_eq!(state.profile.as_ref().unwrap().guild, guild);
            output.drop_without_applying_deltas();
        }
    }

    #[test]
    fn rendering_does_not_hide_saved_drafts_and_clears_survive_hydration() {
        let mut state = State {
            selected: Some(Id(1)),
            channels: vec![model::Channel {
                last_message: None,
                id: Id(1),
                guild: None,
                parent_id: None,
                position: 0,
                name: "Synthetic".into(),
                kind: 1,
                recipients: Vec::new(),
                member_list_id: None,
            }],
            ..Default::default()
        };
        let mut messaging = MessagingUi {
            draft_restore_pending: true,
            ..Default::default()
        };
        let mut output = egui::Context::default().run_ui(Default::default(), |ui| {
            messaging.show(ui, &mut state);
        });
        output.textures_delta.clear();
        assert!(
            state.drafts.is_empty(),
            "merely viewing must not suppress asynchronous hydration"
        );
        state
            .drafts
            .entry(Id(1))
            .or_insert("saved synthetic draft".into());
        assert_eq!(state.drafts[&Id(1)], "saved synthetic draft");
        messaging.clear_draft(&mut state, Id(1));
        state
            .drafts
            .entry(Id(1))
            .or_insert("old disk snapshot".into());
        assert!(state.drafts[&Id(1)].is_empty());

        messaging.draft_restore_pending = false;
        state.drafts.retain(|_, text| !text.is_empty());
        for channel in 1..=64 {
            state.drafts.insert(Id(channel), "synthetic".into());
        }
        messaging.clear_draft(&mut state, Id(1));
        assert_eq!(
            state.drafts.len(),
            63,
            "clearing at the slot limit must free capacity"
        );
        assert_eq!(messaging.draft_changes, [Id(1), Id(1)]);
    }
}
