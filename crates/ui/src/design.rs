//! Discord-style theme tokens, presets and typography shared by every native view.
//!
//! The palette is resolved from egui's light/dark mode plus a process-wide [`Variant`]
//! (standard, deep black, ash grey, or a gradient recolour). Gradient variants paint a
//! backdrop under translucent surfaces; see [`paint_backdrop`].
use egui::{Color32, FontFamily, FontId, RichText, Stroke};
use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};

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
static PRIMARY_COLOR: AtomicU32 = AtomicU32::new(0);

pub fn primary_color() -> Option<[u8; 3]> {
	let value = PRIMARY_COLOR.load(Ordering::Relaxed);
	(value != 0).then_some([(value >> 16) as u8, (value >> 8) as u8, value as u8])
}
pub fn set_primary_color(primary: Option<[u8; 3]>) {
	// The high byte distinguishes custom black from the default accent.
	PRIMARY_COLOR.store(
		primary.map_or(0, |[r, g, b]| u32::from_be_bytes([1, r, g, b])),
		Ordering::Relaxed,
	);
}
/// Shared swatch and direct #RRGGBB input for app, profile and folder colors.
pub fn color_edit(ui: &mut egui::Ui, color: &mut [u8; 3]) -> egui::Response {
	let swatch = ui.color_edit_button_srgb(color);
	let mut value = u32::from_be_bytes([0, color[0], color[1], color[2]]);
	let hex = ui
		.add(
			egui::DragValue::new(&mut value)
				.range(0..=0xFFFFFF)
				.hexadecimal(6, false, true)
				.prefix("#")
				.custom_parser(|text| parse_hex_color(text).map(f64::from))
				.update_while_editing(false),
		)
		.on_hover_text("Hex color: #RRGGBB. Click to type or paste.");
	if hex.changed() {
		let [_, r, g, b] = value.to_be_bytes();
		*color = [r, g, b];
	}
	swatch | hex
}
fn parse_hex_color(text: &str) -> Option<u32> {
	let text = text.trim();
	let text = text.strip_prefix('#').unwrap_or(text);
	if text.len() != 6 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
		return None;
	}
	u32::from_str_radix(text, 16).ok()
}
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
pub const DEFAULT_PRIMARY_COLOR: [u8; 3] = [88, 101, 242];
const PRIMARY: Color32 = Color32::from_rgb(
	DEFAULT_PRIMARY_COLOR[0],
	DEFAULT_PRIMARY_COLOR[1],
	DEFAULT_PRIMARY_COLOR[2],
);
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
		accent: PRIMARY,
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
pub fn builtin_colors(dark: bool, variant: Variant) -> Palette {
	let palette = match variant {
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
			accent: PRIMARY,
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
	};
	customize(palette, primary_color())
}
#[derive(Clone, Copy)]
struct ExtensionPalette {
	colors: [Option<Color32>; 18],
	backdrop: Option<[Color32; 2]>,
}
thread_local! {
	static EXTENSION_THEME: std::cell::Cell<Option<[ExtensionPalette; 2]>> = const { std::cell::Cell::new(None) };
}
const THEME_FIELDS: [&str; 18] = [
	"base",
	"sidebar",
	"chat",
	"raised",
	"hover",
	"selected",
	"border",
	"text_strong",
	"text",
	"muted",
	"link",
	"accent",
	"accent_text",
	"positive",
	"warning",
	"danger",
	"mention_bg",
	"mention_text",
];
fn extension_palette(theme: &extensions::ThemePalette) -> Option<ExtensionPalette> {
	let color = |text: &str| {
		extensions::parse_color(text)
			.ok()
			.map(|[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a))
	};
	let mut colors = [None; 18];
	for (name, value) in &theme.colors {
		let index = THEME_FIELDS.iter().position(|field| *field == name)?;
		colors[index] = Some(color(value)?);
	}
	let backdrop = match &theme.backdrop {
		Some([a, b]) => Some([color(a)?, color(b)?]),
		None => None,
	};
	Some(ExtensionPalette { colors, backdrop })
}
/// Install prevalidated color overrides; malformed themes reset to the built-in appearance.
/// Call [`apply`] after changing this value. No parsing or allocation runs while drawing.
pub fn set_extension_theme(theme: Option<&extensions::Theme>) {
	let palettes = theme.and_then(|theme| {
		Some([
			extension_palette(&theme.light)?,
			extension_palette(&theme.dark)?,
		])
	});
	EXTENSION_THEME.set(palettes);
}
fn recolor(mut palette: Palette, theme: ExtensionPalette) -> Palette {
	for (destination, color) in [
		&mut palette.base,
		&mut palette.sidebar,
		&mut palette.chat,
		&mut palette.raised,
		&mut palette.hover,
		&mut palette.selected,
		&mut palette.border,
		&mut palette.text_strong,
		&mut palette.text,
		&mut palette.muted,
		&mut palette.link,
		&mut palette.accent,
		&mut palette.accent_text,
		&mut palette.positive,
		&mut palette.warning,
		&mut palette.danger,
		&mut palette.mention_bg,
		&mut palette.mention_text,
	]
	.into_iter()
	.zip(theme.colors)
	{
		if let Some(color) = color {
			*destination = color;
		}
	}
	palette.backdrop = theme.backdrop;
	palette.canvas = palette.chat;
	palette.surface = palette.sidebar;
	palette
}
pub fn colors(dark: bool, variant: Variant) -> Palette {
	let mut palette = builtin_colors(dark, variant);
	if let Some(palettes) = EXTENSION_THEME.get() {
		palette = recolor(palette, palettes[usize::from(dark)]);
	}
	customize(palette, primary_color())
}

