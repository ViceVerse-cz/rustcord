//! Native egui views; emits commands without owning transports or session credentials.
mod archives;
mod audio;
pub use audio::{AudioCommand, AudioState, AudioUi};
mod attachments;
pub use attachments::DownloadUi;
mod avatars;
pub use avatars::GifFrames;
mod categories;
mod composer_text;
pub mod design;
mod embeds;
pub mod emoji;
mod emoji_picker;
pub mod fonts;
mod forum;
pub mod icons;
mod invites;
mod markdown;
mod mentions;
mod notifications;
mod pending;
mod profiles;
mod reactions;
mod reading;
mod search;
mod settings;
mod switcher;
mod timeline;
mod typing;
mod user_menu;
mod voice;
use client_core::{Command, MAX_CONTENT, MAX_DRAFT_BYTES, State};
use egui::{RichText, TextEdit};
use model::{Freshness, Id};

pub struct VoiceGain {
	pub input_percent: u16,
	pub output_percent: u16,
}
impl Default for VoiceGain {
	fn default() -> Self {
		Self {
			input_percent: 100,
			output_percent: 100,
		}
	}
}

pub struct AttachmentPaste {
	pub target: egui::Id,
	pub text: Option<String>,
	pub image: Option<std::sync::Arc<egui::ColorImage>>,
}

enum MemberRow {
	Header(String),
	Member(usize, bool),
}

#[derive(Default)]
pub struct MessagingUi {
	member_cache_key: Option<(u64, u64, Option<Id>, bool)>,
	member_cache: Vec<MemberRow>,
	member_count: usize,
	composer_layout: composer_text::Layout,
	channel_cache: categories::Cache,
	search: search::SearchUi,
	settings: settings::Settings,
	switcher: switcher::Switcher,
	focus_switched_composer: bool,
	switcher_frame: bool,
	archives: archives::ArchivesUi,
	archive_parent: Option<Id>,
	forum: forum::ForumUi,
	timeline: timeline::TimelineView,
	edit_modified: Option<(Id, Id, bool)>,
	edit_undo_cleared: bool,
	edit_widget_id: Option<egui::Id>,
	edit_closed_channel: Option<Id>,
	avatars: avatars::Avatars,
	profile: Option<model::User>,
	user_action: Option<user_menu::Action>,
	profile_link: Option<String>,
	pub reading_preferences: model::ReadingPreferences,
	pub reading_status: &'static str,
	pub reading_save_requested: bool,
	reading_sidebar_applied: Option<u16>,
	reading_sidebar_constrained: bool,
	reading_zoom_pending: bool,
	/// Where the open profile was requested from; the popout is placed beside it.
	profile_anchor: Option<(Id, egui::Pos2)>,
	members_narrow_open: bool,
	member_reload_requested: bool,
	guild: Option<Id>,
	collapsed_categories: std::collections::BTreeSet<Id>,
	navigation_channel: Option<Id>,
	pub logout_requested: bool,
	pub reconnect_requested: bool,
	pub draft_changes: Vec<Id>,
	pub draft_restore_pending: bool,
	pub attachment: Option<(String, u64)>,
	/// Downscaled pixels of the selected image attachment, produced off the render thread.
	pub attachment_preview: Option<std::sync::Arc<egui::ColorImage>>,
	attachment_texture: Option<(usize, egui::TextureHandle)>,
	pub attach_requested: bool,
	pub attachment_paste_requested: Option<AttachmentPaste>,
	pub pasted_text: Option<(Id, egui::Id, String)>,
	paste_key_handled: bool,
	pasted_text_frame: Option<u64>,
	pub remove_attachment_requested: bool,
	pub cancel_upload_requested: bool,
	pub upload_busy: bool,
	pub upload_status: Option<String>,
	pending_upload: Option<pending::Upload>,
	pub clear_cache_requested: bool,
	pub voice_available: bool,
	pub voice_inputs: Vec<(String, String)>,
	pub voice_outputs: Vec<(String, String)>,
	pub voice_input: Option<String>,
	pub voice_output: Option<String>,
	pub voice_gain: VoiceGain,
	pub voice_refresh_devices: bool,
	pub voice_device_status: &'static str,
	pub voice_push_to_talk: bool,
	pub voice_noise_suppression: bool,
	pub voice_ptt_active: bool,
	pub voice_privacy_code: Option<String>,
	/// Latest media activity, capped at 64 IDs (512 bytes) by the voice host.
	pub voice_speaking: Vec<Id>,
	pub notifications_enabled: bool,
	pub notification_test_available: bool,
	pub notification_test_requested: bool,
	pub notification_status: &'static str,
	pub storage_status: &'static str,
	/// Release channel and version shown in the title bar.
	pub build: design::Build,
	/// Header pin button rect while the pins popout is open.
	pins_anchor: Option<egui::Rect>,
	editing: Option<(Id, Id, String)>,
	composer_edit: Option<(Id, Id)>,
	edit_sent: bool,
	deleting: Option<(Id, Id)>,
	ime_active: bool,
	mention_menu: mentions::Menu,
	emoji_picker: emoji_picker::Picker,
	/// Set when the user picks a theme preset; the host persists it.
	pub theme_variant_changed: Option<design::Variant>,
}

