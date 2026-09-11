//! Native embed cards. External links share the timeline's explicit confirmation.
use crate::{
	avatars::Avatars,
	markdown::{FormatCache, external_url},
};
use egui::RichText;
use model::{Embed, Gif, Message};
use std::hash::{DefaultHasher, Hash, Hasher};

pub fn has_spoilers(message: &Message) -> bool {
	has_media_spoilers(message) || message.content.contains("||")
}
pub fn has_media_spoilers(message: &Message) -> bool {
	message.attachments.iter().any(|a| {
		a.spoiler
			|| a.filename.starts_with("SPOILER_")
			|| a.description.as_deref().is_some_and(|s| s.contains("||"))
	}) || message.embeds.iter().any(|e| {
		[&e.title, &e.description]
			.into_iter()
			.flatten()
			.any(|s| s.contains("||"))
			|| e.fields
				.iter()
				.any(|f| f.name.contains("||") || f.value.contains("||"))
			|| [&e.author, &e.provider]
				.into_iter()
				.flatten()
				.any(|a| a.name.contains("||"))
			|| e.footer.as_ref().is_some_and(|f| f.text.contains("||"))
	})
}
fn link(
	ui: &mut egui::Ui,
	label: &str,
	url: Option<&str>,
	opening: &mut Option<String>,
	strong: bool,
) {
	let target = url.and_then(external_url);
	let mut text = RichText::new(label);
	if strong {
		text = text.strong();
	}
	if target.is_some() {
		text = text.color(ui.visuals().hyperlink_color);
	}
	let response = ui.add(egui::Label::new(text).wrap().sense(if target.is_some() {
		egui::Sense::click()
	} else {
		egui::Sense::hover()
	}));
	if let Some(target) = target {
		response.widget_info(|| {
			egui::WidgetInfo::labeled(egui::WidgetType::Link, ui.is_enabled(), label)
		});
		if response.on_hover_text("Open link…").clicked() {
			*opening = Some(target);
		}
	}
}
fn text(
	ui: &mut egui::Ui,
	message: &Message,
	part: (u16, &str),
	cache: &mut FormatCache,
	opening: &mut Option<String>,
	profile: &mut Option<model::User>,
	media: (&mut Avatars, bool),
) {
	let formatted = cache.get_part(message.id, part.0, part.1);
	formatted.show_with_images(ui, opening, &message.mentions, profile, media.0, media.1);
	if formatted.limited {
		ui.small("Text display limited");
	}
}
pub fn standalone_media_links(message: &Message) -> bool {
	!message.embeds_suppressed
		&& !has_spoilers(message)
		&& !message.content.trim().is_empty()
		&& message.content.split_whitespace().all(|link| {
			message.embeds.iter().any(|embed| {
				inline_image(embed).is_some()
					&& (embed.url.as_deref() == Some(link)
						|| [&embed.image, &embed.thumbnail]
							.into_iter()
							.flatten()
							.any(|media| media.url.as_deref() == Some(link)))
			})
		})
}

fn inline_image(embed: &Embed) -> Option<&model::EmbedMedia> {
	matches!(embed.kind.as_str(), "image" | "gifv")
		.then(|| embed.image.as_ref().or(embed.thumbnail.as_ref()))
		.flatten()
}

fn gif_for_embed(embed: &Embed, gifs: &client_core::gifs::Gifs) -> Option<Gif> {
	let media = [
		embed.image.as_ref(),
		embed.thumbnail.as_ref(),
		embed.video.as_ref(),
	];
	let matches = |gif: &&Gif| {
		embed.url.as_deref() == Some(gif.url.as_str())
			|| media.iter().flatten().any(|image| {
				image
					.url
					.as_deref()
					.is_some_and(|url| url == gif.url || url == gif.preview)
			})
	};
	if let Some(gif) = gifs
		.favorites
		.iter()
		.find(matches)
		.or_else(|| gifs.view.as_ref()?.page.as_ref()?.gifs.iter().find(matches))
	{
		return Some(gif.clone());
	}
	let preview = media.iter().flatten().find(|image| {
		image.url.as_deref().is_some_and(|url| {
			model::valid_gif_preview(url) && (embed.kind == "gifv" || url.ends_with(".gif"))
		})
	})?;
	let url = embed
		.url
		.as_deref()
		.filter(|url| model::valid_gif_url(url))
		.or_else(|| {
			preview
				.url
				.as_deref()
				.filter(|url| model::valid_gif_url(url))
		})?;
	let mut hash = DefaultHasher::new();
	url.hash(&mut hash);
	let gif = Gif {
		id: format!("chat-{:016x}", hash.finish()),
		title: embed.title.clone().unwrap_or_default(),
		url: url.to_owned(),
		preview: preview.url.clone()?,
		width: preview.width,
		height: preview.height,
	};
	gif.valid().then_some(gif)
}

