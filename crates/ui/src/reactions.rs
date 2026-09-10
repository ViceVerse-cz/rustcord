use model::{Reaction, ReactionEmoji};

pub fn show(
    ui: &mut egui::Ui,
    reactions: Option<&[Reaction]>,
    enabled: bool,
    writing: bool,
    refreshing: bool,
) -> Option<Option<ReactionEmoji>> {
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
            } else { egui::Button::new(label.clone()) };
            let response=ui.add_enabled(enabled && !writing && reaction.emoji.name.is_some(),button.small().selected(reaction.me));
            response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, response.enabled(), reaction.me, &label));
            let verb=if reaction.me {"Remove your reaction"}else{"Add your reaction"};
            let response=response.on_hover_text(format!("{verb}: {}. Count includes super reactions; only normal reactions can be toggled here.",reaction.emoji.label()));
            if response.clicked() {action=Some(Some(reaction.emoji.clone()));}
        }
        ui.add_enabled_ui(enabled && !writing,|ui| {
            egui::containers::menu::MenuButton::new("+ Reaction").ui(ui,|ui| {
                for (name,label) in [("👍","Like"),("❤️","Love"),("😂","Laugh"),("🎉","Celebrate"),("👀","Eyes"),("✅","Done"),("🙏","Thanks"),("😢","Sad")] {
                    if ui.add(crate::emoji::button(ui.ctx(), name, label.into())).clicked() {
                        action=Some(Some(ReactionEmoji{id:None,name:Some(name.into())}));ui.close();
                    }
                }
            });
        });
        if writing {ui.weak("Saving reaction…");}
    });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
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
                    action = show(ui, Some(&values), enabled, false, false);
                });
                assert!(output.platform_output.commands.is_empty());
                output.textures_delta.clear();
            }
            assert_eq!(action, enabled.then(|| Some(values[0].emoji.clone())));
        }
    }
}