impl MessagingUi {
	pub fn timeline_reflows(&self) -> (u64, u64) {
		(
			self.timeline.reflow_frames,
			self.timeline.consecutive_reflows,
		)
	}
	/// Fixture-only entry point: opens People and the profile card for `user` as if clicked.
	pub fn preview_profile(&mut self, user: model::User) {
		self.members_narrow_open = true;
		self.profile = Some(user);
	}
	/// Fixture-only entry point: opens the emoji popout as if the composer button was clicked.
	/// Fixture-only entry point: stage a synthetic attachment as if it had been selected.
	pub fn preview_attachment(
		&mut self,
		filename: &str,
		bytes: u64,
		preview: Option<egui::ColorImage>,
	) {
		self.attachment = Some((filename.to_owned(), bytes));
		self.attachment_preview = preview.map(std::sync::Arc::new);
	}
	fn stage_pending_upload(&mut self, command: &Command) {
		if let Command::Send { nonce, .. } = command
			&& let Some((_, bytes)) = self.attachment.take()
		{
			self.pending_upload = Some(pending::Upload {
				nonce: nonce.clone(),
				bytes,
				preview: self.attachment_texture.take().map(|(_, texture)| texture),
				progress: None,
			});
			self.attachment_preview = None;
			self.upload_busy = true;
		}
	}
	/// Synthetic pending state only; no command is dispatched or file uploaded.
	pub fn preview_sending(&mut self, ctx: &egui::Context, state: &mut State) {
		if !state.demo {
			return;
		}
		let Some(channel) = state.selected else {
			return;
		};
		state
			.drafts
			.insert(channel, "Here’s the attachment — sending it now.".into());
		if let Some(image) = &self.attachment_preview {
			self.attachment_texture = Some((
				std::sync::Arc::as_ptr(image) as usize,
				ctx.load_texture(
					"synthetic-pending",
					egui::ImageData::Color(image.clone()),
					egui::TextureOptions::LINEAR,
				),
			));
		}
		if let Some(command) = state
			.prepare_send_with_attachment(self.attachment.as_ref().map(|(name, _)| name.as_str()))
		{
			self.stage_pending_upload(&command);
			if let Some(upload) = &mut self.pending_upload {
				upload.progress = Some((upload.bytes * 42 / 100, upload.bytes));
			}
		}
	}
	/// Byte counts describe data supplied to HTTP, never message delivery.
	pub fn update_upload_progress(&mut self, progress: Option<(u64, u64)>, sending: bool) {
		if let Some(upload) = &mut self.pending_upload {
			upload.progress = if sending {
				Some((upload.bytes, upload.bytes))
			} else {
				progress
			};
		}
	}
	/// Fixture-only: open the pinned messages popout on the next frame.
	pub fn preview_pins(&mut self) {
		self.search.preview_pins();
	}
	pub fn preview_emoji_picker(&mut self) {
		self.emoji_picker.preview();
	}
	/// Fixture-only: opens the GIFs tab at `section` (`""`, `favorites`, `trending` or a query).
	pub fn preview_gif_picker(&mut self, section: &str) {
		self.emoji_picker.preview_gifs(section);
	}
	/// Fixture-only entry point: opens the search pane and submits `query` on the first frame.
	pub fn preview_search(&mut self, query: &str) {
		self.search.preview(query);
	}
	pub fn downloads(&mut self) -> &mut DownloadUi {
		&mut self.timeline.download
	}
	pub fn audio(&mut self) -> &mut AudioUi {
		&mut self.timeline.audio
	}
	pub fn clear_avatars(&mut self) {
		self.avatars = avatars::Avatars::default();
	}
	pub fn take_avatar_requests(&mut self) -> Vec<String> {
		self.avatars.take_requests()
	}
	pub fn accept_gif_animation(&mut self, key: String, frames: GifFrames) {
		self.avatars.accept_animation(key, frames);
	}
	pub fn accept_avatar(
		&mut self,
		ctx: &egui::Context,
		key: String,
		image: Option<egui::ColorImage>,
	) {
		self.avatars.accept(ctx, key, image);
	}
	pub fn clear(&mut self) {
		*self = Self::default();
	}
	pub fn has_edit(&self) -> bool {
		self.editing.is_some()
	}
	pub fn messages_deleted(&mut self, ctx: &egui::Context, channel: Id, ids: &[Id]) {
		if ids.len() > 100 {
			return;
		}
		if self
			.deleting
			.is_some_and(|(c, id)| c == channel && ids.contains(&id))
		{
			self.deleting = None;
		}
		let Some((edit_channel, message, _)) = &self.editing else {
			return;
		};
		if *edit_channel != channel || !ids.contains(message) {
			return;
		}
		let untouched = self
			.edit_modified
			.is_some_and(|(c, id, modified)| c == channel && id == *message && !modified);
		if let Some(id) = self.edit_widget_id {
			if untouched {
				egui::text_edit::TextEditState::default().store(ctx, id);
			} else if let Some(mut editor) = egui::text_edit::TextEditState::load(ctx, id) {
				editor.clear_undoer();
				editor.store(ctx, id);
			}
		}
		self.edit_sent = false;
		self.edit_undo_cleared = true;
		if untouched {
			self.editing = None;
			self.edit_modified = None;
			self.edit_widget_id = None;
			self.edit_closed_channel = Some(channel);
		}
	}
	fn reconcile_edit(&mut self, state: &State) {
		let Some((channel, id, content)) = &self.editing else {
			self.edit_modified = None;
			self.edit_undo_cleared = false;
			return;
		};
		if self
			.edit_modified
			.is_none_or(|(c, message, _)| c != *channel || message != *id)
		{
			self.edit_undo_cleared = false;
			let modified = state
				.timeline
				.get(*id)
				.filter(|message| message.channel == *channel)
				.is_none_or(|message| message.content != *content);
			self.edit_modified = Some((*channel, *id, modified));
		}
		if state.selected == Some(*channel)
			&& state.timeline.get(*id).is_none()
			&& self.edit_modified.is_some_and(|(_, _, modified)| !modified)
		{
			self.editing = None;
			self.edit_modified = None;
			self.edit_sent = false;
		}
	}
	/// Window title strip: traffic-light inset, centred context title and session state.
	fn title_bar(&mut self, ui: &mut egui::Ui, state: &State, title: &str) {
		let colors = design::palette(ui);
		egui::Panel::top("title-bar")
			.exact_size(36.0)
			.show_separator_line(false)
			.frame(egui::Frame::new().fill(colors.base))
			.show(ui, |ui| {
				let rect = ui.max_rect();
				let drag = ui.interact(rect, ui.id().with("drag"), egui::Sense::click_and_drag());
				if drag.drag_started() {
					ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
				}
				let title_rect = egui::Rect::from_center_size(
					rect.center(),
					egui::vec2(rect.width() * 0.3, rect.height()),
				);
				ui.scope_builder(
					egui::UiBuilder::new().max_rect(title_rect).layout(
						egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
					),
					|ui| {
						ui.add(
							egui::Label::new(design::semibold(ui, title, 13.0).color(colors.text))
								.truncate(),
						);
					},
				);

				ui.scope_builder(
					egui::UiBuilder::new()
						.max_rect(egui::Rect::from_min_max(
							egui::pos2(title_rect.right() + 10.0, rect.top()),
							rect.right_bottom() - egui::vec2(12.0, 0.0),
						))
						.layout(egui::Layout::right_to_left(egui::Align::Center)),
					|ui| {
						ui.spacing_mut().item_spacing.x = 10.0;
						design::build_badge(ui, self.build);
						if state.demo {
							egui::Frame::new()
								.stroke(egui::Stroke::new(1.0, colors.border))
								.corner_radius(10)
								.inner_margin(egui::Margin::symmetric(8, 3))
								.show(ui, |ui| {
									ui.label(
										design::semibold(ui, "OFFLINE PREVIEW", 10.0)
											.color(colors.muted),
									);
								})
								.response
								.on_hover_text("Synthetic data · no network or local storage");
						}
						if !state.demo
							&& state.auth != client_core::auth::AuthState::Authenticated
							&& ui.small_button("Sign in again").clicked()
						{
							self.reconnect_requested = true;
						}
						ui.add(
							egui::Label::new(
								RichText::new(state.status).size(11.0).color(colors.muted),
							)
							.truncate(),
						)
						.on_hover_text(state.status);
					},
				);
			});
	}
	fn member_rows(&mut self, ui: &mut egui::Ui, state: &State) {
		let colors = design::palette(ui);
		let Some(list) = state
			.members
			.as_ref()
			.filter(|list| Some(list.channel) == state.selected)
		else {
			ui.add_space(8.0);
			ui.label(RichText::new("Choose a conversation to see its people.").color(colors.muted));
			return;
		};
		if matches!(list.freshness, Freshness::Stale | Freshness::Unavailable) {
			ui.add_space(8.0);
			ui.label(
				RichText::new(match list.freshness {
					Freshness::Stale => "Awaiting member sync",
					_ => "Member list unavailable",
				})
				.small()
				.color(colors.muted),
			);
			if ui.small_button("Reload people").clicked() {
				self.member_reload_requested = true;
			}
		} else if list.freshness == Freshness::Loading {
			ui.add_space(8.0);
			ui.label(RichText::new("Loading people…").small().color(colors.muted));
		}
		let cache_key = (
			state.generation,
			state.revision,
			state.selected,
			state.gateway_connected,
		);
		if self.member_cache_key != Some(cache_key) {
			let members: Vec<_> = list
				.rows
				.iter()
				.enumerate()
				.filter_map(|(i, m)| m.as_ref().map(|m| (i, m)))
				.collect();
			let online = |(_, member): &(usize, &model::Member)| {
				profiles::member_presence(state, member, list.guild)
					.0
					.is_some_and(|s| matches!(s, "online" | "idle" | "dnd"))
			};
			let (online_members, offline_members): (Vec<_>, Vec<_>) =
				members.iter().copied().partition(online);
			let mut rows = Vec::with_capacity(members.len() + 2);
			if let Some(guild) = list.guild {
				let mut online_members: Vec<_> = online_members
					.into_iter()
					.map(|member| (member, state.member_roles(guild, member.1).0))
					.collect();
				online_members.sort_by(|a, b| match (a.1, b.1) {
					(Some(a), Some(b)) => b.cmp_hierarchy(a),
					(Some(_), None) => std::cmp::Ordering::Less,
					(None, Some(_)) => std::cmp::Ordering::Greater,
					(None, None) => std::cmp::Ordering::Equal,
				});
				for members in
					online_members.chunk_by(|a, b| a.1.map(|r| r.id) == b.1.map(|r| r.id))
				{
					let name = members[0].1.map_or("Online", |r| {
						if r.name.is_empty() {
							"Role"
						} else {
							r.name.as_str()
						}
					});
					rows.push(MemberRow::Header(format!("{name} — {}", members.len())));
					rows.extend(members.iter().map(|(m, _)| MemberRow::Member(m.0, true)));
				}
				if !offline_members.is_empty() {
					rows.push(MemberRow::Header(format!(
						"Offline — {}",
						offline_members.len()
					)));
					rows.extend(
						offline_members
							.iter()
							.map(|m| MemberRow::Member(m.0, false)),
					);
				}
			} else {
				rows.push(MemberRow::Header(format!("Members — {}", members.len())));
				rows.extend(members.iter().map(|m| MemberRow::Member(m.0, online(m))));
			}
			self.member_count = members.len();
			self.member_cache = rows;
			self.member_cache_key = Some(cache_key);
		}
		let member_count = self.member_count;
		if member_count == 0 && list.freshness == Freshness::Fresh {
			ui.add_space(8.0);
			ui.label(
				RichText::new("No people returned for this view.")
					.small()
					.color(colors.muted),
			);
		}
		let hint = if list.guild.is_some() && list.total > member_count as u64 {
			Some(format!(
				"Showing {} of {} members",
				member_count, list.total
			))
		} else {
			None
		};
		egui::ScrollArea::vertical()
			.id_salt(("people", list.channel))
			.auto_shrink([false, false])
			.show_rows(ui, 42.0, self.member_cache.len(), |ui, range| {
				ui.spacing_mut().item_spacing.y = 0.0;
				for index in range {
					match &self.member_cache[index] {
						MemberRow::Header(text) => {
							let (rect, _) = ui.allocate_exact_size(
								egui::vec2(ui.available_width(), 42.0),
								egui::Sense::hover(),
							);
							let mut header = ui.new_child(
								egui::UiBuilder::new()
									.max_rect(egui::Rect::from_min_max(
										rect.left_top() + egui::vec2(8.0, 16.0),
										rect.right_bottom() - egui::vec2(8.0, 0.0),
									))
									.layout(egui::Layout::left_to_right(egui::Align::Center)),
							);
							header
								.add(
									egui::Label::new(
										design::medium(ui, text, 12.0).color(colors.muted),
									)
									.truncate(),
								)
								.on_hover_text(text);
						}
						MemberRow::Member(member_index, online) => {
							let member = list.rows[*member_index]
								.as_ref()
								.expect("cached member row");
							let name = member.nick.as_deref().unwrap_or(&member.user.name);
							let (status, custom, activities) =
								profiles::member_presence(state, member, list.guild);
							let subtitle = profiles::subtitle(custom, activities);
							let (rect, response) = ui.allocate_exact_size(
								egui::vec2(ui.available_width(), 42.0),
								egui::Sense::click(),
							);
							response.widget_info(|| {
								egui::WidgetInfo::labeled(
									egui::WidgetType::Button,
									true,
									format!(
										"{name}, {}, {}",
										status.map_or("presence unknown", profiles::presence_label),
										subtitle.as_deref().unwrap_or_default()
									),
								)
							});
							let row = rect.shrink2(egui::vec2(0.0, 1.0));
							if response.hovered() || response.has_focus() {
								ui.painter().rect_filled(row, 6, colors.hover);
							}
							let mut inner = ui.new_child(
								egui::UiBuilder::new()
									.max_rect(row.shrink2(egui::vec2(8.0, 4.0)))
									.layout(egui::Layout::left_to_right(egui::Align::Center)),
							);
							inner.spacing_mut().item_spacing.x = 12.0;
							inner.push_id(member.user.id.0, |ui| {
								let avatar = self.avatars.show(ui, &member.user, 32.0, state.demo);
								user_menu::show(
									&avatar,
									state,
									&member.user,
									&mut self.profile,
									&mut self.user_action,
								);
								if avatar.clicked() {
									self.profile = Some(member.user.clone());
								}
								if let Some(status) = status {
									design::presence_dot(
										ui,
										avatar.rect,
										profiles::presence_color(status),
										colors.sidebar,
									);
								}
								let text_color = if *online {
									let role_color = list.guild.and_then(|guild| {
										state.member_roles(guild, member).1.map(|role| role.color)
									});
									let background = if response.hovered() || response.has_focus() {
										colors.hover
									} else {
										colors.sidebar
									};
									role_color.map_or(colors.text, |rgb| {
										design::role_name_color(rgb, background, colors.text)
									})
								} else {
									colors.muted
								};
								ui.vertical(|ui| {
									ui.spacing_mut().item_spacing.y = 1.0;
									ui.add(
										egui::Label::new(
											design::medium(ui, name, 15.0).color(text_color),
										)
										.truncate()
										.selectable(false),
									);
									ui.add(
										egui::Label::new(
											RichText::new(subtitle.as_deref().unwrap_or({
												match status {
													Some("online") => "Online",
													Some("idle") => "Away",
													Some("dnd") => "Do not disturb",
													Some("offline" | "invisible") => "Offline",
													_ => "Presence unavailable",
												}
											}))
											.size(12.0)
											.color(colors.muted),
										)
										.truncate()
										.selectable(false),
									);
								});
							});
							user_menu::show(
								&response,
								state,
								&member.user,
								&mut self.profile,
								&mut self.user_action,
							);
							if response.clicked() {
								self.profile = Some(member.user.clone());
							}
						}
					}
				}
			});
		if let Some(hint) = hint {
			ui.label(RichText::new(hint).size(11.0).color(colors.muted));
		}
	}
	/// Channel sidebar: context header, scrolling list and the account card.
	fn sidebar(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		title: &str,
		commands: &mut Vec<Command>,
	) {
		let colors = design::palette(ui);
		egui::Panel::top("sidebar-header")
			.exact_size(48.0)
			.show_separator_line(false)
			.frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 0)))
			.show(ui, |ui| {
				let rect = ui.max_rect();
				ui.horizontal_centered(|ui| {
					ui.add(
						egui::Label::new(
							design::semibold(ui, title, 15.0).color(colors.text_strong),
						)
						.truncate(),
					);
				});
				ui.painter().hline(
					rect.x_range().expand(16.0),
					rect.bottom(),
					egui::Stroke::new(1.0, colors.border),
				);
			});
		self.voice_connection_panel(ui, state, commands);
		egui::Frame::new()
			.inner_margin(egui::Margin {
				left: 8,
				right: 8,
				top: 8,
				bottom: 0,
			})
			.show(ui, |ui| {
				let ctx = ui.ctx().clone();
				if self.guild.is_none() {
					let find = ui
						.add_enabled_ui(
							!self.ime_active
								&& !ctx.input(|input| {
									input
										.events
										.iter()
										.any(|event| matches!(event, egui::Event::Ime(_)))
								}),
							|ui| {
								ui.add_sized(
									[ui.available_width(), 30.0],
									egui::Button::new("Find conversation").truncate(),
								)
							},
						)
						.inner
						.on_hover_text("Search loaded conversations (Ctrl/Cmd+K)");
					find.widget_info(|| {
						egui::WidgetInfo::labeled(
							egui::WidgetType::Button,
							ui.is_enabled(),
							"Find conversation, Ctrl or Command K",
						)
					});
					if find.clicked() {
						self.switcher.open(&ctx);
					}
					ui.add_space(8.0);
				}
				if self.guild.is_none() {
					ui.horizontal(|ui| {
						ui.add_space(8.0);
						ui.label(design::eyebrow(ui, "Direct Messages", colors.muted));
					});
					ui.add_space(4.0);
				}
				let select = self.channel_list(ui, state);
				if let Some(id) = select
					&& let Some(command) = state.select(id)
				{
					commands.push(command);
				}
				if let Some(parent) = self.archive_parent.take()
					&& let Some(command) =
						state.request_archives(parent, model::archives::Kind::Public, None)
				{
					self.search.open = false;
					self.archives.focus = true;
					commands.push(command);
				}
			});
	}
	fn account_card(&mut self, ui: &mut egui::Ui, state: &mut State, commands: &mut Vec<Command>) {
		let colors = design::palette(ui);
		egui::Frame::new()
			.fill(colors.raised)
			.corner_radius(8)
			.inner_margin(egui::Margin::symmetric(8, 6))
			.show(ui, |ui| {
				ui.set_width(ui.available_width());
				ui.horizontal(|ui| {
					ui.spacing_mut().item_spacing.x = 8.0;
					if let Some(user) = &state.user {
						let avatar = self.avatars.show(ui, user, 32.0, state.demo);
						user_menu::show(
							&avatar,
							state,
							user,
							&mut self.profile,
							&mut self.user_action,
						);
						design::presence_dot(ui, avatar.rect, colors.positive, colors.raised);
						if avatar.clicked() {
							self.profile = Some(user.clone());
						}
					}
					ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
						ui.spacing_mut().item_spacing.x = 2.0;
						let settings = icons::button(ui, icons::Icon::Gear, 32.0, "User settings");
						if settings.clicked() {
							self.settings.open = true;
							egui::Popup::close_all(ui.ctx());
						}
						self.mute_toggle(ui, state, commands, true, 32.0);
						self.mute_toggle(ui, state, commands, false, 32.0);
						ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
							ui.vertical(|ui| {
								ui.spacing_mut().item_spacing.y = 0.0;
								ui.add(
									egui::Label::new(
										design::semibold(
											ui,
											state
												.user
												.as_ref()
												.map_or("Your account", |u| u.name.as_str()),
											14.0,
										)
										.color(colors.text_strong),
									)
									.truncate()
									.selectable(false),
								);
								ui.add(
									egui::Label::new(
										RichText::new(if state.demo {
											"Offline preview"
										} else if state.gateway_connected {
											"Online"
										} else {
											"Reconnecting…"
										})
										.size(12.0)
										.color(colors.muted),
									)
									.truncate()
									.selectable(false),
								);
							});
						});
					});
				});
			});
	}
	/// Conversation header: channel identity on the left, tools and search on the right.
	fn channel_header(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		selected_voice: bool,
		show_members: bool,
		wide_members: bool,
		commands: &mut Vec<Command>,
	) {
		let colors = design::palette(ui);
		egui::Panel::top("channel-header")
			.exact_size(48.0)
			.show_separator_line(false)
			.frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 0)))
			.show(ui, |ui| {
				let rect = ui.max_rect();
				ui.painter().hline(
					rect.x_range().expand(16.0),
					rect.bottom(),
					egui::Stroke::new(1.0, colors.border),
				);
				let channel = state
					.selected
					.and_then(|id| state.channels.iter().find(|c| c.id == id))
					.cloned();
				let dm = channel
					.as_ref()
					.is_some_and(|c| c.kind == 1 && c.guild.is_none());
				ui.horizontal_centered(|ui| {
					ui.spacing_mut().item_spacing.x = 8.0;
					match channel.as_ref() {
						Some(c) if c.guild.is_none() => {
							if let Some(user) = c.recipients.first() {
								let avatar = self.avatars.show(ui, user, 24.0, state.demo);
								if dm {
									user_menu::show(
										&avatar,
										state,
										user,
										&mut self.profile,
										&mut self.user_action,
									);
								}
								if dm
									&& let Some(status) = profiles::presence(state, user.id, None).0
								{
									design::presence_dot(
										ui,
										avatar.rect,
										profiles::presence_color(status),
										colors.sidebar,
									);
								}
								if avatar.clicked() {
									self.profile = Some(user.clone());
								}
							} else {
								icons::inline(ui, icons::Icon::People, 22.0, colors.muted);
							}
						}
						Some(c) => {
							let icon = match c.kind {
								2 | 13 => icons::Icon::Speaker,
								15 | 16 => icons::Icon::Forum,
								10..=12 => icons::Icon::Threads,
								_ => icons::Icon::Hash,
							};
							icons::inline(ui, icon, 22.0, colors.muted);
						}
						None => {}
					}
					ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
						ui.spacing_mut().item_spacing.x = 4.0;
						if state.selected.is_some() && !selected_voice {
							if self.search.open && !self.search.pins() {
								ui.allocate_ui_with_layout(
									egui::vec2(
										240.0_f32.min(ui.available_width() * 0.5).max(120.0),
										28.0,
									),
									egui::Layout::left_to_right(egui::Align::Center),
									|ui| self.search.header_input(ui, state, commands),
								);
							} else {
								// Search pill.
								let (pill, response) = ui.allocate_exact_size(
									egui::vec2(144.0, 28.0),
									egui::Sense::click(),
								);
								let enabled = state.can_search();
								response.widget_info(|| {
									egui::WidgetInfo::labeled(
										egui::WidgetType::Button,
										enabled,
										"Search",
									)
								});
								ui.painter().rect_filled(pill, 6, colors.raised);
								let pill_text = if enabled {
									colors.muted
								} else {
									colors.muted.gamma_multiply(0.5)
								};
								ui.painter().text(
									pill.left_center() + egui::vec2(10.0, 0.0),
									egui::Align2::LEFT_CENTER,
									"Search",
									egui::FontId::proportional(13.0),
									pill_text,
								);
								icons::paint(
									ui.painter(),
									icons::Icon::Search,
									egui::Rect::from_center_size(
										pill.right_center() - egui::vec2(14.0, 0.0),
										egui::Vec2::splat(16.0),
									),
									pill_text,
								);
								if enabled
									&& response.on_hover_text("Search this conversation").clicked()
								{
									if state.archives.is_some() {
										commands.push(state.clear_archives());
									}
									self.search.toggle(false);
								}
							}
							ui.add_space(4.0);
							if icons::toggle(
								ui,
								icons::Icon::People,
								32.0,
								show_members,
								"Show member list",
							)
							.clicked()
							{
								if wide_members {
									self.reading_preferences.show_members =
										!self.reading_preferences.show_members;
								} else {
									self.members_narrow_open = !self.members_narrow_open;
								}
							}
							let pins_open = self.search.open && self.search.pins();
							let pins = ui
								.add_enabled_ui(state.can_search() || pins_open, |ui| {
									icons::toggle(
										ui,
										icons::Icon::Pin,
										32.0,
										pins_open,
										"Pinned messages",
									)
								})
								.inner;
							if pins.clicked()
								&& self.search.toggle(true)
								&& let Some(command) = state.request_pins()
							{
								commands.push(command);
							}
							self.pins_anchor =
								(self.search.open && self.search.pins()).then_some(pins.rect);
							if let Some(c) = channel
								.as_ref()
								.filter(|c| c.guild.is_some() && matches!(c.kind, 0 | 5 | 15 | 16))
							{
								let allowed =
									state.can_archive(c.id, model::archives::Kind::Public);
								let archive = ui
									.add_enabled_ui(allowed, |ui| {
										icons::button(ui, icons::Icon::Threads, 32.0, "Threads")
									})
									.inner;
								if archive.clicked() {
									self.archive_parent = Some(c.id);
								}
							}
							let reload = ui
								.add_enabled_ui(
									state.freshness != Freshness::Loading
										&& state
											.selected
											.is_some_and(|id| state.can_read_history(id)),
									|ui| {
										icons::button(
											ui,
											icons::Icon::Reload,
											32.0,
											"Reload history",
										)
									},
								)
								.inner;
							if reload.clicked() {
								self.timeline.follow_latest();
								commands.push(state.history(None));
							}
						}
						if let Some(channel) = state.selected.filter(|_| dm) {
							self.voice_settings(ui, state.demo, state.voice.active.is_some());
							self.call_button(ui, state, channel, commands);
						}
						ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
							// Centre the name block in the fixed-height header even without a subtitle.
							let name = channel
								.as_ref()
								.map_or("Direct Messages", |c| c.name.as_str());
							let subtitle = channel
								.as_ref()
								.filter(|_| dm)
								.and_then(|c| c.recipients.first())
								.and_then(|user| {
									let (_, custom, activities) =
										profiles::presence(state, user.id, None);
									profiles::subtitle(custom, activities)
								});
							let name_height = ui
								.painter()
								.layout_no_wrap(
									name.to_owned(),
									egui::FontId::new(16.0, design::semibold_family(ui.ctx())),
									colors.text_strong,
								)
								.size()
								.y;
							let subtitle_height = subtitle.as_ref().map_or(0.0, |text| {
								1.0 + ui
									.painter()
									.layout_no_wrap(
										text.clone(),
										egui::FontId::proportional(12.0),
										colors.muted,
									)
									.size()
									.y
							});
							ui.vertical(|ui| {
								ui.add_space(
									((ui.available_height() - name_height - subtitle_height) / 2.0)
										.max(0.0),
								);
								ui.spacing_mut().item_spacing.y = 1.0;
								ui.add(
									egui::Label::new(
										design::semibold(ui, name, 16.0).color(colors.text_strong),
									)
									.truncate(),
								);
								{
									if let Some(text) = subtitle {
										ui.add(
											egui::Label::new(
												RichText::new(&text).size(12.0).color(colors.muted),
											)
											.truncate(),
										)
										.on_hover_text(text);
									}
								}
							});
							let in_call = state.voice.active.as_ref().is_some_and(|call| {
								Some(call.channel) == state.selected
									&& matches!(
										call.phase,
										client_core::voice::Phase::Connected
											| client_core::voice::Phase::Waiting
									)
							});
							if in_call {
								ui.add_space(4.0);
								icons::inline(ui, icons::Icon::InCall, 16.0, colors.positive);
								ui.add(
									egui::Label::new(
										design::medium(ui, "In a call", 14.0)
											.color(colors.positive),
									)
									.selectable(false),
								);
							}
						});
					});
				});
			});
	}
	fn clear_draft(&mut self, state: &mut State, channel: Id) {
		if self.draft_restore_pending {
			// Preserve an explicit clear until the outstanding disk snapshot is merged.
			state.drafts.insert(channel, String::new());
		} else {
			state.drafts.remove(&channel);
		}
		self.draft_changes.push(channel);
	}
	fn restore_pending(&mut self, state: &mut State, channel: Id, nonce: &str) {
		if let Some(index) = state.pending.iter().position(|p| {
			p.nonce == nonce && p.channel == channel && p.delivery != model::Delivery::Sending
		}) {
			if !state.drafts.contains_key(&channel) && state.drafts.len() >= 64 {
				state.status =
					"Draft budget full. Clear an existing draft before restoring pending text";
			} else if state.drafts.get(&channel).is_none_or(String::is_empty) {
				let pending = state.pending.remove(index);
				if pending.attachment.is_some() {
					state.status = "Text restored; reselect the attachment before sending again";
				}
				state.drafts.insert(channel, pending.content);
				self.draft_changes.push(channel);
			} else {
				state.status = "Keep or clear the existing draft before restoring pending text";
			}
		}
	}
	fn composer(
		&mut self,
		ui: &mut egui::Ui,
		state: &mut State,
		channel: Id,
		ctx: &egui::Context,
		commands: &mut Vec<Command>,
	) {
		let had_edit = self.editing.is_some();
		self.reconcile_edit(state);
		let closed_here = self.edit_closed_channel.take() == Some(channel);
		if closed_here || (had_edit && self.editing.is_none()) {
			egui::text_edit::TextEditState::default()
				.store(ctx, ui.make_persistent_id("message-edit"));
			ui.weak("Message deleted. The unchanged edit was closed.");
			ctx.request_repaint();
			return;
		}
		typing::show(ui, state, channel, std::time::Instant::now());
		let colors = crate::design::palette(ui);
		let editing_key = self
			.editing
			.as_ref()
			.filter(|(c, _, _)| *c == channel)
			.map(|(c, id, _)| (*c, *id));
		let editing_here = editing_key.is_some();
		if let Some((_, id)) = editing_key {
			if state.timeline.get(id).is_some() {
				self.edit_undo_cleared = false;
			} else if !self.edit_undo_cleared {
				let editor_id = ui.make_persistent_id("message-edit");
				if let Some(mut editor) = egui::text_edit::TextEditState::load(ctx, editor_id) {
					editor.clear_undoer();
					editor.store(ctx, editor_id);
				}
				self.edit_undo_cleared = true;
			}
		}
		if !editing_here && !state.can_compose(channel) {
			// Returning to an edit must restore its focus after visiting a read-only channel.
			self.composer_edit = None;
			self.mention_menu = mentions::Menu::default();
			self.emoji_picker = emoji_picker::Picker::default();
			self.ime_active = false;
			self.focus_switched_composer = false;
			egui::Frame::new()
				.fill(colors.raised)
				.corner_radius(8)
				.inner_margin(12)
				.show(ui, |ui| {
					let mut hint =
						"You don't have permission to send messages in this channel.".to_owned();
					ui.add_enabled(
						false,
						TextEdit::singleline(&mut hint)
							.desired_width(f32::INFINITY)
							.frame(egui::Frame::NONE),
					);
				});
			return;
		}
		let keyboard_enabled = !self.switcher_frame
			&& !self.switcher.is_open()
			&& ctx.memory(|memory| memory.top_modal_layer().is_none());
		let focus_edit =
			keyboard_enabled && editing_key.is_some() && self.composer_edit != editing_key;
		let focus_composer = keyboard_enabled
			&& (std::mem::take(&mut self.focus_switched_composer)
				|| (self.composer_edit != editing_key
					&& (editing_here || self.composer_edit.is_some_and(|(c, _)| c == channel))));
		if self.composer_edit != editing_key {
			self.composer_edit = editing_key;
			self.edit_sent = false;
			self.ime_active = false;
			self.mention_menu = mentions::Menu::default();
			self.emoji_picker = emoji_picker::Picker::default();
		}
		let mut cancel_edit = false;
		if ctx.input(|input| !input.raw.hovered_files.is_empty()) {
			ui.label(if self.upload_busy || self.attachment.is_some() {
				"Remove the current attachment or wait before dropping another file"
			} else if state.can_attach(channel) {
				"Drop one file up to 20 MB to attach it here; Send starts the upload"
			} else {
				"Attaching files is unavailable in this conversation"
			});
		}
		if editing_here {
			let unavailable = editing_key.is_some_and(|(_, id)| state.timeline.get(id).is_none());
			ui.horizontal(|ui| {
				ui.label(
					RichText::new(if unavailable {
						"Message unavailable · unsent edit"
					} else {
						"Editing message"
					})
					.small()
					.color(colors.accent),
				);
				cancel_edit = ui.small_button("Cancel edit").clicked();
				if unavailable
					&& ui.small_button("Copy edit text").clicked()
					&& let Some((_, _, text)) = &self.editing
				{
					ui.ctx().copy_text(text.clone());
				}
				if self.edit_sent {
					ui.small("Save requested · check connection status before retrying");
				}
			});
			ui.add_space(6.0);
		} else if let Some(reply) = state.reply {
			let author = state
				.timeline
				.get(reply)
				.map_or("an earlier message", |message| message.author.name.as_str());
			let label = format!("Replying to {author}");
			ui.horizontal(|ui| {
				ui.label(RichText::new(label).small().color(colors.accent));
				if ui
					.add_enabled(
						state.can_open_reply_target(reply),
						egui::Button::new("View original").small(),
					)
					.on_disabled_hover_text(if state.timeline.is_deleted(reply) {
						"The original message was deleted"
					} else {
						"Wait for readable, current message history"
					})
					.clicked()
				{
					self.timeline.reply_target = Some(reply);
				}
				if ui.small_button("Cancel reply").clicked() {
					state.reply = None;
				}
			});
			ui.add_space(6.0);
		}
		let upload_in_timeline = self.pending_upload.as_ref().is_some_and(|upload| {
			state.pending.iter().any(|p| {
				p.nonce == upload.nonce
					&& p.channel == channel
					&& p.delivery == model::Delivery::Sending
			})
		});
		if !editing_here
			&& !upload_in_timeline
			&& (self.upload_busy || self.upload_status.is_some())
		{
			ui.horizontal_wrapped(|ui| {
				ui.label(
					self.upload_status
						.as_deref()
						.unwrap_or("Preparing attachment…"),
				);
				if self.upload_busy && ui.button("Cancel upload").clicked() {
					self.cancel_upload_requested = true;
				}
			});
		}
		let full = state.draft_bytes() >= MAX_DRAFT_BYTES
			|| (!state.drafts.contains_key(&channel) && state.drafts.len() >= 64);
		if full && !editing_here {
			ui.label("Draft budget full. Clear an existing draft to continue.");
			if state.drafts.contains_key(&channel) && ui.button("Clear this draft").clicked() {
				self.clear_draft(state, channel);
			}
			return;
		}
		let ime_this_frame = keyboard_enabled
			&& ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(_))));
		if keyboard_enabled {
			ctx.input(|i| {
				for event in &i.events {
					if let egui::Event::Ime(event) = event {
						match event {
							egui::ImeEvent::Preedit { text, .. } => {
								self.ime_active = !text.is_empty()
							}
							egui::ImeEvent::Commit(_) => self.ime_active = false,
							_ => {}
						}
					}
				}
			});
		}
		let composer_id = ui.make_persistent_id(if editing_here {
			"message-edit"
		} else {
			"message-input"
		});
		if editing_here {
			self.edit_widget_id = Some(composer_id);
		}
		if focus_composer {
			ctx.memory_mut(|m| m.request_focus(composer_id));
		}
		if focus_edit {
			let mut edit_state = egui::text_edit::TextEditState::default();
			let count = self
				.editing
				.as_ref()
				.map_or(0, |(_, _, content)| content.chars().count());
			edit_state
				.cursor
				.set_char_range(Some(egui::text::CCursorRange::one(
					egui::text::CCursor::new(count),
				)));
			edit_state.store(ctx, composer_id);
		}
		let pasted_text = self.pasted_text.take();
		let paste_key_released = ctx.input(|input| {
			input.events.iter().any(|event| {
				matches!(
					event,
					egui::Event::Key {
						key: egui::Key::V,
						pressed: false,
						..
					}
				)
			})
		});
		let paste_enabled = keyboard_enabled
			&& !editing_here
			&& !self.ime_active
			&& !ime_this_frame
			&& ctx.memory(|m| m.has_focus(composer_id));
		if let Some((paste_channel, target, text)) = pasted_text {
			if paste_enabled && paste_channel == channel && target == composer_id {
				ctx.input_mut(|i| i.events.push(egui::Event::Paste(text)));
				self.pasted_text_frame = Some(ctx.cumulative_frame_nr());
			} else {
				state.status = "Paste cancelled; focus the message and paste again";
			}
		} else if paste_enabled && self.pasted_text_frame != Some(ctx.cumulative_frame_nr()) {
			let mut request = AttachmentPaste {
				target: composer_id,
				text: None,
				image: None,
			};
			let mut requested = false;
			ctx.input_mut(|input| {
				// eframe consumes native paste key-down. File-only clipboards can emit
				// no Paste event, so the matching key-up is also a paste trigger.
				let shortcut = input.events.iter().any(|event| {
					matches!(event,
					egui::Event::Key { key: egui::Key::V, pressed, repeat: false, modifiers, .. }
					if !modifiers.shift && (modifiers.ctrl || modifiers.command || modifiers.alt)
						&& (*pressed || !self.paste_key_handled))
				});
				let has_paste = input.events.iter().any(|event| {
					matches!(event, egui::Event::Paste(_) | egui::Event::PasteImage(_))
				});
				if has_paste || shortcut {
					requested = true;
					self.paste_key_handled = true;
					input.events.retain_mut(|event| match event {
						egui::Event::Paste(text) => {
							request.text = Some(std::mem::take(text));
							false
						}
						egui::Event::PasteImage(image) => {
							request.image = Some(image.clone());
							false
						}
						egui::Event::Key {
							key: egui::Key::V, ..
						} => false,
						// Option+V can also produce a platform text character.
						egui::Event::Text(_) if shortcut => false,
						_ => true,
					});
				}
			});
			if requested {
				if request
					.text
					.as_ref()
					.is_some_and(|text| text.len() > MAX_DRAFT_BYTES)
				{
					state.status = "Pasted text exceeds the draft limit";
				} else if request
					.image
					.as_ref()
					.is_some_and(|image| image.pixels.len() > 4 * 1024 * 1024)
				{
					state.status = "Paste an image with at most 4 million pixels";
				} else {
					self.attachment_paste_requested = Some(request);
					self.upload_busy = true;
				}
			}
		}
		if paste_key_released {
			self.paste_key_handled = false;
		}
		let composer_content = if editing_here {
			self.editing
				.as_ref()
				.map_or("", |(_, _, text)| text.as_str())
		} else {
			state.drafts.get(&channel).map_or("", String::as_str)
		};
		let count_before = composer_content.chars().count();
		let mention_enabled = keyboard_enabled
			&& !self.ime_active
			&& !ime_this_frame
			&& ctx.memory(|m| m.has_focus(composer_id));
		let mention_users = if mention_enabled || composer_content.contains("<@") {
			mentions::known_users(state, channel)
		} else {
			Vec::new()
		};
		let cursor = egui::text_edit::TextEditState::load(ctx, composer_id)
			.and_then(|s| s.cursor.char_range())
			.filter(|r| r.is_empty())
			.map(|r| r.primary.index.0);
		self.mention_menu.refresh(
			channel,
			composer_content,
			cursor.filter(|_| mention_enabled),
			&mention_users,
			&state.channels,
			&state.guilds,
		);
		let mention_pick = if mention_enabled {
			self.mention_menu.keys(ctx)
		} else {
			None
		};
		let mut editing = if editing_here {
			self.editing.take()
		} else {
			None
		};
		let demo = state.demo;
		let enter = keyboard_enabled
			&& !self.ime_active
			&& !ime_this_frame
			&& ctx.memory(|m| m.has_focus(composer_id))
			&& ctx.input_mut(|i| {
				// consume_key matches Shift/Alt too; only a deliberate plain Enter sends.
				let send = i.events.iter().any(|event| {
					matches!(event, egui::Event::Key {
						key: egui::Key::Enter, pressed: true, repeat: false, modifiers, ..
					} if *modifiers == egui::Modifiers::NONE)
				});
				send && i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
			});
		let placeholder = state.channels.iter().find(|c| c.id == channel).map_or_else(
			|| "Message".to_owned(),
			|c| {
				if c.guild.is_some() {
					format!("Message #{}", c.name)
				} else {
					format!("Message @{}", c.name)
				}
			},
		);
		let can_attach = !editing_here
			&& state.can_attach(channel)
			&& !self.upload_busy
			&& self.attachment.is_none();
		let can_send = if let Some((edit_channel, message)) = editing_key {
			state.freshness == Freshness::Fresh
				&& state.can_edit(edit_channel, message)
				&& count_before > 0
		} else {
			state.can_send(channel)
				&& (self.attachment.is_none() || state.can_attach(channel))
				&& !self.upload_busy
				&& !(state.demo && self.attachment.is_some())
				&& (count_before > 0 || self.attachment.is_some())
		};
		egui::Frame::new()
            .fill(colors.raised)
            .corner_radius(8)
            .inner_margin(egui::Margin::symmetric(10, 6))
            .show(ui, |ui| {
                // Outer frame bounds for the autocomplete popout: undo the inner margin.
                let composer_anchor = egui::Rect::from_min_max(
                    egui::pos2(ui.max_rect().left() - 10.0, ui.max_rect().top() - 6.0),
                    egui::pos2(ui.max_rect().right() + 10.0, ui.max_rect().top()),
                );
                if !editing_here && let Some((filename, bytes)) = self.attachment.clone() {
                    self.attachment_tray(ui, state, &filename, bytes);
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    let attach = ui
                        .add_enabled_ui(can_attach, |ui| {
                            icons::button(ui, icons::Icon::Attach, 28.0, "Attach a file")
                        })
                        .inner
                        .on_hover_text("Choose, drop, or paste one file (Ctrl/Cmd/Option+V) up to 20 MB. Send starts the upload.");
                    if attach.clicked() {
                        self.attach_requested = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let send_button = ui
                            .add_enabled_ui(can_send, |ui| {
                                icons::button(
                                    ui,
                                    icons::Icon::Send,
                                    28.0,
                                    if editing_here { "Save edit" } else { "Send message" },
                                )
                            })
                            .inner;
                        let send = enter || send_button.clicked();
                        if count_before + 200 >= MAX_CONTENT {
                            ui.label(
                                RichText::new(format!("{}", MAX_CONTENT.saturating_sub(count_before)))
                                    .size(11.0)
                                    .color(if count_before >= MAX_CONTENT { colors.danger } else { colors.muted }),
                            );
                        }
                        let pick = ui
                            .add_enabled_ui(!self.ime_active && !ime_this_frame, |ui| {
                                self.emoji_picker
                                    .show(ui, state, channel, &mut self.avatars, commands)
                            })
                            .inner;
                        // A chosen GIF is its own message; the typed draft stays untouched.
                        let pick = match pick {
                            Some(emoji_picker::Pick::Insert(text)) => Some(text),
                            Some(emoji_picker::Pick::Send(url)) => {
                                if editing_here {
                                    state.status = "Finish or cancel the edit before sending a GIF.";
                                } else if self.upload_busy || !state.can_send(channel) {
                                    state.status = "Sending is unavailable with the current connection or permissions";
                                } else {
                                    let saved = state.drafts.insert(channel, url);
                                    let command = state.prepare_send();
                                    match saved {
                                        Some(saved) => {
                                            state.drafts.insert(channel, saved);
                                        }
                                        None => {
                                            state.drafts.remove(&channel);
                                        }
                                    }
                                    if let Some(command) = command {
                                        self.timeline.follow_latest();
                                        commands.push(command);
                                    }
                                }
                                None
                            }
                            None => None,
                        };
                        let edit = ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width());
                        cancel_edit |= keyboard_enabled && editing_here && !self.ime_active && !ime_this_frame
                            && ctx.memory(|m| m.has_focus(composer_id) || m.had_focus_last_frame(composer_id))
                            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
                        let remaining = if editing_here { MAX_CONTENT * 4 } else { MAX_DRAFT_BYTES.saturating_sub(state.draft_bytes()) };
                        let mut new_draft = String::new();
                        let draft = if let Some((_, _, content)) = &mut editing {
                            content
                        } else {
                            state.drafts.get_mut(&channel).unwrap_or(&mut new_draft)
                        };
                        let mut mention_changed = false;
                        if let Some(pick) = pick {
                            let mut edit_state =
                                egui::text_edit::TextEditState::load(ctx, composer_id).unwrap_or_default();
                            let range = edit_state.cursor.char_range();
                            if let Some(cursor) = emoji_picker::insert(draft, &pick, range, remaining) {
                                edit_state
                                    .cursor
                                    .set_char_range(Some(egui::text::CCursorRange::one(
                                        egui::text::CCursor::new(cursor),
                                    )));
                                edit_state.store(ctx, composer_id);
                                mention_changed = true;
                            } else {
                                state.status =
                                    "Emoji will not fit. Shorten this message or free draft space.";
                            }
                        }
                        if let Some(pick) = mention_pick
                            && let Some(cursor) = mentions::insert(draft, pick)
                        {
                            let mut edit_state =
                                egui::text_edit::TextEditState::load(ctx, composer_id).unwrap_or_default();
                            edit_state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(
                                    egui::text::CCursor::new(cursor),
                                )));
                            edit_state.store(ctx, composer_id);
                            mention_changed = true;
                        }
                        let rich_layout = &mut self.composer_layout;
                        if !self.ime_active
                            && !ime_this_frame
                            && ctx.input(|i| {
                                i.key_pressed(egui::Key::Backspace) || i.key_pressed(egui::Key::Delete)
                            })
                        {
                            rich_layout.galley(
                                ui,
                                draft,
                                ui.available_width(),
                                &mention_users,
                                &mut self.avatars,
                                demo,
                            );
                            rich_layout.select_deleted_inline(ctx, composer_id);
                        }
                        let mut layouter = |ui: &egui::Ui, buffer: &dyn egui::TextBuffer, width: f32| {
                            rich_layout.galley(
                                ui,
                                buffer.as_str(),
                                width,
                                &mention_users,
                                &mut self.avatars,
                                demo,
                            )
                        };
                        let mut output = TextEdit::multiline(draft)
                            .interactive(keyboard_enabled)
                            .layouter(&mut layouter)
                            .id(composer_id)
                            .event_filter(egui::EventFilter {
                                horizontal_arrows: true, vertical_arrows: true, escape: editing_here,
                                ..Default::default()
                            })
                            .char_limit(MAX_CONTENT)
                            .desired_rows(1)
                            .desired_width(f32::INFINITY)
                            // Match the 28px icon row so the hint sits on the same centre line.
                            .min_size(egui::vec2(0.0, 28.0))
                            .align(egui::Align2::LEFT_CENTER)
                            .frame(egui::Frame::NONE)
                            .hint_text(placeholder.as_str())
                            .show(ui);
                        rich_layout.paint(ui, &output);
                        if !self.ime_active && !ime_this_frame {
                            rich_layout.snap_cursor(&mut output, ctx);
                        }
                        if mention_changed {
                            output.response.request_focus();
                        }
                        let mention_cursor = output
                            .cursor_range
                            .filter(|r| r.is_empty())
                            .map(|r| r.primary.index.0)
                            .filter(|_| mention_enabled);
                        self.mention_menu
                            .refresh(channel, draft, mention_cursor, &mention_users, &state.channels, &state.guilds);
                        if let Some(pick) = self.mention_menu.show(ui, composer_anchor, &mut self.avatars, demo)
                            && let Some(cursor) = mentions::insert(draft, pick)
                        {
                            output
                                .state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(
                                    egui::text::CCursor::new(cursor),
                                )));
                            output.state.store(ctx, composer_id);
                            output.response.request_focus();
                            mention_changed = true;
                        }
                        let edit = output.response;
                        let cleared = draft.is_empty();
                        if edit.changed() || mention_changed {
                            if editing_here && let Some((_, _, modified)) = &mut self.edit_modified {
                                *modified = true;
                            }
                            if editing_here {
                                self.edit_sent = false;
                            } else if cleared {
                                self.clear_draft(state, channel);
                            } else {
                                if !new_draft.is_empty() {
                                    state.drafts.insert(channel, new_draft);
                                }
                                self.draft_changes.push(channel);
                            }
                        }
                        edit
                            }).inner
                        }).inner;
                        if send && !cancel_edit {
                            if let Some((edit_channel, message, content)) = &editing {
                                if state.freshness == Freshness::Fresh
                                    && let Some(command) = state.prepare_edit(*edit_channel, *message, content.clone())
                                {
                                    commands.push(command);
                                    self.edit_sent = true;
                                } else {
                                    state.status = "Edit kept. Wait for your current message and connection, and enter nonempty text.";
                                }
                            } else if !self.upload_busy && !(state.demo && self.attachment.is_some())
                                && let Some(command) = state.prepare_send_with_attachment(self.attachment.as_ref().map(|(name, _)| name.as_str())) {
                                // Consume the selection in this UI pass, before desktop dispatch.
                                // A second render or Send gesture must not enqueue it again.
                                self.stage_pending_upload(&command);
                                self.timeline.follow_latest();
                                commands.push(command);
                            }
                            edit.request_focus();
                        }
                    });
                });
                if let Some((edit_channel, message)) = editing_key {
                    if state.freshness != Freshness::Fresh || !state.can_edit(edit_channel, message) { ui.weak("Editing this message is unavailable. Your text is kept until you cancel."); }
                } else if !state.can_send(channel) {
                    ui.weak("Sending messages is unavailable in this conversation. Your draft is kept.");
                } else if self.attachment.is_some() && !state.can_attach(channel) {
                    ui.weak("Attaching files is unavailable here. Remove the attachment to send only text.");
                }
            });
		if editing_here {
			self.editing = if cancel_edit { None } else { editing };
			if cancel_edit {
				self.edit_sent = false;
			}
		}
		if self.edit_sent
			&& self.editing.as_ref().is_some_and(|(channel, id, content)| {
				state.timeline.get(*id).is_some_and(|message| {
					message.channel == *channel && message.content == *content
				})
			}) {
			self.editing = None;
			self.edit_sent = false;
		}
	}
	/// Selected-file card above the composer input, in the style of Discord's upload tray.
	fn attachment_tray(&mut self, ui: &mut egui::Ui, state: &State, filename: &str, bytes: u64) {
		let colors = design::palette(ui);
		let texture = match &self.attachment_preview {
			Some(image) => {
				let key = std::sync::Arc::as_ptr(image) as usize;
				if self
					.attachment_texture
					.as_ref()
					.is_none_or(|(k, _)| *k != key)
				{
					let handle = ui.ctx().load_texture(
						"attachment-preview",
						egui::ImageData::Color(image.clone()),
						egui::TextureOptions::LINEAR,
					);
					self.attachment_texture = Some((key, handle));
				}
				self.attachment_texture.as_ref().map(|(_, handle)| handle)
			}
			None => {
				self.attachment_texture = None;
				None
			}
		};
		ui.add_space(4.0);
		let remove = ui
			.horizontal(|ui| {
				ui.spacing_mut().item_spacing.x = 12.0;
				ui.add_space(4.0);
				attachments::pending_card(ui, filename, bytes, texture, !self.upload_busy)
			})
			.inner;
		if remove {
			self.remove_attachment_requested = true;
		}
		ui.add_space(2.0);
		ui.horizontal(|ui| {
			ui.add_space(4.0);
			ui.label(
				RichText::new(if state.demo {
					"Uploads are disabled in offline preview"
				} else if !state.can_attach(state.selected.unwrap_or(Id(0))) {
					"Attaching files is unavailable here"
				} else {
					"Not uploaded yet · Send uploads this file with your message"
				})
				.size(12.0)
				.color(colors.muted),
			);
		});
		ui.add_space(8.0);
		let line = ui.max_rect().x_range();
		let y = ui.cursor().top();
		ui.painter()
			.hline(line, y, egui::Stroke::new(1.0, colors.border));
		ui.add_space(6.0);
	}
	pub fn show(&mut self, ui: &mut egui::Ui, state: &mut State) -> Vec<Command> {
		if self
			.pending_upload
			.as_ref()
			.is_some_and(|upload| !state.pending.iter().any(|p| p.nonce == upload.nonce))
		{
			self.pending_upload = None;
		}
		self.timeline.audio.seen = false;
		let mut commands = Vec::new();
		let ctx = ui.ctx().clone();
		let settings_open = self.settings.open;
		if settings_open {
			self.show_settings(&ctx, state);
			ui.disable();
		}
		// Foreground confirmation handles Escape before background search/archive shortcuts.
		markdown::confirm_external_link(&ctx, &mut self.timeline.opening);
		let colors = crate::design::palette(ui);
		if !settings_open
			&& !self.switcher.is_open()
			&& !self.ime_active
			&& ctx.input(|input| {
				input.focused
					&& !input
						.events
						.iter()
						.any(|event| matches!(event, egui::Event::Ime(_)))
			}) && ctx.input_mut(|input| input.consume_key(egui::Modifiers::COMMAND, egui::Key::K))
		{
			self.switcher.open(&ctx);
		}
		self.switcher_frame = self.switcher.is_open();
		if let Some(channel) = self.switcher.show(&ctx, state) {
			if state.selected != Some(channel)
				&& let Some(command) = state.select(channel)
			{
				commands.push(command);
			}
			self.focus_switched_composer = state.selected == Some(channel)
				&& state
					.channels
					.iter()
					.any(|known| known.id == channel && known.supports_text());
		}
		if self.navigation_channel != state.selected {
			self.navigation_channel = state.selected;
			self.guild = state.selected.and_then(|id| {
				state
					.channels
					.iter()
					.find(|channel| channel.id == id)
					.and_then(|channel| channel.guild)
			});
		}
		let title = self
			.guild
			.and_then(|id| state.guilds.iter().find(|g| g.id == id))
			.map_or_else(|| "Direct Messages".to_owned(), |g| g.name.clone());
		self.title_bar(ui, state, &title);
		// Server rail and channel list share one resizable column so the account card can
		// span both, like Discord's bottom-left user pill.
		let rail = notifications::RAIL_WIDTH;
		let sidebar_max = self.prepare_reading_sidebar(ui, "navigation", rail);
		let navigation = egui::Panel::left("navigation")
			.resizable(true)
			.default_size(rail + f32::from(self.reading_preferences.sidebar_width).min(sidebar_max))
			.size_range(rail + 190.0..=rail + sidebar_max)
			.frame(egui::Frame::new().fill(colors.base).inner_margin(0))
			.show(ui, |ui| {
				egui::Panel::bottom("account-footer")
					.show_separator_line(false)
					.frame(egui::Frame::new().inner_margin(egui::Margin {
						left: 8,
						right: 8,
						top: 8,
						bottom: 8,
					}))
					.show(ui, |ui| self.account_card(ui, state, &mut commands));
				self.notification_rail(ui, state, &mut commands);
				// The lists sit on their own rounded surface beside the rail, above the card.
				ui.painter().rect_filled(
					ui.available_rect_before_wrap(),
					egui::CornerRadius {
						nw: 8,
						sw: 8,
						..Default::default()
					},
					colors.sidebar,
				);
				self.sidebar(ui, state, &title, &mut commands);
			});
		self.record_reading_sidebar(navigation.response.rect.width() - rail);
		let selected_voice = state
			.channels
			.iter()
			.any(|c| Some(c.id) == state.selected && c.kind == 2);
		let selected_forum = state.selected.is_some_and(|id| state.is_forum(id));
		if let Some(id) = state.posting.created.take()
			&& let Some(command) = state.select(id)
		{
			commands.push(command);
		}
		let wide_members = ui.available_width() >= 720.0;
		self.search.sync(&ctx, state, &mut commands);
		let search_open =
			self.search.open && !self.search.pins() && state.selected.is_some() && !selected_voice;
		let show_members = !selected_voice
			&& !search_open
			&& state.selected.is_some()
			&& if wide_members {
				self.reading_preferences.show_members
			} else {
				self.members_narrow_open
			};
		if search_open {
			let width = if wide_members {
				search::PANE_WIDTH.min(ui.available_width() * 0.45)
			} else {
				(ui.available_width() * 0.6).max(240.0)
			};
			egui::Panel::right("search-pane")
				.resizable(false)
				.exact_size(width)
				.frame(
					egui::Frame::new()
						.fill(colors.sidebar)
						.inner_margin(egui::Margin::same(12)),
				)
				.show(ui, |ui| {
					self.search.pane(ui, state, &mut commands);
				});
		}
		if show_members {
			if state
				.members
				.as_ref()
				.is_none_or(|list| Some(list.channel) != state.selected)
				&& let Some(command) = state.request_members()
			{
				commands.push(command);
			}
			if wide_members {
				egui::Panel::right("people-pane")
					.resizable(false)
					.exact_size(240.0)
					.frame(egui::Frame::new().inner_margin(egui::Margin {
						left: 8,
						right: 8,
						top: 8,
						bottom: 8,
					}))
					.show(ui, |ui| {
						self.member_rows(ui, state);
					});
			} else {
				let mut open = self.members_narrow_open;
				egui::Window::new("Members")
					.open(&mut open)
					.collapsible(false)
					.resizable(false)
					.default_width(248.0)
					.default_height(350.0)
					.show(&ctx, |ui| {
						self.member_rows(ui, state);
					});
				self.members_narrow_open = open;
			}
		} else if state.members.is_some() {
			commands.push(state.close_members());
		}
		if std::mem::take(&mut self.member_reload_requested)
			&& let Some(command) = state.request_members()
		{
			commands.push(command);
		}
		egui::CentralPanel::default()
			.frame(egui::Frame::new().fill(colors.chat).inner_margin(0))
			.show(ui, |ui| {
				self.channel_header(
					ui,
					state,
					selected_voice,
					show_members,
					wide_members,
					&mut commands,
				);
				self.call_bar(ui, state, &mut commands);
				self.timeline.download.show_status(ui);
				let Some(channel) = state.selected else {
					ui.add_space((ui.available_height() * 0.32).max(24.0));
					ui.vertical_centered(|ui| {
						ui.label(
							design::semibold(ui, "No conversation selected", 20.0)
								.color(colors.text_strong),
						);
						ui.add_space(8.0);
						ui.label(
							RichText::new("Pick a channel or direct message from the list.")
								.color(colors.muted),
						);
					});
					return;
				};
				if selected_voice {
					self.voice_channel(ui, state, channel, &mut commands);
					return;
				}
				if selected_forum {
					self.forum.show(ui, state, channel, &mut commands);
					return;
				}
				egui::Panel::bottom("composer")
					.show_separator_line(false)
					.frame(
						egui::Frame::new()
							.fill(colors.chat)
							.inner_margin(egui::Margin {
								left: 16,
								right: 16,
								top: 4,
								bottom: 20,
							}),
					)
					.show(ui, |ui| {
						self.composer(ui, state, channel, &ctx, &mut commands);
					});
				let notices: Vec<String> = [
					match state.freshness {
						Freshness::Fresh => None,
						Freshness::Loading => Some("Loading history…"),
						Freshness::Stale => Some("Cached history · awaiting sync"),
						Freshness::Unavailable => Some("Conversation unavailable"),
					}
					.map(str::to_owned),
					state.read_state.status.map(str::to_owned),
					(state.archived_thread.is_some() && state.archived_thread == state.selected)
						.then(|| "Opened from archive".to_owned()),
					(state.history_targeted
						|| state.history_before.is_some()
						|| state.history_after.is_some())
					.then(|| {
						"Browsing message history \u{b7} Jump to present to return".to_owned()
					}),
				]
				.into_iter()
				.flatten()
				.collect();
				if !notices.is_empty() || state.can_load_older() {
					egui::Frame::new()
						.inner_margin(egui::Margin::symmetric(16, 4))
						.show(ui, |ui| {
							ui.horizontal_wrapped(|ui| {
								ui.label(
									RichText::new(notices.join(" · "))
										.size(11.0)
										.color(colors.muted),
								);
								ui.with_layout(
									egui::Layout::right_to_left(egui::Align::Center),
									|ui| {
										if state.can_load_older()
											&& ui
												.add(
													egui::Button::new(
														RichText::new("Load earlier messages")
															.size(11.0)
															.color(colors.link),
													)
													.small()
													.frame(false),
												)
												.clicked() && let Some(command) = state.older_history()
										{
											commands.push(command);
										}
									},
								);
							});
						});
				}
				egui::Frame::new()
					.inner_margin(egui::Margin {
						left: 0,
						right: 0,
						top: 4,
						bottom: 0,
					})
					.show(ui, |ui| {
						self.timeline.hide_media_links = self.reading_preferences.hide_media_links;
						self.timeline.show(
							ui,
							state,
							&mut self.editing,
							&mut self.deleting,
							&mut self.avatars,
							&mut self.profile,
							self.pending_upload.as_ref(),
						);
						if let Some(nonce) = self.timeline.restore_pending.take() {
							self.restore_pending(state, channel, &nonce);
						}
						self.cancel_upload_requested |=
							std::mem::take(&mut self.timeline.cancel_upload);
						if let Some(gif) = self.timeline.gif_favorite.take() {
							state.toggle_gif_favorite(&gif);
						}
						for code in std::mem::take(&mut self.timeline.invite_requests) {
							if let Some(command) = state.request_invite(code) {
								commands.push(command);
							}
						}
						if std::mem::take(&mut self.timeline.edit_started) {
							self.edit_modified = None;
							self.edit_undo_cleared = false;
							self.composer_edit = None;
							self.edit_sent = false;
						}
						if let Some(target) = self.timeline.reply_target.take() {
							if let Some(command) = state.open_reply_target(target) {
								commands.push(command);
							}
							ctx.request_repaint();
						} else if std::mem::take(&mut self.timeline.unread_jump) {
							if let Some(command) = state.open_unread() {
								commands.push(command);
							}
							ctx.request_repaint();
						} else if std::mem::take(&mut self.timeline.load_newer) {
							if let Some(command) = state.newer_history() {
								commands.push(command);
							}
							ctx.request_repaint();
						} else if std::mem::take(&mut self.timeline.latest) {
							commands.push(state.history(None));
						} else if std::mem::take(&mut self.timeline.load_older)
							&& let Some(command) = state.older_history()
						{
							commands.push(command);
						}
					});
			});
		if state
			.archives
			.as_ref()
			.is_some_and(|view| self.guild != Some(view.guild))
		{
			commands.push(state.clear_archives());
		}
		if let Some((channel, message, pinned)) = self.timeline.pin_request.take()
			&& let Some(command) = state.prepare_pin(channel, message, pinned)
		{
			commands.push(command);
		}
		if let Some(channel) = state.pins_changed.take()
			&& self.search.open
			&& self.search.pins()
			&& state.selected == Some(channel)
			&& let Some(command) = state.request_pins()
		{
			commands.push(command);
		}
		if let Some(anchor) = self.pins_anchor {
			let dm = state
				.channels
				.iter()
				.any(|c| Some(c.id) == state.selected && c.guild.is_none());
			self.search
				.pins_popout(&ctx, state, anchor, dm, &mut commands);
			if !(self.search.open && self.search.pins()) {
				self.pins_anchor = None;
			}
		}
		// A forum pane lists its own archived posts inline instead of the floating window.
		if !(selected_forum
			&& state
				.archives
				.as_ref()
				.is_some_and(|view| Some(view.parent) == state.selected))
		{
			self.archives.show(&ctx, state, &mut commands);
		}
		if let Some(id) = self.timeline.channel_reference.take()
			&& let Some(target) = state
				.channels
				.iter()
				.find(|c| c.id == id && c.guild.is_some() && c.supports_text())
		{
			self.guild = target.guild;
			if let Some(command) = state.select(id) {
				commands.push(command);
			}
		}
		if let Some(message) = self.timeline.mark_read.take()
			&& !settings_open
			&& state.search_target.is_none()
			&& !state.history_targeted
			&& let Some(command) = state.prepare_mark_read(message)
		{
			commands.push(command);
		}
		if let Some((message, emoji)) = self.timeline.reaction.take() {
			if let Some(emoji) = emoji {
				if let Some(command) = state.prepare_reaction(message, emoji) {
					commands.push(command);
				}
			} else {
				state.refresh_reactions(message);
			}
		}
		if let Some(action) = self.user_action.take().or(self.timeline.user_action.take())
			&& let Some(command) = user_menu::prepare(action, state)
		{
			commands.push(command);
		}
		if let Some(user) = &self.profile {
			let profile_guild = state
				.channels
				.iter()
				.find(|channel| Some(channel.id) == state.selected)
				.and_then(|channel| channel.guild);
			if state
				.profile
				.as_ref()
				.is_none_or(|p| p.user != user.id || p.guild != profile_guild)
			{
				self.profile_link = None;
				if state.demo {
					state.profile = Some(client_core::profile::ProfileView {
						user: user.id,
						guild: profile_guild,
						request: 0,
						loading: false,
						error: None,
						data: Some(profiles::synthetic(user, profile_guild)),
					});
				} else if let Some(command) = state.request_profile(user.id, profile_guild) {
					commands.push(command);
				}
			}
			let anchor = match self.profile_anchor {
				Some((id, pos)) if id == user.id => pos,
				_ => {
					let pos = ctx
						.input(|i| i.pointer.interact_pos().or(i.pointer.latest_pos()))
						.unwrap_or_else(|| ctx.content_rect().center());
					self.profile_anchor = Some((user.id, pos));
					pos
				}
			};
			match profiles::show(
				ui,
				user,
				state.profile.as_ref(),
				state,
				&mut self.avatars,
				&mut self.profile_link,
				anchor,
			) {
				Some(profiles::Action::Profile(user)) => {
					self.profile = Some(user);
					self.profile_link = None;
					commands.push(state.clear_profile());
				}
				Some(profiles::Action::Close) => {
					self.profile = None;
					self.profile_link = None;
					self.profile_anchor = None;
					commands.push(state.clear_profile());
				}
				Some(profiles::Action::Retry) => {
					if let Some(command) = state.request_profile(user.id, profile_guild) {
						commands.push(command);
					}
				}
				Some(profiles::Action::Message(channel)) => {
					self.profile = None;
					self.profile_link = None;
					self.profile_anchor = None;
					commands.push(state.clear_profile());
					if let Some(command) = state.select(channel) {
						commands.push(command);
					}
				}
				None => {}
			}
		} else {
			self.profile_anchor = None;
		}

		self.reconcile_edit(state);
		if self.deleting.is_some_and(|(channel, message)| {
			state.selected != Some(channel) || state.timeline.get(message).is_none()
		}) {
			self.deleting = None;
		}
		if let Some((channel, message)) = self.deleting {
			egui::Window::new("Delete message from Discord?")
				.collapsible(false)
				.show(&ctx, |ui| {
					ui.label(
						state
							.channels
							.iter()
							.find(|c| c.id == channel)
							.map_or("Original conversation unavailable", |c| c.name.as_str()),
					);
					ui.label("This removes the selected message from the conversation.");
					let allowed = state.can_delete(channel, message);
					if !allowed {
						ui.weak("Deleting this message is unavailable.");
					}
					if ui
						.add_enabled(allowed, egui::Button::new("Delete message"))
						.clicked() && let Some(command) = state.prepare_delete(channel, message)
					{
						commands.push(command);
						self.deleting = None;
					}
					if ui.button("Keep message").clicked() {
						self.deleting = None;
					}
				});
		}
		self.voice_ptt_active = self.voice_push_to_talk
			&& state.voice.active.is_some()
			&& !state.demo
			&& !ctx.egui_wants_keyboard_input()
			&& ctx.input(|input| input.focused && input.key_down(egui::Key::V));
		commands
	}
}