pub fn show(
	ui: &mut egui::Ui,
	message: &Message,
	cache: &mut FormatCache,
	images: &mut Avatars,
	opening: &mut Option<String>,
	profile: &mut Option<model::User>,
	state: &client_core::State,
) -> Option<Gif> {
	if message.embeds_suppressed {
		return None;
	}
	let demo = state.demo;
	let mut favorite_action = None;
	for (index, embed) in message.embeds.iter().enumerate() {
		ui.push_id(("embed", index), |ui| {
			if let Some(image) = inline_image(embed) {
				let gif = gif_for_embed(embed, &state.gifs);
				let response = images
					.show_gif_embed(ui, embed, gif.as_ref(), egui::vec2(480.0, 320.0), demo)
					.interact(egui::Sense::click());
				let star = gif.map(|gif| {
					let favorite = state.is_gif_favorite(&gif);
					let star_rect = egui::Rect::from_min_size(
						response.rect.right_top() + egui::vec2(-34.0, 4.0),
						egui::Vec2::splat(30.0),
					);
					let star =
						ui.interact(star_rect, ui.id().with("favorite"), egui::Sense::click());
					ui.painter()
						.rect_filled(star_rect, 6, egui::Color32::from_black_alpha(190));
					crate::icons::paint(
						ui.painter(),
						if favorite {
							crate::icons::Icon::StarFill
						} else {
							crate::icons::Icon::Star
						},
						star_rect.shrink(5.0),
						if favorite {
							crate::design::palette(ui).warning
						} else {
							egui::Color32::WHITE
						},
					);
					if star.has_focus() {
						ui.painter().rect_stroke(
							star_rect,
							6,
							ui.visuals().selection.stroke,
							egui::StrokeKind::Inside,
						);
					}
					star.widget_info(|| {
						egui::WidgetInfo::selected(
							egui::WidgetType::Checkbox,
							ui.is_enabled(),
							favorite,
							"Favorite GIF",
						)
					});
					if star.clicked() {
						favorite_action = Some(gif);
					}
					star.on_hover_text(if favorite {
						"Remove from GIF favorites"
					} else {
						"Save to GIF favorites"
					})
				});
				if !star
					.as_ref()
					.is_some_and(|star| star.hovered() || star.clicked())
					&& response.on_hover_text("Open image…").clicked()
				{
					*opening = embed
						.url
						.as_deref()
						.or(image.url.as_deref())
						.and_then(external_url);
				}
				ui.add_space(6.0);
				return;
			}
			let colors = crate::design::palette(ui);
			let color = embed.color.map_or(colors.accent, |c| {
				egui::Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8)
			});
			let width = ui.available_width().min(480.0);
			let frame = egui::Frame::new()
				.fill(colors.raised)
				.corner_radius(5)
				.inner_margin(12)
				.show(ui, |ui| {
					ui.set_width((width - 24.0).max(1.0));
					// Size independently of the remaining timeline viewport.
					ui.set_max_height(640.0);
					// Bound exceptional cards' geometry; normal cards grow to their content height.
					egui::ScrollArea::vertical()
						.id_salt("embed-content")
						.max_height(640.0)
						.auto_shrink([false, true])
						.show(ui, |ui| {
							let part = 1 + index as u16 * 64;
							let thumbnail = embed
								.thumbnail
								.as_ref()
								.filter(|_| ui.available_width() >= 300.0);
							let body_width = (ui.available_width()
								- if thumbnail.is_some() { 96.0 } else { 0.0 })
							.max(1.0);
							ui.horizontal_top(|ui| {
								ui.vertical(|ui| {
									ui.set_width(body_width);
									if let Some(provider) = &embed.provider {
										link(
											ui,
											&provider.name,
											provider.url.as_deref(),
											opening,
											false,
										);
									}
									if let Some(author) = &embed.author {
										ui.horizontal_wrapped(|ui| {
											if let Some(icon) = &author.icon {
												images.show_embed(
													ui,
													icon,
													egui::vec2(20.0, 20.0),
													demo,
												);
											}
											link(
												ui,
												&author.name,
												author.url.as_deref(),
												opening,
												false,
											);
										});
									}
									if let Some(title) = &embed.title {
										link(ui, title, embed.url.as_deref(), opening, true);
									}
									if let Some(description) = &embed.description {
										text(
											ui,
											message,
											(part, description),
											cache,
											opening,
											profile,
											(images, demo),
										);
									}
								});
								if let Some(image) = thumbnail {
									images.show_embed(ui, image, egui::vec2(84.0, 84.0), demo);
								}
							});
							let mut field = 0;
							while field < embed.fields.len() {
								let columns = if ui.available_width() >= 360.0 {
									3
								} else if ui.available_width() >= 240.0 {
									2
								} else {
									1
								};
								let count = if embed.fields[field].inline {
									embed.fields[field..]
										.iter()
										.take_while(|f| f.inline)
										.take(columns)
										.count()
								} else {
									1
								};
								ui.columns(count, |columns| {
									for (offset, column) in columns.iter_mut().enumerate() {
										let f = &embed.fields[field + offset];
										column.add(
											egui::Label::new(RichText::new(&f.name).strong())
												.wrap()
												.selectable(true),
										);
										text(
											column,
											message,
											(part + 1 + (field + offset) as u16, &f.value),
											cache,
											opening,
											profile,
											(images, demo),
										);
									}
								});
								field += count;
							}
							if let Some(image) = &embed.image {
								images.show_embed(
									ui,
									image,
									egui::vec2(ui.available_width(), 320.0),
									demo,
								);
							}
							if thumbnail.is_none()
								&& let Some(image) = &embed.thumbnail
							{
								images.show_embed(ui, image, egui::vec2(84.0, 84.0), demo);
							}
							if embed.video.is_some()
								|| matches!(embed.kind.as_str(), "video" | "gifv")
							{
								ui.small("Video preview · playback opens in your browser");
								link(
									ui,
									"Open video…",
									embed.url.as_deref().or_else(|| {
										embed.video.as_ref().and_then(|v| v.url.as_deref())
									}),
									opening,
									false,
								);
							} else if embed.title.is_none()
								&& let Some(url) = embed.url.as_deref()
							{
								link(ui, "Open source…", Some(url), opening, false);
							}
							if let Some(footer) = &embed.footer {
								ui.horizontal_wrapped(|ui| {
									if let Some(icon) = &footer.icon {
										images.show_embed(ui, icon, egui::vec2(16.0, 16.0), demo);
									}
									ui.add(
										egui::Label::new(
											RichText::new(&footer.text).small().color(colors.muted),
										)
										.wrap()
										.selectable(true),
									);
								});
							}
							if let Some(timestamp) = &embed.timestamp {
								ui.small(timestamp);
							}
							if embed.limited {
								ui.small("Embed display limited");
							}
							if !matches!(
								embed.kind.as_str(),
								"rich" | "article" | "link" | "image" | "video" | "gifv"
							) {
								ui.small("Additional embed content is not supported");
							}
						});
				});
			ui.painter().line_segment(
				[
					frame.response.rect.left_top() + egui::vec2(1.5, 5.0),
					frame.response.rect.left_bottom() - egui::vec2(-1.5, 5.0),
				],
				egui::Stroke::new(3.0, color),
			);
			ui.add_space(6.0);
		});
	}
	favorite_action
}
pub fn estimated_height(embeds: &[Embed]) -> f32 {
	embeds
		.iter()
		.map(|e| {
			if inline_image(e).is_some() {
				206.0
			} else {
				100.0 + e.fields.len() as f32 * 44.0 + if e.image.is_some() { 200.0 } else { 0.0 }
			}
		})
		.map(|h| h.min(664.0))
		.sum()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn hide_media_links_requires_matching_visible_media_without_caption_or_spoiler() {
		let mut message = test_support::message(1, model::Id(1));
		message.content = "https://klipy.com/gifs/waving-lizard".into();
		message.embeds = vec![Embed {
			kind: "gifv".into(),
			url: Some(message.content.clone()),
			thumbnail: Some(model::EmbedMedia::default()),
			..Default::default()
		}];
		assert!(standalone_media_links(&message));
		message.embeds_suppressed = true;
		assert!(!standalone_media_links(&message));
		message.embeds_suppressed = false;
		message.content.insert_str(0, "Hello! ");
		assert!(!standalone_media_links(&message));
		message.content = "https://example.com/other.gif".into();
		assert!(!standalone_media_links(&message));
		message.content = message.embeds[0].url.clone().unwrap();
		message.embeds[0].kind = "rich".into();
		assert!(!standalone_media_links(&message));
	}

	#[test]
	fn chat_gifs_reuse_favorites_and_reject_unapproved_media() {
		let mut embed = Embed {
			kind: "gifv".into(),
			url: Some("https://klipy.com/gifs/synthetic-wave".into()),
			thumbnail: Some(model::EmbedMedia {
				url: Some("https://static.klipy.com/synthetic/wave.gif".into()),
				width: 320,
				height: 180,
				..Default::default()
			}),
			..Default::default()
		};
		let mut gifs = client_core::gifs::Gifs::default();
		let mut gif = gif_for_embed(&embed, &gifs).unwrap();
		assert!(gif.valid());
		gif.id = "provider-id".into();
		gifs.favorites.push(gif.clone());
		assert_eq!(gif_for_embed(&embed, &gifs), Some(gif));
		gifs.favorites.clear();
		embed.thumbnail.as_mut().unwrap().url = Some("https://example.com/wave.gif".into());
		assert!(gif_for_embed(&embed, &gifs).is_none());
	}

	#[test]
	fn direct_images_and_gifs_use_media_instead_of_cards() {
		let mut embed = Embed {
			kind: "image".into(),
			thumbnail: Some(model::EmbedMedia::default()),
			..Default::default()
		};
		assert!(inline_image(&embed).is_some());
		embed.kind = "gifv".into();
		assert!(inline_image(&embed).is_some());
		embed.kind = "rich".into();
		assert!(inline_image(&embed).is_none());
		embed.kind = "image".into();
		embed.thumbnail = None;
		assert!(inline_image(&embed).is_none());
		embed.image = Some(model::EmbedMedia::default());
		assert!(inline_image(&embed).is_some());
	}

	#[test]
	fn text_spoilers_do_not_hide_ordinary_media_but_keep_reply_previews_conservative() {
		let mut message = test_support::message(1, model::Id(1));
		message.embeds = vec![Embed::default()];
		message.attachments = vec![model::Attachment {
			id: model::Id(2),
			filename: "photo.png".into(),
			description: None,
			content_type: Some("image/png".into()),
			size: 1,
			media: Default::default(),
			spoiler: false,
		}];
		message.content = "Ordinary text".into();
		assert!(!has_spoilers(&message));
		message.content = "Ordinary text ||hidden text||".into();
		assert!(!has_media_spoilers(&message));
		assert!(has_spoilers(&message));
		message.content.clear();
		for field in 0..10 {
			let mut guarded = message.clone();
			let embed = &mut guarded.embeds[0];
			match field {
				0 => guarded.attachments[0].spoiler = true,
				1 => guarded.attachments[0].filename = "SPOILER_photo.png".into(),
				2 => guarded.attachments[0].description = Some("||hidden||".into()),
				3 => embed.title = Some("||hidden||".into()),
				4 => embed.description = Some("||hidden||".into()),
				5 => embed.fields.push(model::EmbedField {
					name: "||hidden||".into(),
					..Default::default()
				}),
				6 => embed.fields.push(model::EmbedField {
					value: "||hidden||".into(),
					..Default::default()
				}),
				7 => {
					embed.author = Some(model::EmbedAuthor {
						name: "||hidden||".into(),
						..Default::default()
					})
				}
				8 => {
					embed.provider = Some(model::EmbedAuthor {
						name: "||hidden||".into(),
						..Default::default()
					})
				}
				_ => {
					embed.footer = Some(model::EmbedFooter {
						text: "||hidden||".into(),
						..Default::default()
					})
				}
			}
			assert!(has_media_spoilers(&guarded), "media safety field {field}");
			assert!(has_spoilers(&guarded), "reply safety field {field}");
		}
	}
}
