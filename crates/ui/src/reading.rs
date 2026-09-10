use crate::MessagingUi;
use egui::containers::panel::PanelState;
use model::ReadingPreferences;

impl MessagingUi {
    pub fn apply_reading_preferences(&mut self, ctx: &egui::Context, value: ReadingPreferences) {
        if !value.is_valid() {
            return;
        }
        self.reading_preferences = value;
        self.reading_sidebar_applied = None;
        let zoom = f32::from(value.zoom_percent) / 100.0;
        if (ctx.zoom_factor() - zoom).abs() > 0.001 {
            // egui applies zoom after this pass, not synchronously inside the settings menu.
            self.reading_zoom_pending = true;
            ctx.set_zoom_factor(zoom);
        }
        ctx.request_repaint();
    }

    /// Called once per desktop frame, before loading preferences, including on sign-in.
    pub fn sync_reading_zoom(&mut self, ctx: &egui::Context) {
        if std::mem::take(&mut self.reading_zoom_pending) {
            return;
        }
        let percent = (ctx.zoom_factor() * 100.0).round().clamp(80.0, 150.0) as u16;
        self.reading_preferences.zoom_percent = percent;
        let zoom = f32::from(percent) / 100.0;
        if (ctx.zoom_factor() - zoom).abs() > 0.001 {
            ctx.set_zoom_factor(zoom);
        }
    }

    pub fn reading_settings(&mut self, ui: &mut egui::Ui, demo: bool) {
        ui.strong("Reading and layout");
        let mut value = self.reading_preferences;
        ui.add(
            egui::Slider::new(&mut value.zoom_percent, 80..=150)
                .text("Zoom")
                .suffix("%"),
        );
        ui.add(egui::Slider::new(&mut value.sidebar_width, 190..=360).text("Sidebar"));
        ui.checkbox(&mut value.show_members, "Show People in wide windows");
        if ui.button("Reset reading and layout").clicked() {
            value = ReadingPreferences::default();
            self.reading_save_requested = true;
        }
        ui.small(if demo {
            "Preview uses session memory only"
        } else {
            self.reading_status
        });
        if !demo
            && self.reading_status.contains("could not")
            && ui.button("Retry saving reading settings").clicked()
        {
            self.reading_save_requested = true;
        }
        if value != self.reading_preferences {
            self.apply_reading_preferences(ui.ctx(), value);
        }
    }

    pub(super) fn prepare_reading_sidebar(&mut self, ui: &egui::Ui) -> f32 {
        // Preserve space for the conversation at high zoom. Temporary viewport limits
        // must not replace the user's preferred width on disk.
        let maximum = (ui.available_width() - 260.0).clamp(190.0, 360.0);
        let constrained = f32::from(self.reading_preferences.sidebar_width) > maximum;
        if self.reading_sidebar_applied != Some(self.reading_preferences.sidebar_width)
            || self.reading_sidebar_constrained != constrained
        {
            ui.ctx()
                .data_mut(|data| data.remove::<PanelState>(egui::Id::new("channels")));
            self.reading_sidebar_applied = Some(self.reading_preferences.sidebar_width);
        }
        self.reading_sidebar_constrained = constrained;
        maximum
    }

