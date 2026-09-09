use crate::markdown::{FormatCache, external_url};
use client_core::State;
use egui::RichText;
use model::{Id, Message};
use std::{
    collections::BTreeMap,
    hash::{DefaultHasher, Hash, Hasher},
};

#[derive(Default)]
pub struct TimelineView {
    heights: BTreeMap<Id, (u64, f32)>,
    width: f32,
    rows: Vec<(Id, f32)>,
    revision: u64,
    channel: Option<Id>,
    anchor: Option<(Id, f32)>,
    following: bool,
    formatted: FormatCache,
    // Exact revealed content prevents a reload that resets model revisions from revealing edits.
    // Pruned with the active window: at most its 500 records / 4 MiB content budget.
    revealed: BTreeMap<Id, String>,
    opening: Option<String>,
    text_size: f32,
    scale: f32,
}
pub fn visible_range(rows: &[(Id, f32)], min: f32, max: f32) -> (usize, usize, f32) {
    let mut top = 0.0;
    let mut first = 0;
    while first < rows.len() && top + rows[first].1 < min {
        top += rows[first].1;
        first += 1;
    }
    let mut end = first;
    let mut bottom = top;
    while end < rows.len() && bottom < max {
        bottom += rows[end].1;
        end += 1;
    }
    (first, end, top)
}
fn anchor_offset(rows: &[(Id, f32)], id: Id, inset: f32) -> f32 {
    if rows.is_empty() {
        return 0.0;
    }
    // Keep the next surviving message at the top; fall back to the previous one at the end.
    let index = rows
        .partition_point(|(row, _)| *row < id)
        .min(rows.len() - 1);
    let within = if rows[index].0 == id {
        inset.clamp(0.0, rows[index].1.max(0.0))
    } else {
        0.0
    };
    rows[..index].iter().map(|(_, height)| *height).sum::<f32>() + within
}
fn layout_key(message: &Message) -> u64 {
    // A layout fingerprint only; spoiler visibility uses exact text instead.
    let mut key = DefaultHasher::new();
    message.content.hash(&mut key);
    message.author.name.hash(&mut key);
    message.edited.hash(&mut key);
    message.reply_to.hash(&mut key);
    message.unsupported.hash(&mut key);
    key.finish()
}
impl TimelineView {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut State,
        editing: &mut Option<(Id, Id, String)>,
        deleting: &mut Option<(Id, Id)>,
        avatars: &mut crate::avatars::Avatars,
        profile: &mut Option<model::User>,
    ) {
        let width = ui.available_width();
        if self.channel != state.selected {
            *self = Self {
                channel: state.selected,
                following: true,
                ..Self::default()
            };
        }
        let text_size = egui::TextStyle::Body.resolve(ui.style()).size;
        let scale = ui.ctx().pixels_per_point();
        let dimensions_changed =
            (self.width - width).abs() > 1.0 || self.text_size != text_size || self.scale != scale;
        let changed = self.revision != state.revision || dimensions_changed;
        let mut offset = None;
        if changed {
            if dimensions_changed {
                self.heights.clear();
            }
            self.width = width;
            self.text_size = text_size;
            self.scale = scale;
            self.heights
                .retain(|id, _| state.timeline.get(*id).is_some());
            self.formatted.retain(|id| state.timeline.get(id).is_some());
            self.revealed.retain(|id, content| {
                state
                    .timeline
                    .get(*id)
                    .is_some_and(|m| m.content == *content)
            });
            self.rows = state
                .timeline
                .iter()
                .map(|m| {
                    let estimate = 58.0
                        + 18.0
                            * (m.content
                                .lines()
                                .map(|line| {
                                    (line.chars().count() as f32 / ((width - 80.0) / 8.0).max(1.0))
                                        .ceil()
                                        .max(1.0)
                                })
                                .sum::<f32>())
                            .min(128.0);
                    let height = self
                        .heights
                        .get(&m.id)
                        .filter(|(key, _)| *key == layout_key(m))
                        .map_or(estimate, |(_, height)| *height);
                    (m.id, height)
                })
                .collect();
            self.revision = state.revision;
            if !self.following
                && let Some((id, inset)) = self.anchor
            {
                offset = Some(anchor_offset(&self.rows, id, inset));
            }
        }
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt(("timeline", state.selected))
            .auto_shrink([false, false])
            .stick_to_bottom(self.following);
        if let Some(offset) = offset {
            scroll = scroll.vertical_scroll_offset(offset);
        }
        let total: f32 = self.rows.iter().map(|(_, height)| height).sum();
        let mut measurements = Vec::new();
        let output = scroll.show_viewport(ui, |ui, viewport| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let (first, end, top) = visible_range(
                &self.rows,
                (viewport.min.y - 100.0).max(0.0),
                viewport.max.y + 100.0,
            );
            let (anchor, _, anchor_top) = visible_range(&self.rows, viewport.min.y, viewport.max.y);
            self.anchor = self
                .rows
                .get(anchor)
                .map(|(id, _)| (*id, viewport.min.y - anchor_top));
            ui.add_space(top);
            for (id, _) in &self.rows[first..end] {
                let Some(message) = state.timeline.get(*id) else {
                    continue;
                };
                let response = ui.push_id(id.0, |ui| {
                    let colors = crate::design::palette(ui);
                    egui::Frame::NONE
                        .inner_margin(egui::Margin::symmetric(16, 10))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(12.0, 4.0);
                            ui.horizontal_top(|ui| {
                                if avatars.show(ui, &message.author, 36.0, state.demo).clicked() { *profile = Some(message.author.clone()); }
                                ui.vertical(|ui| {
                                    ui.set_width(ui.available_width());
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let (menu, _) = egui::containers::menu::MenuButton::from_button(
                                                egui::Button::new(RichText::new("…").color(colors.muted))
                                                    .frame(false)
                                                    .min_size(egui::vec2(30.0, 26.0)),
                                            )
                                            .ui(ui, |ui| {
                                                ui.set_min_width(140.0);
                                                if ui.button("Copy message").clicked() {
                                                    ui.ctx().copy_text(message.content.clone());
                                                    ui.close();
                                                }
                                                if ui.button("Reply").clicked() {
                                                    state.reply = Some(*id);
                                                    ui.close();
                                                }
                                                if state.user.as_ref().map(|u| u.id)
                                                    == Some(message.author.id)
                                                {
                                                    ui.separator();
                                                    if ui.button("Edit message").clicked() {
                                                        *editing = Some((
                                                            message.channel,
                                                            *id,
                                                            message.content.clone(),
                                                        ));
                                                        ui.close();
                                                    }
                                                    if ui.button("Delete message…").clicked() {
                                                        *deleting = Some((message.channel, *id));
                                                        ui.close();
                                                    }
                                                }
                                            });
                                            menu.widget_info(|| {
                                                egui::WidgetInfo::labeled(
                                                    egui::WidgetType::Button,
                                                    ui.is_enabled(),
                                                    format!("Message actions for {}", message.author.name),
                                                )
                                            });
                                            menu.on_hover_text("Message actions");
                                            if message.edited {
                                                ui.label(RichText::new("edited").small().color(colors.muted));
                                            }
                                            ui.with_layout(
                                                egui::Layout::left_to_right(egui::Align::Center),
                                                |ui| {
                                                    ui.add(
                                                        egui::Label::new(
                                                            RichText::new(&message.author.name)
                                                                .strong()
                                                                .color(colors.text),
                                                        )
                                                        .truncate(),
                                                    );
                                                },
                                            );
                                        },
                                    );
                                    if message.reply_to.is_some() {
                                        ui.label(RichText::new("↳ Reply to an earlier message").small().color(colors.muted));
                                    }
                                    let formatted = self.formatted.get(*id, &message.content);
                                    if formatted.spoilers
                                        && self.revealed.get(id) != Some(&message.content)
                                    {
                                        if ui.button("Reveal spoiler").clicked() {
                                            self.revealed.insert(*id, message.content.clone());
                                        }
                                    } else {
                                        ui.add(
                                            egui::Label::new(formatted.layout(ui))
                                                .wrap()
                                                .selectable(true),
                                        );
                                        if formatted.limited {
                                            ui.label(RichText::new("Display limited · Copy message for the full text").small().color(colors.muted));
                                        }
                                        if !formatted.links.is_empty() {
                                            ui.horizontal_wrapped(|ui| {
                                                for (index, url) in formatted.links.iter().enumerate() {
                                                    if ui
                                                        .small_button(format!("Open link {}…", index + 1))
                                                        .on_hover_text(url)
                                                        .clicked()
                                                    {
                                                        self.opening = Some(url.clone());
                                                    }
                                                }
                                            });
                                        }
                                        if formatted.spoilers && ui.small_button("Hide spoiler").clicked() {
                                            self.revealed.remove(id);
                                        }
                                    }
                                    if message.unsupported {
                                        ui.label(RichText::new("Attachment, embed, or system content · Preview unavailable").small().color(colors.muted));
                                    }
                                });
                            });
                        });
                });
                measurements.push((*id, layout_key(message), response.response.rect.height()));
            }
            let used: f32 = self.rows[..end].iter().map(|(_, height)| *height).sum();
            ui.add_space((total - used).max(0.0));
        });
        self.following =
            output.state.offset.y + output.inner_rect.height() >= output.content_size.y - 3.0;
        let mut reflow = false;
        for (id, key, height) in measurements {
            if self
                .heights
                .get(&id)
                .is_none_or(|(old_key, old)| *old_key != key || (*old - height).abs() > 1.0)
            {
                self.heights.insert(id, (key, height));
                reflow = true;
            }
        }
        if reflow {
            self.revision = u64::MAX;
            ui.ctx().request_repaint();
        }
        if !self.following && ui.button("↓ Jump to latest loaded").clicked() {
            self.following = true;
            ui.ctx().request_repaint();
        }
        if let Some(url) = &self.opening {
            let mut close = false;
            egui::Window::new("Open external link?")
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.label("Open this destination in your default browser:");
                    ui.add(egui::Label::new(url).wrap().selectable(true));
                    ui.horizontal(|ui| {
                        if ui.button("Open in browser").clicked() {
                            if let Some(url) = external_url(url) {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                            }
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.opening = None;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_height_virtualization_visits_only_viewport() {
        let rows: Vec<_> = (1..=500)
            .map(|id| (Id(id), if id % 2 == 0 { 100.0 } else { 40.0 }))
            .collect();
        let (start, end, top) = visible_range(&rows, 1000.0, 1500.0);
        assert!(start > 0);
        assert!(end - start < 12);
        assert!(top <= 1000.0);
        assert_eq!(visible_range(&[], 0.0, 100.0), (0, 0, 0.0));
        let neighbors = [(Id(1), 40.0), (Id(3), 100.0), (Id(4), 60.0)];
        assert_eq!(anchor_offset(&neighbors, Id(2), 25.0), 40.0);
        assert_eq!(anchor_offset(&neighbors, Id(5), 25.0), 140.0);
        assert_eq!(anchor_offset(&neighbors, Id(3), 25.0), 65.0);
        assert_eq!(anchor_offset(&neighbors, Id(3), 200.0), 140.0);
        assert_eq!(anchor_offset(&[], Id(2), 25.0), 0.0);
    }
    #[test]
    fn same_id_revision_reset_does_not_reuse_reveal_or_height() {
        let mut message = Message {
            id: Id(1),
            channel: Id(2),
            author: model::User {
                id: Id(3),
                name: "Synthetic".into(),
                avatar: None,
                discriminator: 0,
            },
            content: "||old revealed content||".into(),
            edited: false,
            edited_at: None,
            revision: 0,
            nonce: None,
            reply_to: None,
            unsupported: false,
        };
        let mut view = TimelineView {
            channel: Some(Id(2)),
            ..Default::default()
        };
        view.revealed.insert(message.id, message.content.clone());
        view.heights
            .insert(message.id, (layout_key(&message), 4000.0));
        message.content = "||new concealed content||".into();
        let current_key = layout_key(&message);
        assert_ne!(view.heights[&message.id].0, current_key);
        let mut state = State {
            selected: Some(Id(2)),
            revision: 1,
            ..Default::default()
        };
        state.timeline.insert(message, false, false).unwrap();
        let context = egui::Context::default();
        // Match dimensions so this specifically exercises content invalidation, not resize.
        let output = context.run_ui(Default::default(), |ui| {
            view.width = ui.available_width();
            view.text_size = egui::TextStyle::Body.resolve(ui.style()).size;
            view.scale = ui.ctx().pixels_per_point();
            view.show(
                ui,
                &mut state,
                &mut None,
                &mut None,
                &mut crate::avatars::Avatars::default(),
                &mut None,
            );
        });
        output.drop_without_applying_deltas();
        assert!(view.revealed.is_empty());
        assert_eq!(view.heights[&Id(1)].0, current_key);
        assert!(
            view.heights[&Id(1)].1 < 160.0,
            "A short concealed message must keep a compact row even in an unbounded scroll layout: {}",
            view.heights[&Id(1)].1
        );
    }
}
