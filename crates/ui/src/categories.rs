use crate::{MessagingUi, design};
use client_core::State;
use egui::RichText;
use model::{Channel, Id};
use std::collections::{BTreeMap, BTreeSet};

enum Row<'a> {
    Category(&'a Channel, usize),
    Channel(&'a Channel),
}

fn rows<'a>(
    channels: &'a [Channel],
    guild: Option<Id>,
    collapsed: &BTreeSet<Id>,
    selected: Option<Id>,
) -> Vec<Row<'a>> {
    let mut categories: Vec<_> = channels
        .iter()
        .filter(|c| c.guild == guild && guild.is_some() && c.kind == 4)
        .collect();
    categories.sort_unstable_by_key(|c| (c.position, c.id));
    let category_ids: BTreeSet<_> = categories.iter().map(|c| c.id).collect();
    let mut groups: BTreeMap<Option<Id>, Vec<&Channel>> = BTreeMap::new();
    for channel in channels.iter().filter(|c| c.guild == guild && c.kind != 4) {
        let parent = channel.parent_id.filter(|id| category_ids.contains(id));
        groups.entry(parent).or_default().push(channel);
    }
    for group in groups.values_mut() {
        if guild.is_some() {
            group.sort_unstable_by_key(|c| (c.position, c.id));
        }
    }
    let mut rows: Vec<_> = groups
        .remove(&None)
        .unwrap_or_default()
        .into_iter()
        .map(Row::Channel)
        .collect();
    for category in categories {
        let children = groups.remove(&Some(category.id)).unwrap_or_default();
        rows.push(Row::Category(category, children.len()));
        rows.extend(
            children
                .into_iter()
                .filter(|c| !collapsed.contains(&category.id) || Some(c.id) == selected)
                .map(Row::Channel),
        );
    }
    rows
}

fn kind_label(kind: u8) -> &'static str {
    match kind {
        0 => "Text channel",
        1 => "Direct message",
        2 => "Server voice channel · not implemented",
        3 => "Group direct message",
        5 => "Announcement channel",
        10..=12 => "Thread",
        13 => "Stage channel · not implemented",
        14 => "Directory · not implemented",
        15 => "Forum · not implemented",
        16 => "Media channel · not implemented",
        _ => "Unknown channel type · not implemented",
    }
}

