//! Search the bundled Unicode palette or the selected server's bounded catalog.
use crate::avatars::Avatars;
use client_core::State;
use model::Id;
use std::sync::OnceLock;

const NAMES: &str = include_str!("../../../assets/twemoji/names.tsv");
const CELL: f32 = 38.0;

/// Replace the composer's scalar-index selection without exceeding its character or RAM budget.
pub(crate) fn insert(
    draft: &mut String,
    text: &str,
    range: Option<egui::text::CCursorRange>,
    remaining: usize,
) -> Option<usize> {
    let count = draft.chars().count();
    let (start, end) = range.map_or((count, count), |range| {
        let range = range.as_sorted_char_range();
        (range.start.0.min(count), range.end.0.min(count))
    });
    let inserted = text.chars().count();
    if count - (end - start) + inserted > client_core::MAX_CONTENT {
        return None;
    }
    let byte_start = draft
        .char_indices()
        .nth(start)
        .map_or(draft.len(), |(i, _)| i);
    let byte_end = draft
        .char_indices()
        .nth(end)
        .map_or(draft.len(), |(i, _)| i);
    let bytes = draft.len() - (byte_end - byte_start) + text.len();
    let budget = draft.capacity().saturating_add(remaining);
    if bytes > budget {
        return None;
    }
    // draft_bytes measures capacity, so avoid String's geometric growth crossing the budget.
    if bytes > draft.capacity() {
        let mut replacement = String::with_capacity(bytes);
        if replacement.capacity() > budget {
            return None;
        }
        replacement.push_str(&draft[..byte_start]);
        replacement.push_str(text);
        replacement.push_str(&draft[byte_end..]);
        *draft = replacement;
    } else {
        draft.replace_range(byte_start..byte_end, text);
    }
    Some(start + inserted)
}

fn standard() -> &'static [(&'static str, &'static str)] {
    static ENTRIES: OnceLock<Vec<(&'static str, &'static str)>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        NAMES
            .lines()
            .map(|line| line.split_once('\t').expect("bundled emoji name"))
            .collect()
    })
}

pub(crate) struct Picker {
    open: bool,
    focus: bool,
    channel: Option<Id>,
    generation: u64,
    server: bool,
    query: String,
    matches: Vec<usize>,
}

impl Default for Picker {
    fn default() -> Self {
        // Initialize the static catalog during application creation, outside rendering.
        Self {
            open: false,
            focus: false,
            channel: None,
            generation: 0,
            server: false,
            query: String::new(),
            matches: (0..standard().len()).collect(),
        }
    }
}