#[cfg(test)]
mod composer_tests {
	use super::*;

	#[test]
	fn download_cancel_remains_visible_without_a_text_composer() {
		for selection in [None, Some(Id(10))] {
			let mut state = edit_state();
			state.demo = true;
			state.selected = selection;
			state.channels[0].kind = 2;
			let mut messaging = MessagingUi::default();
			messaging.downloads().active = true;
			messaging.downloads().status = "Saving synthetic attachment".into();
			let ctx = egui::Context::default();
			let output = ctx.run_ui(egui::RawInput::default(), |ui| {
				messaging.show(ui, &mut state);
			});
			assert!(output.shapes.iter().any(|shape| {
				matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Cancel download")
			}));
			output.drop_without_applying_deltas();
		}
	}

	fn edit_state() -> State {
		let user = model::User {
			id: Id(1),
			name: "Alex".into(),
			avatar: None,
			discriminator: 0,
		};
		let mut state = State {
			selected: Some(Id(10)),
			channels: vec![model::Channel {
				id: Id(10),
				guild: None,
				parent_id: None,
				kind: 1,
				name: "Synthetic edit conversation".into(),
				position: 0,
				recipients: vec![],
				last_message: None,
				member_list_id: None,
				message_count: None,
			}],
			user: Some(user.clone()),
			demo: true,
			freshness: Freshness::Fresh,
			auth: client_core::auth::AuthState::Authenticated,
			gateway_connected: true,
			..Default::default()
		};
		state
			.timeline
			.insert(
				model::Message {
					id: Id(20),
					channel: Id(10),
					author: user,
					content: "Original".into(),
					edited: false,
					edited_at: None,
					revision: 0,
					nonce: None,
					reply_to: None,
					kind: 0,
					reply_deleted: false,
					unsupported: false,
					extra_content: Default::default(),
					embeds: vec![],
					attachments: vec![],
					mention_roles: vec![],
					mention_everyone: false,
					suppress_notifications: false,
					mentions: vec![],
					reactions: None,
					embeds_suppressed: false,
				},
				false,
				false,
			)
			.unwrap();
		state.drafts.insert(Id(10), "Unsent draft 👋".into());
		state
	}

