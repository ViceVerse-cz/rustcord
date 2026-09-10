//! Native egui views; emits commands without owning transports or session credentials.
mod archives;
mod attachments;
pub use attachments::DownloadUi;
mod avatars;
mod categories;
mod composer_text;
pub mod design;
mod embeds;
pub mod emoji;
mod emoji_picker;
pub mod fonts;
pub mod icons;
mod markdown;
mod mentions;
mod notifications;
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
    archives: archives::ArchivesUi,
    archive_parent: Option<Id>,
    timeline: timeline::TimelineView,
    avatars: avatars::Avatars,
    profile: Option<model::User>,
    profile_link: Option<String>,
    /// Where the open profile was requested from; the popout is placed beside it.
    profile_anchor: Option<(Id, egui::Pos2)>,
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
    pub attachment: Option<(String, u64)>,
    pub attach_requested: bool,
    pub remove_attachment_requested: bool,
    pub cancel_upload_requested: bool,
    pub upload_busy: bool,
    pub upload_status: Option<String>,
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
    pub notifications_enabled: bool,
    pub notification_test_available: bool,
    pub notification_test_requested: bool,
    pub notification_status: &'static str,
    pub storage_status: &'static str,
    editing: Option<(Id, Id, String)>,
    composer_edit: Option<(Id, Id)>,
    edit_sent: bool,
    deleting: Option<(Id, Id)>,
    ime_active: bool,
    mention_menu: mentions::Menu,
    emoji_picker: emoji_picker::Picker,
    /// Set when the user picks a theme preset; the host persists it.
    pub theme_variant_changed: Option<design::Variant>,
}

