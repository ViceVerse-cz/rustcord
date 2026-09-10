use crate::{MessagingUi, design};
use client_core::{
    Command, State,
    auth::AuthState,
    voice::{Phase, RosterEntry},
};
use egui::RichText;
use model::Id;

impl MessagingUi {
    pub(super) fn voice_channel_button(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        channel: &model::Channel,
        selected: bool,
    ) -> egui::Response {
        let colors = design::palette(ui);
        let call = state
            .voice
            .active
            .as_ref()
            .filter(|c| c.channel == channel.id);
        let connected = call.is_some_and(|c| matches!(c.phase, Phase::Waiting | Phase::Connected));
        let color = if connected {
            colors.accent
        } else {
            colors.text
        };
        let elapsed = call.and_then(elapsed_label);
        let response = ui
            .push_id(channel.id, |ui| {
                ui.add_enabled(
                    state.can_view(channel.id),
                    egui::Button::selectable(
                        selected,
                        RichText::new(format!("     {}", channel.name)).color(color),
                    )
                    .right_text(
                        RichText::new(elapsed.as_deref().unwrap_or(""))
                            .monospace()
                            .size(11.0)
                            .color(color),
                    )
                    .min_size(egui::vec2(ui.available_width(), 36.0))
                    .truncate(),
                )
            })
            .inner;
        speaker(
            ui,
            egui::Rect::from_center_size(
                response.rect.left_center() + egui::vec2(16.0, 0.0),
                egui::vec2(18.0, 18.0),
            ),
            color,
        );
        if elapsed.is_some() && ui.is_rect_visible(response.rect) {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
        }
        response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                response.enabled(),
                selected,
                format!(
                    "{} voice channel{}",
                    channel.name,
                    if connected { ", connected" } else { "" }
                ),
            )
        });
        response.on_hover_text(format!(
            "{} · View voice channel{}",
            channel.name,
            if connected { " · Connected" } else { "" }
        ))
    }

    pub(super) fn voice_participant(
        &mut self,
        ui: &mut egui::Ui,
        state: &State,
        entry: &RosterEntry,
    ) {
        let colors = design::palette(ui);
        let member = entry.member.as_ref().or_else(|| {
            state
                .members
                .as_ref()
                .filter(|list| list.guild == Some(entry.guild))
                .and_then(|list| {
                    list.rows
                        .iter()
                        .flatten()
                        .find(|m| m.user.id == entry.participant.user)
                })
        });
        let user = member.map(|m| &m.user).or_else(|| {
            state
                .user
                .as_ref()
                .filter(|u| u.id == entry.participant.user)
        });
        let name = member
            .and_then(|m| m.nick.as_deref())
            .or_else(|| user.map(|u| u.name.as_str()))
            .unwrap_or("Participant");
        ui.push_id(
            ("voice-participant", entry.channel, entry.participant.user),
            |ui| {
                ui.horizontal(|ui| {
                    ui.set_min_height(36.0);
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if let Some(user) = user {
                        if self.avatars.show(ui, user, 28.0, state.demo).clicked() {
                            self.profile = Some(user.clone());
                        }
                    } else {
                        design::avatar(ui, name, 28.0);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if entry.participant.deafened {
                            status_icon(
                                ui,
                                true,
                                if entry.participant.server_deafened {
                                    "Deafened by server"
                                } else {
                                    "Deafened"
                                },
                            );
                        }
                        if entry.participant.muted {
                            status_icon(
                                ui,
                                false,
                                if entry.participant.server_muted {
                                    "Muted by server"
                                } else {
                                    "Microphone muted"
                                },
                            );
                        }
                        let response = ui
                            .allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), 28.0),
                                egui::Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.add(
                                        egui::Label::new(RichText::new(name).color(colors.muted))
                                            .truncate()
                                            .sense(egui::Sense::click()),
                                    )
                                },
                            )
                            .inner
                            .on_hover_text(name);
                        if response.clicked()
                            && let Some(user) = user
                        {
                            self.profile = Some(user.clone());
                        }
                    });
                });
            },
        );
    }

    pub(super) fn voice_channel(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut State,
        channel: Id,
        commands: &mut Vec<Command>,
    ) {
        let colors = design::palette(ui);
        egui::Frame::new().inner_margin(24).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.heading("In this voice channel");
                if state
                    .voice
                    .active
                    .as_ref()
                    .is_none_or(|call| call.channel != channel)
                {
                    self.call_button(ui, state, channel, commands);
                }
            });
            if state.demo {
                ui.label(
                    RichText::new("Synthetic participants · microphone and speakers are off.")
                        .color(colors.muted),
                );
            } else if let Some(reason) = self.call_unavailable(state, channel).filter(|_| {
                state
                    .voice
                    .active
                    .as_ref()
                    .is_none_or(|call| call.channel != channel)
            }) {
                ui.label(RichText::new(reason).color(colors.muted));
            } else if state
                .voice
                .active
                .as_ref()
                .is_none_or(|call| call.channel != channel)
            {
                ui.label(
                    RichText::new("Join when you’re ready. You can keep browsing while connected.")
                        .color(colors.muted),
                );
            }
            let entries: Vec<_> = state
                .voice
                .roster
                .iter()
                .filter(|e| e.channel == channel)
                .collect();
            ui.add_space(16.0);
            if !state.demo && !state.gateway_connected && !entries.is_empty() {
                ui.label(
                    RichText::new("Last known participants · reconnect to refresh")
                        .small()
                        .color(colors.muted),
                );
            }
            if entries.is_empty() {
                ui.label(
                    RichText::new(if !state.demo && !state.gateway_connected {
                        "Participant list unavailable while disconnected."
                    } else {
                        "Nobody is in this channel yet."
                    })
                    .color(colors.muted),
                );
            } else {
                ui.label(
                    RichText::new(format!("{} participants", entries.len()))
                        .small()
                        .color(colors.muted),
                );
                ui.add_space(8.0);
                ui.set_max_width(520.0);
                egui::ScrollArea::vertical()
                    .id_salt(("voice-roster", channel))
                    .show_rows(ui, 36.0, entries.len(), |ui, range| {
                        for index in range {
                            self.voice_participant(ui, state, entries[index]);
                        }
                    });
            }
        });
    }

    fn call_unavailable(&self, state: &State, channel: Id) -> Option<&'static str> {
        if state.demo {
            Some("Calls are unavailable in the offline preview. No microphone is accessed.")
        } else if !self.voice_available {
            Some(
                "This is the text-only build. Install a voice-enabled build to join voice channels and make calls.",
            )
        } else if state.auth != AuthState::Authenticated || !state.gateway_connected {
            Some("Reconnect to Discord before calling.")
        } else if state.voice.active.is_some() {
            Some("Leave your current call before starting another.")
        } else if !state.can_call(channel) {
            Some("Joining this channel is unavailable with current permission information.")
        } else {
            None
        }
    }

    pub(super) fn call_button(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut State,
        channel: Id,
        commands: &mut Vec<Command>,
    ) -> egui::Response {
        let unavailable = self.call_unavailable(state, channel);
        let incoming = state.voice.incoming == Some(channel);
        let guild = state
            .channels
            .iter()
            .any(|c| c.id == channel && c.kind == 2);
        let response = ui
            .add_enabled(
                unavailable.is_none(),
                egui::Button::new(if guild {
                    "Join voice"
                } else if incoming {
                    "Answer"
                } else {
                    "Call"
                }),
            )
            .on_hover_text(unavailable.unwrap_or(if state.can_speak(channel) {
                "Join audio. Your microphone starts after the call is secured."
            } else {
                "Join to listen. Speaking is unavailable in this channel."
            }));
        if response.clicked()
            && let Some(command) = state.start_call(channel, !guild && !incoming)
        {
            commands.push(command);
        }
        response
    }

    pub(super) fn voice_settings(&mut self, ui: &mut egui::Ui, demo: bool, active: bool) {
        ui.menu_button("Audio", |ui| {
            ui.set_max_width(300.0);
            ui.strong("Voice settings");
            if demo || !self.voice_available {
                ui.label(if demo {
                    "Microphone and speakers are unavailable in the offline preview."
                } else {
                    "Install a voice-enabled build to join voice channels and make calls."
                });
                return;
            }
            if active && let Some(code) = &self.voice_privacy_code {
                ui.label("Voice privacy code");
                ui.add(egui::Label::new(code).selectable(true).wrap());
                ui.label(RichText::new("Compare with the other participants; this code changes with the encrypted call group.").small());
                ui.separator();
            }
            ui.label("Microphone");
            device_combo(ui, "voice-input", &self.voice_inputs, &mut self.voice_input);
            ui.label("Speakers");
            device_combo(ui, "voice-output", &self.voice_outputs, &mut self.voice_output);
            gain_controls(ui, &mut self.voice_gain);
            if ui.button("Refresh audio devices").clicked() {
                self.voice_refresh_devices = true;
            }
            if !self.voice_device_status.is_empty() {
                ui.label(self.voice_device_status);
            }
            ui.separator();
            ui.checkbox(&mut self.voice_push_to_talk, "Push to talk");
            ui.label(RichText::new("Hold V while this window is focused and you are not typing. Mute and deafen always take priority.").small());
            ui.label(RichText::new("Device choices and levels apply to this session. Microphone capture begins only after you join a secured call.").small());
        });
    }

    pub(super) fn call_bar(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut State,
        commands: &mut Vec<Command>,
    ) {
        if state.voice.active.is_none() && state.voice.incoming.is_none() {
            return;
        }
        let colors = design::palette(ui);
        egui::Panel::top("dm-call")
            .frame(
                egui::Frame::new()
                    .fill(colors.raised)
                    .inner_margin(egui::Margin::symmetric(24, 12)),
            )
            .show(ui, |ui| {
                if let Some(call) = &state.voice.active {
                    let channel = call.channel;
                    let can_speak = state.can_speak(channel);
                    let phase = call.phase;
                    let mut muted = call.muted || !can_speak;
                    let mut deafened = call.deafened;
                    let error = call.error;
                    let server_muted = call.server_muted;
                    let server_deafened = call.server_deafened;
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(if state.demo {
                                "Voice preview"
                            } else {
                                phase.label()
                            })
                            .strong()
                            .color(colors.accent),
                        );
                        if let Some(elapsed) = elapsed_label(call) {
                            ui.label(RichText::new(elapsed).monospace().color(colors.accent));
                            ui.ctx()
                                .request_repaint_after(std::time::Duration::from_secs(1));
                        }
                        ui.add(
                            egui::Label::new(
                                state
                                    .channels
                                    .iter()
                                    .find(|c| c.id == channel)
                                    .map_or("Direct message", |c| c.name.as_str()),
                            )
                            .truncate(),
                        );
                    });
                    if call.guild.is_none() {
                        ui.horizontal_wrapped(|ui| {
                            for participant in call.participants.iter().take(2) {
                                let user = state
                                    .user
                                    .as_ref()
                                    .filter(|user| user.id == participant.user)
                                    .or_else(|| {
                                        state.channels.iter().find(|c| c.id == channel).and_then(
                                            |c| {
                                                c.recipients
                                                    .iter()
                                                    .find(|u| u.id == participant.user)
                                            },
                                        )
                                    });
                                if let Some(user) = user {
                                    self.avatars.show(ui, user, 22.0, state.demo);
                                    ui.label(&user.name);
                                } else {
                                    ui.label("Participant");
                                }
                                ui.label(
                                    RichText::new(if participant.deafened {
                                        "Deafened"
                                    } else if participant.muted {
                                        "Muted"
                                    } else {
                                        "Joined"
                                    })
                                    .small()
                                    .color(colors.muted),
                                );
                            }
                        });
                    }
                    if let Some(error) = error {
                        ui.label(error);
                    }
                    if server_deafened || server_muted {
                        ui.label(
                            RichText::new(if server_deafened {
                                "Deafened by the server"
                            } else {
                                "Muted by the server"
                            })
                            .small()
                            .color(colors.muted),
                        );
                    }
                    if self.voice_push_to_talk {
                        ui.label(
                            RichText::new("Push to talk · hold V while focused and not typing")
                                .small()
                                .color(colors.muted),
                        );
                    }
                    ui.horizontal_wrapped(|ui| {
                        let controls =
                            self.voice_available && !state.demo && phase != Phase::Failed;
                        let mute_changed = ui
                            .add_enabled(
                                controls && can_speak,
                                egui::Checkbox::new(&mut muted, "Mute microphone"),
                            )
                            .changed();
                        let deafen_changed = ui
                            .add_enabled(controls, egui::Checkbox::new(&mut deafened, "Deafen"))
                            .changed();
                        if (mute_changed || deafen_changed)
                            && let Some(command) = state.set_call_mute(muted, deafened)
                        {
                            commands.push(command);
                        }
                        self.voice_settings(ui, state.demo, true);
                        if ui
                            .add_enabled(
                                !state.demo,
                                egui::Button::new(if phase == Phase::Failed {
                                    "Dismiss call"
                                } else {
                                    "Leave voice"
                                }),
                            )
                            .clicked()
                            && let Some(command) = state.leave_call()
                        {
                            commands.push(command);
                        }
                    });
                    if !can_speak && !state.demo {
                        ui.weak("Speaking is unavailable in this channel. You can still listen.");
                    } else if !state.demo
                        && state.permission(channel, model::permissions::USE_VAD) != Some(true)
                    {
                        ui.weak(
                            "Push-to-talk is required to speak here. Enable it in Voice settings.",
                        );
                    }
                }
                if let Some(channel) = state.voice.incoming {
                    if state.voice.active.is_some() {
                        ui.separator();
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.strong("Incoming DM call");
                        ui.add(
                            egui::Label::new(
                                state
                                    .channels
                                    .iter()
                                    .find(|c| c.id == channel)
                                    .map_or("Direct message", |c| c.name.as_str()),
                            )
                            .truncate(),
                        );
                    });
                    if let Some(reason) = self.call_unavailable(state, channel) {
                        ui.label(RichText::new(reason).small().color(colors.muted));
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(
                                self.call_unavailable(state, channel).is_none(),
                                egui::Button::new("Answer"),
                            )
                            .clicked()
                            && let Some(command) = state.start_call(channel, false)
                        {
                            commands.push(command);
                        }
                        if ui
                            .add_enabled(!state.demo, egui::Button::new("Decline"))
                            .clicked()
                            && let Some(command) = state.decline_call()
                        {
                            commands.push(command);
                        }
                    });
                }
            });
    }
}

