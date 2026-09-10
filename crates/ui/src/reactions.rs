use model::{Reaction, ReactionEmoji};

pub fn show(
    ui: &mut egui::Ui,
    reactions: Option<&[Reaction]>,
    enabled: bool,
    writing: bool,
    refreshing: bool,
    media: (&mut crate::avatars::Avatars, bool),
) -> Option<Option<ReactionEmoji>> {
    if reactions.is_some_and(<[Reaction]>::is_empty) && !writing {
        return None;
    }
    let mut action = None;
    ui.horizontal_wrapped(|ui| {
        let Some(reactions)=reactions else {
            ui.weak(if refreshing {"Updating reactions…"}else{"Reactions unavailable"});
            if ui.add_enabled(enabled && !refreshing,egui::Button::new("Reload reactions").small()).clicked() {action=Some(None);}
            return;
        };
        for reaction in reactions {
            let label=format!("{} {}",reaction.emoji.label(),reaction.count);
            let button = if reaction.emoji.id.is_none() {
                crate::emoji::button(ui.ctx(), &reaction.emoji.label(), reaction.count.to_string())
            } else if let Some(image) = reaction.emoji.id.and_then(|id| media.0.custom_image(ui.ctx(), id, 18.0, media.1)) {
                egui::Button::image_and_text(image.alt_text(reaction.emoji.label()), reaction.count.to_string()).image_tint_follows_text_color(false)
            } else { egui::Button::new(label.clone()) };
            let response=ui.add_enabled(enabled && !writing && reaction.emoji.name.is_some(),button.small().selected(reaction.me));
            response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, response.enabled(), reaction.me, &label));
            let verb=if reaction.me {"Remove your reaction"}else{"Add your reaction"};
            let response=response.on_hover_text(format!("{verb}: {}. Count includes super reactions; only normal reactions can be toggled here.",reaction.emoji.label()));
            if response.clicked() {action=Some(Some(reaction.emoji.clone()));}
        }
        if writing {ui.weak("Saving reaction…");}
    });
    action
}

pub fn add_button(
    ui: &mut egui::Ui,
    enabled: bool,
    writing: bool,
) -> Option<Option<ReactionEmoji>> {
    let mut action = None;
    ui.add_enabled_ui(enabled && !writing, |ui| {
        let response = crate::icons::button(ui, crate::icons::Icon::Smile, 28.0, "Add reaction");
        egui::Popup::menu(&response).show(|ui| {
            for (name, label) in [
                ("👍", "Like"),
                ("❤️", "Love"),
                ("😂", "Laugh"),
                ("🎉", "Celebrate"),
                ("👀", "Eyes"),
                ("✅", "Done"),
                ("🙏", "Thanks"),
                ("😢", "Sad"),
            ] {
                if ui
                    .add(crate::emoji::button(ui.ctx(), name, label.into()))
                    .clicked()
                {
                    action = Some(Some(ReactionEmoji {
                        id: None,
                        name: Some(name.into()),
                    }));
                    ui.close();
                }
            }
        });
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), "Add reaction")
        });
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_reactions_do_not_allocate_a_row() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let before = ui.min_rect();
            assert_eq!(
                show(
                    ui,
                    Some(&[]),
                    true,
                    false,
                    false,
                    (&mut crate::avatars::Avatars::default(), true),
                ),
                None
            );
            assert_eq!(ui.min_rect(), before);
        });
        output.drop_without_applying_deltas();
    }

    #[test]
    fn keyboard_reaction_toggle_and_disabled_refresh_emit_only_local_actions() {
        let values = vec![Reaction {
            emoji: ReactionEmoji {
                id: None,
                name: Some("👍".into()),
            },
            count: 3,
            me: true,
            me_burst: false,
        }];
        for enabled in [true, false] {
            let ctx = egui::Context::default();
            crate::emoji::install(&ctx).unwrap();
            let mut action = None;
            for key in [None, Some(egui::Key::Tab), Some(egui::Key::Enter)] {
                let input = egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(240.0, 180.0),
                    )),
                    events: key
                        .map(|key| {
                            vec![egui::Event::Key {
                                key,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: egui::Modifiers::NONE,
                            }]
                        })
                        .unwrap_or_default(),
                    ..Default::default()
                };
                let mut output = ctx.run_ui(input, |ui| {
                    action = show(
                        ui,
                        Some(&values),
                        enabled,
                        false,
                        false,
                        (&mut crate::avatars::Avatars::default(), true),
                    );
                });
                assert!(output.platform_output.commands.is_empty());
                output.textures_delta.clear();
            }
            assert_eq!(action, enabled.then(|| Some(values[0].emoji.clone())));
        }
    }
}