impl MessagingUi {
    /// Fixture-only entry point: opens People and the profile card for `user` as if clicked.
    pub fn preview_profile(&mut self, user: model::User) {
        self.members_narrow_open = true;
        self.profile = Some(user);
    }
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
    /// Window title strip: traffic-light inset, centred context title and session state.
    fn title_bar(&mut self, ui: &mut egui::Ui, state: &State, title: &str) {
        let colors = design::palette(ui);
        egui::Panel::top("title-bar")
            .exact_size(36.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(colors.base))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let drag = ui.interact(rect, ui.id().with("drag"), egui::Sense::click_and_drag());
                if drag.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    title,
                    egui::FontId::new(13.0, crate::design::semibold_family(ui.ctx())),
                    colors.text,
                );

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(rect.shrink2(egui::vec2(12.0, 0.0)))
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        let (label, hint) = if state.demo {
                            (
                                "OFFLINE PREVIEW",
                                "Synthetic data · no network or local storage",
                            )
                        } else {
                            (
                                "EXPERIMENTAL",
                                "Unofficial Discord client · live compatibility is unverified",
                            )
                        };
                        egui::Frame::new()
                            .fill(colors.accent.gamma_multiply(0.22))
                            .corner_radius(4)
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .show(ui, |ui| {
                                ui.label(
                                    design::semibold(ui, label, 10.0).color(colors.text_strong),
                                );
                            })
                            .response
                            .on_hover_text(hint);
                        if !state.demo
                            && state.auth != client_core::auth::AuthState::Authenticated
                            && ui.small_button("Sign in again").clicked()
                        {
                            self.reconnect_requested = true;
                        }
                        ui.add(
                            egui::Label::new(
                                RichText::new(state.status).size(11.0).color(colors.muted),
                            )
                            .truncate(),
                        );
                    },
                );
            });
    }
    fn member_rows(&mut self, ui: &mut egui::Ui, state: &State) {
        let colors = design::palette(ui);
        let Some(list) = state
            .members
            .as_ref()
            .filter(|list| Some(list.channel) == state.selected)
        else {
            ui.add_space(8.0);
            ui.label(RichText::new("Choose a conversation to see its people.").color(colors.muted));
            return;
        };
        if matches!(list.freshness, Freshness::Stale | Freshness::Unavailable) {
            ui.add_space(8.0);
            ui.label(
                RichText::new(match list.freshness {
                    Freshness::Stale => "Awaiting member sync",
                    _ => "Member list unavailable",
                })
                .small()
                .color(colors.muted),
            );
            if ui.small_button("Reload people").clicked() {
                self.member_reload_requested = true;
            }
        } else if list.freshness == Freshness::Loading {
            ui.add_space(8.0);
            ui.label(RichText::new("Loading people…").small().color(colors.muted));
        }
        let members: Vec<_> = list.rows.iter().flatten().collect();
        let online = |member: &&model::Member| {
            member
                .status
                .as_deref()
                .is_some_and(|s| !matches!(s, "offline" | "invisible"))
        };
        let (online_members, offline_members): (Vec<_>, Vec<_>) =
            members.iter().copied().partition(online);
        enum Row<'a> {
            Header(String),
            Member(&'a model::Member, bool),
        }
        let mut rows = Vec::with_capacity(members.len() + 2);
        if list.guild.is_none() {
            rows.push(Row::Header(format!("Members — {}", members.len())));
            rows.extend(members.iter().map(|m| Row::Member(m, online(m))));
        } else {
            if !online_members.is_empty() || list.freshness == Freshness::Fresh {
                rows.push(Row::Header(format!("Online — {}", online_members.len())));
                rows.extend(online_members.iter().map(|m| Row::Member(m, true)));
            }
            if !offline_members.is_empty() {
                rows.push(Row::Header(format!("Offline — {}", offline_members.len())));
                rows.extend(offline_members.iter().map(|m| Row::Member(m, false)));
            }
        }
        if members.is_empty() && list.freshness == Freshness::Fresh {
            ui.add_space(8.0);
            ui.label(
                RichText::new("No people returned for this view.")
                    .small()
                    .color(colors.muted),
            );
        }
        let hint = if list.guild.is_some() && list.total > members.len() as u64 {
            Some(format!(
                "Showing {} of {} members",
                members.len(),
                list.total
            ))
        } else {
            None
        };
        egui::ScrollArea::vertical()
            .id_salt(("people", list.channel))
            .auto_shrink([false, false])
            .show_rows(ui, 42.0, rows.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for index in range {
                    match &rows[index] {
                        Row::Header(text) => {
                            let (rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 42.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().text(
                                egui::pos2(rect.left() + 8.0, rect.bottom() - 6.0),
                                egui::Align2::LEFT_BOTTOM,
                                text.to_uppercase(),
                                egui::FontId::new(12.0, crate::design::semibold_family(ui.ctx())),
                                colors.muted,
                            );
                        }
                        Row::Member(member, online) => {
                            let name = member.nick.as_deref().unwrap_or(&member.user.name);
                            let subtitle = member.custom_status.as_deref();
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 42.0),
                                egui::Sense::click(),
                            );
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    format!(
                                        "{name}, {}",
                                        member
                                            .status
                                            .as_deref()
                                            .map_or("presence unknown", profiles::presence_label)
                                    ),
                                )
                            });
                            let row = rect.shrink2(egui::vec2(0.0, 1.0));
                            if response.hovered() || response.has_focus() {
                                ui.painter().rect_filled(row, 6, colors.hover);
                            }
                            let mut inner = ui.new_child(
                                egui::UiBuilder::new()
                                    .max_rect(row.shrink2(egui::vec2(8.0, 4.0)))
                                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
                            );
                            inner.spacing_mut().item_spacing.x = 12.0;
                            inner.push_id(member.user.id.0, |ui| {
                                let avatar = self.avatars.show(ui, &member.user, 32.0, state.demo);
                                if let Some(status) = member.status.as_deref() {
                                    design::presence_dot(
                                        ui,
                                        avatar.rect,
                                        profiles::presence_color(status),
                                        colors.sidebar,
                                    );
                                }
                                let text_color = if *online { colors.text } else { colors.muted };
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 1.0;
                                    ui.add(
                                        egui::Label::new(
                                            design::medium(ui, name, 15.0).color(text_color),
                                        )
                                        .truncate()
                                        .selectable(false),
                                    );
                                    if let Some(custom) = subtitle {
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(custom)
                                                    .size(12.0)
                                                    .color(colors.muted),
                                            )
                                            .truncate()
                                            .selectable(false),
                                        );
                                    }
                                });
                            });
                            if response.clicked() {
                                self.profile = Some(member.user.clone());
                            }
                        }
                    }
                }
            });
        if let Some(hint) = hint {
            ui.label(RichText::new(hint).size(11.0).color(colors.muted));
        }
    }
    /// Channel sidebar: context header, scrolling list and the account card.
    fn sidebar(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut State,
        title: &str,
        commands: &mut Vec<Command>,
    ) {
        let colors = design::palette(ui);
        egui::Panel::top("sidebar-header")
            .exact_size(48.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 0)))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                ui.horizontal_centered(|ui| {
                    ui.add(
                        egui::Label::new(
                            design::semibold(ui, title, 15.0).color(colors.text_strong),
                        )
                        .truncate(),
                    );
                });
                ui.painter().hline(
                    rect.x_range().expand(16.0),
                    rect.bottom(),
                    egui::Stroke::new(1.0, colors.border),
                );
            });
        egui::Panel::bottom("account-footer")
            .show_separator_line(false)
            .frame(egui::Frame::new().inner_margin(8))
            .show(ui, |ui| self.account_card(ui, state));
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 8,
                bottom: 0,
            })
            .show(ui, |ui| {
                if self.guild.is_none() {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(design::eyebrow(ui, "Direct Messages", colors.muted));
                    });
                    ui.add_space(4.0);
                }
                let select = self.channel_list(ui, state);
                if let Some(id) = select
                    && let Some(command) = state.select(id)
                {
                    commands.push(command);
                }
                if let Some(parent) = self.archive_parent.take()
                    && let Some(command) =
                        state.request_archives(parent, model::archives::Kind::Public, None)
                {
                    self.search.open = false;
                    self.archives.focus = true;
                    commands.push(command);
                }
            });
    }
    fn account_card(&mut self, ui: &mut egui::Ui, state: &State) {
        let colors = design::palette(ui);
        egui::Frame::new()
            .fill(colors.raised)
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    if let Some(user) = &state.user {
                        let avatar = self.avatars.show(ui, user, 32.0, state.demo);
                        design::presence_dot(ui, avatar.rect, colors.positive, colors.raised);
                        if avatar.clicked() {
                            self.profile = Some(user.clone());
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        let settings = icons::button(ui, icons::Icon::Gear, 32.0, "User settings");
                        egui::Popup::menu(&settings).show(|ui| self.settings_menu(ui, state));
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 0.0;
                                ui.add(
                                    egui::Label::new(
                                        design::semibold(
                                            ui,
                                            state
                                                .user
                                                .as_ref()
                                                .map_or("Your account", |u| u.name.as_str()),
                                            14.0,
                                        )
                                        .color(colors.text_strong),
                                    )
                                    .truncate()
                                    .selectable(false),
                                );
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(if state.demo {
                                            "Offline preview"
                                        } else if state.gateway_connected {
                                            "Online"
                                        } else {
                                            "Reconnecting…"
                                        })
                                        .size(12.0)
                                        .color(colors.muted),
                                    )
                                    .truncate()
                                    .selectable(false),
                                );
                            });
                        });
                    });
                });
            });
    }
    fn settings_menu(&mut self, ui: &mut egui::Ui, state: &State) {
        let colors = design::palette(ui);
        ui.set_min_width(260.0);
        ui.label(design::eyebrow(ui, "Appearance", colors.muted));
        egui::widgets::global_theme_preference_buttons(ui);
        ui.add_space(6.0);
        ui.label(design::eyebrow(ui, "Theme", colors.muted));
        let current = design::variant();
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for variant in design::Variant::ALL {
                let swatch = design::colors(ui.visuals().dark_mode, variant);
                let (rect, response) =
                    ui.allocate_exact_size(egui::Vec2::splat(28.0), egui::Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::RadioButton,
                        true,
                        variant == current,
                        variant.label(),
                    )
                });
                let painter = ui.painter();
                match swatch.backdrop {
                    Some([top, bottom]) => {
                        painter.circle_filled(rect.center(), 12.0, bottom);
                        painter.circle_filled(rect.center() - egui::vec2(3.0, 3.0), 7.0, top);
                    }
                    None => {
                        painter.circle_filled(rect.center(), 12.0, swatch.chat);
                        painter.circle_filled(
                            rect.center() + egui::vec2(3.0, 3.0),
                            6.0,
                            swatch.base,
                        );
                    }
                }
                painter.circle_stroke(
                    rect.center(),
                    12.0,
                    egui::Stroke::new(
                        if variant == current { 2.0 } else { 1.0 },
                        if variant == current {
                            colors.accent
                        } else {
                            colors.border
                        },
                    ),
                );
                if response.on_hover_text(variant.label()).clicked() && variant != current {
                    design::set_variant(variant);
                    design::apply(ui.ctx());
                    self.theme_variant_changed = Some(variant);
                }
            }
        });
        ui.label(
            RichText::new(format!("{} · saved with your appearance", current.label()))
                .small()
                .color(colors.muted),
        );
        ui.separator();
        ui.label(design::eyebrow(ui, "Notifications", colors.muted));
        ui.add_enabled(
            !state.demo || self.notification_test_available,
            egui::Checkbox::new(
                &mut self.notifications_enabled,
                "System notifications for this session",
            ),
        );
        ui.label(
            RichText::new(
                "Message previews are hidden. OS notification history may remain after logout.",
            )
            .small()
            .color(colors.muted),
        );
        ui.label(
            RichText::new(if state.demo && !self.notification_test_available {
                "Offline preview never sends system notifications."
            } else {
                self.notification_status
            })
            .small()
            .color(colors.muted),
        );
        if self.notification_test_available
            && self.notifications_enabled
            && ui.button("Send generic test notification").clicked()
        {
            self.notification_test_requested = true;
        }
        if !state.demo && !state.notification_preferences_known() {
            ui.label(
                RichText::new("Alerts wait for your Discord notification preferences.")
                    .small()
                    .color(colors.muted),
            );
        }
        ui.separator();
        ui.label(design::eyebrow(ui, "Local storage", colors.muted));
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
            .add_enabled(!state.demo, egui::Button::new("Clear cache"))
            .clicked()
        {
            self.clear_cache_requested = true;
            ui.close();
        }
        ui.separator();
        if ui
            .button(
                RichText::new(if state.demo {
                    "Exit preview"
                } else {
                    "Log out"
                })
                .color(colors.danger),
            )
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
    }
    /// Conversation header: channel identity on the left, tools and search on the right.
    fn channel_header(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut State,
        selected_voice: bool,
        show_members: bool,
        wide_members: bool,
        commands: &mut Vec<Command>,
    ) {
        let colors = design::palette(ui);
        egui::Panel::top("channel-header")
            .exact_size(48.0)
            .show_separator_line(false)
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 0)))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                ui.painter().hline(
                    rect.x_range().expand(16.0),
                    rect.bottom(),
                    egui::Stroke::new(1.0, colors.border),
                );
                let channel = state
                    .selected
                    .and_then(|id| state.channels.iter().find(|c| c.id == id))
                    .cloned();
                let dm = channel
                    .as_ref()
                    .is_some_and(|c| c.kind == 1 && c.guild.is_none());
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    match channel.as_ref() {
                        Some(c) if c.guild.is_none() => {
                            if let Some(user) = c.recipients.first() {
                                let avatar = self.avatars.show(ui, user, 24.0, state.demo);
                                if avatar.clicked() {
                                    self.profile = Some(user.clone());
                                }
                            } else {
                                icons::inline(ui, icons::Icon::People, 22.0, colors.muted);
                            }
                        }
                        Some(c) => {
                            let icon = match c.kind {
                                2 | 13 => icons::Icon::Speaker,
                                15 | 16 => icons::Icon::Forum,
                                10..=12 => icons::Icon::Threads,
                                _ => icons::Icon::Hash,
                            };
                            icons::inline(ui, icon, 22.0, colors.muted);
                        }
                        None => {}
                    }
                    let tools = if dm { 210.0 } else { 300.0 };
                    ui.allocate_ui_with_layout(
                        egui::vec2((ui.available_width() - tools).max(40.0), 28.0),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.add(
                                egui::Label::new(
                                    design::semibold(
                                        ui,
                                        channel
                                            .as_ref()
                                            .map_or("Direct Messages", |c| c.name.as_str()),
                                        16.0,
                                    )
                                    .color(colors.text_strong),
                                )
                                .truncate(),
                            );
                        },
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        if state.selected.is_some() && !selected_voice {
                            // Search pill.
                            let (pill, response) = ui
                                .allocate_exact_size(egui::vec2(144.0, 28.0), egui::Sense::click());
                            let enabled = state.can_search();
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    enabled,
                                    "Search",
                                )
                            });
                            ui.painter().rect_filled(pill, 6, colors.raised);
                            let pill_text = if enabled {
                                colors.muted
                            } else {
                                colors.muted.gamma_multiply(0.5)
                            };
                            ui.painter().text(
                                pill.left_center() + egui::vec2(10.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                "Search",
                                egui::FontId::proportional(13.0),
                                pill_text,
                            );
                            icons::paint(
                                ui.painter(),
                                icons::Icon::Search,
                                egui::Rect::from_center_size(
                                    pill.right_center() - egui::vec2(14.0, 0.0),
                                    egui::Vec2::splat(16.0),
                                ),
                                pill_text,
                            );
                            if enabled
                                && response.on_hover_text("Search this conversation").clicked()
                            {
                                if state.archives.is_some() {
                                    commands.push(state.clear_archives());
                                }
                                self.search.toggle(false);
                            }
                            ui.add_space(4.0);
                            if icons::toggle(
                                ui,
                                icons::Icon::People,
                                32.0,
                                show_members,
                                "Show member list",
                            )
                            .clicked()
                            {
                                if wide_members {
                                    self.members_hidden = !self.members_hidden;
                                } else {
                                    self.members_narrow_open = !self.members_narrow_open;
                                }
                            }
                            let pins = ui
                                .add_enabled_ui(state.can_search(), |ui| {
                                    icons::button(ui, icons::Icon::Pin, 32.0, "Pinned messages")
                                })
                                .inner;
                            if pins.clicked()
                                && self.search.toggle(true)
                                && let Some(command) = state.request_pins()
                            {
                                commands.push(command);
                            }
                            if let Some(c) = channel
                                .as_ref()
                                .filter(|c| c.guild.is_some() && matches!(c.kind, 0 | 5 | 15 | 16))
                            {
                                let allowed =
                                    state.can_archive(c.id, model::archives::Kind::Public);
                                let archive = ui
                                    .add_enabled_ui(allowed, |ui| {
                                        icons::button(ui, icons::Icon::Threads, 32.0, "Threads")
                                    })
                                    .inner;
                                if archive.clicked() {
                                    self.archive_parent = Some(c.id);
                                }
                            }
                            let reload = ui
                                .add_enabled_ui(state.freshness != Freshness::Loading, |ui| {
                                    icons::button(ui, icons::Icon::Reload, 32.0, "Reload history")
                                })
                                .inner;
                            if reload.clicked() {
                                self.timeline.follow_latest();
                                commands.push(state.history(None));
                            }
                        }
                        if let Some(channel) = state.selected.filter(|_| dm) {
                            self.voice_settings(ui, state.demo, state.voice.active.is_some());
                            self.call_button(ui, state, channel, commands);
                        }
                    });
                });
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
        let editing_key = self
            .editing
            .as_ref()
            .filter(|(c, _, _)| *c == channel)
            .map(|(c, id, _)| (*c, *id));
        let editing_here = editing_key.is_some();
        let focus_edit = editing_key.is_some() && self.composer_edit != editing_key;
        let focus_composer = self.composer_edit != editing_key
            && (editing_here || self.composer_edit.is_some_and(|(c, _)| c == channel));
        if self.composer_edit != editing_key {
            self.composer_edit = editing_key;
            self.edit_sent = false;
            self.ime_active = false;
            self.mention_menu = mentions::Menu::default();
            self.emoji_picker = emoji_picker::Picker::default();
        }
        let mut cancel_edit = false;
        let mut discard = None;
        if ctx.input(|input| !input.raw.hovered_files.is_empty()) {
            ui.label(if self.upload_busy || self.attachment.is_some() {
                "Remove the current attachment or wait before dropping another file"
            } else if state.gateway_connected && state.freshness == Freshness::Fresh {
                "Drop one file up to 20 MB to attach it here; Send starts the upload"
            } else {
                "Reconnect and reload this conversation before attaching a file"
            });
        }
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
                if let Some(filename) = &pending.attachment {
                    ui.label(format!("File: {filename}"));
                }
                if pending.delivery != Delivery::Sending && ui.small_button("Restore to draft").on_hover_text("For an unknown outcome, check the official client first; sending again can duplicate it.").clicked() { discard = Some(index); }
            });
        }
        if let Some(index) = discard {
            if !state.drafts.contains_key(&channel) && state.drafts.len() >= 64 {
                state.status =
                    "Draft budget full. Clear an existing draft before restoring pending text";
            } else if state.drafts.get(&channel).is_none_or(String::is_empty) {
                let pending = state.pending.remove(index);
                if pending.attachment.is_some() {
                    state.status = "Text restored; reselect the attachment before sending again";
                }
                state.drafts.insert(channel, pending.content);
                self.draft_changes.push(channel);
            } else {
                state.status = "Keep or clear the existing draft before restoring pending text";
            }
        }
        if editing_here {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Editing message")
                        .small()
                        .color(colors.accent),
                );
                cancel_edit = ui.small_button("Cancel edit").clicked();
                if self.edit_sent {
                    ui.small("Save requested · check connection status before retrying");
                }
            });
            ui.add_space(6.0);
        } else if let Some(reply) = state.reply {
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
        if !editing_here && let Some((filename, bytes)) = &self.attachment {
            if state.demo {
                ui.label("Uploads are disabled in offline preview");
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{filename} · {bytes} bytes"));
                if ui
                    .add_enabled(!self.upload_busy, egui::Button::new("Remove attachment"))
                    .clicked()
                {
                    self.remove_attachment_requested = true;
                }
            });
        }
        if !editing_here && (self.upload_busy || self.upload_status.is_some()) {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    self.upload_status
                        .as_deref()
                        .unwrap_or("Preparing attachment…"),
                );
                if self.upload_busy && ui.button("Cancel upload").clicked() {
                    self.cancel_upload_requested = true;
                }
            });
        }
        let full = state.draft_bytes() >= MAX_DRAFT_BYTES
            || (!state.drafts.contains_key(&channel) && state.drafts.len() >= 64);
        if full && !editing_here {
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
        let composer_id = ui.make_persistent_id(if editing_here {
            "message-edit"
        } else {
            "message-input"
        });
        if focus_composer {
            ctx.memory_mut(|m| m.request_focus(composer_id));
        }
        if focus_edit {
            let mut edit_state = egui::text_edit::TextEditState::default();
            let count = self
                .editing
                .as_ref()
                .map_or(0, |(_, _, content)| content.chars().count());
            edit_state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(count),
                )));
            edit_state.store(ctx, composer_id);
        }
        let composer_content = if editing_here {
            self.editing
                .as_ref()
                .map_or("", |(_, _, text)| text.as_str())
        } else {
            state.drafts.get(&channel).map_or("", String::as_str)
        };
        let count_before = composer_content.chars().count();
        let mention_enabled =
            !self.ime_active && !ime_this_frame && ctx.memory(|m| m.has_focus(composer_id));
        let mention_users = if mention_enabled || composer_content.contains("<@") {
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
            composer_content,
            cursor.filter(|_| mention_enabled),
            &mention_users,
            &state.channels,
        );
        let mention_pick = if mention_enabled {
            self.mention_menu.keys(ctx)
        } else {
            None
        };
        let mut editing = if editing_here {
            self.editing.take()
        } else {
            None
        };
        let demo = state.demo;
        let enter = !self.ime_active
            && !ime_this_frame
            && ctx.memory(|m| m.has_focus(composer_id))
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let placeholder = state.channels.iter().find(|c| c.id == channel).map_or_else(
            || "Message".to_owned(),
            |c| {
                if c.guild.is_some() {
                    format!("Message #{}", c.name)
                } else {
                    format!("Message @{}", c.name)
                }
            },
        );
        let can_attach = !editing_here
            && state.gateway_connected
            && state.freshness == Freshness::Fresh
            && !self.upload_busy
            && self.attachment.is_none();
        let can_send = state.freshness == Freshness::Fresh
            && if editing_here {
                count_before > 0
            } else {
                !self.upload_busy
                    && !(state.demo && self.attachment.is_some())
                    && (count_before > 0 || self.attachment.is_some())
            };
        egui::Frame::new()
            .fill(colors.raised)
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let attach = ui
                        .add_enabled_ui(can_attach, |ui| {
                            icons::button(ui, icons::Icon::Attach, 28.0, "Attach a file")
                        })
                        .inner
                        .on_hover_text("Choose or drop one file up to 20 MB. Upload starts only when you press Send.");
                    if attach.clicked() {
                        self.attach_requested = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let send_button = ui
                            .add_enabled_ui(can_send, |ui| {
                                icons::button(
                                    ui,
                                    icons::Icon::Send,
                                    28.0,
                                    if editing_here { "Save edit" } else { "Send message" },
                                )
                            })
                            .inner;
                        let send = enter || send_button.clicked();
                        if count_before + 200 >= MAX_CONTENT {
                            ui.label(
                                RichText::new(format!("{}", MAX_CONTENT.saturating_sub(count_before)))
                                    .size(11.0)
                                    .color(if count_before >= MAX_CONTENT { colors.danger } else { colors.muted }),
                            );
                        }
                        let pick = ui
                            .add_enabled_ui(!self.ime_active && !ime_this_frame, |ui| {
                                self.emoji_picker
                                    .show(ui, state, channel, &mut self.avatars)
                            })
                            .inner;
                        let edit = ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width());
                        cancel_edit |= editing_here && !self.ime_active && !ime_this_frame
                            && ctx.memory(|m| m.has_focus(composer_id) || m.had_focus_last_frame(composer_id))
                            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
                        let remaining = if editing_here { MAX_CONTENT * 4 } else { MAX_DRAFT_BYTES.saturating_sub(state.draft_bytes()) };
                        let mut new_draft = String::new();
                        let draft = if let Some((_, _, content)) = &mut editing {
                            content
                        } else {
                            state.drafts.get_mut(&channel).unwrap_or(&mut new_draft)
                        };
                        let mut mention_changed = false;
                        if let Some(pick) = pick {
                            let mut edit_state =
                                egui::text_edit::TextEditState::load(ctx, composer_id).unwrap_or_default();
                            let range = edit_state.cursor.char_range();
                            if let Some(cursor) = emoji_picker::insert(draft, &pick, range, remaining) {
                                edit_state
                                    .cursor
                                    .set_char_range(Some(egui::text::CCursorRange::one(
                                        egui::text::CCursor::new(cursor),
                                    )));
                                edit_state.store(ctx, composer_id);
                                mention_changed = true;
                            } else {
                                state.status =
                                    "Emoji will not fit. Shorten this message or free draft space.";
                            }
                        }
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
                        let mut rich_layout = composer_text::Layout::default();
                        if !self.ime_active
                            && !ime_this_frame
                            && ctx.input(|i| {
                                i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::Delete)
                            })
                        {
                            rich_layout.galley(
                                ui,
                                draft,
                                ui.available_width(),
                                &mention_users,
                                &mut self.avatars,
                                demo,
                            );
                            rich_layout.select_deleted_inline(ctx, composer_id);
                        }
                        let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, width: f32| {
                            rich_layout.galley(
                                ui,
                                buffer.as_str(),
                                width,
                                &mention_users,
                                &mut self.avatars,
                                demo,
                            )
                        };
                        let mut output = TextEdit::multiline(draft)
                            .layouter(&mut layouter)
                            .id(composer_id)
                            .event_filter(egui::EventFilter {
                                horizontal_arrows: true, vertical_arrows: true, escape: editing_here,
                                ..Default::default()
                            })
                            .char_limit(MAX_CONTENT)
                            .desired_rows(1)
                            .desired_width(f32::INFINITY)
                            .frame(egui::Frame::NONE)
                            .hint_text(placeholder.as_str())
                            .show(ui);
                        rich_layout.paint(ui, &output);
                        if !self.ime_active && !ime_this_frame {
                            rich_layout.snap_cursor(&mut output, ctx);
                        }
                        if mention_changed {
                            output.response.request_focus();
                        }
                        let mention_cursor = output
                            .cursor_range
                            .filter(|r| r.is_empty())
                            .map(|r| r.primary.index.0)
                            .filter(|_| mention_enabled);
                        self.mention_menu
                            .refresh(channel, draft, mention_cursor, &mention_users, &state.channels);
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
                        let cleared = draft.is_empty();
                        if edit.changed() || mention_changed {
                            if editing_here {
                                self.edit_sent = false;
                            } else if cleared {
                                self.clear_draft(state, channel);
                            } else {
                                if !new_draft.is_empty() {
                                    state.drafts.insert(channel, new_draft);
                                }
                                self.draft_changes.push(channel);
                            }
                        }
                        edit
                            }).inner
                        }).inner;
                        if send && !cancel_edit {
                            if let Some((edit_channel, message, content)) = &editing {
                                let owned = state.timeline.get(*message).is_some_and(|m| m.channel == *edit_channel && state.user.as_ref().is_some_and(|u| u.id == m.author.id));
                                if owned && state.auth == client_core::auth::AuthState::Authenticated
                                    && state.freshness == Freshness::Fresh && !content.trim().is_empty()
                                    && content.chars().count() <= MAX_CONTENT
                                {
                                    commands.push(Command::Edit { channel: *edit_channel, message: *message, content: content.clone() });
                                    self.edit_sent = true;
                                } else {
                                    state.status = "Edit kept. Wait for your current message and connection, and enter nonempty text.";
                                }
                            } else if !self.upload_busy && !(state.demo && self.attachment.is_some())
                                && let Some(command) = state.prepare_send_with_attachment(self.attachment.as_ref().map(|(name, _)| name.as_str())) {
                                commands.push(command);
                            }
                            edit.request_focus();
                        }
                    });
                });
            });
        if editing_here {
            self.editing = if cancel_edit { None } else { editing };
            if cancel_edit {
                self.edit_sent = false;
            }
        }
        if self.edit_sent
            && self.editing.as_ref().is_some_and(|(channel, id, content)| {
                state.timeline.get(*id).is_some_and(|message| {
                    message.channel == *channel && message.content == *content
                })
            })
        {
            self.editing = None;
            self.edit_sent = false;
        }
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
        let title = self
            .guild
            .and_then(|id| state.guilds.iter().find(|g| g.id == id))
            .map_or_else(|| "Direct Messages".to_owned(), |g| g.name.clone());
        self.title_bar(ui, state, &title);
        self.notification_rail(ui, state, &mut commands);
        // The lists and conversation share one rounded surface beside the rail.
        let content = ui.available_rect_before_wrap();
        ui.painter().rect_filled(
            content,
            egui::CornerRadius {
                nw: 8,
                ..Default::default()
            },
            colors.sidebar,
        );
        egui::Panel::left("channels")
            .resizable(true)
            .default_size(240.0)
            .size_range(200.0..=360.0)
            .frame(egui::Frame::new().inner_margin(0))
            .show(ui, |ui| self.sidebar(ui, state, &title, &mut commands));
        let selected_voice = state
            .channels
            .iter()
            .any(|c| Some(c.id) == state.selected && c.kind == 2);
        let wide_members = ui.available_width() >= 720.0;
        let show_members = !selected_voice
            && state.selected.is_some()
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
                    .exact_size(240.0)
                    .frame(egui::Frame::new().inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 8,
                        bottom: 8,
                    }))
                    .show(ui, |ui| {
                        self.member_rows(ui, state);
                    });
            } else {
                let mut open = self.members_narrow_open;
                egui::Window::new("Members")
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
            .frame(egui::Frame::new().fill(colors.chat).inner_margin(0))
            .show(ui, |ui| {
                self.channel_header(
                    ui,
                    state,
                    selected_voice,
                    show_members,
                    wide_members,
                    &mut commands,
                );
                self.call_bar(ui, state, &mut commands);
                let Some(channel) = state.selected else {
                    ui.add_space((ui.available_height() * 0.32).max(24.0));
                    ui.vertical_centered(|ui| {
                        ui.label(
                            design::semibold(ui, "No conversation selected", 20.0)
                                .color(colors.text_strong),
                        );
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new("Pick a channel or direct message from the list.")
                                .color(colors.muted),
                        );
                    });
                    return;
                };
                if selected_voice {
                    self.voice_channel(ui, state, channel, &mut commands);
                    return;
                }
                egui::Panel::bottom("composer")
                    .show_separator_line(false)
                    .frame(
                        egui::Frame::new()
                            .fill(colors.chat)
                            .inner_margin(egui::Margin {
                                left: 16,
                                right: 16,
                                top: 4,
                                bottom: 20,
                            }),
                    )
                    .show(ui, |ui| {
                        self.timeline.download.show_status(ui);
                        self.composer(ui, state, channel, &ctx, &mut commands);
                    });
                let notices: Vec<String> = [
                    match state.freshness {
                        Freshness::Fresh => None,
                        Freshness::Loading => Some("Loading history…"),
                        Freshness::Stale => Some("Cached history · awaiting sync"),
                        Freshness::Unavailable => Some("Conversation unavailable"),
                    }
                    .map(str::to_owned),
                    state.read_state.status.map(str::to_owned),
                    (state.archived_thread.is_some() && state.archived_thread == state.selected)
                        .then(|| "Opened from archive".to_owned()),
                    state
                        .history_before
                        .is_some()
                        .then(|| "Browsing earlier messages · Jump to latest to return".to_owned()),
                ]
                .into_iter()
                .flatten()
                .collect();
                if !notices.is_empty() || state.can_load_older() {
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(16, 4))
                        .show(ui, |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(
                                    RichText::new(notices.join(" · "))
                                        .size(11.0)
                                        .color(colors.muted),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if state.can_load_older()
                                            && ui
                                                .add(
                                                    egui::Button::new(
                                                        RichText::new("Load earlier messages")
                                                            .size(11.0)
                                                            .color(colors.link),
                                                    )
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
                        });
                }
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 0,
                        right: 0,
                        top: 4,
                        bottom: 0,
                    })
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
        if state
            .archives
            .as_ref()
            .is_some_and(|view| self.guild != Some(view.guild))
        {
            commands.push(state.clear_archives());
        }
        self.search.show(&ctx, state, &mut commands);
        self.archives.show(&ctx, state, &mut commands);
        if let Some(id) = self.timeline.channel_reference.take()
            && let Some(target) = state
                .channels
                .iter()
                .find(|c| c.id == id && c.guild.is_some() && c.supports_text())
        {
            self.guild = target.guild;
            if let Some(command) = state.select(id) {
                commands.push(command);
            }
        }
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
            let anchor = match self.profile_anchor {
                Some((id, pos)) if id == user.id => pos,
                _ => {
                    let pos = ctx
                        .input(|i| i.pointer.interact_pos().or(i.pointer.latest_pos()))
                        .unwrap_or_else(|| ctx.content_rect().center());
                    self.profile_anchor = Some((user.id, pos));
                    pos
                }
            };
            match profiles::show(
                ui,
                user,
                state.profile.as_ref(),
                state,
                &mut self.avatars,
                &mut self.profile_link,
                anchor,
            ) {
                Some(profiles::Action::Profile(user)) => {
                    self.profile = Some(user);
                    self.profile_link = None;
                    commands.push(state.clear_profile());
                }
                Some(profiles::Action::Close) => {
                    self.profile = None;
                    self.profile_link = None;
                    self.profile_anchor = None;
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
                    self.profile_anchor = None;
                    commands.push(state.clear_profile());
                    if let Some(command) = state.select(channel) {
                        commands.push(command);
                    }
                }
                None => {}
            }
        } else {
            self.profile_anchor = None;
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

    fn edit_state() -> State {
        let user = model::User {
            id: Id(1),
            name: "Alex".into(),
            avatar: None,
            discriminator: 0,
        };
        let mut state = State {
            selected: Some(Id(10)),
            user: Some(user.clone()),
            demo: true,
            freshness: Freshness::Fresh,
            auth: client_core::auth::AuthState::Authenticated,
            gateway_connected: true,
            ..Default::default()
        };
        state
            .timeline
            .insert(
                model::Message {
                    id: Id(20),
                    channel: Id(10),
                    author: user,
                    content: "Original".into(),
                    edited: false,
                    edited_at: None,
                    revision: 0,
                    nonce: None,
                    reply_to: None,
                    kind: 0,
                    unsupported: false,
                    embeds: vec![],
                    attachments: vec![],
                    mentions: vec![],
                    reactions: None,
                    embeds_suppressed: false,
                },
                false,
                false,
            )
            .unwrap();
        state.drafts.insert(Id(10), "Unsent draft 👋".into());
        state
    }

    fn edit_frame(
        ctx: &egui::Context,
        view: &mut MessagingUi,
        state: &mut State,
        events: Vec<egui::Event>,
    ) -> Vec<Command> {
        let mut commands = vec![];
        let output = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                view.composer(ui, state, state.selected.unwrap(), ctx, &mut commands);
            },
        );
        output.drop_without_applying_deltas();
        commands
    }

    fn edit_key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    #[test]
    fn inline_edit_uses_mentions_ime_and_preserves_draft_until_confirmed() {
        let ctx = egui::Context::default();
        let mut state = edit_state();
        let mut view = MessagingUi {
            editing: Some((Id(10), Id(20), "Original".into())),
            ..Default::default()
        };
        assert!(edit_frame(&ctx, &mut view, &mut state, vec![]).is_empty());
        assert!(
            edit_frame(
                &ctx,
                &mut view,
                &mut state,
                vec![egui::Event::Text(" @Al".into())]
            )
            .is_empty()
        );
        assert!(
            edit_frame(
                &ctx,
                &mut view,
                &mut state,
                vec![edit_key(egui::Key::Enter)]
            )
            .is_empty()
        );
        assert_eq!(view.editing.as_ref().unwrap().2, "Original <@1> ");
        assert!(
            edit_frame(
                &ctx,
                &mut view,
                &mut state,
                vec![
                    egui::Event::Ime(egui::ImeEvent::Commit("語".into())),
                    edit_key(egui::Key::Enter)
                ]
            )
            .is_empty()
        );
        let commands = edit_frame(
            &ctx,
            &mut view,
            &mut state,
            vec![edit_key(egui::Key::Enter)],
        );
        assert!(
            matches!(commands.as_slice(), [Command::Edit { channel: Id(10), message: Id(20), content }] if content.trim_end() == "Original <@1> 語")
        );
        assert!(view.has_edit(), "keep text until the service confirms it");
        state.command_rejected(commands.into_iter().next().unwrap());
        edit_frame(&ctx, &mut view, &mut state, vec![]);
        assert_eq!(view.editing.as_ref().unwrap().2, "Original <@1> 語\n");
        assert_eq!(state.drafts[&Id(10)], "Unsent draft 👋");
        assert!(
            view.draft_changes.is_empty(),
            "edits must not overwrite persisted unsent drafts"
        );
        let mut confirmed = state.timeline.get(Id(20)).unwrap().clone();
        confirmed.content = view.editing.as_ref().unwrap().2.clone();
        confirmed.edited = true;
        edit_frame(
            &ctx,
            &mut view,
            &mut state,
            vec![egui::Event::Text("newer".into())],
        );
        state.timeline.insert(confirmed, false, false).unwrap();
        edit_frame(&ctx, &mut view, &mut state, vec![]);
        assert!(
            view.has_edit(),
            "an earlier acknowledgement must not discard newer input"
        );
        state.freshness = Freshness::Fresh;
        let commands = edit_frame(
            &ctx,
            &mut view,
            &mut state,
            vec![edit_key(egui::Key::Enter)],
        );
        assert!(matches!(commands.as_slice(), [Command::Edit { .. }]));
        let mut confirmed = state.timeline.get(Id(20)).unwrap().clone();
        confirmed.content = view.editing.as_ref().unwrap().2.clone();
        state.timeline.insert(confirmed, false, false).unwrap();
        edit_frame(
            &ctx,
            &mut view,
            &mut state,
            vec![egui::Event::Text("same-frame".into())],
        );
        assert!(
            view.has_edit(),
            "input arriving with confirmation still belongs to the edit"
        );
        let commands = edit_frame(
            &ctx,
            &mut view,
            &mut state,
            vec![edit_key(egui::Key::Enter)],
        );
        assert!(matches!(commands.as_slice(), [Command::Edit { .. }]));
        let mut confirmed = state.timeline.get(Id(20)).unwrap().clone();
        confirmed.content = view.editing.as_ref().unwrap().2.clone();
        state.timeline.insert(confirmed, false, false).unwrap();
        edit_frame(&ctx, &mut view, &mut state, vec![]);
        assert!(!view.has_edit());
        assert_eq!(state.drafts[&Id(10)], "Unsent draft 👋");
    }

    #[test]
    fn inline_edit_survives_navigation_blocks_invalid_saves_and_cancels_without_sending() {
        let ctx = egui::Context::default();
        let mut state = edit_state();
        let mut view = MessagingUi {
            editing: Some((Id(10), Id(20), "Replacement".into())),
            ..Default::default()
        };
        edit_frame(&ctx, &mut view, &mut state, vec![]);
        state.selected = Some(Id(11));
        edit_frame(&ctx, &mut view, &mut state, vec![]);
        assert_eq!(view.editing.as_ref().unwrap().2, "Replacement");
        assert!(!state.drafts.contains_key(&Id(11)));
        state.selected = Some(Id(10));
        edit_frame(&ctx, &mut view, &mut state, vec![]);
        state.freshness = Freshness::Stale;
        assert!(
            edit_frame(
                &ctx,
                &mut view,
                &mut state,
                vec![edit_key(egui::Key::Enter)]
            )
            .is_empty()
        );
        assert!(view.has_edit());
        state.freshness = Freshness::Fresh;
        state.user.as_mut().unwrap().id = Id(99);
        assert!(
            edit_frame(
                &ctx,
                &mut view,
                &mut state,
                vec![edit_key(egui::Key::Enter)]
            )
            .is_empty()
        );
        assert!(view.has_edit());
        assert!(
            edit_frame(
                &ctx,
                &mut view,
                &mut state,
                vec![edit_key(egui::Key::Escape)]
            )
            .is_empty()
        );
        assert!(!view.has_edit());
        assert_eq!(state.drafts[&Id(10)], "Unsent draft 👋");
        assert!(view.draft_changes.is_empty());
    }

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
                            custom_status: None,
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
    fn inline_edit_keeps_the_pending_draft_attachment_separate() {
        let ctx = egui::Context::default();
        let mut state = edit_state();
        let mut view = MessagingUi {
            editing: Some((Id(10), Id(20), "Replacement".into())),
            attachment: Some(("draft.txt".into(), 32)),
            upload_busy: true,
            ..Default::default()
        };
        edit_frame(&ctx, &mut view, &mut state, vec![]);
        let commands = edit_frame(
            &ctx,
            &mut view,
            &mut state,
            vec![edit_key(egui::Key::Enter)],
        );
        assert!(
            matches!(commands.as_slice(), [Command::Edit { content, .. }] if content == "Replacement")
        );
        assert_eq!(state.drafts[&Id(10)], "Unsent draft 👋");
        assert_eq!(view.attachment, Some(("draft.txt".into(), 32)));
        assert!(
            !view.attach_requested
                && !view.remove_attachment_requested
                && !view.cancel_upload_requested
        );
    }

    #[test]
    fn attachment_only_enter_sends_once_and_busy_upload_blocks_resending() {
        for busy in [true, false] {
            let ctx = egui::Context::default();
            let mut state = State {
                selected: Some(Id(1)),
                auth: client_core::auth::AuthState::Authenticated,
                freshness: Freshness::Fresh,
                ..Default::default()
            };
            let mut messaging = MessagingUi {
                attachment: Some(("synthetic.txt".into(), 32)),
                upload_busy: busy,
                ..Default::default()
            };
            let mut commands = Vec::new();
            let mut editor = egui::Id::NULL;
            ctx.run_ui(Default::default(), |ui| {
                editor = ui.make_persistent_id("message-input");
                messaging.composer(ui, &mut state, Id(1), &ctx, &mut commands);
            })
            .drop_without_applying_deltas();
            ctx.memory_mut(|m| m.request_focus(editor));
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
                |ui| messaging.composer(ui, &mut state, Id(1), &ctx, &mut commands),
            )
            .drop_without_applying_deltas();
            assert_eq!(commands.len(), usize::from(!busy));
            if !busy {
                assert!(
                    matches!(&commands[0], Command::Send { content, .. } if content.is_empty())
                );
                assert_eq!(
                    state.pending[0].attachment.as_deref(),
                    Some("synthetic.txt")
                );
            }
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
