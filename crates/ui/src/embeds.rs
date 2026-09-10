//! Native embed cards. External links share the timeline's explicit confirmation.
use crate::{
    avatars::Avatars,
    markdown::{FormatCache, external_url},
};
use egui::RichText;
use model::{Embed, Id, Message};

pub fn has_spoilers(message: &Message) -> bool {
    message.content.contains("||")
        || message.embeds.iter().any(|e| {
            [&e.title, &e.description]
                .into_iter()
                .flatten()
                .any(|s| s.contains("||"))
                || e.fields
                    .iter()
                    .any(|f| f.name.contains("||") || f.value.contains("||"))
                || [&e.author, &e.provider]
                    .into_iter()
                    .flatten()
                    .any(|a| a.name.contains("||"))
                || e.footer.as_ref().is_some_and(|f| f.text.contains("||"))
        })
}
fn link(
    ui: &mut egui::Ui,
    label: &str,
    url: Option<&str>,
    opening: &mut Option<String>,
    strong: bool,
) {
    let target = url.and_then(external_url);
    let mut text = RichText::new(label);
    if strong {
        text = text.strong();
    }
    if target.is_some() {
        text = text.color(ui.visuals().hyperlink_color);
    }
    let response = ui.add(egui::Label::new(text).wrap().sense(if target.is_some() {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    }));
    if let Some(target) = target {
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Link, ui.is_enabled(), label)
        });
        if response.on_hover_text("Open link…").clicked() {
            *opening = Some(target);
        }
    }
}
fn text(
    ui: &mut egui::Ui,
    id: Id,
    part: u16,
    source: &str,
    cache: &mut FormatCache,
    opening: &mut Option<String>,
) {
    let formatted = cache.get_part(id, part, source);
    formatted.show(ui, opening);
    if formatted.limited {
        ui.small("Text display limited");
    }
}
pub fn show(
    ui: &mut egui::Ui,
    message: &Message,
    cache: &mut FormatCache,
    images: &mut Avatars,
    opening: &mut Option<String>,
    demo: bool,
) {
    if message.embeds_suppressed {
        return;
    }
    for (index, embed) in message.embeds.iter().enumerate() {
        ui.push_id(("embed", index), |ui| {
            let colors = crate::design::palette(ui);
            let color = embed.color.map_or(colors.accent, |c| {
                egui::Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
            });
            let width = ui.available_width().min(480.0);
            let frame = egui::Frame::new()
                .fill(colors.raised)
                .corner_radius(5)
                .inner_margin(12)
                .show(ui, |ui| {
                    ui.set_width((width - 24.0).max(1.0));
                    // Size independently of the remaining timeline viewport.
                    ui.set_max_height(640.0);
                    // Bound exceptional cards' geometry; normal cards grow to their content height.
                    egui::ScrollArea::vertical()
                        .id_salt("embed-content")
                        .max_height(640.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            let part = 1 + index as u16 * 64;
                            let thumbnail = embed
                                .thumbnail
                                .as_ref()
                                .filter(|_| ui.available_width() >= 300.0);
                            let body_width = (ui.available_width()
                                - if thumbnail.is_some() { 96.0 } else { 0.0 })
                            .max(1.0);
                            ui.horizontal_top(|ui| {
                                ui.vertical(|ui| {
                                    ui.set_width(body_width);
                                    if let Some(provider) = &embed.provider {
                                        link(
                                            ui,
                                            &provider.name,
                                            provider.url.as_deref(),
                                            opening,
                                            false,
                                        );
                                    }
                                    if let Some(author) = &embed.author {
                                        ui.horizontal_wrapped(|ui| {
                                            if let Some(icon) = &author.icon {
                                                images.show_embed(
                                                    ui,
                                                    icon,
                                                    egui::vec2(20.0, 20.0),
                                                    demo,
                                                );
                                            }
                                            link(
                                                ui,
                                                &author.name,
                                                author.url.as_deref(),
                                                opening,
                                                false,
                                            );
                                        });
                                    }
                                    if let Some(title) = &embed.title {
                                        link(ui, title, embed.url.as_deref(), opening, true);
                                    }
                                    if let Some(description) = &embed.description {
                                        text(ui, message.id, part, description, cache, opening);
                                    }
                                });
                                if let Some(image) = thumbnail {
                                    images.show_embed(ui, image, egui::vec2(84.0, 84.0), demo);
                                }
                            });
                            let mut field = 0;
                            while field < embed.fields.len() {
                                let columns = if ui.available_width() >= 360.0 {
                                    3
                                } else if ui.available_width() >= 240.0 {
                                    2
                                } else {
                                    1
                                };
                                let count = if embed.fields[field].inline {
                                    embed.fields[field..]
                                        .iter()
                                        .take_while(|f| f.inline)
                                        .take(columns)
                                        .count()
                                } else {
                                    1
                                };
                                ui.columns(count, |columns| {
                                    for (offset, column) in columns.iter_mut().enumerate() {
                                        let f = &embed.fields[field + offset];
                                        column.add(
                                            egui::Label::new(RichText::new(&f.name).strong())
                                                .wrap()
                                                .selectable(true),
                                        );
                                        text(
                                            column,
                                            message.id,
                                            part + 1 + (field + offset) as u16,
                                            &f.value,
                                            cache,
                                            opening,
                                        );
                                    }
                                });
                                field += count;
                            }
                            if let Some(image) = &embed.image {
                                images.show_embed(
                                    ui,
                                    image,
                                    egui::vec2(ui.available_width(), 320.0),
                                    demo,
                                );
                            }
                            if thumbnail.is_none()
                                && let Some(image) = &embed.thumbnail
                            {
                                images.show_embed(ui, image, egui::vec2(84.0, 84.0), demo);
                            }
                            if embed.video.is_some()
                                || matches!(embed.kind.as_str(), "video" | "gifv")
                            {
                                ui.small("Video preview · playback opens in your browser");
                                link(
                                    ui,
                                    "Open video…",
                                    embed.url.as_deref().or_else(|| {
                                        embed.video.as_ref().and_then(|v| v.url.as_deref())
                                    }),
                                    opening,
                                    false,
                                );
                            } else if embed.title.is_none()
                                && let Some(url) = embed.url.as_deref()
                            {
                                link(ui, "Open source…", Some(url), opening, false);
                            }
                            if let Some(footer) = &embed.footer {
                                ui.horizontal_wrapped(|ui| {
                                    if let Some(icon) = &footer.icon {
                                        images.show_embed(ui, icon, egui::vec2(16.0, 16.0), demo);
                                    }
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&footer.text).small().color(colors.muted),
                                        )
                                        .wrap()
                                        .selectable(true),
                                    );
                                });
                            }
                            if let Some(timestamp) = &embed.timestamp {
                                ui.small(timestamp);
                            }
                            if embed.limited {
                                ui.small("Embed display limited");
                            }
                            if !matches!(
                                embed.kind.as_str(),
                                "rich" | "article" | "link" | "image" | "video" | "gifv"
                            ) {
                                ui.small("Additional embed content is not supported");
                            }
                        });
                });
            ui.painter().line_segment(
                [
                    frame.response.rect.left_top() + egui::vec2(1.5, 5.0),
                    frame.response.rect.left_bottom() - egui::vec2(-1.5, 5.0),
                ],
                egui::Stroke::new(3.0, color),
            );
            ui.add_space(6.0);
        });
    }
}
pub fn estimated_height(embeds: &[Embed]) -> f32 {
    embeds
        .iter()
        .map(|e| 100.0 + e.fields.len() as f32 * 44.0 + if e.image.is_some() { 200.0 } else { 0.0 })
        .map(|h| h.min(664.0))
        .sum()
}
