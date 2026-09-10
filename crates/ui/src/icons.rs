//! Vector glyphs painted with egui primitives; no icon font or bitmap assets.
use crate::design;
use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Vec2, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    ChevronDown,
    ChevronRight,
    Gear,
    Microphone,
    Headphones,
    Pin,
    People,
    Search,
    Plus,
    Smile,
    Bell,
    Phone,
    Video,
    Reply,
    Pencil,
    More,
    Inbox,
    Help,
    Reload,
    Threads,
    Speaker,
    Hash,
    Forum,
    Send,
    Attach,
    Close,
    External,
}

/// Paint `icon` centred in `rect` with `color`.
pub fn paint(painter: &egui::Painter, icon: Icon, rect: Rect, color: Color32) {
    let size = rect.width().min(rect.height());
    let rect = Rect::from_center_size(rect.center(), Vec2::splat(size));
    let p = |x: f32, y: f32| -> Pos2 { rect.min + vec2(size * x, size * y) };
    let width = (size / 12.0).clamp(1.4, 2.2);
    let stroke = Stroke::new(width, color);
    let line = |points: Vec<Pos2>| Shape::line(points, stroke);
    match icon {
        Icon::ChevronDown => painter.add(line(vec![p(0.25, 0.38), p(0.5, 0.63), p(0.75, 0.38)])),
        Icon::ChevronRight => painter.add(line(vec![p(0.38, 0.25), p(0.63, 0.5), p(0.38, 0.75)])),
        Icon::Gear => {
            let c = rect.center();
            for i in 0..8 {
                let angle = std::f32::consts::TAU * i as f32 / 8.0;
                let dir = vec2(angle.cos(), angle.sin());
                painter.line_segment(
                    [c + dir * size * 0.28, c + dir * size * 0.44],
                    Stroke::new(width * 1.6, color),
                );
            }
            painter.circle_stroke(c, size * 0.3, Stroke::new(width * 1.4, color));
            painter.circle_stroke(c, size * 0.11, stroke)
        }
        Icon::Microphone => {
            painter.rect_filled(
                Rect::from_min_max(p(0.36, 0.08), p(0.64, 0.56)),
                (size * 0.14) as u8,
                color,
            );
            painter.add(line(vec![
                p(0.22, 0.45),
                p(0.24, 0.62),
                p(0.5, 0.74),
                p(0.76, 0.62),
                p(0.78, 0.45),
            ]));
            painter.line_segment([p(0.5, 0.74), p(0.5, 0.9)], stroke);
            painter.line_segment([p(0.32, 0.9), p(0.68, 0.9)], stroke)
        }
        Icon::Headphones => {
            painter.add(line(vec![
                p(0.15, 0.62),
                p(0.15, 0.45),
                p(0.25, 0.25),
                p(0.5, 0.15),
                p(0.75, 0.25),
                p(0.85, 0.45),
                p(0.85, 0.62),
            ]));
            for x in [0.1, 0.7] {
                painter.rect_filled(
                    Rect::from_min_max(p(x, 0.55), p(x + 0.2, 0.86)),
                    (size * 0.08) as u8,
                    color,
                );
            }
            painter.add(Shape::Noop)
        }
        Icon::Pin => {
            painter.add(Shape::convex_polygon(
                vec![
                    p(0.38, 0.12),
                    p(0.62, 0.12),
                    p(0.62, 0.42),
                    p(0.78, 0.58),
                    p(0.22, 0.58),
                    p(0.38, 0.42),
                ],
                color,
                Stroke::NONE,
            ));
            painter.line_segment([p(0.5, 0.58), p(0.5, 0.9)], Stroke::new(width * 1.3, color))
        }
        Icon::People => {
            painter.circle_filled(p(0.38, 0.32), size * 0.14, color);
            painter.add(Shape::convex_polygon(
                vec![
                    p(0.1, 0.82),
                    p(0.14, 0.62),
                    p(0.3, 0.52),
                    p(0.46, 0.52),
                    p(0.62, 0.62),
                    p(0.66, 0.82),
                ],
                color,
                Stroke::NONE,
            ));
            painter.circle_stroke(p(0.7, 0.34), size * 0.11, stroke);
            painter.add(line(vec![p(0.72, 0.52), p(0.84, 0.6), p(0.9, 0.8)]))
        }
        Icon::Search => {
            painter.circle_stroke(p(0.44, 0.44), size * 0.26, Stroke::new(width * 1.2, color));
            painter.line_segment(
                [p(0.64, 0.64), p(0.88, 0.88)],
                Stroke::new(width * 1.5, color),
            )
        }
        Icon::Plus => {
            painter.line_segment([p(0.5, 0.2), p(0.5, 0.8)], Stroke::new(width * 1.3, color));
            painter.line_segment([p(0.2, 0.5), p(0.8, 0.5)], Stroke::new(width * 1.3, color))
        }
        Icon::Close => {
            painter.line_segment(
                [p(0.25, 0.25), p(0.75, 0.75)],
                Stroke::new(width * 1.3, color),
            );
            painter.line_segment(
                [p(0.75, 0.25), p(0.25, 0.75)],
                Stroke::new(width * 1.3, color),
            )
        }
        Icon::Smile => {
            painter.circle_stroke(rect.center(), size * 0.38, Stroke::new(width * 1.2, color));
            painter.circle_filled(p(0.37, 0.4), size * 0.055, color);
            painter.circle_filled(p(0.63, 0.4), size * 0.055, color);
            painter.add(line(vec![
                p(0.3, 0.58),
                p(0.4, 0.68),
                p(0.5, 0.71),
                p(0.6, 0.68),
                p(0.7, 0.58),
            ]))
        }
        Icon::Bell => {
            painter.add(Shape::convex_polygon(
                vec![
                    p(0.2, 0.7),
                    p(0.26, 0.62),
                    p(0.28, 0.4),
                    p(0.36, 0.22),
                    p(0.5, 0.15),
                    p(0.64, 0.22),
                    p(0.72, 0.4),
                    p(0.74, 0.62),
                    p(0.8, 0.7),
                ],
                color,
                Stroke::NONE,
            ));
            painter.add(line(vec![
                p(0.42, 0.8),
                p(0.46, 0.86),
                p(0.54, 0.86),
                p(0.58, 0.8),
            ]))
        }
        Icon::Phone => painter.add(Shape::line(
            vec![
                p(0.2, 0.3),
                p(0.3, 0.2),
                p(0.42, 0.36),
                p(0.36, 0.46),
                p(0.54, 0.64),
                p(0.64, 0.58),
                p(0.8, 0.7),
                p(0.7, 0.8),
                p(0.44, 0.7),
                p(0.3, 0.56),
            ],
            Stroke::new(width * 1.4, color),
        )),
        Icon::Video => {
            painter.rect_filled(
                Rect::from_min_max(p(0.12, 0.28), p(0.62, 0.72)),
                (size * 0.08) as u8,
                color,
            );
            painter.add(Shape::convex_polygon(
                vec![p(0.64, 0.45), p(0.88, 0.3), p(0.88, 0.7), p(0.64, 0.55)],
                color,
                Stroke::NONE,
            ))
        }
        Icon::Reply => {
            painter.add(line(vec![p(0.4, 0.25), p(0.15, 0.45), p(0.4, 0.65)]));
            painter.add(line(vec![
                p(0.15, 0.45),
                p(0.6, 0.45),
                p(0.78, 0.55),
                p(0.85, 0.78),
            ]))
        }
        Icon::Pencil => {
            painter.add(Shape::closed_line(
                vec![
                    p(0.15, 0.85),
                    p(0.2, 0.62),
                    p(0.66, 0.16),
                    p(0.84, 0.34),
                    p(0.38, 0.8),
                ],
                stroke,
            ));
            painter.line_segment([p(0.56, 0.26), p(0.74, 0.44)], stroke)
        }
        Icon::More => {
            for x in [0.25, 0.5, 0.75] {
                painter.circle_filled(p(x, 0.5), size * 0.07, color);
            }
            painter.add(Shape::Noop)
        }
        Icon::Inbox => {
            painter.add(line(vec![
                p(0.15, 0.55),
                p(0.22, 0.2),
                p(0.78, 0.2),
                p(0.85, 0.55),
            ]));
            painter.add(line(vec![
                p(0.15, 0.55),
                p(0.15, 0.82),
                p(0.85, 0.82),
                p(0.85, 0.55),
                p(0.66, 0.55),
                p(0.6, 0.66),
                p(0.4, 0.66),
                p(0.34, 0.55),
                p(0.15, 0.55),
            ]))
        }
        Icon::Help => {
            painter.circle_stroke(rect.center(), size * 0.38, Stroke::new(width * 1.2, color));
            painter.add(line(vec![
                p(0.38, 0.4),
                p(0.42, 0.3),
                p(0.5, 0.27),
                p(0.6, 0.32),
                p(0.6, 0.42),
                p(0.5, 0.5),
                p(0.5, 0.58),
            ]));
            painter.circle_filled(p(0.5, 0.7), size * 0.05, color)
        }
        Icon::Reload => {
            let c = rect.center();
            let points: Vec<Pos2> = (0..20)
                .map(|i| {
                    let angle = -1.2 + 5.0 * i as f32 / 19.0;
                    c + vec2(angle.cos(), angle.sin()) * size * 0.3
                })
                .collect();
            painter.add(line(points));
            painter.add(Shape::convex_polygon(
                vec![p(0.6, 0.12), p(0.78, 0.2), p(0.62, 0.36)],
                color,
                Stroke::NONE,
            ))
        }
        Icon::Threads => {
            painter.line_segment([p(0.25, 0.2), p(0.25, 0.8)], stroke);
            painter.line_segment([p(0.25, 0.45), p(0.75, 0.45)], stroke);
            painter.line_segment([p(0.5, 0.25), p(0.5, 0.65)], stroke);
            painter.line_segment([p(0.25, 0.8), p(0.75, 0.8)], stroke)
        }
        Icon::Speaker => {
            painter.add(Shape::convex_polygon(
                vec![
                    p(0.08, 0.36),
                    p(0.28, 0.36),
                    p(0.5, 0.15),
                    p(0.5, 0.85),
                    p(0.28, 0.64),
                    p(0.08, 0.64),
                ],
                color,
                Stroke::NONE,
            ));
            for x in [0.62, 0.76] {
                painter.add(line(vec![p(x, 0.3), p(x + 0.08, 0.5), p(x, 0.7)]));
            }
            painter.add(Shape::Noop)
        }
        Icon::Hash => {
            let heavy = Stroke::new(width * 1.25, color);
            painter.line_segment([p(0.2, 0.36), p(0.85, 0.36)], heavy);
            painter.line_segment([p(0.15, 0.64), p(0.8, 0.64)], heavy);
            painter.line_segment([p(0.42, 0.12), p(0.3, 0.88)], heavy);
            painter.line_segment([p(0.7, 0.12), p(0.58, 0.88)], heavy)
        }
        Icon::Forum => {
            painter.rect_stroke(
                Rect::from_min_max(p(0.15, 0.18), p(0.85, 0.7)),
                (size * 0.1) as u8,
                stroke,
                StrokeKind::Inside,
            );
            painter.line_segment([p(0.3, 0.36), p(0.7, 0.36)], stroke);
            painter.line_segment([p(0.3, 0.52), p(0.58, 0.52)], stroke);
            painter.add(line(vec![p(0.3, 0.7), p(0.3, 0.86), p(0.48, 0.7)]))
        }
        Icon::Send => painter.add(Shape::convex_polygon(
            vec![p(0.15, 0.15), p(0.88, 0.5), p(0.15, 0.85), p(0.3, 0.5)],
            color,
            Stroke::NONE,
        )),
        Icon::External => {
            painter.add(line(vec![p(0.55, 0.18), p(0.82, 0.18), p(0.82, 0.45)]));
            painter.line_segment(
                [p(0.82, 0.18), p(0.46, 0.54)],
                Stroke::new(width * 1.2, color),
            );
            painter.add(line(vec![
                p(0.7, 0.6),
                p(0.7, 0.82),
                p(0.18, 0.82),
                p(0.18, 0.3),
                p(0.4, 0.3),
            ]))
        }
        Icon::Attach => {
            painter.circle_stroke(rect.center(), size * 0.38, Stroke::new(width * 1.2, color));
            painter.line_segment([p(0.5, 0.3), p(0.5, 0.7)], Stroke::new(width * 1.2, color));
            painter.line_segment([p(0.3, 0.5), p(0.7, 0.5)], Stroke::new(width * 1.2, color))
        }
    };
}

