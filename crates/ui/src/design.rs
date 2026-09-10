//! Discord-style theme tokens, presets and typography shared by every native view.
//!
//! The palette is resolved from egui's light/dark mode plus a process-wide [`Variant`]
//! (standard, deep black, ash grey, or a gradient recolour). Gradient variants paint a
//! backdrop under translucent surfaces; see [`paint_backdrop`].
use egui::{Color32, FontFamily, FontId, RichText, Stroke};
use std::sync::atomic::{AtomicU8, Ordering};

/// Recolour preset layered over the light/dark preference.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum Variant {
    /// Discord's refreshed dark or light surfaces, following the light/dark preference.
    #[default]
    Standard = 0,
    /// Deep black surfaces for OLED displays.
    Onyx = 1,
    /// Classic grey surfaces.
    Ash = 2,
    MidnightBlurple = 3,
    CrimsonMoon = 4,
    Forest = 5,
    Sunset = 6,
}
impl Variant {
    pub const ALL: [Variant; 7] = [
        Variant::Standard,
        Variant::Onyx,
        Variant::Ash,
        Variant::MidnightBlurple,
        Variant::CrimsonMoon,
        Variant::Forest,
        Variant::Sunset,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Variant::Standard => "Default",
            Variant::Onyx => "Onyx",
            Variant::Ash => "Ash",
            Variant::MidnightBlurple => "Midnight Blurple",
            Variant::CrimsonMoon => "Crimson Moon",
            Variant::Forest => "Forest",
            Variant::Sunset => "Sunset",
        }
    }
    /// Stable identifier for persistence.
    pub fn key(self) -> &'static str {
        match self {
            Variant::Standard => "standard",
            Variant::Onyx => "onyx",
            Variant::Ash => "ash",
            Variant::MidnightBlurple => "midnight-blurple",
            Variant::CrimsonMoon => "crimson-moon",
            Variant::Forest => "forest",
            Variant::Sunset => "sunset",
        }
    }
    pub fn from_key(key: &str) -> Option<Variant> {
        Variant::ALL.into_iter().find(|v| v.key() == key)
    }
    /// Gradient variants ignore the light/dark preference and always use dark text.
    pub fn is_gradient(self) -> bool {
        matches!(
            self,
            Variant::MidnightBlurple | Variant::CrimsonMoon | Variant::Forest | Variant::Sunset
        )
    }
    fn from_u8(value: u8) -> Variant {
        Variant::ALL
            .into_iter()
            .find(|v| *v as u8 == value)
            .unwrap_or_default()
    }
}
static VARIANT: AtomicU8 = AtomicU8::new(0);
pub fn variant() -> Variant {
    Variant::from_u8(VARIANT.load(Ordering::Relaxed))
}
/// Select a variant; call [`apply`] afterwards so egui's own widgets follow it.
pub fn set_variant(variant: Variant) {
    VARIANT.store(variant as u8, Ordering::Relaxed);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Palette {
    /// Window frame: title bar and server rail.
    pub base: Color32,
    /// Channel and member lists.
    pub sidebar: Color32,
    /// Conversation area.
    pub chat: Color32,
    /// Composer, cards, inputs and popovers.
    pub raised: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub border: Color32,
    /// Headings and author names.
    pub text_strong: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub link: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    pub positive: Color32,
    pub warning: Color32,
    pub danger: Color32,
    pub mention_bg: Color32,
    pub mention_text: Color32,
    /// Two-stop backdrop gradient (top-left to bottom-right) under translucent surfaces.
    pub backdrop: Option<[Color32; 2]>,
    /// Alias of `chat`, kept for older call sites.
    pub canvas: Color32,
    /// Alias of `sidebar`, kept for older call sites.
    pub surface: Color32,
}
const fn rgb(value: u32) -> Color32 {
    Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}
const fn rgba(value: u32, alpha: u8) -> Color32 {
    Color32::from_rgba_premultiplied(
        ((value >> 16) as u8 as u32 * alpha as u32 / 255) as u8,
        ((value >> 8) as u8 as u32 * alpha as u32 / 255) as u8,
        (value as u8 as u32 * alpha as u32 / 255) as u8,
        alpha,
    )
}
const BLURPLE: Color32 = rgb(0x5865f2);
const MENTION_BG: Color32 = rgba(0x5865f2, 76);
fn dark_common(
    base: Color32,
    sidebar: Color32,
    chat: Color32,
    raised: Color32,
    hover: Color32,
    selected: Color32,
    border: Color32,
) -> Palette {
    Palette {
        base,
        sidebar,
        chat,
        raised,
        hover,
        selected,
        border,
        text_strong: rgb(0xf2f3f5),
        text: rgb(0xdbdee1),
        muted: rgb(0x9a9ba1),
        link: rgb(0x00a8fc),
        accent: BLURPLE,
        accent_text: Color32::WHITE,
        positive: rgb(0x23a55a),
        warning: rgb(0xf0b232),
        danger: rgb(0xf23f43),
        mention_bg: MENTION_BG,
        mention_text: rgb(0xc9cdfb),
        backdrop: None,
        canvas: chat,
        surface: sidebar,
    }
}
fn gradient(stops: [u32; 2]) -> Palette {
    let mut p = dark_common(
        Color32::from_black_alpha(140),
        Color32::from_black_alpha(90),
        Color32::from_black_alpha(90),
        Color32::from_black_alpha(120),
        Color32::from_white_alpha(18),
        Color32::from_white_alpha(34),
        Color32::from_white_alpha(28),
    );
    p.text = rgb(0xe8e9ec);
    p.muted = rgb(0xb2b4bb);
    p.backdrop = Some([rgb(stops[0]), rgb(stops[1])]);
    p
}
pub fn colors(dark: bool, variant: Variant) -> Palette {
    match variant {
        Variant::Standard if dark => dark_common(
            rgb(0x121214),
            rgb(0x1a1a1e),
            rgb(0x1a1a1e),
            rgb(0x222327),
            rgb(0x26272c),
            rgb(0x2f3036),
            rgb(0x29292e),
        ),
        Variant::Standard => Palette {
            base: rgb(0xe3e5e8),
            sidebar: rgb(0xf2f3f5),
            chat: Color32::WHITE,
            raised: rgb(0xebedef),
            hover: rgb(0xe9eaed),
            selected: rgb(0xd7d9dc),
            border: rgb(0xdfe1e5),
            text_strong: rgb(0x060607),
            text: rgb(0x313338),
            muted: rgb(0x5c5e66),
            link: rgb(0x006ce7),
            accent: BLURPLE,
            accent_text: Color32::WHITE,
            positive: rgb(0x23a55a),
            warning: rgb(0xf0b232),
            danger: rgb(0xda373c),
            mention_bg: MENTION_BG,
            mention_text: rgb(0x3c45a5),
            backdrop: None,
            canvas: Color32::WHITE,
            surface: rgb(0xf2f3f5),
        },
        Variant::Onyx => dark_common(
            Color32::BLACK,
            rgb(0x070708),
            rgb(0x070708),
            rgb(0x141416),
            rgb(0x17171a),
            rgb(0x222226),
            rgb(0x1c1c20),
        ),
        Variant::Ash => dark_common(
            rgb(0x1e1f22),
            rgb(0x2b2d31),
            rgb(0x313338),
            rgb(0x383a40),
            rgb(0x35373c),
            rgb(0x404249),
            rgb(0x3f4147),
        ),
        Variant::MidnightBlurple => gradient([0x3c3f9e, 0x0f1030]),
        Variant::CrimsonMoon => gradient([0x8a1d3d, 0x130508]),
        Variant::Forest => gradient([0x2f5f3d, 0x0b1a11]),
        Variant::Sunset => gradient([0xd3653e, 0x3b1d63]),
    }
}
pub fn palette(ui: &egui::Ui) -> Palette {
    colors(ui.visuals().dark_mode, variant())
}
pub fn palette_for(ctx: &egui::Context) -> Palette {
    colors(ctx.theme() == egui::Theme::Dark, variant())
}
/// Paint the gradient backdrop behind every panel; a no-op for opaque variants.
pub fn paint_backdrop(ctx: &egui::Context) {
    let Some([top, bottom]) = palette_for(ctx).backdrop else {
        return;
    };
    let rect = ctx.content_rect();
    let mut mesh = egui::Mesh::default();
    let mid = Color32::from_rgb(
        ((top.r() as u16 + bottom.r() as u16) / 2) as u8,
        ((top.g() as u16 + bottom.g() as u16) / 2) as u8,
        ((top.b() as u16 + bottom.b() as u16) / 2) as u8,
    );
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), mid);
    mesh.colored_vertex(rect.right_bottom(), bottom);
    mesh.colored_vertex(rect.left_bottom(), mid);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ctx.layer_painter(egui::LayerId::background())
        .add(egui::Shape::mesh(mesh));
}