fn gain_controls(ui: &mut egui::Ui, gain: &mut crate::VoiceGain) -> [egui::Response; 2] {
    let label = ui.label("Microphone gain");
    let input = ui
        .add(
            egui::Slider::new(&mut gain.input_percent, 0..=200)
                .suffix("%")
                .step_by(1.0),
        )
        .labelled_by(label.id);
    let label = ui.label("Speaker volume");
    let output = ui
        .add(
            egui::Slider::new(&mut gain.output_percent, 0..=200)
                .suffix("%")
                .step_by(1.0),
        )
        .labelled_by(label.id);
    ui.label(RichText::new("100% keeps the original level. Boosting above 100% can clip.").small());
    if ui.small_button("Reset levels").clicked() {
        *gain = crate::VoiceGain::default();
    }
    [input, output]
}

fn elapsed_label(call: &client_core::voice::Call) -> Option<String> {
    if !matches!(call.phase, Phase::Waiting | Phase::Connected) {
        return None;
    }
    let seconds = call.connected_at?.elapsed().as_secs();
    Some(format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds / 60 % 60,
        seconds % 60
    ))
}

pub(super) fn speaker(ui: &egui::Ui, rect: egui::Rect, color: egui::Color32) {
    let point = |x, y| rect.min + egui::vec2(rect.width() * x, rect.height() * y);
    let stroke = egui::Stroke::new(1.6, color);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            point(0.05, 0.35),
            point(0.25, 0.35),
            point(0.50, 0.12),
            point(0.50, 0.88),
            point(0.25, 0.65),
            point(0.05, 0.65),
        ],
        color,
        egui::Stroke::NONE,
    ));
    for x in [0.64, 0.80] {
        ui.painter().add(egui::Shape::line(
            vec![point(x, 0.25), point(x + 0.08, 0.5), point(x, 0.75)],
            stroke,
        ));
    }
}

