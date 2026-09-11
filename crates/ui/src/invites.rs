//! Fixed-height native invite cards; only visible cards request bounded metadata.
use crate::avatars::Avatars;
use client_core::invites::valid_code;
use model::Message;
fn code(raw: &str) -> Option<String> {
	let normalized;
	let raw = if raw.starts_with("discord.gg/") || raw.starts_with("discord.com/invite/") {
		normalized = format!("https://{raw}");
		&normalized
	} else {
		raw
	};
	let url = url::Url::parse(raw).ok()?;
	if !matches!(url.scheme(), "https" | "http")
		|| !url.username().is_empty()
		|| url.password().is_some()
		|| url.port().is_some()
	{
		return None;
	}
	let path = url.path().trim_end_matches('/');
	let code = match url.host_str()? {
		"discord.gg" | "www.discord.gg" => path.strip_prefix('/')?,
		"discord.com" | "www.discord.com" | "discordapp.com" | "www.discordapp.com" => {
			path.strip_prefix("/invite/")?
		}
		_ => return None,
	};
	valid_code(code).then(|| code.to_owned())
}
fn codes(message: &Message) -> Vec<String> {
	if message.embeds_suppressed || message.content.contains("||") {
		return Vec::new();
	}
	let mut found = Vec::new();
	let mut in_code = false;
	for event in pulldown_cmark::Parser::new(&message.content) {
		use pulldown_cmark::{Event, Tag, TagEnd};
		match event {
			Event::Start(Tag::CodeBlock(_)) => in_code = true,
			Event::End(TagEnd::CodeBlock) => in_code = false,
			Event::Start(Tag::Link { dest_url, .. }) if !in_code => {
				if let Some(code) = code(&dest_url)
					&& !found.contains(&code)
				{
					found.push(code);
				}
			}
			Event::Text(text) if !in_code => {
				for token in text.split_whitespace() {
					let token = token.trim_matches(|c: char| {
						matches!(
							c,
							'<' | '>' | '(' | ')' | '[' | ']' | ',' | '.' | '!' | '?' | ';' | '"'
						)
					});
					if let Some(code) = code(token)
						&& !found.contains(&code)
					{
						found.push(code);
					}
					if found.len() == 3 {
						break;
					}
				}
			}
			_ => {}
		}
		if found.len() == 3 {
			break;
		}
	}
	found
}
pub fn estimated_height(message: &Message) -> f32 {
	codes(message).len() as f32 * 284.0
}
pub fn show(
	ui: &mut egui::Ui,
	message: &Message,
	state: &client_core::State,
	images: &mut Avatars,
	requests: &mut Vec<String>,
	join: &mut Option<String>,
) {
	let demo = state.demo;
	for code in codes(message) {
		ui.push_id(("invite", &code), |ui| {
			let colors = crate::design::palette(ui);
			let width = ui.available_width().min(360.0);
			ui.allocate_ui_with_layout(
				egui::vec2(width, 280.0),
				egui::Layout::top_down(egui::Align::Min),
				|ui| {
					egui::Frame::new()
						.fill(colors.raised)
						.corner_radius(8)
						.stroke(egui::Stroke::new(1.0, colors.border))
						.inner_margin(0)
						.show(ui, |ui| {
							ui.set_width(width);

							let entry = state
								.invites
								.get(&code)
								.filter(|(at, _)| at.elapsed().as_secs() < 300);
							let embed = entry
								.and_then(|(_, value)| value.as_ref())
								.and_then(|r| r.as_ref().ok());
							let member = embed.is_some_and(|p| state.guild(p.guild).is_some());
							let embed = embed.map(|p| &p.embed);
							if !demo
								&& entry.is_none() && requests.len() < 8
								&& !requests.contains(&code)
							{
								requests.push(code.clone());
							}
							let banner_size = egui::vec2(ui.available_width(), 88.0);
							let banner_rect =
								if let Some(banner) = embed.and_then(|e| e.image.as_ref()) {
									images.show_banner(ui, banner, banner_size, demo).rect
								} else {
									let (rect, _) =
										ui.allocate_exact_size(banner_size, egui::Sense::hover());
									ui.painter().rect_filled(
										rect,
										egui::CornerRadius {
											nw: 8,
											ne: 8,
											sw: 0,
											se: 0,
										},
										colors.accent.gamma_multiply(0.12),
									);
									rect
								};
							let icon_rect = egui::Rect::from_min_size(
								banner_rect.left_bottom() + egui::vec2(12.0, -24.0),
								egui::vec2(48.0, 48.0),
							);
							ui.painter()
								.rect_filled(icon_rect.expand(4.0), 10, colors.raised);
							if let Some(icon) = embed.and_then(|e| e.thumbnail.as_ref()) {
								let mut icon_ui =
									ui.new_child(egui::UiBuilder::new().max_rect(icon_rect));
								images.show_embed(&mut icon_ui, icon, icon_rect.size(), demo);
							} else {
								ui.painter().rect_filled(icon_rect, 8, colors.sidebar);
							}
							egui::Frame::new().inner_margin(12).show(ui, |ui| {
								ui.set_width((width - 24.0).max(1.0));
								ui.add_space(24.0);
								ui.vertical(|ui| {
									ui.label(
										egui::RichText::new("SERVER INVITE")
											.size(10.0)
											.color(colors.muted),
									);
									ui.add(
										egui::Label::new(
											egui::RichText::new(
												embed
													.and_then(|e| e.title.as_deref())
													.unwrap_or("Server invite"),
											)
											.strong()
											.size(18.0),
										)
										.truncate(),
									);
								});
								ui.add_space(6.0);
								if let Some(embed) = embed {
									ui.add(
										egui::Label::new(
											embed
												.description
												.as_deref()
												.unwrap_or("Member counts unavailable"),
										)
										.truncate(),
									);
								} else {
									ui.weak(if demo {
										"Preview unavailable offline"
									} else if entry.is_some_and(|(_, v)| matches!(v, Some(Err(_))))
									{
										"Invite expired or preview unavailable"
									} else {
										"Loading server preview…"
									});
								}
								ui.add_space(12.0);
								let current = state.invite_join.code == code;
								let pending = current && state.invite_join.pending;
								let accepted =
									current && matches!(state.invite_join.result, Some(Ok(_)));
								let label = if member {
									"Already joined"
								} else if pending {
									"Joining…"
								} else if accepted {
									"Invite accepted"
								} else {
									"Join server"
								};
								ui.add_enabled_ui(state.can_join_invite(&code), |ui| {
									if ui
										.add_sized(
											[ui.available_width(), 36.0],
											egui::Button::new(
												egui::RichText::new(label)
													.strong()
													.color(egui::Color32::WHITE),
											)
											.fill(colors.accent)
											.corner_radius(6),
										)
										.clicked()
									{
										*join = Some(code.clone());
									}
								});
								if current && let Some(Err(f)) = state.invite_join.result {
									ui.add(
										egui::Label::new(egui::RichText::new(f.label()).small())
											.truncate(),
									)
									.on_hover_text(f.label());
								}
							});
						});
				},
			);
			ui.add_space(4.0);
		});
	}
}
#[cfg(test)]
mod tests {
	#[test]
	fn invite_urls_are_origin_and_path_checked() {
		assert_eq!(
			super::code("https://discord.gg/abc-123").as_deref(),
			Some("abc-123")
		);
		assert_eq!(
			super::code("https://discord.com/invite/abc").as_deref(),
			Some("abc")
		);
		for bad in [
			"https://discord.gg.evil.test/abc",
			"https://discord.gg@evil.test/abc",
			"https://evil@discord.gg/abc",
			"https://discord.gg/a/b",
			"https://discord.gg/%2e%2e",
			"https://discord.gg/",
		] {
			assert!(super::code(bad).is_none());
		}
	}
}