fn customize(mut palette: Palette, primary: Option<[u8; 3]>) -> Palette {
	if let Some([r, g, b]) = primary {
		palette.accent = Color32::from_rgb(r, g, b);
		palette.accent_text = if contrast(Color32::WHITE, palette.accent) >= 4.5 {
			Color32::WHITE
		} else {
			Color32::BLACK
		};
	}
	palette
}
pub fn palette(ui: &egui::Ui) -> Palette {
	opaque_surfaces(colors(ui.visuals().dark_mode, variant()))
}
pub fn palette_for(ctx: &egui::Context) -> Palette {
	opaque_surfaces(colors(ctx.theme() == egui::Theme::Dark, variant()))
}
/// Main surfaces preserve gradient presets; widgets and popouts use opaque surfaces.
pub fn window_palette(ui: &egui::Ui) -> Palette {
	colors(ui.visuals().dark_mode, variant())
}
fn opaque_surfaces(mut palette: Palette) -> Palette {
	let backdrop = palette
		.backdrop
		.map_or(palette.chat.to_opaque(), |[top, bottom]| {
			mix(top, bottom, 0.5)
		});
	for surface in [
		&mut palette.base,
		&mut palette.sidebar,
		&mut palette.chat,
		&mut palette.raised,
		&mut palette.canvas,
		&mut palette.surface,
	] {
		*surface = backdrop.blend(*surface);
	}
	palette
}
/// Paint the gradient backdrop behind every panel; a no-op for opaque variants.
pub fn paint_backdrop(ctx: &egui::Context) {
	let Some([top, bottom]) = colors(ctx.theme() == egui::Theme::Dark, variant()).backdrop else {
		return;
	};
	let rect = ctx.content_rect();
	let mut mesh = egui::Mesh::default();
	let mid = Color32::from_rgba_premultiplied(
		((top.r() as u16 + bottom.r() as u16) / 2) as u8,
		((top.g() as u16 + bottom.g() as u16) / 2) as u8,
		((top.b() as u16 + bottom.b() as u16) / 2) as u8,
		((top.a() as u16 + bottom.a() as u16) / 2) as u8,
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
// Called by `fonts::install` for one context; until then the weight families resolve to
// the default face so headless contexts (tests) never reference an unknown family.
thread_local! {
	// One context per UI thread, compared by egui's Arc identity; headless contexts stay isolated.
	static WEIGHT_CONTEXT: std::cell::RefCell<Option<(egui::Context, bool)>> = const { std::cell::RefCell::new(None) };
}
pub fn weights_installed(ctx: &egui::Context) {
	ctx.data_mut(|d| d.insert_temp(egui::Id::unique(WEIGHTS_KEY), true));
	WEIGHT_CONTEXT.with(|cache| *cache.borrow_mut() = Some((ctx.clone(), true)));
}
fn weight(ctx: &egui::Context, name: &str) -> FontFamily {
	let installed = WEIGHT_CONTEXT.with(|cache| {
		let mut cache = cache.borrow_mut();
		if let Some((cached, installed)) = &*cache
			&& cached == ctx
		{
			return *installed;
		}
		let installed =
			ctx.data(|d| d.get_temp::<bool>(egui::Id::unique(WEIGHTS_KEY))) == Some(true);
		*cache = Some((ctx.clone(), installed));
		installed
	});
	if installed {
		{
			static MEDIUM_NAME: std::sync::OnceLock<std::sync::Arc<str>> =
				std::sync::OnceLock::new();
			static SEMIBOLD_NAME: std::sync::OnceLock<std::sync::Arc<str>> =
				std::sync::OnceLock::new();
			let cached = if name == MEDIUM {
				&MEDIUM_NAME
			} else {
				&SEMIBOLD_NAME
			};
			FontFamily::Name(cached.get_or_init(|| name.into()).clone())
		}
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
		let p = opaque_surfaces(colors(theme == egui::Theme::Dark, variant));
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
/// Space reserved at the left of window strips for macOS traffic lights.
pub const TRAFFIC_LIGHT_INSET: f32 = if cfg!(target_os = "macos") { 72.0 } else { 0.0 };
/// Width of the Windows caption buttons drawn by [`window_controls`]; zero elsewhere.
pub const WINDOW_CONTROLS_WIDTH: f32 = if cfg!(target_os = "windows") {
	3.0 * 46.0
} else {
	0.0
};

/// Make `rect` behave like a native title bar: drag moves the window and, where the app
/// draws its own frame (Windows), a double click toggles maximize.
pub fn window_drag(ui: &mut egui::Ui, rect: egui::Rect) {
	let response = ui.interact(
		rect,
		ui.id().with("window-drag"),
		egui::Sense::click_and_drag(),
	);
	if response.drag_started() {
		ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
	}
	if cfg!(target_os = "windows") && response.double_clicked() {
		let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
		ui.ctx()
			.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
	}
}

/// Discord-style caption buttons (minimize, maximize/restore, close) for the undecorated
/// Windows frame. Lay out in a right-to-left `ui`; a no-op on other platforms.
pub fn window_controls(ui: &mut egui::Ui) {
	if !cfg!(target_os = "windows") {
		return;
	}
	let p = palette(ui);
	let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
	let height = ui.available_height().clamp(28.0, 36.0);
	ui.spacing_mut().item_spacing.x = 0.0;
	let caption =
		|ui: &mut egui::Ui, label: &str, danger: bool| -> (egui::Response, egui::Rect, Color32) {
			let (rect, response) =
				ui.allocate_exact_size(egui::vec2(46.0, height), egui::Sense::click());
			response
				.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
			let hovered = response.hovered() || response.has_focus();
			if hovered {
				ui.painter()
					.rect_filled(rect, 0, if danger { rgb(0xc42b1c) } else { p.hover });
			}
			let color = if hovered && danger {
				Color32::WHITE
			} else if hovered {
				p.text_strong
			} else {
				p.text
			};
			(response, rect, color)
		};
	let (close, rect, color) = caption(ui, "Close", true);
	let c = rect.center();
	let stroke = Stroke::new(1.0, color);
	ui.painter().line_segment(
		[c + egui::vec2(-5.0, -5.0), c + egui::vec2(5.0, 5.0)],
		stroke,
	);
	ui.painter().line_segment(
		[c + egui::vec2(-5.0, 5.0), c + egui::vec2(5.0, -5.0)],
		stroke,
	);
	if close.clicked() {
		ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
	}
	let (toggle, rect, color) = caption(ui, if maximized { "Restore" } else { "Maximize" }, false);
	let c = rect.center();
	let stroke = Stroke::new(1.0, color);
	if maximized {
		let back = egui::Rect::from_center_size(c + egui::vec2(1.5, -1.5), egui::Vec2::splat(9.0));
		let front = egui::Rect::from_center_size(c + egui::vec2(-1.5, 1.5), egui::Vec2::splat(9.0));
		ui.painter().line_segment(
			[back.left_top() + egui::vec2(0.0, 0.0), back.right_top()],
			stroke,
		);
		ui.painter()
			.line_segment([back.right_top(), back.right_bottom()], stroke);
		ui.painter().line_segment(
			[back.left_top(), back.left_top() + egui::vec2(0.0, 3.0)],
			stroke,
		);
		ui.painter().line_segment(
			[
				back.right_bottom(),
				back.right_bottom() - egui::vec2(3.0, 0.0),
			],
			stroke,
		);
		ui.painter()
			.rect_stroke(front, 1, stroke, egui::StrokeKind::Middle);
	} else {
		let square = egui::Rect::from_center_size(c, egui::Vec2::splat(10.0));
		ui.painter()
			.rect_stroke(square, 1, stroke, egui::StrokeKind::Middle);
	}
	if toggle.clicked() {
		ui.ctx()
			.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
	}
	let (minimize, rect, color) = caption(ui, "Minimize", false);
	let c = rect.center();
	ui.painter().line_segment(
		[c + egui::vec2(-5.0, 0.5), c + egui::vec2(5.0, 0.5)],
		Stroke::new(1.0, color),
	);
	if minimize.clicked() {
		ui.ctx()
			.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
	}
	ui.add_space(6.0);
}

/// Full-width accent call to action with centred text and an optional leading icon.
pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
	let p = palette(ui);
	wide_button(ui, label, None, p.accent, Stroke::NONE, p.accent_text)
}
pub fn primary_icon_button(
	ui: &mut egui::Ui,
	icon: crate::icons::Icon,
	label: &str,
) -> egui::Response {
	let p = palette(ui);
	wide_button(ui, label, Some(icon), p.accent, Stroke::NONE, p.accent_text)
}
/// Full-width neutral companion to [`primary_button`].
pub fn secondary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
	let p = palette(ui);
	wide_button(
		ui,
		label,
		None,
		p.raised,
		Stroke::new(1.0, p.border),
		p.text_strong,
	)
}
fn wide_button(
	ui: &mut egui::Ui,
	label: &str,
	icon: Option<crate::icons::Icon>,
	fill: Color32,
	stroke: Stroke,
	text: Color32,
) -> egui::Response {
	let p = palette(ui);
	let (rect, response) =
		ui.allocate_exact_size(egui::vec2(ui.available_width(), 44.0), egui::Sense::click());
	response.widget_info(|| {
		egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
	});
	let enabled = ui.is_enabled();
	let fill = if !enabled {
		fill.gamma_multiply(0.5)
	} else if response.is_pointer_button_down_on() {
		fill.gamma_multiply(0.85)
	} else if response.hovered() {
		fill.linear_multiply(1.12)
	} else {
		fill
	};
	let text = if enabled {
		text
	} else {
		text.gamma_multiply(0.6)
	};
	let painter = ui.painter();
	painter.rect(rect, 8, fill, stroke, egui::StrokeKind::Inside);
	if response.has_focus() {
		painter.rect_stroke(
			rect.expand(2.0),
			10,
			Stroke::new(2.0, p.accent),
			egui::StrokeKind::Outside,
		);
	}
	let galley = painter.layout_no_wrap(
		label.to_owned(),
		FontId::new(15.0, medium_family(ui.ctx())),
		text,
	);
	let icon_size = if icon.is_some() { 20.0 } else { 0.0 };
	let gap = if icon.is_some() { 10.0 } else { 0.0 };
	let total = icon_size + gap + galley.size().x;
	let mut x = rect.center().x - total * 0.5;
	if let Some(icon) = icon {
		let icon_rect = egui::Rect::from_center_size(
			egui::pos2(x + icon_size * 0.5, rect.center().y),
			egui::Vec2::splat(icon_size),
		);
		crate::icons::paint(painter, icon, icon_rect, text);
		x += icon_size + gap;
	}
	painter.galley(
		egui::pos2(x, rect.center().y - galley.size().y * 0.5),
		galley,
		text,
	);
	if enabled && response.hovered() {
		ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
	}
	response
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
	paint_avatar(ui, name, size, rect);
	response.on_hover_text(name)
}
pub(crate) fn paint_avatar(ui: &egui::Ui, name: &str, size: f32, rect: egui::Rect) {
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
	fn hex_colors_accept_pasted_values_without_applying_partial_or_invalid_input() {
		for text in ["#FF4000", "ff4000", " #Ff4000 "] {
			assert_eq!(parse_hex_color(text), Some(0xFF4000));
		}
		assert_eq!(parse_hex_color("#000000"), Some(0));
		assert_eq!(parse_hex_color("FFFFFF"), Some(0xFFFFFF));
		for text in [
			"",
			"#FF4",
			"#FF40000",
			"#FF4000FF",
			"#GG4000",
			"+FF400",
			"éFF400",
		] {
			assert_eq!(parse_hex_color(text), None);
		}
	}
	#[test]
	fn primary_color_preserves_readable_controls() {
		for dark in [false, true] {
			let base = colors(dark, Variant::Standard);
			for rgb in [[0, 0, 0], [255, 255, 255], [255, 220, 0], [90, 40, 200]] {
				let p = customize(base, Some(rgb));
				assert_eq!(p.accent.to_array()[..3], rgb);
				assert!(contrast(p.accent_text, p.accent) >= 4.5);
				assert_eq!((p.text, p.raised), (base.text, base.raised));
			}
			assert_eq!(customize(base, None), base);
			assert_eq!(base.accent, Color32::from_rgb(88, 101, 242));
		}
	}
	#[test]
	fn popup_surfaces_stay_opaque_for_every_preset() {
		for variant in Variant::ALL {
			for dark in [false, true] {
				let base = colors(dark, variant);
				let popup = opaque_surfaces(base);
				for surface in [
					popup.base,
					popup.sidebar,
					popup.chat,
					popup.raised,
					popup.canvas,
					popup.surface,
				] {
					assert_eq!(surface.a(), 255);
				}
				assert_eq!(popup.text, base.text);
			}
		}
	}
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

/// Release channel shown in the title bar; stable builds show nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Channel {
	#[default]
	Stable,
	Nightly,
	Dev,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Build {
	pub channel: Channel,
	pub version: &'static str,
}
/// Gradient release pill with a glow, a channel glyph and the version. None for stable builds.
pub fn build_badge(ui: &mut egui::Ui, build: Build) -> Option<egui::Response> {
	let (label, hint, stops) = match build.channel {
		Channel::Stable => return None,
		Channel::Nightly => (
			"NIGHTLY",
			"Nightly build from the latest main. Unofficial client; live compatibility is unverified.",
			[rgb(0x5865f2), rgb(0xa06cff)],
		),
		Channel::Dev => (
			"DEV",
			"Local development build. Unofficial client; live compatibility is unverified.",
			[rgb(0xf0b232), rgb(0xe8590c)],
		),
	};
	let font = FontId::new(10.0, semibold_family(ui.ctx()));
	let title = ui
		.painter()
		.layout_no_wrap(label.to_owned(), font, Color32::WHITE);
	let version = (!build.version.is_empty()).then(|| {
		ui.painter().layout_no_wrap(
			format!("v{}", build.version),
			FontId::monospace(10.0),
			Color32::from_white_alpha(210),
		)
	});
	const HEIGHT: f32 = 20.0;
	const PAD: f32 = 8.0;
	const GLYPH: f32 = 12.0;
	let width = PAD
		+ GLYPH
		+ 5.0 + title.size().x
		+ version.as_ref().map_or(0.0, |v| 13.0 + v.size().x)
		+ PAD;
	let (rect, response) = ui.allocate_exact_size(egui::vec2(width, HEIGHT), egui::Sense::hover());
	let painter = ui.painter();
	let radius = HEIGHT / 2.0;
	// Soft glow, then a pill whose caps carry the gradient end colours.
	painter.rect_filled(
		rect.expand(3.0),
		radius + 3.0,
		stops[0].gamma_multiply(0.14),
	);
	painter.rect_filled(
		rect.expand(1.0),
		radius + 1.0,
		stops[0].gamma_multiply(0.22),
	);
	let left = egui::pos2(rect.left() + radius, rect.center().y);
	let right = egui::pos2(rect.right() - radius, rect.center().y);
	painter.circle_filled(left, radius, stops[0]);
	painter.circle_filled(right, radius, stops[1]);
	let mut mesh = egui::Mesh::default();
	mesh.colored_vertex(egui::pos2(left.x, rect.top()), stops[0]);
	mesh.colored_vertex(egui::pos2(right.x, rect.top()), stops[1]);
	mesh.colored_vertex(egui::pos2(right.x, rect.bottom()), stops[1]);
	mesh.colored_vertex(egui::pos2(left.x, rect.bottom()), stops[0]);
	mesh.add_triangle(0, 1, 2);
	mesh.add_triangle(0, 2, 3);
	painter.add(egui::Shape::mesh(mesh));
	// Top highlight line for a glassy edge.
	painter.line_segment(
		[
			egui::pos2(rect.left() + radius, rect.top() + 1.0),
			egui::pos2(rect.right() - radius, rect.top() + 1.0),
		],
		Stroke::new(1.0, Color32::from_white_alpha(56)),
	);
	let glyph = egui::Rect::from_center_size(
		egui::pos2(rect.left() + PAD + GLYPH / 2.0, rect.center().y),
		egui::Vec2::splat(GLYPH),
	);
	match build.channel {
		Channel::Nightly => {
			// Crescent moon: a white disc with a pill-coloured bite.
			painter.circle_filled(glyph.center(), 4.6, Color32::WHITE);
			painter.circle_filled(glyph.center() + egui::vec2(2.4, -1.8), 4.0, stops[0]);
		}
		Channel::Dev | Channel::Stable => {
			// Code chevrons: < >
			let s = Stroke::new(1.5, Color32::WHITE);
			let c = glyph.center();
			painter.line_segment([c + egui::vec2(-1.5, -3.5), c + egui::vec2(-4.5, 0.0)], s);
			painter.line_segment([c + egui::vec2(-4.5, 0.0), c + egui::vec2(-1.5, 3.5)], s);
			painter.line_segment([c + egui::vec2(1.5, -3.5), c + egui::vec2(4.5, 0.0)], s);
			painter.line_segment([c + egui::vec2(4.5, 0.0), c + egui::vec2(1.5, 3.5)], s);
		}
	}
	let mut x = glyph.right() + 5.0;
	painter.galley(
		egui::pos2(x, rect.center().y - title.size().y / 2.0),
		title.clone(),
		Color32::WHITE,
	);
	x += title.size().x;
	if let Some(version) = version {
		x += 6.0;
		painter.line_segment(
			[
				egui::pos2(x, rect.top() + 5.0),
				egui::pos2(x, rect.bottom() - 5.0),
			],
			Stroke::new(1.0, Color32::from_white_alpha(90)),
		);
		x += 7.0;
		painter.galley(
			egui::pos2(x, rect.center().y - version.size().y / 2.0),
			version,
			Color32::WHITE,
		);
	}
	Some(response.on_hover_text(hint))
}

/// Discord-style settings row with a pill switch on the right. Clicking anywhere on the row
/// toggles `enabled`; the accessible label is `label`.
pub fn switch(
	ui: &mut egui::Ui,
	label: &str,
	description: Option<&str>,
	enabled: &mut bool,
) -> egui::Response {
	let p = palette(ui);
	let width = ui.available_width();
	let text_width = (width - 64.0).max(80.0);
	let title = ui.painter().layout(
		label.to_owned(),
		FontId::new(16.0, medium_family(ui.ctx())),
		p.text_strong,
		text_width,
	);
	let detail = description.map(|text| {
		ui.painter().layout(
			text.to_owned(),
			FontId::proportional(13.0),
			p.muted,
			text_width,
		)
	});
	let text_height = title.size().y + detail.as_ref().map_or(0.0, |d| d.size().y + 4.0);
	let (rect, mut response) = ui.allocate_exact_size(
		egui::vec2(width, text_height.max(24.0) + 16.0),
		egui::Sense::click(),
	);
	if response.clicked() {
		*enabled = !*enabled;
		response.mark_changed();
	}
	response.widget_info(|| {
		egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *enabled, label)
	});
	let painter = ui.painter();
	if response.hovered() && ui.is_enabled() {
		painter.rect_filled(rect.expand2(egui::vec2(8.0, 0.0)), 6, p.hover);
	}
	let mut y = rect.top() + 8.0;
	painter.galley(egui::pos2(rect.left(), y), title.clone(), p.text_strong);
	y += title.size().y + 4.0;
	if let Some(detail) = detail {
		painter.galley(egui::pos2(rect.left(), y), detail, p.muted);
	}
	let pill = egui::Rect::from_center_size(
		egui::pos2(rect.right() - 20.0, rect.top() + 8.0 + title.size().y / 2.0),
		egui::vec2(40.0, 24.0),
	);
	let mut fill = if *enabled { p.positive } else { p.muted };
	if !ui.is_enabled() {
		fill = fill.gamma_multiply(0.4);
	}
	painter.rect_filled(pill, 12, fill);
	let knob = egui::pos2(
		if *enabled {
			pill.right() - 12.0
		} else {
			pill.left() + 12.0
		},
		pill.center().y,
	);
	painter.circle_filled(knob, 9.0, Color32::WHITE);
	crate::icons::paint(
		painter,
		if *enabled {
			crate::icons::Icon::Check
		} else {
			crate::icons::Icon::Close
		},
		egui::Rect::from_center_size(knob, egui::Vec2::splat(10.0)),
		fill,
	);
	if response.has_focus() {
		painter.rect_stroke(
			pill.expand(3.0),
			15,
			Stroke::new(2.0, p.accent),
			egui::StrokeKind::Outside,
		);
	}
	response
}

/// Rounded settings card that groups related rows on the raised surface.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
	let p = palette(ui);
	egui::Frame::new()
		.fill(p.raised)
		.stroke(Stroke::new(1.0, p.border))
		.corner_radius(8)
		.inner_margin(egui::Margin::symmetric(16, 12))
		.show(ui, |ui| {
			ui.set_width(ui.available_width());
			add(ui)
		})
		.inner
}

/// Linear blend of two colours in premultiplied space; `t` = 0 keeps `a`, 1 gives `b`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
	let lerp = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
	Color32::from_rgba_premultiplied(
		lerp(a.r(), b.r()),
		lerp(a.g(), b.g()),
		lerp(a.b(), b.b()),
		lerp(a.a(), b.a()),
	)
}

#[cfg(test)]
mod extension_theme_tests {
	use super::*;
	#[test]
	fn extension_colors_keep_aliases_and_user_accent() {
		let theme = extensions::ThemePalette {
			colors: [
				("chat".into(), "#112233".into()),
				("sidebar".into(), "#445566".into()),
				("accent".into(), "#ff0000".into()),
			]
			.into(),
			backdrop: Some(["#010203".into(), "#040506".into()]),
		};
		let palette = recolor(
			builtin_colors(true, Variant::Standard),
			extension_palette(&theme).unwrap(),
		);
		assert_eq!(palette.chat, rgb(0x112233));
		assert_eq!(palette.canvas, palette.chat);
		assert_eq!(palette.surface, palette.sidebar);
		assert_eq!(palette.backdrop, Some([rgb(0x010203), rgb(0x040506)]));
		assert_eq!(customize(palette, Some([3, 4, 5])).accent, rgb(0x030405));
		let malformed = extensions::ThemePalette {
			colors: [("chat".into(), "invalid".into())].into(),
			..Default::default()
		};
		assert!(extension_palette(&malformed).is_none());
	}
}