/// Square icon button that highlights on hover and exposes `label` to accessibility.
pub fn button(ui: &mut egui::Ui, icon: Icon, size: f32, label: &str) -> Response {
    let colors = design::palette(ui);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 6, colors.hover);
    }
    let color = if !ui.is_enabled() {
        colors.muted.gamma_multiply(0.5)
    } else if response.hovered() || response.has_focus() {
        colors.text_strong
    } else {
        colors.muted
    };
    paint(ui.painter(), icon, rect.shrink(size * 0.2), color);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response.on_hover_text(label)
}

/// Toggleable variant: `active` keeps the icon in the strong text colour.
pub fn toggle(ui: &mut egui::Ui, icon: Icon, size: f32, active: bool, label: &str) -> Response {
    let colors = design::palette(ui);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 6, colors.hover);
    }
    let color = if active || response.hovered() || response.has_focus() {
        colors.text_strong
    } else {
        colors.muted
    };
    paint(ui.painter(), icon, rect.shrink(size * 0.2), color);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), active, label)
    });
    response.on_hover_text(label)
}

/// Inline glyph used beside labels (channel kinds, section headers).
pub fn inline(ui: &mut egui::Ui, icon: Icon, size: f32, color: Color32) -> Rect {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint(ui.painter(), icon, rect, color);
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_icon_paints_inside_its_rect() {
        let ctx = egui::Context::default();
        let icons = [
            Icon::ChevronDown,
            Icon::ChevronRight,
            Icon::Gear,
            Icon::Microphone,
            Icon::Headphones,
            Icon::Pin,
            Icon::People,
            Icon::Search,
            Icon::Plus,
            Icon::Smile,
            Icon::Bell,
            Icon::Phone,
            Icon::Video,
            Icon::Reply,
            Icon::Pencil,
            Icon::More,
            Icon::Inbox,
            Icon::Help,
            Icon::Reload,
            Icon::Threads,
            Icon::Speaker,
            Icon::Hash,
            Icon::Forum,
            Icon::Send,
            Icon::Attach,
            Icon::Close,
            Icon::External,
        ];
        let output = ctx.run_ui(Default::default(), |ui| {
            for icon in icons {
                let rect = Rect::from_min_size(egui::pos2(10.0, 10.0), Vec2::splat(24.0));
                paint(ui.painter(), icon, rect, Color32::WHITE);
                assert!(button(ui, icon, 32.0, "icon").rect.width() == 32.0);
            }
        });
        for shape in &output.shapes {
            let bounds = shape.shape.visual_bounding_rect();
            assert!(bounds.is_negative() || bounds.min.x >= -1.0, "{:?}", bounds);
        }
        output.drop_without_applying_deltas();
    }
}