impl Picker {
    fn filter(&mut self) {
        let query = self.query.trim().to_lowercase();
        self.matches.clear();
        self.matches.extend(
            standard()
                .iter()
                .enumerate()
                .filter(|(_, (text, name))| name.contains(&query) || text.contains(&query))
                .map(|(index, _)| index),
        );
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        channel: Id,
        avatars: &mut Avatars,
    ) -> Option<String> {
        if self.channel != Some(channel) || self.generation != state.generation {
            self.channel = Some(channel);
            self.generation = state.generation;
            self.open = false;
            self.server = false;
            self.query.clear();
            self.filter();
        }
        let trigger = ui.button("Emoji").on_hover_text("Insert an emoji");
        if trigger.clicked() {
            self.open = !self.open;
            self.focus = self.open;
        }
        if !self.open {
            return None;
        }
        if ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.open = false;
            trigger.request_focus();
            return None;
        }
        let guild_id = state
            .channels
            .iter()
            .find(|c| c.id == channel)
            .and_then(|c| c.guild);
        let guild = guild_id.and_then(|id| state.guilds.iter().find(|g| g.id == id));
        let mut open = true;
        let mut selected = None;
        egui::Window::new("Emoji picker")
            .id(egui::Id::new("emoji-picker"))
            .open(&mut open)
            .collapsible(false)
            .default_width(360.0)
            .min_width(260.0)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                let search = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .char_limit(64)
                        .hint_text("Search emoji by name")
                        .desired_width(f32::INFINITY),
                );
                search.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, true, "Search emoji by name")
                });
                if self.focus {
                    search.request_focus();
                    self.focus = false;
                }
                if search.changed() {
                    self.filter();
                }
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.server, false, "Unicode");
                    ui.add_enabled_ui(guild_id.is_some(), |ui| {
                        ui.selectable_value(&mut self.server, true, "This server");
                    })
                    .response
                    .on_hover_text("Custom emoji from the current server; cross-server use is not verified.");
                });
                ui.separator();
                let columns = ((ui.available_width() + ui.spacing().item_spacing.x)
                    / (CELL + ui.spacing().item_spacing.x))
                    .floor()
                    .clamp(1.0, 10.0) as usize;
                if self.server {
                    let Some(emojis) = guild.and_then(|g| g.emojis.as_deref()) else {
                        ui.weak("This server's emoji catalog is not available yet.");
                        return;
                    };
                    let query = self.query.trim().to_lowercase();
                    let matching: Vec<_> = emojis.iter().filter(|emoji| {
                        emoji.name.to_lowercase().contains(&query)
                    }).collect();
                    ui.weak("Restricted or unavailable emoji are disabled. Animated emoji use a still preview.");
                    if matching.is_empty() {
                        ui.weak(if emojis.is_empty() { "This server has no custom emoji." } else { "No matching emoji." });
                    }
                    egui::ScrollArea::vertical()
                        .id_salt(("server-emoji", channel, &self.query))
                        .max_height(260.0)
                        .show_rows(ui, CELL, matching.len().div_ceil(columns), |ui, rows| {
                            for row in rows {
                                ui.horizontal(|ui| {
                                    for emoji in matching.iter().skip(row * columns).take(columns) {
                                        let button = if let Some(image) = avatars.custom_image(ui.ctx(), emoji.id, 26.0, state.demo) {
                                            egui::Button::image(image.alt_text(&emoji.name))
                                        } else {
                                            egui::Button::new(&emoji.name).truncate()
                                        };
                                        let response = ui.add_enabled_ui(emoji.usable(), |ui| ui.add_sized([CELL, CELL], button)).inner;
                                        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, emoji.usable(), &emoji.name));
                                        let clicked = response.clicked();
                                        response.on_hover_text(&emoji.name).on_disabled_hover_text(format!("{} — unavailable or permission not verified", emoji.name));
                                        if clicked && emoji.usable() {
                                            selected = Some(emoji.markup());
                                        }
                                    }
                                });
                            }
                        });
                } else {
                    if self.matches.is_empty() {
                        ui.weak("No matching emoji.");
                    }
                    egui::ScrollArea::vertical()
                        .id_salt(("unicode-emoji", channel, &self.query))
                        .max_height(260.0)
                        .show_rows(ui, CELL, self.matches.len().div_ceil(columns), |ui, rows| {
                            for row in rows {
                                ui.horizontal(|ui| {
                                    for &index in self.matches.iter().skip(row * columns).take(columns) {
                                        let (text, name) = standard()[index];
                                        let button = if let Some(image) = crate::emoji::image(ui.ctx(), text, 26.0) {
                                            egui::Button::image(image.alt_text(name))
                                        } else {
                                            egui::Button::new(text)
                                        };
                                        let response = ui.add_sized([CELL, CELL], button);
                                        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
                                        if response.on_hover_text(name).clicked() {
                                            selected = Some(text.to_owned());
                                        }
                                    }
                                });
                            }
                        });
                }
            });
        self.open = open && selected.is_none();
        if !open && selected.is_none() {
            trigger.request_focus();
        }
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insertion_replaces_unicode_selection_and_respects_character_and_capacity_budgets() {
        use egui::text::{CCursor, CCursorRange};
        let mut draft = "前👩🏽‍💻後".to_owned();
        let selection = Some(CCursorRange::two(CCursor::new(5), CCursor::new(1)));
        assert_eq!(insert(&mut draft, "❤️", selection, 0), Some(3));
        assert_eq!(draft, "前❤️後");
        let markup = "<a:party_blob:123456789>";
        assert_eq!(
            insert(&mut draft, markup, None, 100),
            Some(4 + markup.len())
        );
        assert_eq!(draft, format!("前❤️後{markup}"));
        let end = draft.chars().count();
        assert_eq!(
            insert(
                &mut draft,
                "😀",
                Some(CCursorRange::one(CCursor::new(usize::MAX))),
                100
            ),
            Some(end + 1)
        );
        assert!(draft.ends_with("😀"));

        let mut draft = "a".repeat(client_core::MAX_CONTENT);
        assert_eq!(insert(&mut draft, "😀", None, 100), None);
        assert_eq!(draft.len(), client_core::MAX_CONTENT);
        let mut draft = String::new();
        assert_eq!(insert(&mut draft, "😀", None, 3), None);
        assert!(draft.is_empty());
        assert_eq!(insert(&mut draft, "😀", None, 4), Some(1));
        assert_eq!(draft.capacity(), 4);
        let selection = Some(CCursorRange::two(CCursor::new(0), CCursor::new(1)));
        assert_eq!(insert(&mut draft, "👍", selection, 0), Some(1));
        assert_eq!(draft, "👍");
        assert_eq!(draft.capacity(), 4);
    }

    #[test]
    fn palette_search_preserves_complete_sequences_and_is_bounded() {
        let atlas = include_str!("../../../assets/twemoji/index.tsv");
        let atlas: std::collections::BTreeSet<_> = atlas
            .lines()
            .map(|l| l.split_once('\t').unwrap().0)
            .collect();
        assert_eq!(standard().len(), 3953);
        assert!(NAMES.len() < 300_000);
        for (text, name) in standard() {
            assert!(atlas.contains(text.replace('\u{fe0f}', "").as_str()));
            assert!(!name.is_empty());
        }
        let mut picker = Picker {
            query: "WOMAN TECHNOLOGIST".into(),
            ..Default::default()
        };
        picker.filter();
        assert!(picker.matches.iter().any(|&i| standard()[i].0 == "👩🏽‍💻"));
        picker.query = "❤️".into();
        picker.filter();
        assert!(picker.matches.iter().any(|&i| standard()[i].0 == "❤️"));
        picker.query = "not an emoji name".into();
        picker.filter();
        assert!(picker.matches.is_empty());
    }

    #[test]
    fn server_grid_only_requests_visible_images_and_resets_on_navigation() {
        let state = State {
            guilds: vec![model::Guild {
                id: Id(1),
                name: "Synthetic server".into(),
                icon: None,
                emojis: Some(
                    (1..=1000)
                        .map(|id| model::CustomEmoji {
                            id: Id(id),
                            name: format!("emoji_{id}"),
                            animated: false,
                            available: true,
                            managed: false,
                            roles: Some(vec![]),
                        })
                        .collect(),
                ),
            }],
            channels: vec![model::Channel {
                id: Id(2),
                guild: Some(Id(1)),
                parent_id: None,
                position: 0,
                name: "Synthetic channel".into(),
                kind: 0,
                recipients: vec![],
                member_list_id: None,
                last_message: None,
            }],
            ..State::default()
        };
        let mut picker = Picker {
            open: true,
            server: true,
            channel: Some(Id(2)),
            generation: state.generation,
            ..Picker::default()
        };
        let mut avatars = Avatars::default();
        let ctx = egui::Context::default();
        for _ in 0..3 {
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 480.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    assert!(picker.show(ui, &state, Id(2), &mut avatars).is_none());
                },
            );
            output.textures_delta.clear();
        }
        let requests = avatars.take_requests();
        assert!(!requests.is_empty());
        assert!(
            requests.len() < 100,
            "offscreen emoji must not queue image requests"
        );
        picker.query = "old query".into();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(640.0, 480.0),
                )),
                ..Default::default()
            },
            |ui| {
                assert!(picker.show(ui, &state, Id(3), &mut avatars).is_none());
            },
        );
        output.textures_delta.clear();
        assert!(!picker.open && !picker.server && picker.query.is_empty());
    }
}
