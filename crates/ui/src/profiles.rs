//! Native profile card populated only from the selected service profile.
use crate::{
    avatars::Avatars,
    design,
    markdown::{Formatted, external_url},
};
use client_core::{State, profile::ProfileView};
use egui::{Color32, RichText};
use model::{Id, User};

pub enum Action {
    Close,
    Retry,
    Message(Id),
    Profile(User),
}
fn heading(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(11.0)
            .strong()
            .color(design::palette(ui).muted),
    );
}
pub fn show(
    ui: &mut egui::Ui,
    user: &User,
    view: Option<&ProfileView>,
    state: &State,
    avatars: &mut Avatars,
    opening: &mut Option<String>,
) -> Option<Action> {
    let colors = design::palette(ui);
    let viewport = ui.ctx().content_rect();
    let width = 400.0_f32.min(viewport.width() - 56.0).max(240.0);
    let data = view.and_then(|v| v.data.as_ref());
    let mut action = None;
    let modal = egui::Modal::new(egui::Id::new("user-profile-card"))
        .backdrop_color(Color32::from_black_alpha(150))
        .frame(
            egui::Frame::new()
                .fill(colors.surface)
                .corner_radius(12)
                .stroke(egui::Stroke::new(1.0, colors.border)),
        )
        .show(ui.ctx(), |ui| {
            ui.set_width(width);
            ui.set_max_height(viewport.height() - 56.0);
            let banner = if let Some(data) = data {
                avatars
                    .show_banner(ui, data, egui::vec2(width, 140.0), state.demo)
                    .rect
            } else {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(width, 140.0), egui::Sense::hover());
                ui.painter().rect_filled(rect, 10, colors.raised);
                rect
            };
            let close_rect = egui::Rect::from_min_size(
                banner.right_top() + egui::vec2(-76.0, 10.0),
                egui::vec2(66.0, 28.0),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(close_rect), |ui| {
                if ui
                    .add(egui::Button::new("Close ×").wrap_mode(egui::TextWrapMode::Extend))
                    .clicked()
                {
                    action = Some(Action::Close);
                }
            });
            let avatar_rect = egui::Rect::from_min_size(
                banner.left_bottom() + egui::vec2(20.0, -42.0),
                egui::vec2(88.0, 88.0),
            );
            ui.painter()
                .circle_filled(avatar_rect.center(), 49.0, colors.surface);
            ui.scope_builder(egui::UiBuilder::new().max_rect(avatar_rect), |ui| {
                if let Some(data) = data {
                    avatars.show_profile_avatar(ui, data, 88.0, state.demo);
                } else {
                    avatars.show(ui, user, 88.0, state.demo);
                }
            });
            egui::Frame::new()
                .inner_margin(egui::Margin {
                    left: 20,
                    right: 20,
                    top: 8,
                    bottom: 20,
                })
                .show(ui, |ui| {
                    let display = data
                        .and_then(|p| {
                            p.guild
                                .as_ref()
                                .and_then(|g| g.nick.as_deref())
                                .or(p.global_name.as_deref())
                        })
                        .unwrap_or(&user.name);
                    ui.add(egui::Label::new(RichText::new(display).size(26.0).strong()).truncate());
                    if let Some(data) = data {
                        let username = if data.user.discriminator > 0 {
                            format!("{}#{:04}", data.username, data.user.discriminator)
                        } else {
                            data.username.clone()
                        };
                        ui.add(
                            egui::Label::new(RichText::new(username).color(colors.muted))
                                .truncate(),
                        );
                        let pronouns = data
                            .guild
                            .as_ref()
                            .map(|g| g.pronouns.as_str())
                            .filter(|s| !s.is_empty())
                            .unwrap_or(&data.pronouns);
                        if !pronouns.is_empty() {
                            ui.label(RichText::new(pronouns).small().color(colors.muted));
                        }
                    }
                    if let Some(status) = state
                        .members
                        .as_ref()
                        .and_then(|m| m.rows.iter().flatten().find(|m| m.user.id == user.id))
                        .and_then(|m| m.status.as_deref())
                    {
                        ui.small(match status {
                            "online" => "Online",
                            "idle" => "Away",
                            "dnd" => "Do not disturb",
                            _ => "Offline",
                        });
                    }
                    if let Some(channel) = state
                        .channels
                        .iter()
                        .find(|c| c.kind == 1 && c.recipients.iter().any(|u| u.id == user.id))
                        && ui
                            .add_sized(
                                [ui.available_width(), 32.0],
                                egui::Button::new("Message").fill(colors.accent),
                            )
                            .clicked()
                    {
                        action = Some(Action::Message(channel.id));
                    }
                    if state.demo {
                        ui.small("Offline preview · synthetic profile");
                    }
                    if view.is_none_or(|v| v.loading) {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Loading profile…");
                        });
                    }
                    if let Some(error) = view.and_then(|v| v.error) {
                        ui.label(error);
                        if ui.button("Retry profile").clicked() {
                            action = Some(Action::Retry);
                        }
                    }
                    if let Some(data) = data {
                        ui.separator();
                        egui::ScrollArea::vertical()
                            .id_salt("profile-details")
                            .max_height(
                                (viewport.height() - 84.0 - (ui.cursor().top() - banner.top()))
                                    .max(80.0),
                            )
                            .show(ui, |ui| {
                                let bio = data
                                    .guild
                                    .as_ref()
                                    .map(|g| g.bio.as_str())
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or(&data.bio);
                                if !bio.is_empty() {
                                    heading(ui, "ABOUT ME");
                                    let mut linked_user = None;
                                    Formatted::parse(bio).show_with_images(
                                        ui,
                                        opening,
                                        &[],
                                        &mut linked_user,
                                        avatars,
                                        state.demo,
                                    );
                                    if let Some(user) = linked_user {
                                        action = Some(Action::Profile(user));
                                    }
                                    ui.add_space(10.0);
                                }
                                heading(ui, "DISCORD MEMBER SINCE");
                                let seconds = ((user.id.0 >> 22) + 1_420_070_400_000) / 1000;
                                if let Ok(date) =
                                    time::OffsetDateTime::from_unix_timestamp(seconds as i64)
                                {
                                    ui.label(date.date().to_string());
                                }
                                if let Some(joined) =
                                    data.guild.as_ref().and_then(|g| g.joined_at.as_deref())
                                {
                                    heading(ui, "SERVER MEMBER SINCE");
                                    ui.label(joined.split('T').next().unwrap_or(joined));
                                }
                                if !data.badges.is_empty() {
                                    ui.add_space(8.0);
                                    heading(ui, "BADGES");
                                    ui.horizontal_wrapped(|ui| {
                                        for badge in &data.badges {
                                            ui.label(RichText::new(&badge.description).small())
                                                .on_hover_text(&badge.id);
                                        }
                                    });
                                }
                                if !data.connections.is_empty() {
                                    ui.add_space(8.0);
                                    heading(ui, "CONNECTIONS");
                                    for connection in &data.connections {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.strong(&connection.kind);
                                            ui.label(&connection.name);
                                            if connection.verified {
                                                ui.small("Verified");
                                            }
                                        });
                                    }
                                }
                                if !data.mutual_guilds.is_empty() {
                                    ui.add_space(8.0);
                                    heading(ui, "MUTUAL SERVERS");
                                    for guild in &data.mutual_guilds {
                                        if let Some(known) =
                                            state.guilds.iter().find(|g| g.id == guild.id)
                                        {
                                            ui.label(&known.name);
                                        } else {
                                            ui.label(format!("Server {}", guild.id));
                                        }
                                    }
                                }
                                if data.limited {
                                    ui.small("Some profile details were limited");
                                }
                                ui.add_space(12.0);
                                if ui.small_button("Copy user ID").clicked() {
                                    ui.ctx().copy_text(user.id.to_string());
                                }
                            });
                    }
                });
        });
    if modal.should_close() && opening.is_none() {
        action = Some(Action::Close);
    }
    if let Some(url) = opening.as_ref() {
        let mut close = false;
        let response =
            egui::Modal::new(egui::Id::new("profile-external-link")).show(ui.ctx(), |ui| {
                ui.heading("Open external link?");
                ui.add(egui::Label::new(url).wrap());
                ui.horizontal(|ui| {
                    if ui.button("Open in browser").clicked() {
                        if let Some(target) = external_url(url) {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(target));
                        }
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            });
        if close || response.should_close() {
            *opening = None;
        }
    }
    action
}

// Explicit offline-only data; never a fallback for a failed service request.
pub fn synthetic(user: &User, guild: Option<Id>) -> model::UserProfile {
    model::UserProfile {
        user:user.clone(), username:"serein.preview".into(),global_name:Some(user.name.clone()),
        banner:Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()),accent_color:Some(0x315c68),
        bio:"Building a quieter place for conversations.\n**Native profile preview** · all details here are synthetic.".into(),pronouns:"they / them".into(),
        badges:vec![model::ProfileBadge{id:"preview".into(),description:"Synthetic badge".into()}],connections:vec![model::ProfileConnection{kind:"GitHub".into(),name:"synthetic-profile".into(),verified:false}],
        mutual_guilds:guild.map(|id|vec![model::ProfileGuild{id,nick:None}]).unwrap_or_default(),guild:None,limited:false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_card_shows_selected_data_without_network_and_closes_with_escape() {
        fn text(shape: &egui::Shape, output: &mut String) {
            match shape {
                egui::Shape::Text(s) => output.push_str(&s.galley.job.text),
                egui::Shape::Vec(items) => {
                    for shape in items {
                        text(shape, output);
                    }
                }
                _ => {}
            }
        }
        let user = User {
            id: Id(2),
            name: "Synthetic person".into(),
            avatar: None,
            discriminator: 0,
        };
        let profile = ProfileView {
            user: user.id,
            guild: None,
            request: 1,
            loading: false,
            error: None,
            data: Some(synthetic(&user, None)),
        };
        let state = State {
            demo: true,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let mut images = Avatars::default();
        let mut opening = None;
        let mut painted = String::new();
        for _ in 0..3 {
            let output = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1000.0, 900.0),
                    )),
                    ..Default::default()
                },
                |ui| {
                    assert!(
                        show(ui, &user, Some(&profile), &state, &mut images, &mut opening)
                            .is_none()
                    );
                },
            );
            for shape in &output.shapes {
                text(&shape.shape, &mut painted);
            }
            assert!(output.platform_output.commands.is_empty());
            output.drop_without_applying_deltas();
        }
        assert!(
            painted.contains("Synthetic person")
                && painted.contains("ABOUT ME")
                && painted.contains("they / them")
        );
        assert!(images.take_requests().is_empty());
        let output = ctx.run_ui(
            egui::RawInput {
                events: vec![egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }],
                ..Default::default()
            },
            |ui| {
                assert!(matches!(
                    show(ui, &user, Some(&profile), &state, &mut images, &mut opening),
                    Some(Action::Close)
                ));
            },
        );
        output.drop_without_applying_deltas();
    }
}