fn status_icon(ui: &mut egui::Ui, deafened: bool, label: &str) {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(17.0, 20.0), egui::Sense::hover());
    let point = |x, y| rect.min + egui::vec2(rect.width() * x, rect.height() * y);
    let stroke = egui::Stroke::new(1.5, design::palette(ui).muted);
    if deafened {
        ui.painter().add(egui::Shape::line(
            vec![
                point(0.12, 0.65),
                point(0.12, 0.35),
                point(0.25, 0.15),
                point(0.5, 0.08),
                point(0.75, 0.15),
                point(0.88, 0.35),
                point(0.88, 0.65),
            ],
            stroke,
        ));
        for x in [0.12, 0.72] {
            ui.painter().rect_filled(
                egui::Rect::from_min_max(point(x, 0.48), point(x + 0.16, 0.83)),
                2,
                stroke.color,
            );
        }
    } else {
        ui.painter().rect_stroke(
            egui::Rect::from_min_max(point(0.33, 0.06), point(0.67, 0.58)),
            4,
            stroke,
            egui::StrokeKind::Inside,
        );
        ui.painter().add(egui::Shape::line(
            vec![
                point(0.14, 0.45),
                point(0.18, 0.65),
                point(0.5, 0.76),
                point(0.82, 0.65),
                point(0.86, 0.45),
            ],
            stroke,
        ));
        ui.painter()
            .line_segment([point(0.5, 0.76), point(0.5, 0.95)], stroke);
        ui.painter()
            .line_segment([point(0.3, 0.95), point(0.7, 0.95)], stroke);
    }
    ui.painter()
        .line_segment([point(0.04, 0.98), point(0.98, 0.02)], stroke);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, label));
    response.on_hover_text(label);
}

