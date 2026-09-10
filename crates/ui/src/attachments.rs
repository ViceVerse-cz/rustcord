//! Inline service attachments; decoded pixels reuse the existing bounded media cache.
use crate::{avatars::Avatars, markdown::external_url};
use model::{Attachment, Id, Message};

pub fn show(
    ui: &mut egui::Ui,
    message: &Message,
    images: &mut Avatars,
    viewing: &mut Option<(Id, Id)>,
    opening: &mut Option<String>,
    download: &mut DownloadUi,
    demo: bool,
) {
    for group in message
        .attachments
        .chunk_by(|a, b| a.is_image() == b.is_image())
    {
        if group[0].is_image() {
            let (columns, size) = image_layout(group.len(), ui.available_width());
            ui.scope(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                for row in group.chunks(columns) {
                    ui.horizontal_top(|ui| {
                        for attachment in row {
                            ui.push_id(("attachment", attachment.id), |ui| {
                                let response = images
                                    .show_embed(ui, &attachment.media, size, demo)
                                    .interact(egui::Sense::click());
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::WidgetType::Button,
                                        ui.is_enabled(),
                                        format!("View image {}", attachment.filename),
                                    )
                                });
                                if response
                                    .on_hover_text(
                                        attachment
                                            .description
                                            .as_deref()
                                            .unwrap_or("Enlarge image"),
                                    )
                                    .clicked()
                                {
                                    *viewing = Some((message.id, attachment.id));
                                }
                            });
                        }
                    });
                }
            });
        } else {
            for attachment in group {
                ui.push_id(("attachment", attachment.id), |ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(&attachment.filename).small()).wrap(),
                    );
                    ui.small(format!(
                        "{} bytes · inline preview unavailable",
                        attachment.size
                    ));
                    ui.horizontal_wrapped(|ui| {
                        download_button(ui, attachment, download, demo);
                        open_original(ui, attachment, opening);
                    });
                    ui.add_space(6.0);
                });
            }
        }
    }
}
fn image_layout(count: usize, width: f32) -> (usize, egui::Vec2) {
    let columns = if count > 1 && width >= 280.0 { 2 } else { 1 };
    let width = ((width.min(420.0) - (columns - 1) as f32 * 6.0) / columns as f32).max(1.0);
    (
        columns,
        egui::vec2(width, if count > 1 { 180.0 } else { 280.0 }),
    )
}