pub const SEMIBOLD: &str = "semibold";
pub const MEDIUM: &str = "medium";
const WEIGHTS_KEY: &str = "serein-font-weights";
/// Called by `fonts::install` for one context; until then the weight families resolve to
/// the default face so headless contexts (tests) never reference an unknown family.
pub fn weights_installed(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique(WEIGHTS_KEY), true));
}
fn weight(ctx: &egui::Context, name: &str) -> FontFamily {
    if ctx.data(|d| d.get_temp::<bool>(egui::Id::unique(WEIGHTS_KEY))) == Some(true) {
        FontFamily::Name(name.into())
    } else {
        FontFamily::Proportional
    }
}
pub fn semibold_family(ctx: &egui::Context) -> FontFamily {
    weight(ctx, SEMIBOLD)
}
pub fn medium_family(ctx: &egui::Context) -> FontFamily {
    weight(ctx, MEDIUM)
}
pub fn semibold(ui: &egui::Ui, text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(FontId::new(size, semibold_family(ui.ctx())))
}
pub fn medium(ui: &egui::Ui, text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).font(FontId::new(size, medium_family(ui.ctx())))
}
/// Uppercase section heading used above channel categories and member groups.
pub fn eyebrow(ui: &egui::Ui, text: impl Into<String>, color: Color32) -> RichText {
    semibold(ui, text.into().to_uppercase(), 12.0).color(color)
}