impl MessagingUi {
    pub(super) fn channel_list(&mut self, ui: &mut egui::Ui, state: &State) -> Option<Id> {
        // Session-only keys are pruned on navigation updates, never accumulated in egui memory.
        let categories: BTreeSet<_> = state
            .channels
            .iter()
            .filter(|c| c.kind == 4)
            .map(|c| c.id)
            .collect();
        self.collapsed_categories
            .retain(|id| categories.contains(id));
        let rows = rows(
            &state.channels,
            self.guild,
            &self.collapsed_categories,
            state.selected,
        );
        let colors = design::palette(ui);
        let mut selected = None;
        if rows.is_empty() {
            ui.label(RichText::new("No conversations available here.").color(colors.muted));
        }
        egui::ScrollArea::vertical()
            .id_salt(("channel-list", self.guild))
            .show_rows(ui, 36.0, rows.len(), |ui, range| {
                ui.spacing_mut().item_spacing.y = 4.0;
                for index in range {
                    match rows[index] {
                        Row::Category(category, count) => {
                            let collapsed = self.collapsed_categories.contains(&category.id);
                            let label =
                                format!("{}  {}", if collapsed { ">" } else { "v" }, category.name);
                            let response = ui
                                .push_id(category.id, |ui| {
                                    ui.add(
                                        egui::Button::new(
                                            RichText::new(label)
                                                .size(11.0)
                                                .strong()
                                                .color(colors.muted),
                                        )
                                        .frame(false)
                                        .right_text(count.to_string())
                                        .min_size(egui::vec2(ui.available_width(), 36.0))
                                        .truncate(),
                                    )
                                })
                                .inner
                                .on_hover_text(format!(
                                    "{} category · {} channels · {}",
                                    category.name,
                                    count,
                                    if collapsed { "Expand" } else { "Collapse" }
                                ));
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    true,
                                    format!(
                                        "{} category, {}, {} channels",
                                        category.name,
                                        if collapsed { "collapsed" } else { "expanded" },
                                        count
                                    ),
                                )
                            });
                            if response.clicked() {
                                if collapsed {
                                    self.collapsed_categories.remove(&category.id);
                                } else {
                                    self.collapsed_categories.insert(category.id);
                                }
                            }
                        }
                        Row::Channel(channel) => {
                            let active = state.selected == Some(channel.id);
                            let symbol = match channel.kind {
                                1 | 3 => "@",
                                2 | 13 => "♫",
                                15 | 16 => "▤",
                                _ => "#",
                            };
                            let name = if channel.supports_text() {
                                format!("{symbol}   {}", channel.name)
                            } else {
                                format!("{symbol}   {} · unavailable", channel.name)
                            };
                            let response = ui
                                .push_id(channel.id, |ui| {
                                    ui.horizontal(|ui| {
                                        if channel.guild.is_none()
                                            && let Some(user) = channel.recipients.first()
                                            && self
                                                .avatars
                                                .show(ui, user, 32.0, state.demo)
                                                .clicked()
                                        {
                                            self.profile = Some(user.clone());
                                        }
                                        ui.add_enabled(
                                            channel.supports_text(),
                                            egui::Button::selectable(
                                                active,
                                                RichText::new(name).color(if active {
                                                    colors.accent
                                                } else {
                                                    colors.text
                                                }),
                                            )
                                            .right_text(())
                                            .min_size(egui::vec2(ui.available_width(), 36.0))
                                            .corner_radius(7)
                                            .truncate(),
                                        )
                                    })
                                    .inner
                                })
                                .inner
                                .on_hover_text(format!(
                                    "{} · {}",
                                    channel.name,
                                    kind_label(channel.kind)
                                ));
                            if active {
                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        response.rect.left_top() + egui::vec2(0.0, 9.0),
                                        egui::vec2(3.0, 18.0),
                                    ),
                                    2,
                                    colors.accent,
                                );
                            }
                            if response.clicked() {
                                selected = Some(channel.id);
                            }
                        }
                    }
                }
            });
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn channel(id: u64, kind: u8, position: i32, parent_id: Option<Id>) -> Channel {
        Channel {
            id: Id(id),
            guild: Some(Id(100)),
            parent_id,
            position,
            name: format!("Synthetic {id}"),
            kind,
            recipients: vec![],
            member_list_id: None,
        }
    }
    #[test]
    fn service_order_orphans_collapsed_selection_and_category_buttons() {
        let channels = vec![
            channel(8, 0, 2, Some(Id(4))),
            channel(4, 4, 1, None),
            channel(7, 0, 2, Some(Id(4))),
            channel(9, 2, 0, Some(Id(5))),
            channel(5, 4, 2, None),
            channel(3, 0, 0, Some(Id(99))),
            channel(2, 0, 1, None),
        ];
        let ids = |rows: Vec<Row<'_>>| {
            rows.into_iter()
                .map(|r| match r {
                    Row::Channel(c) | Row::Category(c, _) => c.id.0,
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids(rows(&channels, Some(Id(100)), &BTreeSet::new(), None)),
            [3, 2, 4, 7, 8, 5, 9]
        );
        assert_eq!(
            ids(rows(
                &channels,
                Some(Id(100)),
                &BTreeSet::from([Id(4)]),
                Some(Id(8))
            )),
            [3, 2, 4, 8, 5, 9]
        );
        assert!(!channels[1].supports_text());
        assert_eq!(kind_label(15), "Forum · not implemented");
        let mut state = State {
            channels,
            demo: true,
            ..State::default()
        };
        assert!(state.select(Id(4)).is_none());
        let ctx = egui::Context::default();
        let mut view = MessagingUi {
            guild: Some(Id(100)),
            collapsed_categories: BTreeSet::from([Id(999)]),
            ..MessagingUi::default()
        };
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            assert!(view.channel_list(ui, &state).is_none());
        });
        output.textures_delta.clear();
        assert!(view.collapsed_categories.is_empty());
        assert!(state.selected.is_none());
        // A category is a keyboard-operable button, never a history-selection command.
        state.channels = vec![channel(4, 4, 0, None), channel(8, 0, 0, Some(Id(4)))];
        let ctx = egui::Context::default();
        for key in [egui::Key::Tab, egui::Key::Enter] {
            let input = egui::RawInput {
                events: vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ui| {
                assert!(view.channel_list(ui, &state).is_none());
            });
            output.textures_delta.clear();
        }
        assert!(view.collapsed_categories.contains(&Id(4)));
        assert!(state.selected.is_none());
    }
}
