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
    pub(super) channel_reference: Option<Id>,
    channel_labels: u64,
    pub(super) mark_read: Option<Id>,
    auto_read_attempt: Option<Id>,
    at_current_latest: bool,
    pub(super) reaction: Option<(Id, Option<model::ReactionEmoji>)>,
    toolbar: Option<(Id, egui::Rect)>,
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
    revealed: BTreeMap<Id, (String, Vec<model::Embed>, Vec<model::Attachment>)>,
    viewing: Option<(Id, Id)>,
    pub(super) download: crate::attachments::DownloadUi,
    opening: Option<String>,
    text_size: f32,
    scale: f32,
    pub(super) load_older: bool,
    pub(super) latest: bool,
    jump: bool,
    unread_boundary: Option<Id>,
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
    message.reactions.hash(&mut key);
    for user in &message.mentions {
        user.id.hash(&mut key);
        user.name.hash(&mut key);
    }
    message.author.name.hash(&mut key);
    message.edited.hash(&mut key);
    message.reply_to.hash(&mut key);
    message.unsupported.hash(&mut key);
    message.kind.hash(&mut key);
    message.attachments.hash(&mut key);
    message.embeds.hash(&mut key);
    message.embeds_suppressed.hash(&mut key);
    key.finish()
}
// Discord snowflakes carry milliseconds since 2015-01-01. All u64 IDs fit time's range.
fn timestamp(id: Id) -> time::OffsetDateTime {
    time::OffsetDateTime::from_unix_timestamp(((id.0 >> 22) / 1000) as i64 + 1_420_070_400)
        .expect("snowflake timestamp is in range")
}
fn grouped(previous: Option<&Message>, message: &Message, boundary: Option<Id>) -> bool {
    previous.is_some_and(|previous| {
        previous.author.id == message.author.id
            && message.reply_to.is_none()
            && !message.unsupported
            && !previous.unsupported
            && boundary != Some(message.id)
            && timestamp(previous.id).date() == timestamp(message.id).date()
            && (timestamp(message.id) - timestamp(previous.id)).whole_seconds() < 300
    })
}
fn row_key(message: &Message, previous: Option<&Message>, boundary: Option<Id>) -> u64 {
    let mut key = DefaultHasher::new();
    layout_key(message).hash(&mut key);
    grouped(previous, message, boundary).hash(&mut key);
    previous
        .is_none_or(|p| timestamp(p.id).date() != timestamp(message.id).date())
        .hash(&mut key);
    (boundary == Some(message.id)).hash(&mut key);
    key.finish()
}
fn divider(ui: &mut egui::Ui, label: String, unread: bool) {
    let colors = crate::design::palette(ui);
    let color = if unread { colors.accent } else { colors.muted };
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        let font = egui::TextStyle::Small.resolve(ui.style());
        let text = ui.painter().layout_no_wrap(label.clone(), font, color);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::hover());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Label, ui.is_enabled(), &label)
        });
        let gap = (rect.width() - text.size().x - 24.0).max(0.0) / 2.0;
        for (a, b) in [
            (rect.left(), rect.left() + gap),
            (rect.right() - gap, rect.right()),
        ] {
            ui.painter().line_segment(
                [
                    egui::pos2(a, rect.center().y),
                    egui::pos2(b, rect.center().y),
                ],
                egui::Stroke::new(1.0, if unread { color } else { colors.border }),
            );
        }
        ui.painter().galley(
            egui::pos2(
                rect.center().x - text.size().x / 2.0,
                rect.center().y - text.size().y / 2.0,
            ),
            text,
            color,
        );
    });
    ui.add_space(8.0);
}
fn action_button(ui: &mut egui::Ui, icon: &str, label: &str) -> egui::Response {
    let response = ui.add(
        egui::Button::new(RichText::new(if icon == "✎" { "" } else { icon }).size(18.0))
            .frame(false)
            .min_size(egui::vec2(28.0, 28.0)),
    );
    if icon == "✎" {
        let center = response.rect.center();
        ui.painter().add(egui::Shape::closed_line(
            [
                (-6.0, 6.0),
                (-5.0, 1.0),
                (3.0, -7.0),
                (7.0, -3.0),
                (-1.0, 5.0),
            ]
            .map(|(x, y)| center + egui::vec2(x, y))
            .to_vec(),
            ui.style().interact(&response).fg_stroke,
        ));
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), label)
    });
    response.on_hover_text(label)
}
fn message_actions(
    ui: &mut egui::Ui,
    message: &Message,
    own: bool,
    mark_read: Option<&mut Option<Id>>,
    reply: &mut Option<Id>,
    editing: &mut Option<(Id, Id, String)>,
    deleting: &mut Option<(Id, Id)>,
) {
    let (menu, _) = egui::containers::menu::MenuButton::from_button(
        egui::Button::new(RichText::new("…").color(crate::design::palette(ui).muted))
            .frame(false)
            .small()
            .min_size(egui::vec2(28.0, 28.0)),
    )
    .ui(ui, |ui| {
        ui.set_min_width(140.0);
        if ui.button("Copy message").clicked() {
            ui.ctx().copy_text(message.display_text().into_owned());
            ui.close();
        }
        if ui.button("Reply").clicked() {
            *reply = Some(message.id);
            ui.close();
        }
        if ui
            .add_enabled(
                mark_read.is_some(),
                egui::Button::new("Mark read through here"),
            )
            .clicked()
        {
            if let Some(mark_read) = mark_read {
                *mark_read = Some(message.id);
            }
            ui.close();
        }
        if own {
            ui.separator();
            if ui
                .add_enabled(!message.unsupported, egui::Button::new("Edit message"))
                .clicked()
            {
                *editing = Some((message.channel, message.id, message.content.clone()));
                ui.close();
            }
            if ui.button("Delete message…").clicked() {
                *deleting = Some((message.channel, message.id));
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
}
impl TimelineView {
    pub(super) fn viewing_latest(&self, channel: Id) -> bool {
        self.channel == Some(channel) && self.following && self.at_current_latest
    }
    pub(super) fn follow_latest(&mut self) {
        self.following = true;
        self.jump = true;
        self.anchor = None;
    }
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
        let channel_changed = self.channel != state.selected;
        if channel_changed {
            *self = Self {
                channel: state.selected,
                following: true,
                download: std::mem::take(&mut self.download),
                jump: true,
                ..Self::default()
            };
        }
        let boundary = state
            .selected
            .and_then(|channel| state.read_marker(channel))
            .and_then(|read| {
                state
                    .timeline
                    .iter()
                    .find(|m| read.is_none_or(|id| m.id > id))
                    .map(|m| m.id)
            });
        if self.unread_boundary != boundary {
            self.unread_boundary = boundary;
            self.revision = u64::MAX;
        }
        let text_size = egui::TextStyle::Body.resolve(ui.style()).size;
        let scale = ui.ctx().pixels_per_point();
        let mut labels_changed = false;
        if self.revision != state.revision {
            // ponytail: hash bounded channel labels per state update; use a dedicated
            // navigation revision only if profiling shows this scan is significant.
            let mut labels = DefaultHasher::new();
            for channel in state
                .channels
                .iter()
                .filter(|c| c.guild.is_some() && c.supports_text())
            {
                channel.id.hash(&mut labels);
                channel.guild.hash(&mut labels);
                channel.name.hash(&mut labels);
            }
            let labels = labels.finish();
            labels_changed = self.channel_labels != labels;
            self.channel_labels = labels;
        }
        let dimensions_changed = (self.width - width).abs() > 1.0
            || self.text_size != text_size
            || self.scale != scale
            || labels_changed;
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
                state.timeline.get(*id).is_some_and(|m| {
                    m.content == content.0 && m.embeds == content.1 && m.attachments == content.2
                })
            });
            let mut previous = None;
            self.rows = state
                .timeline
                .iter()
                .map(|m| {
                    let key = row_key(m, previous, self.unread_boundary);
                    previous = Some(m);
                    let estimate = (if m.embeds_suppressed {
                        0.0
                    } else {
                        crate::embeds::estimated_height(&m.embeds)
                    }) + crate::attachments::estimated_height(&m.attachments)
                        + 58.0
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
                        .filter(|(old_key, _)| *old_key == key)
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
        if state.timeline.is_empty() {
            ui.label(match state.freshness {
                model::Freshness::Loading => "Loading messages…",
                model::Freshness::Unavailable => "You cannot view this conversation.",
                model::Freshness::Stale => "History is not available yet. Use Reload to try again.",
                model::Freshness::Fresh => "No messages yet. Start the conversation below.",
            });
        }
        if !state.history_pending
            && let Some(target) = state.search_target.take()
        {
            if state.timeline.get(target).is_some() {
                self.following = false;
                self.jump = false;
                self.anchor = Some((target, 0.0));
                offset = Some(anchor_offset(&self.rows, target, 0.0));
            } else {
                state.status = "Search message was not returned; it may have been removed";
            }
        }
        let mut scroll = egui::ScrollArea::vertical()
            .max_height((ui.available_height() - 36.0).max(0.0))
            .id_salt(("timeline", state.selected))
            .auto_shrink([false, false])
            .stick_to_bottom(self.following);
        let total: f32 = self.rows.iter().map(|(_, height)| height).sum();
        if std::mem::take(&mut self.jump) {
            offset = Some(total);
        }
        if let Some(offset) = offset {
            scroll = scroll.vertical_scroll_offset(offset);
        }
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
            let keyboard_focus = ui.memory(|m| m.focused()).and_then(|id| ui.ctx().read_response(id));
            let retained_toolbar = self.toolbar.filter(|(_, rect)| egui::Popup::is_any_open(ui.ctx()) || keyboard_focus.as_ref().is_some_and(|r| rect.contains_rect(r.rect)));
            for index in first..end {
                let (id, _) = &self.rows[index];
                let can_mark_read = state.can_mark_read(*id);
                let Some(message) = state.timeline.get(*id) else {
                    continue;
                };
                let previous = index.checked_sub(1).and_then(|i| state.timeline.get(self.rows[i].0));
                let compact = grouped(previous, message, self.unread_boundary);
                let new_day = previous.is_none_or(|p| timestamp(p.id).date() != timestamp(*id).date());
                let response = ui.push_id(id.0, |ui| {
                    if new_day {
                        let date = timestamp(*id);
                        divider(ui, format!("{} {}, {} · UTC", date.month(), date.day(), date.year()), false);
                    }
                    if self.unread_boundary == Some(*id) {
                        divider(ui, "New messages".into(), true);
                    }
                    let colors = crate::design::palette(ui);
                    let background = ui.painter().add(egui::Shape::Noop);
                    let mut time_rect = None;
                    let row = egui::Frame::NONE
                        .inner_margin(egui::Margin { left: 8, right: 8, top: if compact { 2 } else { 12 }, bottom: 2 })
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(12.0, 4.0);
                            ui.horizontal_top(|ui| {
                                if compact {
                                    time_rect = Some(ui.allocate_exact_size(egui::vec2(36.0, 24.0), egui::Sense::hover()).0);
                                } else if avatars.show(ui, &message.author, 36.0, state.demo).clicked() { *profile = Some(message.author.clone()); }
                                ui.vertical(|ui| {
                                    ui.set_width(ui.available_width());
                                    if !compact {
                                        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                            ui.add(egui::Label::new(RichText::new(&message.author.name).strong().color(colors.text)).truncate());
                                            let time = timestamp(*id);
                                            ui.label(RichText::new(format!("{:02}:{:02}", time.hour(), time.minute())).size(11.0).color(colors.muted)).on_hover_text(format!("{} UTC", time));
                                        });
                                    }
                                    if let Some(reply) = message.reply_to {
                                        // Reuse only loaded content; never fetch a thread while painting.
                                        let preview = state.timeline.get(reply).map_or_else(
                                            || "↳ Earlier message · outside loaded history".into(),
                                            |m| if crate::embeds::has_spoilers(m) { format!("↳ {} · Spoiler", m.author.name) } else { format!("↳ {}: {}", m.author.name, m.display_text().chars().take(120).collect::<String>().replace('\n', " ")) },
                                        );
                                        ui.add(egui::Label::new(RichText::new(preview).small().color(colors.muted)).truncate());
                                    }
                                    if let Some(summary) = message.system_summary() {
                                        ui.label(RichText::new(summary).color(colors.muted));
                                    }
                                    let formatted = self.formatted.get(*id, &message.content);
                                    let spoilers = formatted.spoilers || crate::embeds::has_spoilers(message);
                                    if spoilers
                                        && !self.revealed.get(id).is_some_and(|(content, embeds, attachments)| content == &message.content && embeds == &message.embeds && attachments == &message.attachments)
                                    {
                                        if ui.button("Reveal spoiler").clicked() {
                                            self.revealed.insert(*id, (message.content.clone(), message.embeds.clone(), message.attachments.clone()));
                                        }
                                    } else {
                                        formatted.show_references(ui, &mut self.opening, &message.mentions, profile, (&state.channels, &mut self.channel_reference), (avatars, state.demo));
                                        if formatted.limited {
                                            ui.label(RichText::new("Display limited · Copy message for the full text").small().color(colors.muted));
                                        }
                                        crate::embeds::show(ui, message, &mut self.formatted, avatars, &mut self.opening, profile, state.demo);
                                        crate::attachments::show(ui, message, avatars, &mut self.viewing, &mut self.opening, &mut self.download, state.demo);
                                        if spoilers && ui.small_button("Hide spoiler").clicked() {
                                            self.revealed.remove(id);
                                        }
                                    }
                                    if message.edited {
                                        ui.label(RichText::new("(edited)").small().color(colors.muted));
                                    }
                                    if message.unsupported && message.system_summary().is_none() {
                                        ui.label(RichText::new(format!("Unsupported message type {} · Preview unavailable", message.kind)).small().color(colors.muted));
                                    }
                                    if let Some(action)=crate::reactions::show(ui,message.reactions.as_deref(),
                                        state.gateway_connected && state.freshness==model::Freshness::Fresh,
                                        state.reactions.writing.is_some(), state.reactions.invalidated(message.id), (avatars, state.demo)) {
                                        self.reaction=Some((*id,action));
                                    }
                                });
                            });
                        });
                    let rect = row.response.rect;
                    let focus = ui.interact(rect, ui.id().with("message-focus"), egui::Sense::focusable_noninteractive());
                    focus.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, format!("Message by {}. Tab for actions.", message.author.name)));
                    let retained = retained_toolbar.is_some_and(|(active, _)| active == *id);
                    let hovered = ui.rect_contains_pointer(rect) && !egui::Popup::is_any_open(ui.ctx()) && retained_toolbar.is_none_or(|(active, _)| active == *id);
                    if hovered || focus.has_focus() || keyboard_focus.as_ref().is_some_and(|r| r.id == focus.id) || retained {
                        ui.painter().set(background, egui::Shape::rect_filled(rect, 0.0, colors.surface));
                        if let Some(rect) = time_rect {
                            let time = timestamp(*id);
                            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, format!("{:02}:{:02}", time.hour(), time.minute()), egui::FontId::proportional(10.0), colors.muted);
                            ui.interact(rect, ui.id().with("timestamp"), egui::Sense::hover()).on_hover_text(format!("{} UTC", time));
                        }
                        let own = state.user.as_ref().is_some_and(|u| u.id == message.author.id);
                        let toolbar_rect = egui::Rect::from_min_size(egui::pos2(rect.right() - if own { 128.0 } else { 98.0 }, rect.top()), egui::vec2(if own { 120.0 } else { 90.0 }, 28.0));
                        // A child overlay keeps hover from changing wrapping or cached row heights.
                        let mut toolbar = ui.new_child(egui::UiBuilder::new().id_salt("hover-actions").max_rect(toolbar_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
                        toolbar.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
                        toolbar.spacing_mut().button_padding = egui::vec2(4.0, 2.0);
                        toolbar.spacing_mut().interact_size.y = 28.0;
                        toolbar.painter().rect_filled(toolbar_rect, 5.0, colors.raised);
                        if let Some(action) = crate::reactions::add_button(&mut toolbar, state.gateway_connected && state.freshness == model::Freshness::Fresh, state.reactions.writing.is_some()) {
                            self.reaction = Some((*id, action));
                        }
                        if action_button(&mut toolbar, "↩", "Reply").clicked() { state.reply = Some(*id); }
                        if own && !message.unsupported && action_button(&mut toolbar, "✎", "Edit message").clicked() { *editing = Some((message.channel, *id, message.content.clone())); }
                        message_actions(&mut toolbar, message, own, can_mark_read.then_some(&mut self.mark_read), &mut state.reply, editing, deleting);
                        self.toolbar = Some((*id, toolbar_rect));
                    }
                });
                measurements.push((*id, row_key(message, previous, self.unread_boundary), response.response.rect.height()));
            }
            let used: f32 = self.rows[..end].iter().map(|(_, height)| *height).sum();
            ui.add_space((total - used).max(0.0));
        });
        self.following =
            output.state.offset.y + output.inner_rect.height() >= output.content_size.y - 3.0;
        self.at_current_latest = state.timeline.iter().last().is_some_and(|message| {
            state.channels.iter().any(|channel| {
                Some(channel.id) == state.selected && channel.last_message == Some(message.id)
            })
        });
        if self.following
            && ui.input(|i| i.focused)
            && let Some(message) = state.timeline.iter().last()
            && self.auto_read_attempt != Some(message.id)
            && self.at_current_latest
            && state.can_mark_read(message.id)
        {
            // One automatic attempt per viewed latest message; failed ACKs remain manually retryable.
            self.auto_read_attempt = Some(message.id);
            self.mark_read = Some(message.id);
        }
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
        // A user scroll near the top requests one page; a short initial view never drains history.
        self.load_older = !self.following
            && output.state.offset.y < 160.0
            && ui.input(|i| {
                i.smooth_scroll_delta().y > 0.0
                    && i.pointer
                        .hover_pos()
                        .is_some_and(|pos| output.inner_rect.contains(pos))
            })
            && state.can_load_older();
        if (!self.following || state.history_before.is_some())
            && ui
                .button(
                    if state
                        .selected
                        .is_some_and(|channel| state.unread(channel) == Some(true))
                    {
                        "↓ New messages · Jump to latest"
                    } else {
                        "↓ Jump to latest"
                    },
                )
                .clicked()
        {
            if state.history_before.is_some() {
                self.latest = true;
            }
            self.follow_latest();
            ui.ctx().request_repaint();
        }
        if let Some((message_id, attachment_id)) = self.viewing {
            let attachment = state
                .timeline
                .get(message_id)
                .filter(|m| {
                    !crate::embeds::has_spoilers(m)
                        || self
                            .revealed
                            .get(&m.id)
                            .is_some_and(|(content, embeds, attachments)| {
                                content == &m.content
                                    && embeds == &m.embeds
                                    && attachments == &m.attachments
                            })
                })
                .and_then(|m| {
                    m.attachments
                        .iter()
                        .find(|a| a.id == attachment_id && a.is_image())
                });
            if attachment.is_none_or(|a| {
                !crate::attachments::viewer(ui, a, avatars, &mut self.download, state.demo)
            }) {
                self.viewing = None;
            }
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
    fn text_message(id: u64) -> Message {
        Message {
            id: Id(id),
            channel: Id(20),
            author: model::User {
                id: Id(2),
                name: "Robin".into(),
                avatar: None,
                discriminator: 0,
            },
            content: "Synthetic text with enough words to wrap in a narrow viewport.".into(),
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
            reactions: Some(vec![]),
            embeds_suppressed: false,
        }
    }
    #[test]
    fn system_events_render_wrap_and_keep_unknown_fallbacks() {
        fn text(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| text(s, out)),
                _ => {}
            }
        }
        for (width, dark) in [(900.0, true), (280.0, false)] {
            let ctx = egui::Context::default();
            crate::design::apply(&ctx);
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            let mut state = State {
                selected: Some(Id(20)),
                demo: true,
                ..Default::default()
            };
            for (id, kind, content) in [(1, 7, ""), (2, 4, "new channel name"), (3, 222, "")] {
                let mut message = text_message(id);
                let old_key = layout_key(&message);
                message.kind = kind;
                assert_ne!(old_key, layout_key(&message));
                message.unsupported = true;
                message.content = content.into();
                assert!(!grouped(Some(&message), &message, None));
                state.timeline.insert(message, false, false).unwrap();
            }
            let mut view = TimelineView::default();
            let mut avatars = crate::avatars::Avatars::default();
            let mut painted = vec![];
            for _ in 0..5 {
                painted.clear();
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        view.show(
                            ui,
                            &mut state,
                            &mut None,
                            &mut None,
                            &mut avatars,
                            &mut None,
                        )
                    },
                );
                for shape in &output.shapes {
                    text(&shape.shape, &mut painted);
                }
                output.drop_without_applying_deltas();
            }
            assert!(
                painted
                    .iter()
                    .any(|s| s == "Welcome, Robin! Joined the server.")
            );
            assert!(
                painted
                    .iter()
                    .any(|s| s == "Robin changed the channel name.")
            );
            assert!(painted.iter().any(|s| s.contains("new channel name")));
            assert_eq!(
                painted
                    .iter()
                    .filter(|s| s.contains("Preview unavailable"))
                    .count(),
                1
            );
            assert!(
                painted
                    .iter()
                    .any(|s| s.contains("Unsupported message type 222"))
            );
        }
    }
    #[test]
    fn grouping_respects_dates_replies_unread_and_five_minute_gaps() {
        let mut first = text_message(1);
        let mut next = text_message((60_000 << 22) | 1);
        assert_eq!(timestamp(first.id).date().to_string(), "2015-01-01");
        assert!(grouped(Some(&first), &next, None));
        next.edited = true;
        assert!(grouped(Some(&first), &next, None));
        next.edited = false;
        assert!(!grouped(Some(&first), &next, Some(next.id)));
        assert_ne!(
            row_key(&next, Some(&first), None),
            row_key(&next, None, None)
        );
        next.reply_to = Some(first.id);
        assert!(!grouped(Some(&first), &next, None));
        next.reply_to = None;
        next.id = Id(300_000 << 22);
        assert!(!grouped(Some(&first), &next, None));
        first.id = Id(86_340_000 << 22);
        next.id = Id(86_400_000 << 22);
        assert!(!grouped(Some(&first), &next, None));
        assert_eq!(timestamp(Id(u64::MAX)).year(), 2154);
    }
    #[test]
    fn hover_actions_keep_layout_stable_and_support_keyboard_reply() {
        fn texts(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((
                    t.galley.job.text.clone(),
                    t.galley.rect.translate(t.pos.to_vec2()),
                )),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        texts(shape, out);
                    }
                }
                _ => {}
            }
        }
        for (width, dark) in [(900.0, true), (360.0, false)] {
            let ctx = egui::Context::default();
            crate::design::apply(&ctx);
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            let mut state = State {
                selected: Some(Id(20)),
                demo: true,
                gateway_connected: true,
                freshness: model::Freshness::Fresh,
                ..Default::default()
            };
            let first = text_message(1);
            state.user = Some(first.author.clone());
            state.timeline.insert(first, false, false).unwrap();
            let mut second = text_message(60_000 << 22);
            second.content = "Grouped continuation".into();
            second.edited = true;
            state.timeline.insert(second, false, false).unwrap();
            let mut view = TimelineView::default();
            let mut avatars = crate::avatars::Avatars::default();
            let mut editing = None;
            let mut render =
                |view: &mut TimelineView, state: &mut State, events: Vec<egui::Event>| {
                    let output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 600.0),
                            )),
                            events,
                            ..Default::default()
                        },
                        |ui| view.show(ui, state, &mut editing, &mut None, &mut avatars, &mut None),
                    );
                    let mut painted = vec![];
                    for shape in &output.shapes {
                        texts(&shape.shape, &mut painted);
                    }
                    output.drop_without_applying_deltas();
                    painted
                };
            for _ in 0..5 {
                render(&mut view, &mut state, vec![]);
            }
            let idle = render(&mut view, &mut state, vec![]);
            assert!(
                !idle
                    .iter()
                    .any(|(t, _)| t == "00:01" || t == "↩" || t == "✎")
            );
            assert!(idle.iter().any(|(t, _)| t == "(edited)"));
            let row = idle
                .iter()
                .find(|(t, _)| t.contains("Grouped continuation"))
                .unwrap()
                .1;
            let heights = view.heights.clone();
            let hovered = render(
                &mut view,
                &mut state,
                vec![egui::Event::PointerMoved(row.center())],
            );
            assert!(hovered.iter().any(|(t, _)| t == "00:01"));
            assert!(hovered.iter().any(|(t, _)| t == "↩"));
            assert_eq!(view.toolbar.unwrap().1.width(), 120.0);
            assert_eq!(view.heights, heights);
            let point = view.toolbar.unwrap().1.left_top() + egui::vec2(44.0, 14.0);
            for pressed in [true, false] {
                render(
                    &mut view,
                    &mut state,
                    vec![
                        egui::Event::PointerMoved(point),
                        egui::Event::PointerButton {
                            pos: point,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert_eq!(state.reply.take(), Some(Id(60_000 << 22)));
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
            render(&mut view, &mut state, vec![egui::Event::PointerGone]);
            // Tab reaches an avatar, its message row, then reaction and reply actions.
            let key = |key| egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            };
            for _ in 0..12 {
                let painted = render(&mut view, &mut state, vec![key(egui::Key::Tab)]);
                let focused = ctx
                    .memory(|m| m.focused())
                    .and_then(|id| ctx.read_response(id));
                if focused.is_some_and(|r| {
                    painted.iter().any(|(text, rect)| {
                        text == "↩" && r.rect.width() < 40.0 && r.rect.contains_rect(*rect)
                    })
                }) {
                    render(&mut view, &mut state, vec![key(egui::Key::Enter)]);
                    break;
                }
            }
            assert!(
                state.reply.is_some(),
                "Keyboard navigation must reach Reply"
            );
        }
    }
    #[test]
    fn auto_read_requires_focused_latest_and_does_not_retry_failed_marker() {
        let mut state = State {
            auth: client_core::auth::AuthState::Authenticated,
            gateway_connected: true,
            freshness: model::Freshness::Fresh,
            selected: Some(Id(20)),
            channels: vec![model::Channel {
                id: Id(20),
                guild: None,
                parent_id: None,
                position: 0,
                name: "Synthetic DM".into(),
                kind: 1,
                recipients: vec![],
                member_list_id: None,
                last_message: Some(Id(1)),
            }],
            ..Default::default()
        };
        state
            .timeline
            .insert(text_message(1), false, false)
            .unwrap();
        let ctx = egui::Context::default();
        let mut view = TimelineView::default();
        let mut avatars = crate::avatars::Avatars::default();
        let mut frame = |view: &mut TimelineView, state: &mut State, focused| {
            ctx.run_ui(
                egui::RawInput {
                    focused,
                    ..Default::default()
                },
                |ui| {
                    view.show(ui, state, &mut None, &mut None, &mut avatars, &mut None);
                },
            )
            .drop_without_applying_deltas();
        };
        frame(&mut view, &mut state, false);
        assert!(view.mark_read.is_none());
        frame(&mut view, &mut state, true);
        assert_eq!(view.mark_read.take(), Some(Id(1)));
        frame(&mut view, &mut state, true);
        assert!(view.mark_read.is_none());
        state.channels[0].last_message = Some(Id(3));
        state
            .timeline
            .insert(text_message(2), false, false)
            .unwrap();
        state.revision += 1;
        frame(&mut view, &mut state, true);
        assert!(
            view.mark_read.is_none(),
            "Historical window is not the latest message"
        );
    }
    #[test]
    fn native_layout_virtualizes_preserves_anchor_and_jumps_after_scrolling() {
        for (width, dark) in [(900.0, true), (360.0, false)] {
            let mut state = State {
                selected: Some(Id(20)),
                revision: 1,
                demo: true,
                ..Default::default()
            };
            for id in 1..=500 {
                state
                    .timeline
                    .insert(text_message(id), false, false)
                    .unwrap();
            }
            let ctx = egui::Context::default();
            crate::design::apply(&ctx);
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            let mut view = TimelineView::default();
            let mut avatars = crate::avatars::Avatars::default();
            let mut render = |view: &mut TimelineView, state: &mut State| {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 600.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        view.show(ui, state, &mut None, &mut None, &mut avatars, &mut None);
                    },
                )
                .drop_without_applying_deltas();
            };
            for _ in 0..8 {
                render(&mut view, &mut state);
            }
            assert!(view.following);
            assert!(
                view.heights.len() < 60,
                "Only visible rows and overscan are measured"
            );
            view.following = false;
            view.anchor = Some((Id(200), 5.0));
            view.revision = u64::MAX;
            for _ in 0..8 {
                render(&mut view, &mut state);
            }
            assert!(!view.following);
            let anchor = view.anchor.unwrap();
            assert_eq!(anchor.0, Id(200));
            state.timeline.insert(text_message(0), false, true).unwrap();
            state.revision += 1;
            for _ in 0..4 {
                render(&mut view, &mut state);
            }
            assert_eq!(view.anchor.unwrap().0, anchor.0);
            view.jump = true;
            view.following = true;
            for _ in 0..8 {
                render(&mut view, &mut state);
            }
            assert!(
                view.following,
                "Explicit jump must override persisted scroll state"
            );
        }
    }
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
    fn channel_rename_invalidates_offscreen_reference_heights() {
        let message = Message {
            id: Id(1),
            channel: Id(2),
            author: model::User {
                id: Id(3),
                name: "Synthetic".into(),
                avatar: None,
                discriminator: 0,
            },
            content: "<#4> ".repeat(12),
            mentions: vec![],
            reactions: Some(vec![]),
            edited: false,
            edited_at: None,
            revision: 0,
            nonce: None,
            reply_to: None,
            kind: 0,
            unsupported: false,
            embeds: vec![],
            embeds_suppressed: false,
            attachments: vec![],
        };
        let message_key = layout_key(&message);
        let mut tail = message.clone();
        tail.id = Id(2);
        tail.content = "ordinary text ".repeat(350);
        let mut state = State {
            demo: true,
            selected: Some(Id(2)),
            revision: 1,
            ..Default::default()
        };
        state.channels.push(model::Channel {
            id: Id(4),
            guild: Some(Id(5)),
            name: "a".into(),
            kind: 0,
            parent_id: None,
            position: 0,
            recipients: vec![],
            member_list_id: None,
            last_message: None,
        });
        state.timeline.insert(message, false, false).unwrap();
        state.timeline.insert(tail, false, false).unwrap();
        let mut view = TimelineView {
            channel: state.selected,
            anchor: Some((Id(1), 0.0)),
            ..Default::default()
        };
        let mut images = crate::avatars::Avatars::default();
        let context = egui::Context::default();
        let render =
            |view: &mut TimelineView, state: &mut State, images: &mut crate::avatars::Avatars| {
                context
                    .run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(360.0, 300.0),
                            )),
                            ..Default::default()
                        },
                        |ui| view.show(ui, state, &mut None, &mut None, images, &mut None),
                    )
                    .drop_without_applying_deltas();
            };
        for _ in 0..3 {
            render(&mut view, &mut state, &mut images);
        }
        let short_height = view.heights[&Id(1)].1;
        view.following = false;
        view.anchor = Some((Id(2), 400.0));
        state.revision += 1;
        for _ in 0..3 {
            render(&mut view, &mut state, &mut images);
        }
        assert_eq!(view.heights[&Id(1)].1, short_height);
        assert_eq!(view.anchor.unwrap().0, Id(2));
        state.apply(client_core::Envelope {
            generation: state.generation,
            event: client_core::Event::ChannelChanged(model::ChannelPatch {
                id: Id(4),
                name: model::Patch::Value("a-much-longer-channel-reference".into()),
                last_message: model::Patch::Absent,
                parent_id: model::Patch::Absent,
                position: model::Patch::Absent,
                kind: model::Patch::Absent,
            }),
        });
        assert_eq!(layout_key(state.timeline.get(Id(1)).unwrap()), message_key);
        render(&mut view, &mut state, &mut images);
        assert!(
            !view.heights.contains_key(&Id(1)),
            "An offscreen row must lose its old label-dependent height even though its message did not change"
        );
        view.following = false;
        view.anchor = Some((Id(1), 0.0));
        state.revision += 1;
        for _ in 0..3 {
            render(&mut view, &mut state, &mut images);
        }
        assert!(
            view.heights[&Id(1)].1 > short_height + 20.0,
            "The renamed references must be measured with their new wrapped labels"
        );
        assert!(images.take_requests().is_empty());
    }
    #[test]
    fn navigation_preserves_active_download_controls() {
        let mut view = TimelineView::default();
        view.download.active = true;
        view.download.status = "Downloading: 1 / 2 KiB".into();
        view.download.cancel_requested = true;
        let mut state = State::default();
        let context = egui::Context::default();
        for channel in [Some(Id(2)), Some(Id(3)), None] {
            state.selected = channel;
            context
                .run_ui(Default::default(), |ui| {
                    view.show(
                        ui,
                        &mut state,
                        &mut None,
                        &mut None,
                        &mut crate::avatars::Avatars::default(),
                        &mut None,
                    );
                })
                .drop_without_applying_deltas();
            assert!(view.download.active && view.download.cancel_requested);
            assert_eq!(view.download.status, "Downloading: 1 / 2 KiB");
        }
    }
    #[test]
    fn same_id_revision_reset_does_not_reuse_reveal_or_height() {
        let mut message = Message {
            reactions: Some(vec![]),
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
            kind: 0,
            unsupported: false,
            embeds: vec![],
            attachments: vec![],
            mentions: Vec::new(),
            embeds_suppressed: false,
        };
        let mut view = TimelineView {
            channel: Some(Id(2)),
            ..Default::default()
        };
        view.revealed.insert(
            message.id,
            (
                message.content.clone(),
                message.embeds.clone(),
                message.attachments.clone(),
            ),
        );
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
        assert_eq!(
            view.heights[&Id(1)].0,
            row_key(state.timeline.get(Id(1)).unwrap(), None, None)
        );
        assert!(
            view.heights[&Id(1)].1 < 210.0,
            "A short concealed message must keep a compact row even in an unbounded scroll layout: {}",
            view.heights[&Id(1)].1
        );
    }
    #[test]
    fn embed_cards_conceal_spoilers_and_invalidate_reveals_on_embed_only_edits() {
        fn painted_text(shape: &egui::Shape, text: &mut String) {
            match shape {
                egui::Shape::Text(value) => text.push_str(&value.galley.job.text),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        painted_text(shape, text);
                    }
                }
                _ => {}
            }
        }
        let mut message = Message {
            reactions: Some(vec![]),
            id: Id(1),
            channel: Id(2),
            author: model::User {
                id: Id(3),
                name: "Synthetic".into(),
                avatar: None,
                discriminator: 0,
            },
            content: "Ordinary text".into(),
            edited: false,
            edited_at: None,
            revision: 0,
            nonce: None,
            reply_to: None,
            kind: 0,
            unsupported: false,
            attachments: vec![],
            mentions: Vec::new(),
            embeds_suppressed: false,
            embeds: vec![model::Embed {
                kind: "rich".into(),
                title: Some("||Hidden title||".into()),
                description: Some("Embed description".into()),
                image: Some(model::EmbedMedia {
                    url: Some("https://example.com/image.png".into()),
                    width: 320,
                    height: 120,
                    ..Default::default()
                }),
                ..Default::default()
            }],
        };
        // A card near the viewport bottom retains its natural height.
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(480.0, 80.0),
                )),
                ..Default::default()
            },
            |ui| {
                super::super::embeds::show(
                    ui,
                    &message,
                    &mut FormatCache::default(),
                    &mut crate::avatars::Avatars::default(),
                    &mut None,
                    &mut None,
                    true,
                );
                assert!(ui.min_rect().height() > 200.0);
            },
        );
        output.textures_delta.clear();
        let mut state = State {
            demo: true,
            selected: Some(Id(2)),
            ..Default::default()
        };
        state
            .timeline
            .insert(message.clone(), false, false)
            .unwrap();
        let mut view = TimelineView {
            channel: state.selected,
            ..Default::default()
        };
        let mut images = crate::avatars::Avatars::default();
        let ctx = egui::Context::default();
        let render =
            |view: &mut TimelineView, state: &mut State, images: &mut crate::avatars::Avatars| {
                let output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(900.0, 900.0),
                        )),
                        ..Default::default()
                    },
                    |ui| view.show(ui, state, &mut None, &mut None, images, &mut None),
                );
                assert!(
                    output.platform_output.commands.is_empty(),
                    "Rendering must not open external links"
                );
                let mut text = String::new();
                for shape in &output.shapes {
                    painted_text(&shape.shape, &mut text);
                }
                output.drop_without_applying_deltas();
                text
            };
        render(&mut view, &mut state, &mut images);
        let concealed = render(&mut view, &mut state, &mut images);
        assert!(concealed.contains("Reveal spoiler"));
        assert!(!concealed.contains("Hidden title"));
        assert!(!concealed.contains("Embed description"));
        view.revealed.insert(
            message.id,
            (
                message.content.clone(),
                message.embeds.clone(),
                message.attachments.clone(),
            ),
        );
        let revealed = render(&mut view, &mut state, &mut images);
        assert!(revealed.contains("Hidden title"));
        assert!(revealed.contains("Embed description"));
        assert!(
            images.take_requests().is_empty(),
            "Demo images never enqueue network requests"
        );
        let previous_key = layout_key(&message);
        message.embeds[0].title = Some("||Changed secret||".into());
        assert_ne!(previous_key, layout_key(&message));
        state.timeline.insert(message.clone(), true, false).unwrap();
        state.revision += 1;
        let changed = render(&mut view, &mut state, &mut images);
        assert!(view.revealed.is_empty());
        assert!(!changed.contains("Changed secret"));
        message.embeds[0].title = Some("Visible title".into());
        message.embeds_suppressed = true;
        state.timeline.insert(message.clone(), true, false).unwrap();
        state.revision += 1;
        let suppressed = render(&mut view, &mut state, &mut images);
        assert!(!suppressed.contains("Visible title"));
        assert!(!suppressed.contains("Embed description"));
        // Attachments are independent of SUPPRESS_EMBEDS, but never of spoiler consent.
        message.embeds.clear();
        message.content.clear();
        message.attachments = vec![model::Attachment {
            id: Id(7),
            filename: "SPOILER_hidden.png".into(),
            description: None,
            content_type: Some("image/png".into()),
            size: 100,
            spoiler: true,
            media: model::EmbedMedia {
                url: Some("https://cdn.discordapp.com/attachments/2/7/hidden.png".into()),
                width: 320,
                height: 120,
                ..Default::default()
            },
        }];
        assert!(message.attachments[0].is_image());
        state.demo = false; // Only collects image request keys; there is no network worker in this test.
        state.timeline.insert(message.clone(), true, false).unwrap();
        state.revision += 1;
        view.viewing = Some((message.id, Id(7)));
        let hidden = render(&mut view, &mut state, &mut images);
        assert!(!hidden.contains("SPOILER_hidden.png"));
        assert!(view.viewing.is_none());
        assert!(
            images
                .take_requests()
                .iter()
                .all(|key| !key.starts_with("embed:")),
            "Hidden attachments must not request media; the visible author avatar is independent"
        );
        view.revealed.insert(
            message.id,
            (
                message.content.clone(),
                message.embeds.clone(),
                message.attachments.clone(),
            ),
        );
        view.viewing = Some((message.id, Id(7)));
        render(&mut view, &mut state, &mut images); // Modal sizing pass precedes visible paint.
        let shown = render(&mut view, &mut state, &mut images);
        assert!(shown.contains("SPOILER_hidden.png"));
        assert!(shown.contains("Download"));
        assert_eq!(images.take_requests().len(), 1);
        let previous_key = layout_key(&message);
        message.attachments[0].description = Some("Changed attachment".into());
        assert_ne!(previous_key, layout_key(&message));
        state.timeline.insert(message, true, false).unwrap();
        state.revision += 1;
        let hidden_again = render(&mut view, &mut state, &mut images);
        assert!(view.viewing.is_none() && view.revealed.is_empty());
        assert!(!hidden_again.contains("SPOILER_hidden.png"));
        assert!(images.take_requests().is_empty());
    }
}