    pub(super) fn record_reading_sidebar(&mut self, width: f32) {
        // Applying settings from within this panel invalidates its old geometry;
        // wait for the following frame before observing a user resize.
        if !self.reading_sidebar_constrained
            && self.reading_sidebar_applied == Some(self.reading_preferences.sidebar_width)
            && width.is_finite()
            && (190.0..=360.0).contains(&width.round())
        {
            self.reading_preferences.sidebar_width = width.round() as u16;
            self.reading_sidebar_applied = Some(self.reading_preferences.sidebar_width);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_controls_reset_retry_and_preserve_session_notification_opt_in() {
        fn labels(shape: &egui::Shape, found: &mut Vec<(String, egui::Rect)>) {
            match shape {
                egui::Shape::Text(text) => found.push((
                    text.galley.job.text.clone(),
                    text.galley.rect.translate(text.pos.to_vec2()),
                )),
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        labels(shape, found);
                    }
                }
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let mut view = MessagingUi {
            notifications_enabled: true,
            ..Default::default()
        };
        let frame = |view: &mut MessagingUi, events| {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(480.0, 360.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| view.reading_settings(ui, false),
            );
            assert!(output.platform_output.commands.is_empty());
            let mut found = vec![];
            for shape in &output.shapes {
                labels(&shape.shape, &mut found);
            }
            output.drop_without_applying_deltas();
            found
        };
        let custom = ReadingPreferences {
            zoom_percent: 125,
            sidebar_width: 300,
            show_members: false,
        };
        view.apply_reading_preferences(&ctx, custom);
        for _ in 0..3 {
            frame(&mut view, vec![]);
        }
        assert!((ctx.zoom_factor() - 1.25).abs() < 0.001);
        assert_eq!(view.reading_preferences, custom);
        let found = frame(&mut view, vec![]);
        let people = found
            .iter()
            .find(|(text, _)| text == "Show People in wide windows")
            .unwrap()
            .1
            .center();
        for pressed in [true, false] {
            frame(
                &mut view,
                vec![
                    egui::Event::PointerMoved(people),
                    egui::Event::PointerButton {
                        pos: people,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(view.reading_preferences.show_members);
        for _ in 0..2 {
            let found = frame(&mut view, vec![]);
            let reset = found
                .iter()
                .find(|(text, _)| text == "Reset reading and layout")
                .unwrap()
                .1
                .center();
            for pressed in [true, false] {
                frame(
                    &mut view,
                    vec![
                        egui::Event::PointerMoved(reset),
                        egui::Event::PointerButton {
                            pos: reset,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            assert_eq!(view.reading_preferences, ReadingPreferences::default());
            assert!(
                std::mem::take(&mut view.reading_save_requested),
                "Even an already-default reset overrides a pending startup load"
            );
            for _ in 0..2 {
                frame(&mut view, vec![]);
            }
        }
        assert!(
            view.notifications_enabled,
            "Reading defaults do not change OS notification consent"
        );
        view.reading_status = "Reading settings could not be saved";
        let found = frame(&mut view, vec![]);
        let retry = found
            .iter()
            .find(|(text, _)| text == "Retry saving reading settings")
            .unwrap()
            .1
            .center();
        for pressed in [true, false] {
            frame(
                &mut view,
                vec![
                    egui::Event::PointerMoved(retry),
                    egui::Event::PointerButton {
                        pos: retry,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
        }
        assert!(view.reading_save_requested);
        view.apply_reading_preferences(
            &ctx,
            ReadingPreferences {
                zoom_percent: 151,
                ..custom
            },
        );
        assert_eq!(view.reading_preferences, ReadingPreferences::default());
    }

    #[test]
    fn sidebar_constraints_do_not_replace_saved_width_and_zoom_is_bounded() {
        let ctx = egui::Context::default();
        let mut view = MessagingUi::default();
        view.apply_reading_preferences(
            &ctx,
            ReadingPreferences {
                sidebar_width: 350,
                ..Default::default()
            },
        );
        let sidebar_frame = |view: &mut MessagingUi, width, events| {
            let mut rendered = 0.0;
            ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 400.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let maximum = view.prepare_reading_sidebar(ui);
                    let panel = egui::Panel::left("channels")
                        .default_size(
                            f32::from(view.reading_preferences.sidebar_width).min(maximum),
                        )
                        .size_range(190.0..=maximum)
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.label("Synthetic navigation");
                        });
                    rendered = panel.response.rect.width();
                    view.record_reading_sidebar(rendered);
                },
            )
            .drop_without_applying_deltas();
            rendered
        };
        for width in [900.0, 480.0, 900.0] {
            let actual = sidebar_frame(&mut view, width, vec![]);
            assert_eq!(view.reading_preferences.sidebar_width, 350);
            assert!((actual - if width < 600.0 { 220.0 } else { 350.0 }).abs() < 1.0);
        }
        let edge = egui::pos2(350.0, 200.0);
        let resized = egui::pos2(280.0, 200.0);
        sidebar_frame(&mut view, 900.0, vec![egui::Event::PointerMoved(edge)]);
        sidebar_frame(
            &mut view,
            900.0,
            vec![egui::Event::PointerButton {
                pos: edge,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        sidebar_frame(&mut view, 900.0, vec![egui::Event::PointerMoved(resized)]);
        sidebar_frame(
            &mut view,
            900.0,
            vec![egui::Event::PointerButton {
                pos: resized,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        sidebar_frame(&mut view, 900.0, vec![]);
        assert_eq!(
            view.reading_preferences.sidebar_width, 280,
            "Dragging the native panel edge updates the saved preference"
        );
        view.apply_reading_preferences(
            &ctx,
            ReadingPreferences {
                sidebar_width: 270,
                ..Default::default()
            },
        );
        assert!((sidebar_frame(&mut view, 900.0, vec![]) - 270.0).abs() < 1.0);
        ctx.set_zoom_factor(1.4);
        sidebar_frame(&mut view, 900.0, vec![]);
        view.sync_reading_zoom(&ctx);
        assert_eq!(view.reading_preferences.zoom_percent, 140);
        ctx.set_zoom_factor(1.7);
        sidebar_frame(&mut view, 900.0, vec![]);
        view.sync_reading_zoom(&ctx);
        sidebar_frame(&mut view, 900.0, vec![]);
        assert_eq!(view.reading_preferences.zoom_percent, 150);
        assert!((ctx.zoom_factor() - 1.5).abs() < 0.001);
    }
}