pub fn apply(ctx: &egui::Context) {
    let variant = variant();
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let p = colors(theme == egui::Theme::Dark, variant);
        let mut style = (*ctx.style_of(theme)).clone();
        style.text_styles.insert(
            egui::TextStyle::Heading,
            FontId::new(20.0, semibold_family(ctx)),
        );
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(15.0));
        style.text_styles.insert(
            egui::TextStyle::Button,
            FontId::new(14.0, medium_family(ctx)),
        );
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(12.0));
        style
            .text_styles
            .insert(egui::TextStyle::Monospace, FontId::monospace(14.0));
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        style.spacing.interact_size.y = 32.0;
        style.spacing.menu_margin = egui::Margin::same(8);
        style.visuals.panel_fill = p.chat;
        style.visuals.window_fill = p.raised.to_opaque();
        style.visuals.window_corner_radius = 8.into();
        style.visuals.menu_corner_radius = 8.into();
        style.visuals.window_stroke = Stroke::new(1.0, p.border);
        style.visuals.window_shadow = egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(
                if p.backdrop.is_some() || theme == egui::Theme::Dark {
                    120
                } else {
                    40
                },
            ),
        };
        style.visuals.popup_shadow = style.visuals.window_shadow;
        style.visuals.override_text_color = Some(p.text);
        style.visuals.weak_text_color = Some(p.muted);
        style.visuals.hyperlink_color = p.link;
        style.visuals.extreme_bg_color = p.raised;
        style.visuals.text_edit_bg_color = Some(p.raised);
        style.visuals.code_bg_color = p.raised;
        style.visuals.faint_bg_color = p.hover;
        style.visuals.selection.bg_fill = p.accent.gamma_multiply(0.35);
        style.visuals.selection.stroke = Stroke::new(1.0, p.accent);
        style.visuals.text_cursor.stroke = Stroke::new(2.0, p.text);
        for widget in [
            &mut style.visuals.widgets.noninteractive,
            &mut style.visuals.widgets.inactive,
            &mut style.visuals.widgets.hovered,
            &mut style.visuals.widgets.active,
            &mut style.visuals.widgets.open,
        ] {
            widget.corner_radius = 4.into();
            widget.fg_stroke = Stroke::new(1.0, p.text);
            widget.bg_stroke = Stroke::NONE;
            widget.expansion = 0.0;
        }
        style.visuals.widgets.noninteractive.bg_fill = p.sidebar;
        style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        style.visuals.widgets.inactive.bg_fill = p.raised;
        style.visuals.widgets.inactive.weak_bg_fill = p.raised;
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
        style.visuals.widgets.hovered.bg_fill = p.selected;
        style.visuals.widgets.hovered.weak_bg_fill = p.hover;
        style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text_strong);
        style.visuals.widgets.active.bg_fill = p.selected;
        style.visuals.widgets.active.weak_bg_fill = p.selected;
        style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, p.text_strong);
        style.visuals.widgets.open.bg_fill = p.selected;
        style.visuals.widgets.open.weak_bg_fill = p.selected;
        style.visuals.widgets.open.fg_stroke = Stroke::new(1.0, p.text_strong);
        ctx.set_style_of(theme, style);
    }
}
pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let p = palette(ui);
    ui.add(
        egui::Button::new(medium(ui, label, 15.0).color(p.accent_text))
            .fill(p.accent)
            .stroke(Stroke::NONE)
            .corner_radius(8)
            .min_size(egui::vec2(ui.available_width(), 44.0)),
    )
}
/// Discord's deterministic fallback avatar colours, keyed by the display name.
fn fallback_avatar_color(name: &str) -> Color32 {
    const COLORS: [u32; 5] = [0x5865f2, 0x757e8a, 0x3ba55c, 0xfaa61a, 0xed4245];
    let hash = name
        .bytes()
        .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
    rgb(COLORS[(hash % COLORS.len() as u32) as usize])
}
pub fn avatar(ui: &mut egui::Ui, name: &str, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let initials: String = name
        .split_whitespace()
        .take(2)
        .filter_map(|part| part.chars().find(|c| c.is_alphanumeric()))
        .flat_map(char::to_uppercase)
        .take(2)
        .collect();
    ui.painter()
        .circle_filled(rect.center(), size * 0.5, fallback_avatar_color(name));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials,
        FontId::new(size * 0.36, semibold_family(ui.ctx())),
        Color32::WHITE,
    );
    response.on_hover_text(name)
}
/// Presence dot with a surface-coloured ring, bottom-right of an avatar `rect`.
pub fn presence_dot(ui: &egui::Ui, rect: egui::Rect, color: Color32, ring: Color32) {
    let radius = (rect.width() * 0.16).clamp(4.0, 8.0);
    let center = rect.right_bottom() - egui::vec2(radius + 0.5, radius + 0.5);
    ui.painter().circle_filled(center, radius + 2.0, ring);
    ui.painter().circle_filled(center, radius, color);
}

