//! Visible-avatar requests and a small texture working set. Disk/network work lives outside egui.
use egui::{ColorImage, TextureHandle};
use model::User;
use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

const TEXTURES: usize = 64;
const TEXTURE_BYTES: usize = 16 * 1024 * 1024;
const REQUESTS: usize = 128;
const RETRY: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(crate) struct Avatars {
    textures: VecDeque<(String, TextureHandle)>,
    attempts: HashMap<String, (Instant, bool)>,
    requests: Vec<String>,
}
impl Avatars {
    pub fn take_requests(&mut self) -> Vec<String> {
        std::mem::take(&mut self.requests)
    }
    fn request(&mut self, key: String) {
        let now = Instant::now();
        self.attempts
            .retain(|_, at| now.duration_since(at.0) < RETRY);
        if key.len() <= 2054
            && self.attempts.len() < REQUESTS
            && self.requests.len() < REQUESTS
            && !self.attempts.contains_key(&key)
        {
            self.attempts.insert(key.clone(), (now, false));
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
                && image.size[0]
                    <= if key.starts_with("embed:")
                        || key.starts_with("banner-")
                        || key.starts_with("member-banner-")
                    {
                        512
                    } else {
                        128
                    }
                && image.size[1]
                    <= if key.starts_with("embed:")
                        || key.starts_with("banner-")
                        || key.starts_with("member-banner-")
                    {
                        512
                    } else {
                        128
                    }
                && image.pixels.len() == image.size[0] * image.size[1]
        }) else {
            if let Some(attempt) = self.attempts.get_mut(&key) {
                attempt.1 = true;
            }
            return;
        };
        self.attempts.remove(&key);
        self.textures.retain(|(old, _)| *old != key);
        while self.textures.len() >= TEXTURES
            || self
                .textures
                .iter()
                .map(|(_, t)| t.byte_size())
                .sum::<usize>()
                + image.pixels.len() * 4
                > TEXTURE_BYTES
        {
            self.textures.pop_front();
        }
        let texture = ctx.load_texture("service-image", image, egui::TextureOptions::LINEAR);
        self.textures.push_back((key, texture));
    }
    pub(crate) fn custom_image(
        &mut self,
        ctx: &egui::Context,
        id: model::Id,
        size: f32,
        demo: bool,
    ) -> Option<egui::Image<'static>> {
        let key = format!("emoji-{id}");
        if demo
            && matches!(id.0, 9001 | 9002)
            && !self.textures.iter().any(|(stored, _)| stored == &key)
        {
            let mut image = ColorImage::filled([32, 32], egui::Color32::TRANSPARENT);
            for y in 3..29 {
                for x in 3..29 {
                    if (x + y + id.0 as usize) % 10 < 7 {
                        image.pixels[y * 32 + x] = if id.0 == 9001 {
                            egui::Color32::from_rgb(55, 180, 165)
                        } else {
                            egui::Color32::from_rgb(240, 150, 70)
                        };
                    }
                }
            }
            self.attempts.insert(key.clone(), (Instant::now(), false));
            self.accept(ctx, key.clone(), Some(image));
        }
        if let Some(index) = self.textures.iter().position(|(stored, _)| stored == &key) {
            let entry = self.textures.remove(index).expect("located emoji texture");
            let image = egui::Image::new(&entry.1).fit_to_exact_size(egui::Vec2::splat(size));
            self.textures.push_back(entry);
            Some(image)
        } else {
            if !demo {
                self.request(key);
            }
            None
        }
    }
    /// Paints the profile banner (or its accent color) into `rect`; corners follow the card.
    pub fn paint_banner(
        &mut self,
        ui: &mut egui::Ui,
        profile: &model::UserProfile,
        rect: egui::Rect,
        corner: egui::CornerRadius,
        demo: bool,
    ) {
        let color = profile
            .accent_color
            .or(profile.theme_colors.map(|c| c[0]))
            .map(|rgb| egui::Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8))
            .unwrap_or(crate::design::palette(ui).accent.gamma_multiply(0.4));
        ui.painter().rect_filled(rect, corner, color);
        if ui.is_rect_visible(rect)
            && let Some(key) = profile.banner_key()
        {
            if demo && !self.textures.iter().any(|(stored, _)| stored == &key) {
                let mut image = ColorImage::filled([128, 48], color);
                let stripe = color.lerp_to_gamma(egui::Color32::WHITE, 0.16);
                for y in 0..48 {
                    for x in 0..128 {
                        if (x + y) % 48 < 12 {
                            image.pixels[y * 128 + x] = stripe;
                        }
                    }
                }
                self.attempts.insert(key.clone(), (Instant::now(), false));
                self.accept(ui.ctx(), key.clone(), Some(image));
            }
            if let Some(index) = self.textures.iter().position(|(stored, _)| stored == &key) {
                let entry = self.textures.remove(index).expect("located texture");
                let source = entry.1.size_vec2();
                let scale = (rect.width() / source.x).max(rect.height() / source.y);
                let uv_size = rect.size() / (source * scale);
                let uv = egui::Rect::from_center_size(egui::pos2(0.5, 0.5), uv_size);
                egui::Image::new((entry.1.id(), rect.size()))
                    .uv(uv)
                    .corner_radius(corner)
                    .paint_at(ui, rect);
                self.textures.push_back(entry);
            } else if !demo {
                self.request(key);
            }
        }
    }
    /// Small square artwork (badge or server tag). Falls back to a neutral disc until loaded.
    pub fn show_icon(
        &mut self,
        ui: &mut egui::Ui,
        key: Option<String>,
        size: f32,
        demo: bool,
        label: &str,
    ) -> egui::Response {
        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::splat(size), egui::Sense::hover());
        if ui.is_rect_visible(rect) {
            let colors = crate::design::palette(ui);
            if let Some(key) = key {
                if demo && !self.textures.iter().any(|(stored, _)| stored == &key) {
                    // Original synthetic emblem; never bundled third-party badge artwork.
                    let seed = key
                        .bytes()
                        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
                    let tint = egui::Color32::from_rgb(
                        90 + (seed % 120) as u8,
                        120 + ((seed >> 8) % 100) as u8,
                        150 + ((seed >> 16) % 90) as u8,
                    );
                    let mut image = ColorImage::filled([32, 32], egui::Color32::TRANSPARENT);
                    for y in 0..32_i32 {
                        for x in 0..32_i32 {
                            let d = (x - 16).pow(2) + (y - 16).pow(2);
                            if d < 196 {
                                image.pixels[(y * 32 + x) as usize] =
                                    if d < 36 { egui::Color32::WHITE } else { tint };
                            }
                        }
                    }
                    self.attempts.insert(key.clone(), (Instant::now(), false));
                    self.accept(ui.ctx(), key.clone(), Some(image));
                }
                if !self.paint(ui, &key, rect, (size * 0.25) as u8) {
                    ui.painter()
                        .circle_filled(rect.center(), size * 0.4, colors.raised);
                    if !demo {
                        self.request(key);
                    }
                }
            } else {
                ui.painter()
                    .circle_filled(rect.center(), size * 0.4, colors.raised);
                ui.painter().circle_stroke(
                    rect.center(),
                    size * 0.4,
                    egui::Stroke::new(1.0, colors.border),
                );
            }
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Image, ui.is_enabled(), label)
        });
        response
    }
    pub fn show_profile_avatar(
        &mut self,
        ui: &mut egui::Ui,
        profile: &model::UserProfile,
        size: f32,
        demo: bool,
    ) -> egui::Response {
        if profile.guild.as_ref().is_none_or(|g| g.avatar.is_none()) {
            return self.show(ui, &profile.user, size, demo);
        }
        let response = crate::design::avatar(ui, &profile.user.name, size);
        if ui.is_rect_visible(response.rect) {
            let key = profile.avatar_key();
            if demo && !self.textures.iter().any(|(stored, _)| stored == &key) {
                let image = ColorImage::filled([32, 32], crate::design::palette(ui).accent);
                self.attempts.insert(key.clone(), (Instant::now(), false));
                self.accept(ui.ctx(), key.clone(), Some(image));
            }
            if !self.paint(
                ui,
                &key,
                ui.layout()
                    .align_size_within_rect(egui::Vec2::splat(size), response.rect),
                (size * 0.5) as u8,
            ) && !demo
            {
                self.request(key);
            }
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Image,
                ui.is_enabled(),
                "Server profile picture",
            )
        });
        response
    }
    fn paint(&mut self, ui: &mut egui::Ui, key: &str, rect: egui::Rect, radius: u8) -> bool {
        let Some(index) = self.textures.iter().position(|(stored, _)| stored == key) else {
            return false;
        };
        let entry = self.textures.remove(index).expect("located texture");
        // paint_at stretches to its rectangle; fit actual pixels inside the stable layout slot.
        let source = entry.1.size_vec2();
        let scale = (rect.width() / source.x).min(rect.height() / source.y);
        let fitted = egui::Rect::from_center_size(rect.center(), source * scale);
        egui::Image::new(&entry.1)
            .corner_radius(radius)
            .paint_at(ui, fitted);
        self.textures.push_back(entry);
        true
    }
    pub fn show_guild(
        &mut self,
        ui: &mut egui::Ui,
        guild: &model::Guild,
        selected: bool,
        demo: bool,
    ) -> egui::Response {
        let short: String = guild
            .name
            .split_whitespace()
            .filter_map(|word| word.chars().next())
            .take(2)
            .collect();
        let colors = crate::design::palette(ui);
        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::splat(48.0), egui::Sense::click());
        let rounded = selected || response.hovered() || response.has_focus();
        let radius: u8 = if rounded { 16 } else { 24 };
        let mut painted = false;
        if ui.is_rect_visible(rect)
            && let Some(key) = guild.icon_key()
        {
            if demo && !self.textures.iter().any(|(stored, _)| stored == &key) {
                let mut image = ColorImage::filled([32, 32], egui::Color32::from_rgb(88, 101, 242));
                for row in [8, 14, 20] {
                    for y in row..row + 3 {
                        for x in 7..25 {
                            image.pixels[y * 32 + x] = egui::Color32::WHITE;
                        }
                    }
                }
                self.attempts.insert(key.clone(), (Instant::now(), false));
                self.accept(ui.ctx(), key.clone(), Some(image));
            }
            painted = self.paint(ui, &key, rect, radius);
            if !painted && !demo {
                self.request(key);
            }
        }
        if !painted {
            ui.painter().rect_filled(
                rect,
                radius,
                if rounded {
                    colors.accent
                } else {
                    colors.raised
                },
            );
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                short,
                egui::FontId::new(16.0, crate::design::medium_family(ui.ctx())),
                if rounded {
                    colors.accent_text
                } else {
                    colors.text
                },
            );
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                ui.is_enabled(),
                selected,
                format!("Server {}", guild.name),
            )
        });
        response.on_hover_text(&guild.name)
    }
    pub fn show_embed(
        &mut self,
        ui: &mut egui::Ui,
        media: &model::EmbedMedia,
        max_size: egui::Vec2,
        demo: bool,
    ) -> egui::Response {
        self.show_media(ui, media, max_size, demo, false)
    }
    pub fn show_large(
        &mut self,
        ui: &mut egui::Ui,
        media: &model::EmbedMedia,
        max_size: egui::Vec2,
        demo: bool,
    ) -> egui::Response {
        self.show_media(ui, media, max_size, demo, true)
    }
    fn show_media(
        &mut self,
        ui: &mut egui::Ui,
        media: &model::EmbedMedia,
        max_size: egui::Vec2,
        demo: bool,
        large: bool,
    ) -> egui::Response {
        // Reserve geometry from bounded metadata so image arrivals do not move the reading anchor.
        let max_size = egui::vec2(
            max_size
                .x
                .min(ui.available_width())
                .clamp(1.0, if large { 4096.0 } else { 512.0 }),
            max_size.y.clamp(1.0, if large { 4096.0 } else { 512.0 }),
        );
        let original = if media.width > 0 && media.height > 0 {
            egui::vec2(
                media.width.min(16384) as f32,
                media.height.min(16384) as f32,
            )
        } else {
            egui::vec2(320.0, 180.0)
        };
        let scale = (max_size.x / original.x)
            .min(max_size.y / original.y)
            .min(if large { f32::INFINITY } else { 1.0 });
        let size = (original * scale).max(egui::vec2(1.0, 1.0));
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
        if ui.is_rect_visible(rect) {
            let source = media.proxy_url.as_deref().or(media.url.as_deref());
            let key = source
                .filter(|source| source.len() <= 2048)
                .map(|source| format!("embed:{source}"));
            let painted = key
                .as_deref()
                .is_some_and(|key| self.paint(ui, key, rect, 5));
            if !painted {
                let colors = crate::design::palette(ui);
                ui.painter().rect_filled(rect, 5, colors.canvas);
                if demo {
                    // Original native landscape, never a service request or bundled third-party image.
                    let ridge = vec![
                        rect.left_bottom(),
                        rect.left_center(),
                        rect.center_top() + egui::vec2(0.0, rect.height() * 0.35),
                        rect.right_bottom(),
                    ];
                    ui.painter().add(egui::Shape::convex_polygon(
                        ridge,
                        colors.accent.gamma_multiply(0.4),
                        egui::Stroke::NONE,
                    ));
                    ui.painter().circle_filled(
                        rect.min + size * egui::vec2(0.8, 0.25),
                        size.y * 0.08,
                        colors.accent,
                    );
                } else if let Some(key) = key.as_ref() {
                    self.request(key.clone());
                }
                if size.x >= 100.0 && size.y >= 32.0 {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        if demo {
                            "Synthetic preview"
                        } else if key.as_ref().is_some_and(|key| {
                            self.attempts.get(key).is_some_and(|(_, failed)| *failed)
                        }) {
                            "Preview unavailable"
                        } else {
                            "Image preview"
                        },
                        egui::FontId::proportional(11.0),
                        colors.muted,
                    );
                }
            }
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Image, ui.is_enabled(), "Embedded image")
        });
        response
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
                self.attempts.insert(key.clone(), (Instant::now(), false));
                self.accept(ui.ctx(), key.clone(), Some(image));
            }
            let rect = ui
                .layout()
                .align_size_within_rect(egui::Vec2::splat(size), response.rect);
            if !self.paint(ui, &key, rect, (size * 0.5) as u8) && !demo {
                self.request(key);
            }
        }
        if response.hovered() || response.has_focus() {
            ui.painter().circle_stroke(
                ui.layout()
                    .align_size_within_rect(egui::Vec2::splat(size), response.rect)
                    .center(),
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
    fn avatar_artwork_matches_fallback_in_justified_layout() {
        let ctx = egui::Context::default();
        let mut images = Avatars::default();
        let user = User {
            id: model::Id(1),
            name: "Synthetic user".into(),
            avatar: None,
            discriminator: 0,
        };
        let mut response_rect = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            ui.with_layout(
                egui::Layout::top_down(egui::Align::Min).with_cross_justify(true),
                |ui| {
                    ui.set_width(200.0);
                    response_rect = images.show(ui, &user, 36.0, true).rect;
                },
            );
        });
        output.textures_delta.clear();
        let texture = images.textures[0].1.id();
        let fallback = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Circle(circle) if circle.radius == 18.0 => Some(circle.center),
                _ => None,
            })
            .unwrap();
        let artwork = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Rect(rect) if rect.fill_texture_id() == texture => Some(rect.rect),
                _ => None,
            })
            .unwrap();
        assert!(response_rect.width() > 36.0);
        assert_ne!(fallback, response_rect.center());
        assert_eq!(artwork.center(), fallback);
        assert_eq!(artwork.size(), egui::Vec2::splat(36.0));
        output.drop_without_applying_deltas();
    }

    #[test]
    fn media_pixels_keep_aspect_without_moving_the_loading_slot() {
        for dimensions in [[120, 40], [40, 120], [80, 80]] {
            for metadata in [(640, 240), (0, 0), (240, 640)] {
                for large in [false, true] {
                    let ctx = egui::Context::default();
                    let mut images = Avatars::default();
                    let media = model::EmbedMedia {
                        url: Some("https://cdn.discordapp.com/attachments/1/2/test.png".into()),
                        width: metadata.0,
                        height: metadata.1,
                        ..Default::default()
                    };
                    let mut slot = egui::Rect::NOTHING;
                    let output = ctx.run_ui(Default::default(), |ui| {
                        slot = images
                            .show_media(ui, &media, egui::vec2(280.0, 180.0), false, large)
                            .rect;
                    });
                    output.drop_without_applying_deltas();
                    let key = images.take_requests().pop().unwrap();
                    images.accept(
                        &ctx,
                        key,
                        Some(ColorImage::filled(dimensions, egui::Color32::WHITE)),
                    );
                    let texture = images.textures[0].1.id();
                    let output = ctx.run_ui(Default::default(), |ui| {
                        assert_eq!(
                            slot,
                            images
                                .show_media(ui, &media, egui::vec2(280.0, 180.0), false, large)
                                .rect
                        );
                    });
                    let meshes = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
                    let bounds = meshes
                        .iter()
                        .filter_map(|shape| match &shape.primitive {
                            egui::epaint::Primitive::Mesh(mesh) if mesh.texture_id == texture => {
                                Some(mesh.calc_bounds())
                            }
                            _ => None,
                        })
                        .reduce(|a, b| a.union(b))
                        .unwrap();
                    // paint_at rounds to device pixels; allow one pixel of rounding.
                    assert!(
                        (bounds.width()
                            - bounds.height() * dimensions[0] as f32 / dimensions[1] as f32)
                            .abs()
                            <= 2.0
                    );
                    assert!(slot.expand(1.0).contains_rect(bounds));
                    output.drop_without_applying_deltas();
                }
            }
        }
    }

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
        for index in 0..40 {
            let key = format!("embed:synthetic-{index}");
            avatars.request(key.clone());
            avatars.accept(
                &ctx,
                key,
                Some(ColorImage::filled([512, 512], egui::Color32::WHITE)),
            );
        }
        assert_eq!(avatars.textures.len(), 16);
        assert_eq!(
            avatars
                .textures
                .iter()
                .map(|(_, texture)| texture.byte_size())
                .sum::<usize>(),
            TEXTURE_BYTES
        );
        let mut preview = Avatars::default();
        let guild = model::Guild {
            emojis: None,
            id: model::Id(10),
            name: "Synthetic server".into(),
            icon: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()),
        };
        let mut output = ctx.run_ui(Default::default(), |ui| {
            preview.show_guild(ui, &guild, true, true);
        });
        output.textures_delta.clear();
        assert!(preview.take_requests().is_empty());
        assert_eq!(preview.textures.len(), 1);
        assert_eq!(preview.textures[0].1.size(), [32, 32]);
        avatars.request("x".repeat(2055));
        assert!(avatars.attempts.keys().all(|key| key.len() <= 2054));
    }
}
