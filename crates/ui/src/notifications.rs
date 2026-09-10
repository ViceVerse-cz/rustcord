use crate::{MessagingUi, design};
use client_core::{Command, State};
use egui::{Align2, Color32, FontId, RichText};
use model::Id;

pub(super) fn badge(ui: &egui::Ui, center: egui::Pos2, count: u32) {
    let label = if count > 99 {
        "99+".into()
    } else {
        count.to_string()
    };
    let width = if count > 99 {
        30.0
    } else if count > 9 {
        24.0
    } else {
        19.0
    };
    let rect = egui::Rect::from_center_size(center, egui::vec2(width, 19.0));
    ui.painter()
        .rect_filled(rect.expand(2.0), 12, design::palette(ui).canvas);
    ui.painter()
        .rect_filled(rect, 10, Color32::from_rgb(196, 42, 65));
    ui.painter().text(
        center,
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(12.0),
        Color32::WHITE,
    );
}
fn indicator(ui: &egui::Ui, rect: egui::Rect, unread: bool, count: u32) {
    if unread {
        ui.painter().circle_filled(
            egui::pos2(rect.left() - 5.0, rect.center().y),
            3.0,
            design::palette(ui).text,
        );
    }
    if count > 0 {
        badge(ui, rect.right_bottom() - egui::vec2(5.0, 5.0), count);
    }
}
impl MessagingUi {
    pub fn viewing_latest(&self, channel: Id) -> bool {
        self.timeline.viewing_latest(channel)
    }
    pub(super) fn notification_rail(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut State,
        commands: &mut Vec<Command>,
    ) {
        let colors = design::palette(ui);
        let mut selected = None;
        let mut guild_badges = std::collections::BTreeMap::<Id, (bool, u32)>::new();
        for channel in &state.channels {
            if let Some(guild) = channel.guild {
                let entry = guild_badges.entry(guild).or_default();
                entry.0 |= state.channel_unread(channel) == Some(true)
                    || state.unread_count(channel.id) > 0;
                entry.1 = entry.1.saturating_add(state.mention_count(channel.id));
            }
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
                        // Existing channel metadata bounds this list; only visible avatar rows request images.
                        for channel in state.channels.iter().filter(|c| {
                            c.guild.is_none()
                                && c.supports_text()
                                && (state.channel_unread(c) == Some(true)
                                    || state.unread_count(c.id) > 0)
                        }) {
                            let response = if let Some(user) = channel.recipients.first() {
                                self.avatars.show(ui, user, 44.0, state.demo)
                            } else {
                                ui.add_sized(
                                    [44.0, 44.0],
                                    egui::Button::new("DM").corner_radius(22),
                                )
                            };
                            let count = state.unread_count(channel.id);
                            indicator(ui, response.rect, true, count);
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    format!(
                                        "Open {}, unread, {} notifications",
                                        channel.name, count
                                    ),
                                )
                            });
                            if response
                                .on_hover_text(format!("{} · {}", channel.name, if state.channel_unread(channel).is_some() { "Unread activity; count may be a lower bound" } else { "Session activity · read sync unavailable" }))
                                .clicked()
                            {
                                self.guild = None;
                                selected = Some(channel.id);
                            }
                        }
                        for guild in &state.guilds {
                            let (unread, count) =
                                guild_badges.get(&guild.id).copied().unwrap_or_default();
                            let response = self.avatars.show_guild(
                                ui,
                                guild,
                                self.guild == Some(guild.id),
                                state.demo,
                            );
                            indicator(ui, response.rect, unread, count);
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    format!(
                                        "Server {}, {}, {} mentions",
                                        guild.name,
                                        if unread { "unread" } else { "read" },
                                        count
                                    ),
                                )
                            });
                            if response.on_hover_text("Red badges count mentions; dots indicate unread activity. Session-observed counts may be a lower bound.").clicked() {
                                self.guild = Some(guild.id);
                            }
                        }
                    });
            });
        if let Some(id) = selected
            && let Some(command) = state.select(id)
        {
            commands.push(command);
        }
    }
}
