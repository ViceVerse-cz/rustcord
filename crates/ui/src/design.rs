//! Shared native colors and spacing for sign-in and conversations.
use egui::{Color32, FontId, RichText, Stroke};

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: Color32,
    pub surface: Color32,
    pub raised: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
}
fn colors(dark: bool) -> Palette {
    if dark {
        Palette {
            canvas: Color32::from_rgb(25, 29, 33),
            surface: Color32::from_rgb(31, 36, 41),
            raised: Color32::from_rgb(41, 47, 53),
            border: Color32::from_rgb(61, 69, 76),
            text: Color32::from_rgb(234, 238, 240),
            muted: Color32::from_rgb(158, 171, 181),
            accent: Color32::from_rgb(132, 213, 194),
            accent_text: Color32::from_rgb(18, 49, 43),
        }
    } else {
        Palette {
            canvas: Color32::from_rgb(251, 250, 247),
            surface: Color32::from_rgb(241, 241, 237),
            raised: Color32::from_rgb(232, 236, 232),
            border: Color32::from_rgb(209, 216, 211),
            text: Color32::from_rgb(34, 46, 45),
            muted: Color32::from_rgb(88, 106, 102),
            accent: Color32::from_rgb(30, 106, 88),
            accent_text: Color32::WHITE,
        }
    }
}
pub fn palette(ui: &egui::Ui) -> Palette {
    colors(ui.visuals().dark_mode)
}
pub fn apply(ctx: &egui::Context) {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let p = colors(theme == egui::Theme::Dark);
        let mut style = (*ctx.style_of(theme)).clone();
        style
            .text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(22.0));
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(15.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(12.0));
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, FontId::monospace(14.0));
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.spacing.interact_size.y = 32.0;
        style.visuals.panel_fill = p.canvas;
        style.visuals.window_fill = p.surface;
        style.visuals.window_corner_radius = 12.into();
        style.visuals.menu_corner_radius = 10.into();
        style.visuals.window_stroke = Stroke::new(1.0, p.border);
        style.visuals.weak_text_color = Some(p.muted);
        style.visuals.hyperlink_color = p.accent;
        style.visuals.extreme_bg_color = p.canvas;
        style.visuals.text_edit_bg_color = Some(p.raised);
        style.visuals.code_bg_color = p.raised;
        style.visuals.faint_bg_color = p.surface;
        style.visuals.selection.bg_fill = p.accent.gamma_multiply(0.25);
        style.visuals.selection.stroke = Stroke::new(1.0, p.accent);
        for widget in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.corner_radius = 7.into();
            widget.fg_stroke = Stroke::new(1.0, p.text);
            widget.bg_stroke = Stroke::new(1.0, p.border);
        }
        style.visuals.widgets.noninteractive.bg_fill = p.surface;
        style.visuals.widgets.inactive.bg_fill = p.raised;
        style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
        style.visuals.widgets.hovered.bg_fill = p.raised;
        style.visuals.widgets.hovered.weak_bg_fill = p.raised;
        style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
        style.visuals.widgets.active.bg_fill = p.raised;
        style.visuals.widgets.active.bg_stroke = Stroke::new(1.5, p.accent);
        style.visuals.widgets.open.bg_fill = p.raised;
        ctx.set_style_of(theme, style);
    }
}
pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let p = palette(ui);
    ui.add(
        egui::Button::new(RichText::new(label).color(p.accent_text).strong())
            .fill(p.accent)
            .stroke(Stroke::NONE)
            .min_size(egui::vec2(ui.available_width(), 44.0)),
    )
}
pub fn avatar(ui: &mut egui::Ui, name: &str, size: f32) -> egui::Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let initials: String = name
        .split_whitespace()
        .take(2)
        .filter_map(|part| part.chars().find(|c| c.is_alphanumeric()))
        .flat_map(char::to_uppercase)
        .take(2)
        .collect();
    ui.painter()
        .circle_filled(rect.center(), size * 0.5, p.raised);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials,
        FontId::proportional(size * 0.37),
        p.accent,
    );
    response.on_hover_text(name)
}