	fn edit_frame(
		ctx: &egui::Context,
		view: &mut MessagingUi,
		state: &mut State,
		events: Vec<egui::Event>,
	) -> Vec<Command> {
		let mut commands = vec![];
		let output = ctx.run_ui(
			egui::RawInput {
				events,
				..Default::default()
			},
			|ui| {
				view.composer(ui, state, state.selected.unwrap(), ctx, &mut commands);
			},
		);
		output.drop_without_applying_deltas();
		// This helper simulates key taps; raw-input tests explicitly model held keys.
		ctx.input_mut(|i| i.keys_down.clear());
		commands
	}

	fn edit_key(key: egui::Key) -> egui::Event {
		egui::Event::Key {
			key,
			physical_key: None,
			pressed: true,
			repeat: false,
			modifiers: egui::Modifiers::NONE,
		}
	}

	#[test]
	fn link_confirmation_escape_preserves_background_search_and_works_without_selection() {
		let ctx = egui::Context::default();
		let mut state = edit_state();
		let mut view = MessagingUi::default();
		let frame = |view: &mut MessagingUi, state: &mut State, events| {
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(1000.0, 700.0),
					)),
					events,
					..Default::default()
				},
				|ui| {
					view.show(ui, state);
				},
			);
			let emitted = output.platform_output.commands.len();
			output.drop_without_applying_deltas();
			assert_eq!(
				emitted, 0,
				"Showing or canceling a link never opens a browser"
			);
		};
		frame(&mut view, &mut state, vec![]);
		view.search.open = true;
		view.timeline.opening = Some("https://discord.com/channels/@me/10/20".into());
		for _ in 0..3 {
			frame(&mut view, &mut state, vec![]);
		}
		frame(&mut view, &mut state, vec![edit_key(egui::Key::Escape)]);
		assert!(view.timeline.opening.is_none());
		assert!(
			view.search.open,
			"Escape belongs to the foreground confirmation"
		);
		state.selected = None;
		view.timeline.opening = Some("https://discord.com/channels/@me/10".into());
		for _ in 0..3 {
			frame(&mut view, &mut state, vec![]);
		}
		assert!(view.timeline.opening.is_some());
		frame(&mut view, &mut state, vec![edit_key(egui::Key::Escape)]);
		assert!(view.timeline.opening.is_none());
	}

	#[test]
	fn conversation_shortcut_preserves_edits_and_drafts_and_never_sends() {
		fn frame(
			ctx: &egui::Context,
			view: &mut MessagingUi,
			state: &mut State,
			events: Vec<egui::Event>,
		) -> Vec<Command> {
			let mut commands = Vec::new();
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(1200.0, 800.0),
					)),
					focused: true,
					events,
					..Default::default()
				},
				|ui| commands.extend(view.show(ui, state)),
			);
			output.drop_without_applying_deltas();
			assert!(!commands.iter().any(|command| matches!(
				command,
				Command::Send { .. } | Command::Edit { .. } | Command::Voice(_)
			)));
			commands
		}
		let shortcut = || egui::Event::Key {
			key: egui::Key::K,
			physical_key: None,
			pressed: true,
			repeat: false,
			modifiers: egui::Modifiers::COMMAND,
		};
		let ctx = egui::Context::default();
		let mut state = edit_state();
		let mut other = state.channels[0].clone();
		other.id = Id(11);
		other.name = "Other conversation".into();
		state.channels.push(other);
		state.drafts.insert(Id(11), "Another unsent draft".into());
		let drafts = state.drafts.clone();
		let mut view = MessagingUi {
			editing: Some((Id(10), Id(20), "Keep this unfinished edit".into())),
			..Default::default()
		};
		frame(&ctx, &mut view, &mut state, vec![]);
		view.ime_active = true;
		frame(&ctx, &mut view, &mut state, vec![shortcut()]);
		assert!(
			!view.switcher.is_open(),
			"Do not interrupt an active composition"
		);
		view.ime_active = false;
		frame(&ctx, &mut view, &mut state, vec![shortcut()]);
		assert!(view.switcher.is_open());
		frame(&ctx, &mut view, &mut state, vec![]);
		frame(
			&ctx,
			&mut view,
			&mut state,
			vec![edit_key(egui::Key::Escape)],
		);
		assert!(!view.switcher.is_open());
		assert_eq!(state.selected, Some(Id(10)));
		assert_eq!(
			view.editing.as_ref().unwrap().2,
			"Keep this unfinished edit"
		);
		frame(&ctx, &mut view, &mut state, vec![shortcut()]);
		frame(&ctx, &mut view, &mut state, vec![]);
		frame(
			&ctx,
			&mut view,
			&mut state,
			vec![edit_key(egui::Key::ArrowDown)],
		);
		let commands = frame(
			&ctx,
			&mut view,
			&mut state,
			vec![edit_key(egui::Key::Enter)],
		);
		assert_eq!(state.selected, Some(Id(11)));
		assert!(commands.iter().any(|command| matches!(
			command,
			Command::History {
				channel: Id(11),
				..
			}
		)));
		assert!(!view.switcher.is_open());
		assert_eq!(state.drafts, drafts);
		assert_eq!(
			view.editing.as_ref().unwrap().2,
			"Keep this unfinished edit"
		);
		for _ in 0..3 {
			frame(&ctx, &mut view, &mut state, vec![]);
		}
		assert!(!view.focus_switched_composer);
		frame(
			&ctx,
			&mut view,
			&mut state,
			vec![egui::Event::Text(" typed".into())],
		);
		assert_eq!(state.drafts[&Id(11)], "Another unsent draft typed");
		assert_eq!(state.drafts[&Id(10)], drafts[&Id(10)]);
		assert_eq!(
			view.editing.as_ref().unwrap().2,
			"Keep this unfinished edit"
		);
	}

	#[test]
	fn switcher_pointer_close_during_ime_cannot_commit_into_the_composer() {
		let ctx = egui::Context::default();
		let mut state = edit_state();
		let drafts = state.drafts.clone();
		let mut view = MessagingUi {
			editing: Some((Id(10), Id(20), "Keep this edit".into())),
			..Default::default()
		};
		let frame = |view: &mut MessagingUi, state: &mut State, events| {
			let mut commands = Vec::new();
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(1200.0, 800.0),
					)),
					focused: true,
					events,
					..Default::default()
				},
				|ui| commands.extend(view.show(ui, state)),
			);
			assert!(
				!commands
					.iter()
					.any(|command| matches!(command, Command::Send { .. } | Command::Edit { .. }))
			);
			output
		};
		frame(&mut view, &mut state, vec![]).drop_without_applying_deltas();
		view.switcher.open(&ctx);
		frame(&mut view, &mut state, vec![]).drop_without_applying_deltas();
		let output = frame(&mut view, &mut state, vec![]);
		let close = output
			.shapes
			.iter()
			.find_map(|shape| match &shape.shape {
				egui::Shape::Text(text) if text.galley.job.text == "Close" => {
					Some(text.pos + text.galley.size() / 2.0)
				}
				_ => None,
			})
			.expect("Rendered picker Close control");
		output.drop_without_applying_deltas();
		frame(
			&mut view,
			&mut state,
			vec![egui::Event::Ime(egui::ImeEvent::Preedit {
				text: "Query composition".into(),
				active_range_chars: None,
			})],
		)
		.drop_without_applying_deltas();
		for pressed in [true, false] {
			frame(
				&mut view,
				&mut state,
				vec![
					egui::Event::PointerMoved(close),
					egui::Event::PointerButton {
						pos: close,
						button: egui::PointerButton::Primary,
						pressed,
						modifiers: egui::Modifiers::NONE,
					},
				],
			)
			.drop_without_applying_deltas();
		}
		assert!(
			view.switcher.is_open(),
			"Pointer close must wait for the query composition"
		);
		frame(
			&mut view,
			&mut state,
			vec![egui::Event::Ime(egui::ImeEvent::Commit(
				"Query composition".into(),
			))],
		)
		.drop_without_applying_deltas();
		frame(&mut view, &mut state, vec![edit_key(egui::Key::Escape)])
			.drop_without_applying_deltas();
		frame(&mut view, &mut state, vec![]).drop_without_applying_deltas();
		assert!(!view.switcher.is_open());
		assert!(
			!view.ime_active,
			"Query IME state must not leak into the composer"
		);
		assert_eq!(state.selected, Some(Id(10)));
		assert_eq!(state.drafts, drafts);
		assert_eq!(view.editing.as_ref().unwrap().2, "Keep this edit");
	}

	#[test]
	fn inline_edit_uses_mentions_ime_and_preserves_draft_until_confirmed() {
		let ctx = egui::Context::default();
		let mut state = edit_state();
		let mut view = MessagingUi {
			editing: Some((Id(10), Id(20), "Original".into())),
			..Default::default()
		};
		assert!(edit_frame(&ctx, &mut view, &mut state, vec![]).is_empty());
		assert!(
			edit_frame(
				&ctx,
				&mut view,
				&mut state,
				vec![egui::Event::Text(" @Al".into())]
			)
			.is_empty()
		);
		assert!(
			edit_frame(
				&ctx,
				&mut view,
				&mut state,
				vec![edit_key(egui::Key::Enter)]
			)
			.is_empty()
		);
		assert_eq!(view.editing.as_ref().unwrap().2, "Original <@1> ");
		assert!(
			edit_frame(
				&ctx,
				&mut view,
				&mut state,
				vec![
					egui::Event::Ime(egui::ImeEvent::Commit("語".into())),
					edit_key(egui::Key::Enter)
				]
			)
			.is_empty()
		);
		let commands = edit_frame(
			&ctx,
			&mut view,
			&mut state,
			vec![edit_key(egui::Key::Enter)],
		);
		assert!(
			matches!(commands.as_slice(), [Command::Edit { channel: Id(10), message: Id(20), content }] if content.trim_end() == "Original <@1> 語")
		);
		assert!(view.has_edit(), "keep text until the service confirms it");
		state.command_rejected(commands.into_iter().next().unwrap());
		edit_frame(&ctx, &mut view, &mut state, vec![]);
		assert_eq!(view.editing.as_ref().unwrap().2, "Original <@1> 語\n");
		assert_eq!(state.drafts[&Id(10)], "Unsent draft 👋");
		assert!(
			view.draft_changes.is_empty(),
			"edits must not overwrite persisted unsent drafts"
		);
		let mut confirmed = state.timeline.get(Id(20)).unwrap().clone();
		confirmed.content = view.editing.as_ref().unwrap().2.clone();
		confirmed.edited = true;
		edit_frame(
			&ctx,
			&mut view,
			&mut state,
			vec![egui::Event::Text("newer".into())],
		);
		state.timeline.insert(confirmed, false, false).unwrap();
		edit_frame(&ctx, &mut view, &mut state, vec![]);
		assert!(
			view.has_edit(),
			"an earlier acknowledgement must not discard newer input"
		);
		state.freshness = Freshness::Fresh;
		let commands = edit_frame(
			&ctx,
			&mut view,
			&mut state,
			vec![edit_key(egui::Key::Enter)],
		);
		assert!(matches!(commands.as_slice(), [Command::Edit { .. }]));
		let mut confirmed = state.timeline.get(Id(20)).unwrap().clone();
		confirmed.content = view.editing.as_ref().unwrap().2.clone();
		state.timeline.insert(confirmed, false, false).unwrap();
		edit_frame(
			&ctx,
			&mut view,
			&mut state,
			vec![egui::Event::Text("same-frame".into())],
		);
		assert!(
			view.has_edit(),
			"input arriving with confirmation still belongs to the edit"
		);
		let commands = edit_frame(
			&ctx,
			&mut view,
			&mut state,
			vec![edit_key(egui::Key::Enter)],
		);
		assert!(matches!(commands.as_slice(), [Command::Edit { .. }]));
		let mut confirmed = state.timeline.get(Id(20)).unwrap().clone();
		confirmed.content = view.editing.as_ref().unwrap().2.clone();
		state.timeline.insert(confirmed, false, false).unwrap();
		edit_frame(&ctx, &mut view, &mut state, vec![]);
		assert!(!view.has_edit());
		assert_eq!(state.drafts[&Id(10)], "Unsent draft 👋");
	}

	#[test]
	fn inline_edit_survives_navigation_blocks_invalid_saves_and_cancels_without_sending() {
		let ctx = egui::Context::default();
		let mut state = edit_state();
		let mut view = MessagingUi {
			editing: Some((Id(10), Id(20), "Replacement".into())),
			..Default::default()
		};
		edit_frame(&ctx, &mut view, &mut state, vec![]);
		state.selected = Some(Id(11));
		edit_frame(&ctx, &mut view, &mut state, vec![]);
		assert_eq!(view.editing.as_ref().unwrap().2, "Replacement");
		assert!(!state.drafts.contains_key(&Id(11)));
		state.selected = Some(Id(10));
		edit_frame(&ctx, &mut view, &mut state, vec![]);
		state.freshness = Freshness::Stale;
		assert!(
			edit_frame(
				&ctx,
				&mut view,
				&mut state,
				vec![edit_key(egui::Key::Enter)]
			)
			.is_empty()
		);
		assert!(view.has_edit());
		state.freshness = Freshness::Fresh;
		let channels = std::mem::take(&mut state.channels);
		assert!(
			edit_frame(
				&ctx,
				&mut view,
				&mut state,
				vec![edit_key(egui::Key::Enter)]
			)
			.is_empty()
		);
		assert!(
			view.has_edit(),
			"Losing channel access keeps the unsaved edit"
		);
		state.channels = channels;
		state.user.as_mut().unwrap().id = Id(99);
		assert!(
			edit_frame(
				&ctx,
				&mut view,
				&mut state,
				vec![edit_key(egui::Key::Enter)]
			)
			.is_empty()
		);
		assert!(view.has_edit());
		assert!(
			edit_frame(
				&ctx,
				&mut view,
				&mut state,
				vec![edit_key(egui::Key::Escape)]
			)
			.is_empty()
		);
		assert!(!view.has_edit());
		assert_eq!(state.drafts[&Id(10)], "Unsent draft 👋");
		assert!(view.draft_changes.is_empty());
	}

	#[test]
	fn unread_pages_and_return_to_present_preserve_drafts_without_automatic_ack() {
		fn collect(shape: &egui::Shape, labels: &mut Vec<(String, egui::Rect)>) {
			match shape {
				egui::Shape::Text(text) => labels.push((
					text.galley.job.text.clone(),
					text.galley.rect.translate(text.pos.to_vec2()),
				)),
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						collect(shape, labels);
					}
				}
				_ => {}
			}
		}
		for (marker, width, dark) in [(None, 760.0, false), (Some(Id(10)), 1100.0, true)] {
			let ctx = egui::Context::default();
			ctx.set_visuals(if dark {
				egui::Visuals::dark()
			} else {
				egui::Visuals::light()
			});
			let mut view = MessagingUi::default();
			let mut state = edit_state();
			state.channels[0].last_message = Some(Id(20));
			state
				.apply_read_state(client_core::read_state::Event::Snapshot {
					entries: Some(vec![(Id(10), marker, 0)]),
					version: Some(1),
					partial: false,
				})
				.unwrap();
			let message = state.timeline.get(Id(20)).unwrap().clone();
			let drafts = state.drafts.clone();
			let frame = |view: &mut MessagingUi, state: &mut State, events| {
				let mut commands = vec![];
				let output = ctx.run_ui(
					egui::RawInput {
						focused: true,
						events,
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(width, 720.0),
						)),
						..Default::default()
					},
					|ui| commands = view.show(ui, state),
				);
				assert!(output.platform_output.commands.is_empty());
				assert!(
					!commands.iter().any(|command| matches!(
						command,
						Command::Send { .. } | Command::Edit { .. } | Command::MarkRead { .. }
					)),
					"Browsing unread pages must not send or acknowledge"
				);
				let mut labels = vec![];
				for shape in &output.shapes {
					collect(&shape.shape, &mut labels);
				}
				output.drop_without_applying_deltas();
				(labels, commands)
			};
			let click = |view: &mut MessagingUi, state: &mut State, label: &str| {
				let (labels, _) = frame(view, state, vec![]);
				let pos = labels
					.iter()
					.find(|(text, _)| {
						text == label || (label == "Jump to present" && text.ends_with(label))
					})
					.expect("navigation control")
					.1
					.center();
				let mut commands = vec![];
				for pressed in [true, false] {
					commands.extend(
						frame(
							view,
							state,
							vec![
								egui::Event::PointerMoved(pos),
								egui::Event::PointerButton {
									pos,
									button: egui::PointerButton::Primary,
									pressed,
									modifiers: egui::Modifiers::NONE,
								},
							],
						)
						.1,
					);
				}
				commands
			};
			for _ in 0..3 {
				frame(&mut view, &mut state, vec![]);
			}
			assert_eq!(state.read_marker(Id(10)), Some(marker));
			let commands = click(&mut view, &mut state, "Jump to unread");
			assert!(commands.iter().any(|command| matches!(command,
                Command::History { channel: Id(10), before: None, after: Some(after), .. } if *after == marker.unwrap_or(Id(0))
            )));
			let (labels, _) = frame(&mut view, &mut state, vec![]);
			assert!(!labels.iter().any(|(text, _)| text == "Next messages"));
			for (first, last) in [(11, 15), (16, 20)] {
				let messages = (first..=last)
					.map(|id| {
						let mut message = message.clone();
						message.id = Id(id);
						message
					})
					.collect();
				state.apply(client_core::Envelope {
					generation: state.generation,
					event: client_core::Event::History {
						channel: Id(10),
						request: state.request,
						older: false,
						messages,
					},
				});
				for _ in 0..3 {
					frame(&mut view, &mut state, vec![]);
				}
				if last == 15 {
					let commands = click(&mut view, &mut state, "Next messages");
					assert!(commands.iter().any(|command| matches!(
						command,
						Command::History {
							before: None,
							after: Some(Id(15)),
							..
						}
					)));
				}
			}
			let (labels, _) = frame(&mut view, &mut state, vec![]);
			assert!(!labels.iter().any(|(text, _)| text == "Next messages"));
			assert_eq!(state.read_marker(Id(10)), Some(marker));
			let commands = click(&mut view, &mut state, "Jump to present");
			assert!(commands.iter().any(|command| matches!(
				command,
				Command::History {
					before: None,
					after: None,
					..
				}
			)));
			assert_eq!(state.drafts, drafts);
		}
	}

	#[test]
	fn reply_controls_are_inert_when_deleted_loading_stale_or_unavailable() {
		fn collect(shape: &egui::Shape, labels: &mut Vec<(String, egui::Rect)>) {
			match shape {
				egui::Shape::Text(text) => labels.push((
					text.galley.job.text.clone(),
					text.galley.rect.translate(text.pos.to_vec2()),
				)),
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						collect(shape, labels);
					}
				}
				_ => {}
			}
		}
		for blocked in 0..5 {
			let ctx = egui::Context::default();
			let mut view = MessagingUi::default();
			let mut state = edit_state();
			let mut source = state.timeline.get(Id(20)).unwrap().clone();
			source.reply_to = Some(Id(19));
			source.reply_deleted = blocked == 0;
			source.kind = 19;
			state.timeline.insert(source.clone(), true, false).unwrap();
			state.reply = Some(Id(19));
			match blocked {
				0 => {}
				1 => {
					state.timeline.delete(Id(19)).unwrap();
				}
				2 => {
					state.freshness = Freshness::Loading;
					state.history_pending = true;
				}
				3 => {
					state.freshness = Freshness::Stale;
				}
				4 => {
					state.channels.clear();
					state.freshness = Freshness::Unavailable;
				}
				_ => unreachable!(),
			}
			let draft = state.drafts.clone();
			let request = state.request;
			let frame = |view: &mut MessagingUi, state: &mut State, events| {
				let mut commands = vec![];
				let output = ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(760.0, 650.0),
						)),
						events,
						..Default::default()
					},
					|ui| commands = view.show(ui, state),
				);
				assert!(output.platform_output.commands.is_empty());
				assert!(!commands.iter().any(|command| matches!(
					command,
					Command::History { .. }
						| Command::Send { .. }
						| Command::Edit { .. }
						| Command::Delete { .. }
						| Command::MarkRead { .. }
				)));
				let mut labels = vec![];
				for shape in &output.shapes {
					collect(&shape.shape, &mut labels);
				}
				output.drop_without_applying_deltas();
				labels
			};
			for _ in 0..3 {
				frame(&mut view, &mut state, vec![]);
			}
			let labels = frame(&mut view, &mut state, vec![]);
			if blocked < 2 {
				assert!(labels.iter().any(|(text, _)| text == "Message deleted"));
			}
			// Activate the visible disabled controls (and the inert deleted label) with real input.
			for (_, rect) in labels.iter().filter(|(text, _)| {
				text == "View original"
					|| text == "Message deleted"
					|| text.starts_with("@Alex  ")
					|| text.starts_with("Earlier message")
			}) {
				let pos = rect.center();
				for pressed in [true, false] {
					frame(
						&mut view,
						&mut state,
						vec![
							egui::Event::PointerMoved(pos),
							egui::Event::PointerButton {
								pos,
								button: egui::PointerButton::Primary,
								pressed,
								modifiers: egui::Modifiers::NONE,
							},
						],
					);
				}
			}
			assert_eq!(state.request, request);
			assert!(state.search_target.is_none());
			assert_eq!(state.drafts, draft);
			assert_eq!(state.reply, Some(Id(19)));
		}
	}

	#[test]
	fn reply_original_buttons_navigate_locally_or_request_one_page_without_sending() {
		fn collect(shape: &egui::Shape, labels: &mut Vec<(String, egui::Rect)>) {
			match shape {
				egui::Shape::Text(text) => labels.push((
					text.galley.job.text.clone(),
					text.galley.rect.translate(text.pos.to_vec2()),
				)),
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						collect(shape, labels);
					}
				}
				_ => {}
			}
		}
		for (loaded, composer, keyboard, width) in [
			(true, false, false, 900.0),
			(false, false, true, 760.0),
			(true, true, true, 900.0),
			(false, true, false, 760.0),
		] {
			let ctx = egui::Context::default();
			let mut view = MessagingUi::default();
			let mut state = edit_state();
			let mut source = state.timeline.get(Id(20)).unwrap().clone();
			source.content = "Reply source".into();
			source.reply_to = Some(Id(19));
			state.timeline.insert(source.clone(), true, false).unwrap();
			if loaded {
				source.id = Id(19);
				source.reply_to = None;
				source.content = "||Hidden original||".into();
				state.timeline.insert(source, false, false).unwrap();
			}
			state.reply = Some(Id(19));
			let draft = state.drafts.clone();
			let frame = |view: &mut MessagingUi, state: &mut State, events| {
				let mut commands = vec![];
				let output = ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(width, 650.0),
						)),
						events,
						..Default::default()
					},
					|ui| commands = view.show(ui, state),
				);
				assert!(output.platform_output.commands.is_empty());
				assert!(!commands.iter().any(|command| matches!(
					command,
					Command::Send { .. }
						| Command::Edit { .. }
						| Command::Delete { .. }
						| Command::MarkRead { .. }
				)));
				let mut labels = vec![];
				for shape in &output.shapes {
					collect(&shape.shape, &mut labels);
				}
				for event in &output.platform_output.events {
					if let egui::output::OutputEvent::FocusGained(info) = event
						&& let Some(label) = &info.label
						&& let Some(response) = ctx
							.memory(|m| m.focused())
							.and_then(|id| ctx.read_response(id))
					{
						labels.push((label.clone(), response.rect));
					}
				}
				output.drop_without_applying_deltas();
				(labels, commands)
			};
			for _ in 0..3 {
				frame(&mut view, &mut state, vec![]);
			}
			let label_matches = |text: &str| {
				if composer {
					text == "View original"
				} else if loaded {
					text == "@Alex  Spoiler"
				} else {
					text == "Earlier message \u{b7} View original"
				}
			};
			let (labels, _) = frame(&mut view, &mut state, vec![]);
			assert!(
				!labels
					.iter()
					.any(|(text, _)| text.contains("Hidden original"))
			);
			let request = state.request;
			let commands = if keyboard {
				ctx.memory_mut(|memory| {
					if let Some(id) = memory.focused() {
						memory.surrender_focus(id);
					}
				});
				frame(&mut view, &mut state, vec![egui::Event::PointerGone]);
				let key = |key| egui::Event::Key {
					key,
					physical_key: None,
					pressed: true,
					repeat: false,
					modifiers: egui::Modifiers::NONE,
				};
				let mut activated = None;
				for _ in 0..60 {
					let (labels, _) = frame(&mut view, &mut state, vec![key(egui::Key::Tab)]);
					if ctx
						.memory(|memory| memory.focused())
						.and_then(|id| ctx.read_response(id))
						.is_some_and(|response| {
							response.rect.height() < 36.0
								&& labels.iter().any(|(text, rect)| {
									label_matches(text) && response.rect.contains(rect.center())
								})
						}) {
						activated =
							Some(frame(&mut view, &mut state, vec![key(egui::Key::Enter)]).1);
						break;
					}
				}
				activated.expect("Tab must reach the original-message button")
			} else {
				let point = labels
					.iter()
					.find(|(text, _)| label_matches(text))
					.unwrap()
					.1
					.center();
				let mut activated = vec![];
				for pressed in [true, false] {
					activated.extend(
						frame(
							&mut view,
							&mut state,
							vec![
								egui::Event::PointerMoved(point),
								egui::Event::PointerButton {
									pos: point,
									button: egui::PointerButton::Primary,
									pressed,
									modifiers: egui::Modifiers::NONE,
								},
							],
						)
						.1,
					);
				}
				activated
			};
			assert_eq!(state.reply, Some(Id(19)));
			assert_eq!(state.drafts, draft);
			assert!(view.draft_changes.is_empty());
			assert_eq!(state.search_target, Some(Id(19)));
			if loaded {
				assert_eq!(state.request, request);
				assert!(
					!commands
						.iter()
						.any(|command| matches!(command, Command::History { .. }))
				);
				for _ in 0..3 {
					frame(&mut view, &mut state, vec![]);
				}
				assert!(state.search_target.is_none());
			} else {
				assert_eq!(
					commands
						.iter()
						.filter(|command| matches!(
							command,
							Command::History {
								channel: Id(10),
								before: Some(Id(20)),
								..
							}
						))
						.count(),
					1
				);
				assert!(state.history_pending);
				assert_eq!(state.freshness, Freshness::Loading);
				for _ in 0..3 {
					let (_, commands) = frame(&mut view, &mut state, vec![]);
					assert!(
						!commands
							.iter()
							.any(|command| matches!(command, Command::History { .. }))
					);
				}
				state.apply(client_core::Envelope {
					generation: state.generation,
					event: client_core::Event::HistoryFailed {
						channel: Id(10),
						request: state.request,
						failure: client_core::auth::Failure::Network,
					},
				});
				let failure = state.status;
				let (_, commands) = frame(&mut view, &mut state, vec![]);
				assert_eq!(
					state.status, failure,
					"Rendering must preserve the actual request error"
				);
				assert!(
					!commands
						.iter()
						.any(|command| matches!(command, Command::History { .. }))
				);
				assert_eq!(state.drafts, draft);
				ctx.memory_mut(|memory| {
					if let Some(id) = memory.focused() {
						memory.surrender_focus(id);
					}
				});
				let mut reload = None;
				for _ in 0..80 {
					let (labels, _) = frame(&mut view, &mut state, vec![edit_key(egui::Key::Tab)]);
					if let Some((_, rect)) =
						labels.iter().find(|(text, _)| text == "Reload history")
					{
						reload = Some(rect.center());
						break;
					}
				}
				let pos = reload.expect("Tab must reach Reload history");
				let mut reloads = 0;
				for pressed in [true, false] {
					let (_, commands) = frame(
						&mut view,
						&mut state,
						vec![
							egui::Event::PointerMoved(pos),
							egui::Event::PointerButton {
								pos,
								button: egui::PointerButton::Primary,
								pressed,
								modifiers: egui::Modifiers::NONE,
							},
						],
					);
					reloads += commands
						.iter()
						.filter(|command| {
							matches!(
								command,
								Command::History {
									channel: Id(10),
									before: None,
									..
								}
							)
						})
						.count();
				}
				assert_eq!(reloads, 1, "Explicit Reload returns to recent history");
			}
		}
	}

	#[test]
	fn deleted_edit_target_keeps_user_changes_but_releases_untouched_original() {
		fn collect(shape: &egui::Shape, labels: &mut Vec<(String, egui::Rect)>) {
			match shape {
				egui::Shape::Text(text) => labels.push((
					text.galley.job.text.clone(),
					text.galley.rect.translate(text.pos.to_vec2()),
				)),
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						collect(shape, labels);
					}
				}
				_ => {}
			}
		}
		let shortcut = |key| egui::Event::Key {
			key,
			physical_key: None,
			pressed: true,
			repeat: false,
			modifiers: egui::Modifiers::COMMAND,
		};
		for (modified, reopen) in [(false, false), (true, false), (true, true)] {
			let ctx = egui::Context::default();
			let mut state = edit_state();
			let mut view = MessagingUi {
				editing: Some((Id(10), Id(20), "Original".into())),
				..Default::default()
			};
			let frame = |view: &mut MessagingUi, state: &mut State, events| {
				let mut commands = vec![];
				let output = ctx.run_ui(
					egui::RawInput {
						screen_rect: Some(egui::Rect::from_min_size(
							egui::Pos2::ZERO,
							egui::vec2(900.0, 650.0),
						)),
						events,
						..Default::default()
					},
					|ui| commands = view.show(ui, state),
				);
				assert!(!commands.iter().any(|command| matches!(
					command,
					Command::Send { .. } | Command::Edit { .. } | Command::Delete { .. }
				)));
				assert!(output.platform_output.commands.is_empty());
				let mut labels = vec![];
				for shape in &output.shapes {
					collect(&shape.shape, &mut labels);
				}
				for event in &output.platform_output.events {
					if let egui::output::OutputEvent::FocusGained(info) = event
						&& let Some(label) = &info.label
						&& let Some(response) = ctx
							.memory(|m| m.focused())
							.and_then(|id| ctx.read_response(id))
					{
						labels.push((label.clone(), response.rect));
					}
				}
				output.drop_without_applying_deltas();
				labels
			};
			for _ in 0..3 {
				frame(&mut view, &mut state, vec![]);
			}
			if modified {
				frame(
					&mut view,
					&mut state,
					vec![
						shortcut(egui::Key::A),
						egui::Event::Text("Only my unsent text".into()),
					],
				);
				assert_eq!(view.editing.as_ref().unwrap().2, "Only my unsent text");
			}
			if reopen {
				let labels = frame(&mut view, &mut state, vec![]);
				let message = labels
					.iter()
					.find(|(text, _)| text.trim_end() == "Original")
					.expect("Loaded message remains visible")
					.1
					.center();
				frame(
					&mut view,
					&mut state,
					vec![egui::Event::PointerMoved(message)],
				);
				ctx.memory_mut(|memory| {
					if let Some(id) = memory.focused() {
						memory.surrender_focus(id);
					}
				});
				let mut activated = false;
				for _ in 0..80 {
					let labels = frame(&mut view, &mut state, vec![edit_key(egui::Key::Tab)]);
					if labels.iter().any(|(text, _)| text == "Edit message") {
						frame(&mut view, &mut state, vec![edit_key(egui::Key::Enter)]);
						activated = true;
						break;
					}
				}
				assert!(activated, "Tab must reach the own-message edit action");
				assert_eq!(view.editing.as_ref().unwrap().2, "Original");
			}
			let retained = view.editing.as_ref().unwrap().2.clone();
			let keep_edit = modified && !reopen;
			assert_eq!(retained != "Original", keep_edit);
			view.deleting = Some((Id(10), Id(20)));
			state.timeline.delete(Id(20)).unwrap();
			state.revision += 1;
			view.messages_deleted(&ctx, Id(10), &[Id(20)]);
			frame(&mut view, &mut state, vec![edit_key(egui::Key::Enter)]);
			assert!(view.deleting.is_none());
			assert_eq!(view.has_edit(), keep_edit);
			if keep_edit {
				frame(&mut view, &mut state, vec![shortcut(egui::Key::Z)]);
				assert_eq!(
					view.editing.as_ref().unwrap().2,
					retained,
					"Undo cannot restore the deleted original"
				);
				let labels = frame(&mut view, &mut state, vec![]);
				assert_eq!(view.editing.as_ref().unwrap().2, retained);
				assert!(labels.iter().any(|(label, _)| label == "Copy edit text"));
				assert!(
					labels
						.iter()
						.any(|(label, _)| label.contains("Message unavailable"))
				);
				frame(&mut view, &mut state, vec![edit_key(egui::Key::Escape)]);
				assert!(!view.has_edit());
			}
			assert_eq!(state.drafts[&Id(10)], "Unsent draft 👋");
			assert!(view.draft_changes.is_empty());
		}
	}

	#[test]
	fn off_channel_deletion_releases_original_and_undo_without_touching_other_input() {
		for modified in [false, true] {
			let ctx = egui::Context::default();
			let mut state = edit_state();
			let mut view = MessagingUi {
				editing: Some((Id(10), Id(20), "Original".into())),
				..Default::default()
			};
			for _ in 0..3 {
				edit_frame(&ctx, &mut view, &mut state, vec![]);
			}
			if modified {
				let select_all = egui::Event::Key {
					key: egui::Key::A,
					physical_key: None,
					pressed: true,
					repeat: false,
					modifiers: egui::Modifiers::COMMAND,
				};
				edit_frame(
					&ctx,
					&mut view,
					&mut state,
					vec![select_all, egui::Event::Text("User replacement".into())],
				);
			}
			let text = view.editing.as_ref().unwrap().2.clone();
			let widget = view.edit_widget_id.unwrap();
			let cursor = egui::text_edit::TextEditState::load(&ctx, widget)
				.unwrap()
				.cursor
				.char_range()
				.unwrap();
			state.selected = Some(Id(11));
			state.channels.push(model::Channel {
				id: Id(11),
				guild: None,
				kind: 1,
				name: "Other conversation".into(),
				last_message: None,
				parent_id: None,
				position: 0,
				recipients: vec![],
				member_list_id: None,
				message_count: None,
			});
			edit_frame(&ctx, &mut view, &mut state, vec![]);
			view.deleting = Some((Id(10), Id(20)));
			for (channel, ids) in [
				(Id(11), vec![Id(20)]),
				(Id(10), vec![Id(21)]),
				(Id(10), vec![Id(20); 101]),
			] {
				view.messages_deleted(&ctx, channel, &ids);
				assert_eq!(view.editing.as_ref().unwrap().2, text);
				assert!(view.deleting.is_some());
			}
			view.messages_deleted(&ctx, Id(10), &[Id(20)]);
			assert!(view.deleting.is_none());
			assert_eq!(view.has_edit(), modified);
			let editor = egui::text_edit::TextEditState::load(&ctx, widget).unwrap();
			assert!(
				editor.undoer().undo(&(cursor, text.clone())).is_none(),
				"Original undo snapshots must be gone before revisiting the conversation"
			);
			if modified {
				assert_eq!(view.editing.as_ref().unwrap().2, "User replacement");
				assert_eq!(editor.cursor.char_range(), Some(cursor));
			}
			// The close notification is scoped to A; B must still accept this frame's input.
			let output = ctx.run_ui(
				egui::RawInput {
					events: vec![egui::Event::Text("B input".into())],
					..Default::default()
				},
				|ui| {
					ctx.memory_mut(|memory| {
						memory.request_focus(ui.make_persistent_id("message-input"))
					});
					view.composer(ui, &mut state, Id(11), &ctx, &mut vec![]);
				},
			);
			output.drop_without_applying_deltas();
			assert_eq!(state.drafts[&Id(11)], "B input");
			assert_eq!(state.drafts[&Id(10)], "Unsent draft 👋");
		}
	}

	#[test]
	fn member_pane_virtualizes_and_preview_never_requests_network() {
		fn collect_text(shape: &egui::Shape, text: &mut Vec<String>) {
			match shape {
				egui::Shape::Text(shape) => text.push(shape.galley.job.text.clone()),
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						collect_text(shape, text);
					}
				}
				_ => {}
			}
		}

		let mut state = State {
			demo: true,
			selected: Some(Id(1)),
			channels: vec![model::Channel {
				last_message: None,
				id: Id(1),
				guild: Some(Id(2)),
				parent_id: None,
				position: 0,
				name: "Synthetic".into(),
				kind: 0,
				recipients: Vec::new(),
				member_list_id: Some("everyone".into()),
				message_count: None,
			}],
			members: Some(model::MemberList {
				channel: Id(1),
				guild: Some(Id(2)),
				request: 1,
				total: 100,
				freshness: Freshness::Fresh,
				rows: (1..=100)
					.map(|id| {
						Some(model::Member {
							user: model::User {
								id: Id(id),
								name: format!("Synthetic {id}"),
								avatar: None,
								discriminator: 0,
							},
							nick: None,
							roles: vec![],
							status: match id {
								1 => Some("online"),
								2 => Some("idle"),
								3 => Some("dnd"),
								4 => Some("offline"),
								5 => Some("unknown"),
								_ => None,
							}
							.map(str::to_owned),
							custom_status: None,
							activities: vec![],
						})
					})
					.collect(),
			}),
			..Default::default()
		};
		state.permissions.guilds.insert(
			Id(2),
			model::permissions::Guild {
				id: Id(2),
				owner: None,
				member: None,
				roles: Some(vec![model::permissions::Role {
					id: Id(8),
					bits: 0,
					name: "Founders".into(),
					color: 0xe78284,
					position: 1,
					hoist: true,
				}]),
			},
		);
		for member in state
			.members
			.as_mut()
			.unwrap()
			.rows
			.iter_mut()
			.flatten()
			.take(2)
		{
			member.roles.push(Id(8));
		}
		let mut messaging = MessagingUi::default();
		let context = egui::Context::default();
		let output = context.run_ui(
			egui::RawInput {
				screen_rect: Some(egui::Rect::from_min_size(
					egui::Pos2::ZERO,
					egui::vec2(1120.0, 760.0),
				)),
				..Default::default()
			},
			|ui| {
				assert!(messaging.show(ui, &mut state).is_empty());
			},
		);
		assert!(messaging.take_avatar_requests().is_empty());
		let mut text = Vec::new();
		for shape in &output.shapes {
			collect_text(&shape.shape, &mut text);
		}
		for heading in ["Founders — 2", "Online — 1", "Offline — 97"] {
			assert!(
				text.iter().any(|label| label == heading),
				"Missing {heading}"
			);
		}
		for status in ["Online", "Away", "Do not disturb", "Offline"] {
			assert!(text.iter().any(|label| label == status), "Missing {status}");
		}
		assert!(
			text.iter()
				.filter(|label| label.as_str() == "Presence unavailable")
				.count() >= 2,
			"Both unknown and absent presence must remain explicit"
		);
		assert!(
			!output.textures_delta.set.is_empty(),
			"Preview must exercise actual image uploads"
		);
		assert!(
			output.textures_delta.set.len() < 30,
			"Offscreen members must not upload textures"
		);
		output.drop_without_applying_deltas();
	}

	#[test]
	fn incoming_custom_status_updates_people_and_open_profile_without_refetch() {
		fn collect(shape: &egui::Shape, labels: &mut Vec<String>) {
			match shape {
				egui::Shape::Text(text) => labels.push(text.galley.job.text.clone()),
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						collect(shape, labels);
					}
				}
				_ => {}
			}
		}
		let mut state = test_support::demo_state();
		state.demo = false; // Exercise normal command admission using synthetic loaded data.
		let channel = state.selected.unwrap();
		let guild = state
			.channels
			.iter()
			.find(|entry| entry.id == channel)
			.unwrap()
			.guild
			.unwrap();
		let user = test_support::message(499, channel).author;
		state.timeline.clear(); // No read acknowledgement is part of this presence-only scenario.
		state.members = Some(model::MemberList {
			channel,
			guild: Some(guild),
			request: 7,
			total: 1,
			freshness: Freshness::Fresh,
			rows: vec![Some(model::Member {
				user: user.clone(),
				nick: None,
				roles: vec![],
				status: Some("online".into()),
				custom_status: Some("Initial synthetic status".into()),
				activities: vec![],
			})],
		});
		state.profile = Some(client_core::profile::ProfileView {
			user: user.id,
			guild: Some(guild),
			request: 314,
			loading: false,
			error: None,
			data: Some(profiles::synthetic(&user, Some(guild))),
		});
		state
			.drafts
			.insert(channel, "Keep this unsent draft".into());
		let mut messaging = MessagingUi {
			navigation_channel: Some(channel),
			guild: Some(guild),
			profile: Some(user.clone()),
			profile_anchor: Some((user.id, egui::pos2(420.0, 150.0))),
			..Default::default()
		};
		let ctx = egui::Context::default();
		design::apply(&ctx);
		let frame = |messaging: &mut MessagingUi, state: &mut State| {
			let mut commands = vec![];
			let output = ctx.run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(1280.0, 900.0),
					)),
					..Default::default()
				},
				|ui| commands = messaging.show(ui, state),
			);
			assert!(
				commands.is_empty(),
				"Presence rendering must not fetch a profile or emit other commands"
			);
			assert!(output.platform_output.commands.is_empty());
			let mut labels = vec![];
			for shape in &output.shapes {
				collect(&shape.shape, &mut labels);
			}
			output.drop_without_applying_deltas();
			labels
		};
		for _ in 0..3 {
			frame(&mut messaging, &mut state);
		}
		let labels = frame(&mut messaging, &mut state);
		assert_eq!(
			labels
				.iter()
				.filter(|text| text.as_str() == "Initial synthetic status")
				.count(),
			2,
			"People and the already-open profile both show the loaded status"
		);
		for custom_status in [Some("Updated synthetic status"), None] {
			state.apply(client_core::Envelope {
				generation: state.generation,
				event: client_core::Event::MemberPresence {
					guild,
					channel,
					request: 7,
					updates: vec![model::MemberPresence {
						user: user.id,
						status: Some("online".into()),
						custom_status: custom_status.map(str::to_owned),
						activities: vec![],
					}],
				},
			});
			for _ in 0..2 {
				frame(&mut messaging, &mut state);
			}
			let labels = frame(&mut messaging, &mut state);
			assert!(!labels.iter().any(|text| text == "Initial synthetic status"));
			assert_eq!(
				labels
					.iter()
					.filter(|text| text.as_str() == "Updated synthetic status")
					.count(),
				if custom_status.is_some() { 2 } else { 0 }
			);
			assert_eq!(state.profile.as_ref().unwrap().request, 314);
			assert_eq!(messaging.profile.as_ref().unwrap().id, user.id);
			assert_eq!(state.drafts[&channel], "Keep this unsent draft");
			assert!(messaging.draft_changes.is_empty());
		}
	}

	#[test]
	fn rich_presence_reaches_lists_dm_header_and_profile_then_clears() {
		fn collect(shape: &egui::Shape, output: &mut String) {
			match shape {
				egui::Shape::Text(text) => {
					output.push_str(&text.galley.job.text);
					output.push('\n');
				}
				egui::Shape::Vec(shapes) => {
					for shape in shapes {
						collect(shape, output);
					}
				}
				_ => {}
			}
		}
		for dm in [false, true] {
			let mut state = test_support::demo_state();
			let user = test_support::message(1, Id(22)).author;
			let channel = if dm { Id(22) } else { state.selected.unwrap() };
			let _ = state.select(channel);
			state.timeline.clear();
			state.history_pending = false;
			let guild = state
				.channels
				.iter()
				.find(|c| c.id == channel)
				.unwrap()
				.guild;
			let _ = state.request_members();
			state.members = Some(model::MemberList {
				guild,
				channel,
				request: 7,
				total: 1,
				freshness: Freshness::Fresh,
				rows: vec![Some(model::Member {
					roles: vec![],
					user: user.clone(),
					nick: None,
					status: Some("online".into()),
					custom_status: None,
					activities: vec![],
				})],
			});
			let mut messaging = MessagingUi {
				navigation_channel: Some(channel),
				guild,
				profile: Some(user.clone()),
				profile_anchor: Some((user.id, egui::pos2(420.0, 150.0))),
				..Default::default()
			};
			let ctx = egui::Context::default();
			design::apply(&ctx);
			for clear in [false, true] {
				let activities = if clear {
					vec![]
				} else {
					vec![model::RichActivity {
						kind: 0,
						name: "Stardew Valley".into(),
						details: Some("Tending the farm".into()),
						state: Some("Spring Day 12".into()),
						image: Some(model::ActivityImage::Asset {
							application: Id(9001),
							asset: Id(9002),
						}),
					}]
				};
				let event = if let Some(guild) = guild {
					client_core::Event::MemberPresence {
						guild,
						channel,
						request: 7,
						updates: vec![model::MemberPresence {
							user: user.id,
							status: Some("online".into()),
							custom_status: None,
							activities,
						}],
					}
				} else {
					client_core::Event::DirectPresence(vec![client_core::presence::Update {
						user: user.id,
						status: model::Patch::Value("online".into()),
						activities: model::Patch::Value(activities),
						custom_status: model::Patch::Absent,
					}])
				};
				state.apply(client_core::Envelope {
					generation: state.generation,
					event,
				});
				let mut painted = String::new();
				let mut artwork = Vec::new();
				for _ in 0..3 {
					painted.clear();
					artwork.clear();
					let output = ctx.run_ui(
						egui::RawInput {
							screen_rect: Some(egui::Rect::from_min_size(
								egui::Pos2::ZERO,
								egui::vec2(1280.0, 900.0),
							)),
							..Default::default()
						},
						|ui| {
							messaging.show(ui, &mut state);
						},
					);
					assert!(output.platform_output.commands.is_empty());
					for shape in &output.shapes {
						collect(&shape.shape, &mut painted);
					}
					// Match this activity's actual texture, not unrelated same-sized UI meshes.
					let activity_texture = messaging.avatars.texture_id(
						&model::ActivityImage::Asset {
							application: Id(9001),
							asset: Id(9002),
						}
						.key(),
					);
					artwork = ctx
						.tessellate(output.shapes.clone(), output.pixels_per_point)
						.iter()
						.filter_map(|shape| match &shape.primitive {
							egui::epaint::Primitive::Mesh(mesh)
								if Some(mesh.texture_id) == activity_texture
									&& shape
										.clip_rect
										.intersect(mesh.calc_bounds())
										.is_positive() =>
							{
								Some(mesh.calc_bounds())
							}
							_ => None,
						})
						.collect();
					output.drop_without_applying_deltas();
				}
				assert_eq!(
					painted.matches("Playing Stardew Valley").count(),
					if clear {
						0
					} else if dm {
						4
					} else {
						2
					},
					"{painted}"
				);
				assert_eq!(painted.contains("Tending the farm"), !clear);
				assert_eq!(painted.contains("Spring Day 12"), !clear);
				assert_eq!(
					artwork.len(),
					usize::from(!clear),
					"Activity image appears and clears with its presence"
				);
			}
		}
	}

	#[test]
	fn profile_uses_open_conversation_not_browsed_sidebar_server() {
		for guild in [None, Some(Id(10))] {
			let user = model::User {
				id: Id(2),
				name: "Synthetic".into(),
				avatar: None,
				discriminator: 0,
			};
			let mut state = State {
				demo: true,
				selected: Some(Id(1)),
				channels: vec![model::Channel {
					last_message: None,
					id: Id(1),
					guild,
					parent_id: None,
					position: 0,
					name: "Open conversation".into(),
					kind: if guild.is_some() { 0 } else { 1 },
					recipients: vec![user.clone()],
					member_list_id: None,
					message_count: None,
				}],
				..Default::default()
			};
			let mut messaging = MessagingUi {
				navigation_channel: state.selected,
				guild: Some(Id(99)),
				profile: Some(user),
				..Default::default()
			};
			let output = egui::Context::default().run_ui(
				egui::RawInput {
					screen_rect: Some(egui::Rect::from_min_size(
						egui::Pos2::ZERO,
						egui::vec2(1120.0, 900.0),
					)),
					..Default::default()
				},
				|ui| {
					messaging.show(ui, &mut state);
				},
			);
			assert_eq!(state.profile.as_ref().unwrap().guild, guild);
			output.drop_without_applying_deltas();
		}
	}

	#[test]
	fn inline_edit_keeps_the_pending_draft_attachment_separate() {
		let ctx = egui::Context::default();
		let mut state = edit_state();
		let mut view = MessagingUi {
			editing: Some((Id(10), Id(20), "Replacement".into())),
			attachment: Some(("draft.txt".into(), 32)),
			upload_busy: true,
			..Default::default()
		};
		edit_frame(&ctx, &mut view, &mut state, vec![]);
		let commands = edit_frame(
			&ctx,
			&mut view,
			&mut state,
			vec![edit_key(egui::Key::Enter)],
		);
		assert!(
			matches!(commands.as_slice(), [Command::Edit { content, .. }] if content == "Replacement")
		);
		assert_eq!(state.drafts[&Id(10)], "Unsent draft 👋");
		assert_eq!(view.attachment, Some(("draft.txt".into(), 32)));
		assert!(
			!view.attach_requested
				&& !view.remove_attachment_requested
				&& !view.cancel_upload_requested
		);
	}

	#[test]
	fn pending_preview_waits_for_confirmation_and_restore_preserves_new_drafts() {
		let ctx = egui::Context::default();
		let mut state = test_support::demo_state();
		let channel = state.selected.unwrap();
		let mut view = MessagingUi::default();
		view.preview_attachment(
			"synthetic.png",
			100,
			Some(egui::ColorImage::filled([2, 2], egui::Color32::WHITE)),
		);
		view.preview_sending(&ctx, &mut state);
		let nonce = state.pending.last().unwrap().nonce.clone();
		assert!(view.pending_upload.as_ref().unwrap().preview.is_some());
		assert!(view.attachment.is_none());
		view.update_upload_progress(None, true);
		assert_eq!(
			view.pending_upload.as_ref().unwrap().progress,
			Some((100, 100))
		);
		assert_eq!(
			state.pending.last().unwrap().delivery,
			model::Delivery::Sending
		);
		state.pending.last_mut().unwrap().delivery = model::Delivery::Ambiguous;
		state.drafts.insert(channel, "New draft".into());
		view.restore_pending(&mut state, channel, &nonce);
		assert_eq!(state.drafts[&channel], "New draft");
		assert!(state.pending.iter().any(|p| p.nonce == nonce));
		state.drafts.remove(&channel);
		view.restore_pending(&mut state, channel, &nonce);
		assert!(state.drafts[&channel].contains("sending it now"));
		assert!(!state.pending.iter().any(|p| p.nonce == nonce));
		ctx.run_ui(Default::default(), |ui| {
			view.show(ui, &mut state);
		})
		.drop_without_applying_deltas();
		assert!(view.pending_upload.is_none());

		view.preview_attachment("synthetic.png", 100, None);
		view.preview_sending(&ctx, &mut state);
		let nonce = state.pending.last().unwrap().nonce.clone();
		let mut confirmed = test_support::message(999_999, channel);
		confirmed.author = state.user.clone().unwrap();
		confirmed.nonce = Some(nonce.clone());
		state.apply(client_core::Envelope {
			generation: state.generation,
			event: client_core::Event::SendResult {
				nonce,
				result: Ok(confirmed),
			},
		});
		ctx.run_ui(Default::default(), |ui| {
			view.show(ui, &mut state);
		})
		.drop_without_applying_deltas();
		assert!(view.pending_upload.is_none());
		assert!(state.timeline.get(Id(999_999)).is_some());
	}

	#[test]
	fn attachment_only_enter_sends_once_and_busy_upload_blocks_resending() {
		for (busy, allowed, modifiers, repeat) in [
			(true, true, egui::Modifiers::NONE, false),
			(false, true, egui::Modifiers::NONE, false),
			(false, false, egui::Modifiers::NONE, false),
			(false, true, egui::Modifiers::SHIFT, false),
			(false, true, egui::Modifiers::ALT, false),
			(false, true, egui::Modifiers::NONE, true),
		] {
			let ctx = egui::Context::default();
			let mut state = test_support::demo_state();
			state.demo = false;
			let channel = state.selected.unwrap();
			state.drafts.remove(&channel);
			if !allowed {
				state.channels.clear();
			}
			let mut messaging = MessagingUi {
				attachment: Some(("synthetic.txt".into(), 32)),
				upload_busy: busy,
				..Default::default()
			};
			let mut commands = Vec::new();
			let mut editor = egui::Id::NULL;
			ctx.run_ui(Default::default(), |ui| {
				editor = ui.make_persistent_id("message-input");
				messaging.composer(ui, &mut state, channel, &ctx, &mut commands);
			})
			.drop_without_applying_deltas();
			assert!(commands.is_empty(), "Selecting a file must not send it");
			ctx.memory_mut(|m| m.request_focus(editor));
			if repeat {
				// egui derives repeat from held keys, overriding the raw event flag.
				ctx.input_mut(|i| i.keys_down.insert(egui::Key::Enter));
			}
			ctx.run_ui(
				egui::RawInput {
					events: vec![egui::Event::Key {
						key: egui::Key::Enter,
						physical_key: None,
						pressed: true,
						repeat,
						modifiers,
					}],
					..Default::default()
				},
				|ui| messaging.composer(ui, &mut state, channel, &ctx, &mut commands),
			)
			.drop_without_applying_deltas();
			let sends = !busy && allowed && modifiers == egui::Modifiers::NONE && !repeat;
			assert_eq!(commands.len(), usize::from(sends));
			if modifiers == egui::Modifiers::SHIFT {
				assert_eq!(state.drafts[&channel], "\n");
			}
			if sends {
				assert!(
					matches!(&commands[0], Command::Send { content, .. } if content.is_empty())
				);
				assert_eq!(
					state.pending[0].attachment.as_deref(),
					Some("synthetic.txt")
				);
				assert!(messaging.attachment.is_none());
				assert!(messaging.upload_busy);
				let mut release = edit_key(egui::Key::Enter);
				if let egui::Event::Key { pressed, .. } = &mut release {
					*pressed = false;
				}
				assert!(
					edit_frame(
						&ctx,
						&mut messaging,
						&mut state,
						vec![release, edit_key(egui::Key::Enter)],
					)
					.is_empty(),
					"Another Send before desktop dispatch must not enqueue the attachment again"
				);
			} else {
				assert!(
					messaging.attachment.is_some(),
					"Unavailable sends retain selected metadata"
				);
			}
		}
	}

	#[test]
	fn rendering_does_not_hide_saved_drafts_and_clears_survive_hydration() {
		let mut state = State {
			selected: Some(Id(1)),
			channels: vec![model::Channel {
				last_message: None,
				id: Id(1),
				guild: None,
				parent_id: None,
				position: 0,
				name: "Synthetic".into(),
				kind: 1,
				recipients: Vec::new(),
				member_list_id: None,
				message_count: None,
			}],
			..Default::default()
		};
		let mut messaging = MessagingUi {
			draft_restore_pending: true,
			..Default::default()
		};
		let mut output = egui::Context::default().run_ui(Default::default(), |ui| {
			messaging.show(ui, &mut state);
		});
		output.textures_delta.clear();
		assert!(
			state.drafts.is_empty(),
			"merely viewing must not suppress asynchronous hydration"
		);
		state
			.drafts
			.entry(Id(1))
			.or_insert("saved synthetic draft".into());
		assert_eq!(state.drafts[&Id(1)], "saved synthetic draft");
		messaging.clear_draft(&mut state, Id(1));
		state
			.drafts
			.entry(Id(1))
			.or_insert("old disk snapshot".into());
		assert!(state.drafts[&Id(1)].is_empty());

		messaging.draft_restore_pending = false;
		state.drafts.retain(|_, text| !text.is_empty());
		for channel in 1..=64 {
			state.drafts.insert(Id(channel), "synthetic".into());
		}
		messaging.clear_draft(&mut state, Id(1));
		assert_eq!(
			state.drafts.len(),
			63,
			"clearing at the slot limit must free capacity"
		);
		assert_eq!(messaging.draft_changes, [Id(1), Id(1)]);
	}
}
