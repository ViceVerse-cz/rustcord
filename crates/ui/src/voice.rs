use crate::{MessagingUi, design};
use client_core::{Command, State, auth::AuthState, voice::Phase};
use egui::RichText;
use model::Id;

impl MessagingUi {
    fn call_unavailable(&self, state: &State, channel: Id) -> Option<&'static str> {
        if state.demo {
            Some("Calls are unavailable in the offline preview. No microphone is accessed.")
        } else if !self.voice_available {
            Some("This is the text-only build. Install a voice-enabled build to make DM calls.")
        } else if state.auth != AuthState::Authenticated || !state.gateway_connected {
            Some("Reconnect to Discord before calling.")
        } else if state.voice.active.is_some() {
            Some("Leave your current call before starting another.")
        } else if !state.can_call(channel) {
            Some("This call needs an available one-to-one DM and its recipient details.")
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
        let response = ui
            .add_enabled(
                unavailable.is_none(),
                egui::Button::new(if incoming { "Answer" } else { "Call" }),
            )
            .on_hover_text(unavailable.unwrap_or(
                "Start a private voice call. Your microphone starts after the call is secured.",
            ));
        if response.clicked()
            && let Some(command) = state.start_call(channel, !incoming)
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
                    "Install a voice-enabled build to make DM calls."
                });
                return;
            }
            if active && let Some(code) = &self.voice_privacy_code {
                ui.label("Voice privacy code");
                ui.add(egui::Label::new(code).selectable(true).wrap());
                ui.label(RichText::new("Compare with the other participant; this code changes with the encrypted call group.").small());
                ui.separator();
            }
            ui.label("Microphone");
            device_combo(ui, "voice-input", &self.voice_inputs, &mut self.voice_input);
            ui.label("Speakers");
            device_combo(ui, "voice-output", &self.voice_outputs, &mut self.voice_output);
            if ui.button("Refresh audio devices").clicked() {
                self.voice_refresh_devices = true;
            }
            if !self.voice_device_status.is_empty() {
                ui.label(self.voice_device_status);
            }
            ui.separator();
            ui.checkbox(&mut self.voice_push_to_talk, "Push to talk");
            ui.label(RichText::new("Hold V while this window is focused and you are not typing. Mute and deafen always take priority.").small());
            ui.label(RichText::new("Device choices apply to this session. Microphone capture begins only after you join a secured call.").small());
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
                    let phase = call.phase;
                    let mut muted = call.muted;
                    let mut deafened = call.deafened;
                    let error = call.error;
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(phase.label()).strong().color(colors.accent));
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
                    ui.horizontal_wrapped(|ui| {
                        for participant in call.participants.iter().take(2) {
                            let user = state
                                .user
                                .as_ref()
                                .filter(|user| user.id == participant.user)
                                .or_else(|| {
                                    state
                                        .channels
                                        .iter()
                                        .find(|c| c.id == channel)
                                        .and_then(|c| {
                                            c.recipients.iter().find(|u| u.id == participant.user)
                                        })
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
                    if let Some(error) = error {
                        ui.label(error);
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
                                controls,
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
                                    "Hang up"
                                }),
                            )
                            .clicked()
                            && let Some(command) = state.leave_call()
                        {
                            commands.push(command);
                        }
                    });
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
