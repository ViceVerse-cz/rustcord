use crate::{MessagingUi, design};
use client_core::State;
use egui::RichText;
use model::{Channel, Id};
use std::collections::{BTreeMap, BTreeSet};

enum Row<'a> {
    Category(&'a Channel, usize),
    Channel(&'a Channel, bool),
    Participant(&'a client_core::voice::RosterEntry),
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
    let parents: BTreeMap<_, _> = channels
        .iter()
        .filter(|c| guild.is_some() && c.guild == guild && matches!(c.kind, 0 | 5 | 15 | 16))
        .map(|c| (c.id, c))
        .collect();
    let mut groups: BTreeMap<Option<Id>, Vec<&Channel>> = BTreeMap::new();
    let mut threads: BTreeMap<Id, Vec<&Channel>> = BTreeMap::new();
    for channel in channels.iter().filter(|c| c.guild == guild && c.kind != 4) {
        if matches!(channel.kind, 10..=12)
            && let Some(parent) = channel.parent_id.and_then(|id| parents.get(&id))
            && parent.parent_id != Some(channel.id)
        {
            threads.entry(parent.id).or_default().push(channel);
            continue;
        }
        let parent = channel
            .parent_id
            .filter(|id| !matches!(channel.kind, 10..=12) && category_ids.contains(id));
        groups.entry(parent).or_default().push(channel);
    }
    for group in groups.values_mut().chain(threads.values_mut()) {
        if guild.is_some() {
            group.sort_unstable_by_key(|c| (c.position, c.id));
        }
    }
    let append = |channel: &'a Channel, collapsed: bool, rows: &mut Vec<Row<'a>>| {
        let children = threads.get(&channel.id);
        if !collapsed
            || Some(channel.id) == selected
            || children.is_some_and(|children| children.iter().any(|c| Some(c.id) == selected))
        {
            rows.push(Row::Channel(channel, false));
            rows.extend(
                children
                    .into_iter()
                    .flatten()
                    .filter(|c| !collapsed || Some(c.id) == selected)
                    .map(|c| Row::Channel(c, true)),
            );
        }
    };
    let mut rows = Vec::with_capacity(channels.len());
    for channel in groups.remove(&None).unwrap_or_default() {
        append(channel, false, &mut rows);
    }
    for category in categories {
        let children = groups.remove(&Some(category.id)).unwrap_or_default();
        let count = children
            .iter()
            .map(|c| 1 + threads.get(&c.id).map_or(0, Vec::len))
            .sum();
        rows.push(Row::Category(category, count));
        for channel in children {
            append(channel, collapsed.contains(&category.id), &mut rows);
        }
    }
    rows
}

fn kind_label(kind: u8) -> &'static str {
    match kind {
        0 => "Text channel",
        1 => "Direct message",
        2 => "Server voice channel",
        3 => "Group direct message",
        5 => "Announcement channel",
        10..=12 => "Thread",
        13 => "Stage channel · not implemented",
        14 => "Directory · not implemented",
        15 => "Forum · loaded posts",
        16 => "Media · loaded posts",
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
        let channel_rows = rows(
            &state.channels,
            self.guild,
            &self.collapsed_categories,
            state.selected,
        );
        let mut participants = BTreeMap::<Id, Vec<_>>::new();
        for entry in &state.voice.roster {
            if Some(entry.guild) == self.guild {
                participants.entry(entry.channel).or_default().push(entry);
            }
        }
        let mut rows = Vec::with_capacity(channel_rows.len() + state.voice.roster.len());
        for row in channel_rows {
            let channel = match &row {
                Row::Channel(channel, _) if channel.kind == 2 => Some(channel.id),
                _ => None,
            };
            rows.push(row);
            if let Some(entries) = channel.and_then(|id| participants.remove(&id)) {
                rows.extend(entries.into_iter().map(Row::Participant));
            }
        }
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
                        Row::Participant(entry) => {
                            ui.horizontal(|ui| {
                                ui.add_space(28.0);
                                self.voice_participant(ui, state, entry);
                            });
                        }
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
                        Row::Channel(channel, nested) => {
                            let active = state.selected == Some(channel.id);
                            if channel.kind == 2 {
                                let response =
                                    self.voice_channel_button(ui, state, channel, active);
                                if response.clicked() {
                                    selected = Some(channel.id);
                                }
                                continue;
                            }
                            let visible = state.can_view(channel.id);
                            let unread = visible && (state.channel_unread(channel) == Some(true)
                                || state.unread_count(channel.id) > 0);
                            let count = if !visible { 0 } else if channel.guild.is_some() {
                                state.mention_count(channel.id)
                            } else {
                                state.unread_count(channel.id)
                            };
                            let symbol = match channel.kind {
                                1 | 3 => "@",
                                2 | 13 => "♫",
                                15 | 16 => "▤",
                                10..=12 => "↳",
                                _ => "#",
                            };
                            let name = if matches!(channel.kind, 15 | 16) {
                                format!(
                                    "{symbol}   {} · {}",
                                    channel.name,
                                    kind_label(channel.kind)
                                )
                            } else if channel.supports_text() {
                                format!("{symbol}   {}", channel.name)
                            } else {
                                format!("{symbol}   {} · unavailable", channel.name)
                            };
                            let response = ui
                                .push_id(channel.id, |ui| {
                                    ui.horizontal(|ui| {
                                        if nested {
                                            ui.add_space(12.0);
                                        }
                                        if channel.guild.is_none()
                                            && let Some(user) = channel.recipients.first()
                                            && self
                                                .avatars
                                                .show(ui, user, 32.0, state.demo)
                                                .clicked()
                                        {
                                            self.profile = Some(user.clone());
                                        }
                                        let archives = channel.guild.is_some() && matches!(channel.kind, 0 | 5 | 15 | 16);
                                        let width = (ui.available_width() - if archives { 68.0 } else { 0.0 }).max(0.0);
                                        let response = ui.allocate_ui(egui::vec2(width, 36.0), |ui| ui.add_enabled(
                                            channel.supports_text() && state.can_view(channel.id),
                                            egui::Button::selectable(
                                                active,
                                                RichText::new(name).color(if active {
                                                    colors.accent
                                                } else {
                                                    if unread { colors.text } else { colors.muted }
                                                }),
                                            )
                                            .right_text(if count > 0 {
                                                "        "
                                            } else if unread {
                                                "●"
                                            } else {
                                                ""
                                            })
                                            .min_size(egui::vec2(width, 36.0))
                                            .corner_radius(7)
                                            .truncate(),
                                        )).inner;
                                        if archives {
                                            let allowed = state.can_archive(channel.id, model::archives::Kind::Public);
                                            let archive = ui.add_enabled(
                                                allowed,
                                                egui::Button::new("Archive").min_size(egui::vec2(60.0, 32.0)),
                                            ).on_hover_text("Browse archived threads; opening only loads their messages");
                                            archive.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button,
                                                allowed && ui.is_enabled(), format!("Archive for {}", channel.name)));
                                            if archive.clicked() { self.archive_parent = Some(channel.id); }
                                        }
                                        response
                                    })
                                    .inner
                                })
                                .inner
                                .on_hover_text(format!(
                                    "{} · {}{}",
                                    channel.name,
                                    kind_label(channel.kind),
                                    if unread && state.channel_unread(channel).is_none() {
                                        " · Session activity; read sync unavailable"
                                    } else if count > 0 {
                                        " · Notification count may be a lower bound"
                                    } else {
                                        ""
                                    }
                                ))
                                .on_disabled_hover_text(if state.can_view(channel.id) {
                                    format!("{} · {}", channel.name, kind_label(channel.kind))
                                } else { "This conversation is unavailable with current permission information".into() });
                            if count > 0 {
                                crate::notifications::badge(
                                    ui,
                                    response.rect.right_center() - egui::vec2(19.0, 0.0),
                                    count,
                                );
                            }
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    response.enabled(),
                                    format!(
                                        "{}{}; {} notifications",
                                        channel.name,
                                        if unread { ", unread" } else { "" },
                                        count
                                    ),
                                )
                            });
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
            last_message: None,
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
                    Row::Channel(c, _) | Row::Category(c, _) => c.id.0,
                    Row::Participant(entry) => entry.participant.user.0,
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
        let mut hierarchy = vec![
            channel(4, 4, 0, None),
            channel(7, 15, 0, Some(Id(4))),
            channel(8, 11, 1, Some(Id(7))),
            channel(9, 12, 0, Some(Id(7))),
            channel(10, 0, 1, Some(Id(4))),
            channel(11, 10, 0, Some(Id(10))),
            channel(12, 16, 2, Some(Id(4))),
            channel(13, 11, 0, Some(Id(12))),
            channel(20, 11, 0, Some(Id(999))), // missing parent
            channel(21, 11, 0, Some(Id(4))),   // category is not a thread parent
            channel(22, 11, 0, Some(Id(23))),  // thread-parent cycle
            channel(23, 12, 0, Some(Id(22))),
            channel(24, 11, 0, Some(Id(24))), // self parent
            channel(25, 11, 0, Some(Id(26))), // other guild
            channel(26, 0, 0, None),
            channel(27, 11, 0, Some(Id(28))), // parent/child source cycle
            channel(28, 0, 0, Some(Id(27))),
        ];
        hierarchy.iter_mut().find(|c| c.id == Id(26)).unwrap().guild = Some(Id(101));
        let expanded = rows(&hierarchy, Some(Id(100)), &BTreeSet::new(), None);
        assert_eq!(expanded.len(), hierarchy.len() - 1);
        assert_eq!(
            ids(expanded),
            [20, 21, 22, 23, 24, 25, 27, 28, 4, 7, 9, 8, 10, 11, 12, 13]
        );
        let collapsed = rows(
            &hierarchy,
            Some(Id(100)),
            &BTreeSet::from([Id(4)]),
            Some(Id(8)),
        );
        assert!(matches!(collapsed.last(), Some(Row::Channel(c, true)) if c.id == Id(8)));
        assert_eq!(ids(collapsed), [20, 21, 22, 23, 24, 25, 27, 28, 4, 7, 8]);
        assert!(!hierarchy[1].supports_text() && !hierarchy[6].supports_text());
        assert!(hierarchy[2].supports_text());
        assert_eq!(kind_label(16), "Media · loaded posts");
        assert!(!channels[1].supports_text());
        assert_eq!(kind_label(15), "Forum · loaded posts");
        let mut state = State {
            user: Some(model::User {
                id: Id(2),
                name: "Synthetic member".into(),
                avatar: None,
                discriminator: 0,
            }),
            guilds: vec![model::Guild {
                id: Id(100),
                name: "Synthetic guild".into(),
                icon: None,
                emojis: None,
            }],
            channels,
            demo: true,
            ..State::default()
        };
        state
            .permissions
            .replace(test_support::permission_snapshot(&state))
            .unwrap();
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
        // Forum containers never request history; their loaded posts remain keyboard-selectable.
        state.channels = vec![channel(7, 15, 0, None), channel(8, 11, 0, Some(Id(7)))];
        state
            .permissions
            .replace(test_support::permission_snapshot(&state))
            .unwrap();
        assert!(state.select(Id(7)).is_none());
        let ctx = egui::Context::default();
        let mut picked = None;
        for key in [egui::Key::Tab, egui::Key::Enter] {
            ctx.run_ui(
                egui::RawInput {
                    events: vec![egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    }],
                    ..Default::default()
                },
                |ui| {
                    picked = view.channel_list(ui, &state).or(picked);
                },
            )
            .drop_without_applying_deltas();
        }
        assert_eq!(picked, Some(Id(8)));
    }
}
