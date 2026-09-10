//! Inline service attachments; decoded pixels reuse the existing bounded media cache.
use crate::{avatars::Avatars, markdown::external_url};
use model::{Attachment, Id, Message};

pub fn show(
    ui: &mut egui::Ui,
    message: &Message,
    images: &mut Avatars,
    viewing: &mut Option<(Id, Id)>,
    opening: &mut Option<String>,
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
                ui.small("File attachment · inline preview unavailable");
                open_original(ui, attachment, opening);
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
                    if ui
                        .add_enabled(!demo && !download.active, egui::Button::new("Download"))
                        .on_disabled_hover_text(if demo {
                            "Downloads are disabled for synthetic preview images"
                        } else {
                            "A download is already active"
                        })
                        .clicked()
                    {
                        download.request = Some(attachment.clone());
                    }
                    if download.active && ui.button("Cancel download").clicked() {
                        download.cancel_requested = true;
                    }
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
            if !download.status.is_empty() {
                ui.small(&download.status);
            }
        });
    !close && !modal.should_close()
}
pub fn estimated_height(attachments: &[Attachment]) -> f32 {
    attachments
        .iter()
        .map(|a| if a.is_image() { 310.0 } else { 70.0 })
        .sum()
}
