use crate::{MessagingUi, design, icons};
use client_core::State;
use egui::RichText;

#[derive(Default)]
pub(super) struct Settings {
	pub open: bool,
	page: Page,
	query: String,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Page {
	Account,
	#[default]
	Appearance,
	Notifications,
	Voice,
	Storage,
}
impl Page {
	const ALL: [Self; 5] = [
		Self::Account,
		Self::Appearance,
		Self::Notifications,
		Self::Voice,
		Self::Storage,
	];
	fn label(self) -> &'static str {
		match self {
			Self::Account => "My Account",
			Self::Appearance => "Appearance",
			Self::Notifications => "Notifications",
			Self::Voice => "Voice & Audio",
			Self::Storage => "Data & Privacy",
		}
	}
	fn matches(self, query: &str) -> bool {
		let keywords = match self {
			Self::Account => "my account profile logout",
			Self::Appearance => {
				"appearance theme dark light system zoom reading layout sidebar people reset"
			}
			Self::Notifications => "notifications desktop system alerts",
			Self::Voice => {
				"voice audio microphone speakers devices volume gain noise suppression push to talk"
			}
			Self::Storage => "data privacy local storage clear cache drafts credentials",
		};
		keywords.contains(query)
	}
}

impl MessagingUi {
	/// Fixture-only entry point for the native offline settings preview.
	pub fn preview_settings(&mut self) {
		self.settings.open = true;
	}
	pub(super) fn show_settings(&mut self, ctx: &egui::Context, state: &State) {
		let colors = design::palette_for(ctx);
		let size = ctx.content_rect().size() - egui::vec2(32.0, 40.0);
		let width = size.x.clamp(280.0, 1100.0);
		let height = size.y.clamp(240.0, 820.0);
		let modal = egui::Modal::new(egui::Id::unique("user-settings"))
            .backdrop_color(egui::Color32::from_black_alpha(180))
            .frame(egui::Frame::new().fill(colors.chat.to_opaque()).corner_radius(12)
                .stroke(egui::Stroke::new(1.0, colors.border)))
            .show(ctx, |ui| {
                ui.set_width(width);
                ui.set_height(height);
                if width >= 620.0 {
                    egui::Panel::left("settings-navigation")
                        .exact_size(220.0).resizable(false)
                        .frame(egui::Frame::new().fill(colors.sidebar.to_opaque()).inner_margin(20))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical().id_salt("settings-navigation-scroll").show(ui, |ui| {
                                self.settings_identity(ui, state);
                                ui.add_space(20.0);
                                self.settings_search(ui);
                                ui.add_space(16.0);
                                ui.label(design::eyebrow(ui, "User settings", colors.muted));
                                for page in Page::ALL {
                                    if page.matches(&self.settings.query.to_lowercase()) && ui.add_sized(
                                        [ui.available_width(), 38.0],
                                        egui::Button::selectable(self.settings.page == page, page.label())
                                    ).clicked() {
                                        self.settings.page = page;
                                    }
                                }
                                ui.add_space(16.0);
                                ui.separator();
                                self.settings_logout(ui, state.demo);
                                ui.add_space(12.0);
                                ui.weak(format!("Serein {}", self.build.version));
                                ui.small("Unofficial · not endorsed by Discord");
                            });
                        });
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().inner_margin(24))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(design::semibold(ui, self.settings.page.label(), 20.0).color(colors.text_strong));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if icons::button(ui, icons::Icon::Close, 32.0, "Close settings (Esc)").clicked() {
                                    self.settings.open = false;
                                }
                            });
                        });
                        if width < 620.0 {
                            self.settings_search(ui);
                            egui::ComboBox::from_id_salt("settings-page").selected_text(self.settings.page.label()).show_ui(ui, |ui| {
                                for page in Page::ALL {
                                    if page.matches(&self.settings.query.to_lowercase()) {
                                        ui.selectable_value(&mut self.settings.page, page, page.label());
                                    }
                                }
                            });
                        }
                        ui.separator();
                        egui::ScrollArea::vertical().id_salt(("settings-content", self.settings.page as u8))
                            .auto_shrink([false, false]).show(ui, |ui| {
                                let padding = ((ui.available_width() - 660.0) / 2.0).max(0.0);
                                egui::Frame::new().inner_margin(egui::Margin { left: padding as i8, right: padding as i8, top: 20, bottom: 20 }).show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.spacing_mut().item_spacing.y = 14.0;
                                    if !Page::ALL.into_iter().any(|p| p.matches(&self.settings.query.to_lowercase())) {
                                        ui.strong("No settings found");
                                        ui.weak("Try theme, notifications, voice, or cache.");
                                        return;
                                    }
                                    match self.settings.page {
                                        Page::Account => {
                                            self.settings_identity(ui, state);
                                            ui.add_space(12.0);
                                            ui.weak("Profile and account changes are managed in Discord.");
                                            ui.separator();
                                            ui.label("Logging out removes the saved login and clears this account’s local cache and drafts.");
                                            self.settings_logout(ui, state.demo);
                                        }
                                        Page::Appearance => {
                                            self.appearance_settings(ui);
                                            ui.add_space(12.0);
                                            ui.separator();
                                            self.reading_settings(ui, state.demo);
                                        }
                                        Page::Notifications => self.notification_settings(ui, state),
                                        Page::Voice => self.voice_settings_menu(ui, state.demo, state.voice.active.is_some()),
                                        Page::Storage => {
                                            ui.heading("Local storage");
                                            ui.label("Messages and drafts are cached on this device. Local cache data is not encrypted by Serein; saved login tokens use the OS credential store.");
                                            self.storage_settings(ui, state);
                                            ui.separator();
                                            ui.strong("Your privacy");
                                            ui.label("Serein does not collect telemetry or upload diagnostics. Discord retains service-side data according to its own policies.");
                                        }
                                    }
                                });
                            });
                    });
            });
		if modal.should_close() {
			self.settings.open = false;
		}
	}

	fn settings_identity(&mut self, ui: &mut egui::Ui, state: &State) {
		ui.horizontal(|ui| {
			if let Some(user) = &state.user {
				self.avatars.show(ui, user, 40.0, state.demo);
			}
			ui.vertical(|ui| {
				ui.add(
					egui::Label::new(design::semibold(
						ui,
						state
							.user
							.as_ref()
							.map_or("Your account", |u| u.name.as_str()),
						16.0,
					))
					.truncate(),
				);
				ui.weak(if state.demo {
					"Offline preview"
				} else {
					"Personal settings"
				});
			});
		});
	}

	fn settings_search(&mut self, ui: &mut egui::Ui) {
		if ui
			.add(
				egui::TextEdit::singleline(&mut self.settings.query)
					.hint_text("Search settings")
					.char_limit(64)
					.desired_width(ui.available_width()),
			)
			.changed()
		{
			let query = self.settings.query.to_lowercase();
			if !self.settings.page.matches(&query)
				&& let Some(page) = Page::ALL.into_iter().find(|p| p.matches(&query))
			{
				self.settings.page = page;
			}
		}
	}

	fn settings_logout(&mut self, ui: &mut egui::Ui, demo: bool) {
		if ui
			.button(
				RichText::new(if demo { "Exit preview" } else { "Log out" })
					.color(design::palette(ui).danger),
			)
			.clicked()
		{
			self.logout_requested = true;
			self.settings.open = false;
		}
	}

	fn appearance_settings(&mut self, ui: &mut egui::Ui) {
		let colors = design::palette(ui);
		ui.label(design::eyebrow(ui, "Appearance", colors.muted));
		egui::widgets::global_theme_preference_buttons(ui);
		ui.add_space(6.0);
		ui.label(design::eyebrow(ui, "Theme", colors.muted));
		let current = design::variant();
		ui.horizontal_wrapped(|ui| {
			ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
			for variant in design::Variant::ALL {
				let swatch = design::colors(ui.visuals().dark_mode, variant);
				let (rect, response) =
					ui.allocate_exact_size(egui::Vec2::splat(28.0), egui::Sense::click());
				response.widget_info(|| {
					egui::WidgetInfo::selected(
						egui::WidgetType::RadioButton,
						true,
						variant == current,
						variant.label(),
					)
				});
				let painter = ui.painter();
				match swatch.backdrop {
					Some([top, bottom]) => {
						painter.circle_filled(rect.center(), 12.0, bottom);
						painter.circle_filled(rect.center() - egui::vec2(3.0, 3.0), 7.0, top);
					}
					None => {
						painter.circle_filled(rect.center(), 12.0, swatch.chat);
						painter.circle_filled(
							rect.center() + egui::vec2(3.0, 3.0),
							6.0,
							swatch.base,
						);
					}
				}
				painter.circle_stroke(
					rect.center(),
					12.0,
					egui::Stroke::new(
						if variant == current { 2.0 } else { 1.0 },
						if variant == current {
							colors.accent
						} else {
							colors.border
						},
					),
				);
				if response.on_hover_text(variant.label()).clicked() && variant != current {
					design::set_variant(variant);
					design::apply(ui.ctx());
					self.theme_variant_changed = Some(variant);
				}
			}
		});
		ui.label(
			RichText::new(format!("{} · saved with your appearance", current.label()))
				.small()
				.color(colors.muted),
		);
	}

	fn notification_settings(&mut self, ui: &mut egui::Ui, state: &State) {
		let colors = design::palette(ui);
		ui.heading("Overview");
		ui.add_enabled_ui(!state.demo || self.notification_test_available, |ui| {
			notification_switch(ui, &mut self.notifications_enabled);
		});
		ui.label(
			RichText::new(
				"Message previews are hidden. OS notification history may remain after logout.",
			)
			.small()
			.color(colors.muted),
		);
		ui.label(
			RichText::new(if state.demo && !self.notification_test_available {
				"Offline preview never sends system notifications."
			} else {
				self.notification_status
			})
			.small()
			.color(colors.muted),
		);
		if self.notification_test_available
			&& self.notifications_enabled
			&& ui.button("Send generic test notification").clicked()
		{
			self.notification_test_requested = true;
		}
		if !state.demo && !state.notification_preferences_known() {
			ui.label(
				RichText::new("Alerts wait for your Discord notification preferences.")
					.small()
					.color(colors.muted),
			);
		}
	}

	fn storage_settings(&mut self, ui: &mut egui::Ui, state: &State) {
		let colors = design::palette(ui);
		ui.label(
			RichText::new(if state.demo {
				"Preview uses session memory only."
			} else {
				self.storage_status
			})
			.small()
			.color(colors.muted),
		);
		if ui
			.add_enabled(!state.demo, egui::Button::new("Clear cache"))
			.clicked()
		{
			self.clear_cache_requested = true;
		}
	}
}