/// Preserve role hue where readable, otherwise move toward the theme's text color.
pub fn role_name_color(rgb: u32, background: Color32, fallback: Color32) -> Color32 {
    let role = Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    for step in 0..=16 {
        let mix = |a: u8, b: u8| ((u32::from(a) * (16 - step) + u32::from(b) * step) / 16) as u8;
        let color = Color32::from_rgb(
            mix(role.r(), fallback.r()),
            mix(role.g(), fallback.g()),
            mix(role.b(), fallback.b()),
        );
        if contrast(color, background) >= 4.5 {
            return color;
        }
    }
    fallback
}
fn luminance(c: Color32) -> f32 {
    let channel = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}
fn contrast(a: Color32, b: Color32) -> f32 {
    let (l1, l2) = (luminance(a) + 0.05, luminance(b) + 0.05);
    l1.max(l2) / l1.min(l2)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn role_colors_remain_readable_in_light_and_dark_palettes() {
        for variant in Variant::ALL {
            for dark in [false, true] {
                let p = colors(dark, variant);
                for rgb in [0, 0xffffff, 0xff0000, 0x00ff00, 0x0000ff, 0xe78284] {
                    for background in [p.sidebar, p.hover] {
                        assert!(
                            contrast(role_name_color(rgb, background, p.text), background) >= 4.5
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn opaque_presets_keep_readable_text_and_keys_round_trip() {
        for variant in Variant::ALL {
            assert_eq!(Variant::from_key(variant.key()), Some(variant));
            for dark in [true, false] {
                let p = colors(dark, variant);
                assert_eq!(p.canvas, p.chat);
                assert_eq!(p.surface, p.sidebar);
                if p.backdrop.is_none() {
                    assert!(contrast(p.text, p.chat) >= 7.0, "{variant:?} body text");
                    assert!(
                        contrast(p.muted, p.sidebar) >= 4.5,
                        "{variant:?} muted text"
                    );
                    assert!(
                        contrast(p.accent_text, p.accent) >= 4.5,
                        "{variant:?} accent text"
                    );
                } else {
                    assert!(variant.is_gradient());
                }
            }
        }
        assert_eq!(Variant::from_key("nonsense"), None);
        assert_eq!(Variant::from_u8(200), Variant::Standard);
    }
}
