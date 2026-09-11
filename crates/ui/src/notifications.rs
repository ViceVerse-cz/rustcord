use crate::{MessagingUi, design};
use client_core::{Command, State};
use egui::{Align2, Color32, FontId};
use model::Id;

/// Fixed width of the server rail column.
pub(super) const RAIL_WIDTH: f32 = 72.0;

pub(super) fn badge(ui: &egui::Ui, center: egui::Pos2, count: u32, ring: Color32) {
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
	let colors = design::palette(ui);
	ui.painter().rect_filled(rect.expand(3.0), 12, ring);
	ui.painter().rect_filled(rect, 10, colors.danger);
	ui.painter().text(
		center,
		Align2::CENTER_CENTER,
		label,
		FontId::new(12.0, crate::design::semibold_family(ui.ctx())),
		Color32::WHITE,
	);
}
/// Discord's rail pill on the window edge: short for unread, taller on hover, full when selected.
fn rail_pill(ui: &egui::Ui, rect: egui::Rect, selected: bool, hovered: bool, unread: bool) {
	let height = if selected {
		40.0
	} else if hovered {
		20.0
	} else if unread {
		8.0
	} else {
		return;
	};
	let pill = egui::Rect::from_center_size(
		egui::pos2(rect.left() - 10.0, rect.center().y),
		egui::vec2(8.0, height),
	);
	ui.painter()
		.rect_filled(pill, 4, design::palette(ui).text_strong);
}
fn indicator(ui: &egui::Ui, rect: egui::Rect, unread: bool, count: u32) {
	rail_pill(ui, rect, false, false, unread);
	if count > 0 {
		badge(
			ui,
			rect.right_bottom() - egui::vec2(8.0, 8.0),
			count,
			design::palette(ui).base,
		);
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
            .exact_size(RAIL_WIDTH)
            .show_separator_line(false)
            .frame(egui::Frame::new().fill(colors.base).inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 4,
                bottom: 8,
            }))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                let home = self.guild.is_none();
                let (rect, response) =
                    ui.allocate_exact_size(egui::Vec2::splat(48.0), egui::Sense::click());
                let hovered = response.hovered() || response.has_focus();
                let fill = if home || hovered { colors.accent } else { colors.raised };
                let radius = if home || hovered { 16 } else { 24 };
                ui.painter().rect_filled(rect, radius, fill);
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    "S",
                    FontId::new(22.0, crate::design::semibold_family(ui.ctx())),
                    if home || hovered { colors.accent_text } else { colors.text },
                );
                rail_pill(ui, rect, home, hovered, false);
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        true,
                        home,
                        "Direct messages",
                    )
                });
                if response.on_hover_text("Direct Messages").clicked() {
                    self.guild = None;
                }
                let (line, _) = ui.allocate_exact_size(egui::vec2(48.0, 2.0), egui::Sense::hover());
                ui.painter().rect_filled(
                    egui::Rect::from_center_size(line.center(), egui::vec2(32.0, 2.0)),
                    1,
                    colors.raised,
                );
                egui::ScrollArea::vertical()
                    .id_salt("guild-list")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        // Existing channel metadata bounds this list; only visible avatar rows request images.
                        for channel in state.channels.iter().filter(|c| {
                            c.guild.is_none()
                                && c.supports_text()
                                && (state.channel_unread(c) == Some(true)
                                    || state.unread_count(c.id) > 0)
                        }) {
                            let response = if let Some(user) = channel.recipients.first() {
                                self.avatars.show(ui, user, 48.0, state.demo)
                            } else {
                                design::avatar(ui, &channel.name, 48.0)
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
                            rail_pill(
                                ui,
                                response.rect,
                                self.guild == Some(guild.id),
                                response.hovered() || response.has_focus(),
                                unread,
                            );
                            if count > 0 {
                                badge(ui, response.rect.right_bottom() - egui::vec2(8.0, 8.0), count, colors.base);
                            }
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
                            if response.on_hover_text("Red badges count mentions; the side pill marks unread activity. Session-observed counts may be a lower bound.").clicked() {
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