fn device_combo(
    ui: &mut egui::Ui,
    id: &str,
    devices: &[(String, String)],
    selected: &mut Option<String>,
) {
    let label = match selected.as_ref() {
        None => "System default",
        Some(id) => devices
            .iter()
            .find(|(key, _)| key == id)
            .map_or("Device unavailable", |(_, label)| label.as_str()),
    };
    egui::ComboBox::from_id_salt(id)
        .selected_text(label)
        .width(256.0)
        .height(220.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, "System default");
            for (id, label) in devices.iter().take(32) {
                ui.selectable_value(selected, Some(id.clone()), label);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gain_sliders_accept_keyboard_input_and_reset_with_the_session() {
        let mut messaging = MessagingUi::default();
        assert_eq!(messaging.voice_gain.input_percent, 100);
        assert_eq!(messaging.voice_gain.output_percent, 100);
        let ctx = egui::Context::default();
        let raw = || egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(240.0, 260.0),
            )),
            ..Default::default()
        };
        ctx.run_ui(raw(), |ui| {
            gain_controls(ui, &mut messaging.voice_gain)[0].request_focus();
        })
        .drop_without_applying_deltas();
        let mut input = raw();
        input.events.push(egui::Event::Key {
            key: egui::Key::ArrowRight,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        ctx.run_ui(input, |ui| {
            let controls = gain_controls(ui, &mut messaging.voice_gain);
            assert!(
                controls
                    .iter()
                    .all(|r| r.rect.right() <= ui.max_rect().right() + 1.0)
            );
        })
        .drop_without_applying_deltas();
        assert_eq!(messaging.voice_gain.input_percent, 101);
        assert_eq!(messaging.voice_gain.output_percent, 100);
        assert!(
            !messaging.voice_refresh_devices,
            "Gain does not enumerate devices"
        );
        messaging.voice_gain.input_percent = u16::MAX;
        messaging.voice_gain.output_percent = 0;
        ctx.run_ui(raw(), |ui| {
            gain_controls(ui, &mut messaging.voice_gain);
        })
        .drop_without_applying_deltas();
        assert_eq!(messaging.voice_gain.input_percent, 200);
        assert_eq!(messaging.voice_gain.output_percent, 0);
        messaging.clear();
        assert_eq!(messaging.voice_gain.input_percent, 100);
        assert_eq!(messaging.voice_gain.output_percent, 100);
    }

    #[test]
    fn guild_voice_requires_explicit_keyboard_join_and_demo_never_emits_media() {
        let mut state = test_support::demo_state();
        state.demo = false;
        assert!(
            state.select(Id(25)).is_none(),
            "Voice selection must not fetch history"
        );
        assert_eq!(state.selected, Some(Id(25)));
        let mut messaging = MessagingUi {
            voice_available: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        let mut commands = vec![];
        context
            .run_ui(Default::default(), |ui| {
                messaging
                    .call_button(ui, &mut state, Id(25), &mut commands)
                    .request_focus();
            })
            .drop_without_applying_deltas();
        assert!(commands.is_empty());
        assert!(state.voice.active.is_none());
        let enter = egui::RawInput {
            events: vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        context
            .run_ui(enter, |ui| {
                messaging.call_button(ui, &mut state, Id(25), &mut commands);
            })
            .drop_without_applying_deltas();
        assert!(matches!(
            commands.as_slice(),
            [Command::Voice(client_core::voice::Command::Join {
                channel: Id(25),
                ring: false,
                ..
            })]
        ));
        let call = state.voice.active.as_mut().unwrap();
        assert!(elapsed_label(call).is_none());
        call.connected_at = Some(std::time::Instant::now() - std::time::Duration::from_secs(3663));
        call.phase = Phase::Waiting;
        assert_eq!(elapsed_label(call).as_deref(), Some("01:01:03"));
        call.phase = Phase::Failed;
        assert!(elapsed_label(call).is_none());
        state.demo = true;
        for width in [640.0, 1120.0] {
            let mut output = context.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 480.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    assert!(
                        messaging
                            .show(ui, &mut state)
                            .iter()
                            .all(|c| !matches!(c, Command::Voice(_) | Command::History { .. }))
                    );
                },
            );
            output.textures_delta.clear();
        }
        assert!(messaging.take_avatar_requests().is_empty());
        state.voice.active = None;
        assert!(messaging.call_unavailable(&state, Id(25)).is_some());
        state.demo = false;
        messaging.voice_available = false;
        assert!(
            messaging
                .call_unavailable(&state, Id(25))
                .unwrap()
                .contains("text-only")
        );
    }

    #[test]
    fn voice_roster_preserves_status_space_with_long_names_and_virtualizes() {
        let mut state = State {
            demo: true,
            selected: Some(Id(25)),
            ..Default::default()
        };
        state.voice.roster = (1..=64)
            .map(|id| RosterEntry {
                guild: Id(10),
                channel: Id(25),
                participant: client_core::voice::Participant {
                    user: Id(id),
                    muted: true,
                    deafened: true,
                    server_muted: false,
                    server_deafened: false,
                },
                member: Some(model::Member {
                    user: model::User {
                        id: Id(id),
                        name: "Long synthetic participant name ".repeat(5),
                        avatar: None,
                        discriminator: 0,
                    },
                    nick: None,
                    status: None,
                    custom_status: None,
                }),
            })
            .collect();
        let mut messaging = MessagingUi::default();
        let ctx = egui::Context::default();
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            ctx.set_theme(theme);
            let mut output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(190.0, 320.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    let width = ui.available_width();
                    let row = ui.scope(|ui| {
                        messaging.voice_participant(ui, &state, &state.voice.roster[0])
                    });
                    assert!(
                        row.response.rect.width() <= width + 1.0,
                        "Long names must not displace the mute/deafen icons"
                    );
                    messaging.voice_channel(ui, &mut state, Id(25), &mut vec![]);
                },
            );
            assert!(
                output.textures_delta.set.len() < 20,
                "Only visible avatars should be loaded"
            );
            output.textures_delta.clear();
        }
        assert!(messaging.take_avatar_requests().is_empty());
    }

    #[test]
    fn viewing_an_incoming_call_never_answers_and_preview_cannot_call() {
        let mut state = State {
            demo: true,
            selected: Some(Id(1)),
            auth: AuthState::Authenticated,
            gateway_connected: true,
            channels: vec![model::Channel {
                last_message: None,
                id: Id(1),
                guild: None,
                parent_id: None,
                position: 0,
                name: "Synthetic DM".into(),
                kind: 1,
                recipients: vec![model::User {
                    id: Id(2),
                    name: "Synthetic peer".into(),
                    avatar: None,
                    discriminator: 0,
                }],
                member_list_id: None,
            }],
            ..Default::default()
        };
        state.voice.incoming = Some(Id(1));
        let mut messaging = MessagingUi {
            voice_available: true,
            ..Default::default()
        };
        let context = egui::Context::default();
        let output = context.run_ui(Default::default(), |ui| {
            let commands = messaging.show(ui, &mut state);
            assert!(
                commands
                    .iter()
                    .all(|command| !matches!(command, Command::Voice(_)))
            );
        });
        output.drop_without_applying_deltas();
        assert!(state.voice.active.is_none());
        assert_eq!(state.voice.incoming, Some(Id(1)));
        assert!(messaging.call_unavailable(&state, Id(1)).is_some());
        state.demo = false;
        let output = context.run_ui(Default::default(), |ui| {
            let commands = messaging.show(ui, &mut state);
            assert!(
                commands
                    .iter()
                    .all(|command| !matches!(command, Command::Voice(_)))
            );
        });
        output.drop_without_applying_deltas();
        assert!(
            state.voice.active.is_none(),
            "Incoming calls require an explicit answer"
        );
        messaging.voice_available = false;
        assert!(
            messaging
                .call_unavailable(&state, Id(1))
                .unwrap()
                .contains("text-only")
        );
        messaging.voice_available = true;
        assert!(messaging.call_unavailable(&state, Id(1)).is_none());
    }
}
