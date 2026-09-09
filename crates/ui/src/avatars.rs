//! Visible-avatar requests and a small texture working set. Disk/network work lives outside egui.
use egui::{ColorImage, TextureHandle};
use model::User;
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

const TEXTURES: usize = 64;
const REQUESTS: usize = 128;
const RETRY: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(crate) struct Avatars {
    textures: VecDeque<(String, TextureHandle)>,
    attempts: HashMap<String, Instant>,
    requests: Vec<String>,
}
impl Avatars {
    pub fn take_requests(&mut self) -> Vec<String> {
        std::mem::take(&mut self.requests)
    }
    fn request(&mut self, key: String) {
        let now = Instant::now();
        self.attempts
            .retain(|_, at| now.duration_since(*at) < RETRY);
        if self.attempts.len() < REQUESTS
            && self.requests.len() < REQUESTS
            && !self.attempts.contains_key(&key)
        {
            self.attempts.insert(key.clone(), now);
            self.requests.push(key);
        }
    }
    pub fn accept(&mut self, ctx: &egui::Context, key: String, image: Option<ColorImage>) {
        if !self.attempts.contains_key(&key) {
            return;
        }
        let Some(image) = image.filter(|image| {
            image.size[0] > 0
                && image.size[1] > 0
                && image.size[0] <= 128
                && image.size[1] <= 128
                && image.pixels.len() == image.size[0] * image.size[1]
        }) else {
            return;
        };
        self.attempts.remove(&key);
        self.textures.retain(|(old, _)| *old != key);
        while self.textures.len() >= TEXTURES {
            self.textures.pop_front();
        }
        let texture = ctx.load_texture("avatar", image, egui::TextureOptions::LINEAR);
        self.textures.push_back((key, texture));
    }
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        user: &User,
        size: f32,
        demo: bool,
    ) -> egui::Response {
        let response = crate::design::avatar(ui, &user.name, size);
        if ui.is_rect_visible(response.rect) {
            let key = if demo {
                format!("preview-{}", user.id)
            } else {
                user.avatar_key()
            };
            if demo && !self.textures.iter().any(|(stored, _)| stored == &key) {
                // Original, synthetic silhouettes exercise the image path without network or assets.
                let background = if user.id.0.is_multiple_of(2) {
                    egui::Color32::from_rgb(63, 99, 111)
                } else {
                    egui::Color32::from_rgb(103, 86, 124)
                };
                let foreground = egui::Color32::from_rgb(224, 237, 227);
                let mut image = ColorImage::filled([32, 32], background);
                for y in 0..32_i32 {
                    for x in 0..32_i32 {
                        if (x - 16).pow(2) + (y - 11).pow(2) < 36
                            || (x - 16).pow(2) + (y - 31).pow(2) < 121
                        {
                            image.pixels[(y * 32 + x) as usize] = foreground;
                        }
                    }
                }
                self.attempts.insert(key.clone(), Instant::now());
                self.accept(ui.ctx(), key.clone(), Some(image));
            }
            if let Some(index) = self.textures.iter().position(|(stored, _)| stored == &key) {
                let entry = self.textures.remove(index).expect("located texture");
                egui::Image::new((entry.1.id(), egui::vec2(size, size)))
                    .corner_radius((size * 0.5) as u8)
                    .paint_at(ui, response.rect);
                self.textures.push_back(entry);
            } else if !demo {
                self.request(key);
            }
        }
        if response.hovered() || response.has_focus() {
            ui.painter().circle_stroke(
                response.rect.center(),
                size * 0.5,
                egui::Stroke::new(1.5, crate::design::palette(ui).accent),
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                format!("View profile for {}", user.name),
            )
        });
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn texture_requests_and_decoded_memory_stay_bounded() {
        let ctx = egui::Context::default();
        let mut avatars = Avatars::default();
        for i in 0..256 {
            avatars.request(i.to_string());
        }
        assert_eq!(avatars.take_requests().len(), REQUESTS);
        assert_eq!(avatars.attempts.len(), REQUESTS);
        avatars.request("0".into());
        assert!(avatars.take_requests().is_empty());
        avatars.accept(
            &ctx,
            "0".into(),
            Some(ColorImage::filled([129, 128], egui::Color32::WHITE)),
        );
        assert!(avatars.textures.is_empty());
        for i in 0..128 {
            avatars.accept(
                &ctx,
                i.to_string(),
                Some(ColorImage::filled([128, 128], egui::Color32::WHITE)),
            );
        }
        assert_eq!(avatars.textures.len(), TEXTURES);
        assert_eq!(
            avatars
                .textures
                .iter()
                .map(|(_, t)| t.byte_size())
                .sum::<usize>(),
            4 * 1024 * 1024
        );
        avatars.accept(
            &ctx,
            "unsolicited".into(),
            Some(ColorImage::filled([128, 128], egui::Color32::WHITE)),
        );
        assert_eq!(avatars.textures.len(), TEXTURES);
    }
}