fn open_original(ui: &mut egui::Ui, attachment: &Attachment, opening: &mut Option<String>) {
    if let Some(target) = attachment.media.url.as_deref().and_then(external_url)
        && ui.small_button("Open original…").clicked()
    {
        *opening = Some(target);
    }
}
#[derive(Default)]
pub struct DownloadUi {
    pub request: Option<Attachment>,
    pub cancel_requested: bool,
    pub active: bool,
    pub status: String,
}
impl DownloadUi {
    pub fn show_status(&mut self, ui: &mut egui::Ui) {
        if self.active || !self.status.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.small(&self.status);
                if self.active && ui.small_button("Cancel download").clicked() {
                    self.cancel_requested = true;
                }
            });
        }
    }
}
fn download_button(
    ui: &mut egui::Ui,
    attachment: &Attachment,
    download: &mut DownloadUi,
    demo: bool,
) {
    if ui
        .add_enabled(
            !demo && !download.active && download.request.is_none(),
            egui::Button::new("Download"),
        )
        .on_hover_text("Choose where to save this file · up to 100 MiB")
        .on_disabled_hover_text(if demo {
            "Downloads are disabled for synthetic attachments"
        } else {
            "A download is already active"
        })
        .clicked()
    {
        download.request = Some(attachment.clone());
    }
}
pub fn viewer(
    ui: &mut egui::Ui,
    attachment: &Attachment,
    images: &mut Avatars,
    download: &mut DownloadUi,
    demo: bool,
) -> bool {
    let colors = crate::design::palette(ui);
    let size = ui.ctx().content_rect().size() - egui::vec2(56.0, 56.0);
    let mut close = false;
    let modal = egui::Modal::new(egui::Id::new("attachment-viewer"))
        .backdrop_color(egui::Color32::from_black_alpha(235))
        .frame(
            egui::Frame::new()
                .fill(colors.canvas)
                .inner_margin(16)
                .corner_radius(10),
        )
        .show(ui.ctx(), |ui| {
            ui.set_width(size.x.max(240.0));
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    close = ui.button("Close ×").clicked();
                    download_button(ui, attachment, download, demo);
                    ui.add(egui::Label::new(&attachment.filename).truncate());
                });
            });
            ui.separator();
            let image_size = egui::vec2(ui.available_width(), (size.y - 150.0).max(120.0));
            ui.vertical_centered(|ui| {
                images.show_large(ui, &attachment.media, image_size, demo);
            });
            ui.add_space(8.0);
            if let Some(description) = &attachment.description {
                ui.add(egui::Label::new(description).truncate())
                    .on_hover_text(description);
            }
            ui.small(format!(
                "{} × {} · {} bytes",
                attachment.media.width, attachment.media.height, attachment.size
            ));
            download.show_status(ui);
        });
    !close && !modal.should_close()
}
pub fn estimated_height(attachments: &[Attachment], width: f32) -> f32 {
    attachments
        .chunk_by(|a, b| a.is_image() == b.is_image())
        .map(|group| {
            if group[0].is_image() {
                let (columns, size) = image_layout(group.len(), width);
                group.len().div_ceil(columns) as f32 * (size.y + 6.0)
            } else {
                group.len() as f32 * 70.0
            }
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_gallery_wraps_without_filenames_and_opens_each_attachment() {
        let mut message = test_support::message(1, Id(2));
        message.attachments = (0..3)
            .map(|id| Attachment {
                id: Id(id + 10),
                filename: format!("synthetic-image-{id}.png"),
                description: None,
                content_type: Some("image/png".into()),
                size: 512,
                spoiler: false,
                media: model::EmbedMedia {
                    width: 640,
                    height: 360,
                    ..Default::default()
                },
            })
            .collect();
        let mut file = message.attachments[0].clone();
        file.id = Id(20);
        file.filename = "synthetic-report.pdf".into();
        file.content_type = Some("application/pdf".into());
        message.attachments.push(file);
        for width in [420.0, 240.0] {
            let ctx = egui::Context::default();
            let mut images = Avatars::default();
            let mut viewing = None;
            let mut opening = None;
            let mut download = DownloadUi::default();
            let mut frame = |events| {
                ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width + 16.0, 800.0),
                        )),
                        events,
                        ..Default::default()
                    },
                    |ui| {
                        ui.set_width(width);
                        show(
                            ui,
                            &message,
                            &mut images,
                            &mut viewing,
                            &mut opening,
                            &mut download,
                            true,
                        );
                    },
                )
            };
            frame(vec![]).drop_without_applying_deltas();
            let output = frame(vec![]);
            let rects: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Rect(shape)
                        if shape.corner_radius == egui::CornerRadius::same(5) =>
                    {
                        Some(shape.rect)
                    }
                    _ => None,
                })
                .collect();
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Text(shape) => Some(shape.galley.job.text.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(rects.len(), 3);
            assert!(labels.contains(&"synthetic-report.pdf"));
            assert!(
                labels
                    .iter()
                    .all(|label| !label.contains("synthetic-image-"))
            );
            for rect in &rects {
                assert!(rect.width() <= width);
                assert!((rect.width() / rect.height() - 16.0 / 9.0).abs() < 0.01);
            }
            if width >= 280.0 {
                assert_eq!(rects[0].top(), rects[1].top());
                assert!(rects[1].left() > rects[0].right());
            } else {
                assert!(rects[1].top() > rects[0].bottom());
            }
            assert!(rects[2].top() > rects[0].bottom());
            let pos = rects[1].center();
            output.drop_without_applying_deltas();
            for pressed in [true, false] {
                frame(vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ])
                .drop_without_applying_deltas();
            }
            assert_eq!(viewing, Some((message.id, message.attachments[1].id)));
            assert!(opening.is_none() && download.request.is_none());
            assert!(images.take_requests().is_empty());
        }
        assert!(
            estimated_height(&message.attachments, 420.0)
                < estimated_height(&message.attachments, 240.0)
        );
    }

    #[test]
    fn nonimage_download_requires_keyboard_action_and_respects_single_job_controls() {
        fn frame(ctx: &egui::Context, key: Option<egui::Key>, draw: impl FnMut(&mut egui::Ui)) {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(640.0, 480.0),
                    )),
                    events: key
                        .into_iter()
                        .map(|key| egui::Event::Key {
                            key,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::NONE,
                        })
                        .collect(),
                    ..Default::default()
                },
                draw,
            );
            assert!(output.platform_output.commands.is_empty());
            output.drop_without_applying_deltas();
        }
        let attachment = Attachment {
            id: Id(3),
            filename: "synthetic-report.pdf".into(),
            description: None,
            content_type: Some("application/pdf".into()),
            size: 512,
            spoiler: false,
            media: model::EmbedMedia {
                url: Some("https://cdn.discordapp.com/attachments/2/3/synthetic-report.pdf".into()),
                ..Default::default()
            },
        };
        let message = Message {
            id: Id(1),
            channel: Id(2),
            author: model::User {
                id: Id(4),
                name: "Synthetic".into(),
                avatar: None,
                discriminator: 0,
            },
            content: String::new(),
            mention_roles: vec![],
            mention_everyone: false,
            suppress_notifications: false,
            mentions: vec![],
            edited: false,
            edited_at: None,
            revision: 0,
            nonce: None,
            reply_to: None,
            kind: 0,
            reply_deleted: false,
            unsupported: false,
            extra_content: Default::default(),
            embeds: vec![],
            embeds_suppressed: false,
            reactions: None,
            attachments: vec![attachment.clone()],
        };
        let ctx = egui::Context::default();
        let mut images = Avatars::default();
        let mut viewing = None;
        let mut opening = None;
        let mut download = DownloadUi::default();
        frame(&ctx, None, |ui| {
            show(
                ui,
                &message,
                &mut images,
                &mut viewing,
                &mut opening,
                &mut download,
                false,
            )
        });
        assert!(images.take_requests().is_empty());
        assert!(viewing.is_none() && opening.is_none() && download.request.is_none());
        for key in [egui::Key::Tab, egui::Key::Enter] {
            frame(&ctx, Some(key), |ui| {
                show(
                    ui,
                    &message,
                    &mut images,
                    &mut viewing,
                    &mut opening,
                    &mut download,
                    false,
                )
            });
        }
        assert_eq!(download.request.as_ref(), Some(&attachment));
        assert!(download.request.as_ref().unwrap().bytes() <= model::MAX_ATTACHMENT_BYTES);
        assert!(images.take_requests().is_empty());
        assert!(viewing.is_none() && opening.is_none());

        for (demo, active, pending) in [
            (true, false, false),
            (false, true, false),
            (false, false, true),
        ] {
            let ctx = egui::Context::default();
            let mut previous = attachment.clone();
            previous.id = Id(9);
            let mut download = DownloadUi {
                active,
                request: pending.then(|| previous.clone()),
                ..Default::default()
            };
            for key in [None, Some(egui::Key::Tab), Some(egui::Key::Enter)] {
                frame(&ctx, key, |ui| {
                    download_button(ui, &attachment, &mut download, demo)
                });
            }
            assert_eq!(download.request, pending.then_some(previous));
        }
        let ctx = egui::Context::default();
        let mut download = DownloadUi {
            active: true,
            status: "Downloading: 256 / 512 bytes".into(),
            ..Default::default()
        };
        frame(&ctx, None, |ui| download.show_status(ui));
        assert!(!download.cancel_requested);
        for key in [egui::Key::Tab, egui::Key::Enter] {
            frame(&ctx, Some(key), |ui| download.show_status(ui));
        }
        assert!(download.cancel_requested);
    }
}