fn notification_switch(ui: &mut egui::Ui, enabled: &mut bool) {
	let colors = design::palette(ui);
	ui.horizontal(|ui| {
		let text_width = (ui.available_width() - 64.0).max(80.0);
		let label = ui.add_sized(
			[text_width, 44.0],
			egui::Label::new(design::medium(ui, "Enable Desktop Notifications", 16.0)).wrap(),
		);
		let (rect, mut response) =
			ui.allocate_exact_size(egui::vec2(44.0, 26.0), egui::Sense::click());
		if response.clicked() {
			*enabled = !*enabled;
			response.mark_changed();
		}
		response.widget_info(|| {
			egui::WidgetInfo::selected(
				egui::WidgetType::Checkbox,
				ui.is_enabled(),
				*enabled,
				"Enable Desktop Notifications",
			)
		});
		let response = response.labelled_by(label.id);
		let fill = if *enabled {
			colors.accent
		} else {
			colors.muted
		};
		ui.painter().rect_filled(
			rect,
			13,
			if ui.is_enabled() {
				fill
			} else {
				fill.gamma_multiply(0.4)
			},
		);
		let x = if *enabled {
			rect.right() - 13.0
		} else {
			rect.left() + 13.0
		};
		ui.painter()
			.circle_filled(egui::pos2(x, rect.center().y), 9.0, egui::Color32::WHITE);
		if response.has_focus() {
			ui.painter().rect_stroke(
				rect.expand(3.0),
				16,
				egui::Stroke::new(2.0, colors.accent),
				egui::StrokeKind::Outside,
			);
		}
	});
	ui.weak("Applies to this session. Turn it on again after restarting Serein.");
}
