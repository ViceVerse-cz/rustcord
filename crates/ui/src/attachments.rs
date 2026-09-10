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
    for attachment in &message.attachments {
        ui.push_id(("attachment", attachment.id), |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&attachment.filename).small()).wrap());
            if attachment.is_image() {
                let response = images
                    .show_embed(ui, &attachment.media, egui::vec2(420.0, 280.0), demo)
                    .interact(egui::Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::Button,
                        ui.is_enabled(),
                        format!("View image {}", attachment.filename),
                    )
                });
                if response
                    .on_hover_text(attachment.description.as_deref().unwrap_or("Enlarge image"))
                    .clicked()
                {
                    *viewing = Some((message.id, attachment.id));
                }
            } else {
                ui.small(format!(
                    "{} bytes · inline preview unavailable",
                    attachment.size
                ));
                ui.horizontal_wrapped(|ui| {
                    download_button(ui, attachment, download, demo);
                    open_original(ui, attachment, opening);
                });
            }
            ui.add_space(6.0);
        });
    }
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
pub fn estimated_height(attachments: &[Attachment]) -> f32 {
    attachments
        .iter()
        .map(|a| if a.is_image() { 310.0 } else { 70.0 })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

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
