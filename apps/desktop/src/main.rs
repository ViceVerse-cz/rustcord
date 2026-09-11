#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod audio;
mod avatars;
mod cache;
mod clipboard;
mod connection;
mod credentials;
mod downloads;
mod game_activity;
mod reading_settings;
#[cfg(feature = "voice")]
mod screen;
mod uploads;
#[cfg(feature = "voice")]
mod voice;
use client_core::{
	Command, Envelope, Event, State,
	auth::{AuthState, Failure, SessionSecret},
};
use eframe::egui;
use model::Delivery;
use std::{sync::Arc, time::Duration};
#[cfg(feature = "developer-session")]
use zeroize::Zeroizing;

fn main() -> eframe::Result {
	let demo = std::env::args().any(|arg| arg == "--demo");
	let options = eframe::NativeOptions {
		viewport: {
			let builder = egui::ViewportBuilder::default()
				.with_inner_size([1120.0, 760.0])
				.with_min_inner_size([760.0, 520.0])
				.with_app_id("org.serein.desktop");
			if cfg!(target_os = "macos") {
				// Discord-style inline title bar: traffic lights sit over the app's own strip.
				builder
					.with_title_shown(false)
					.with_titlebar_shown(false)
					.with_fullsize_content_view(true)
			} else if cfg!(target_os = "windows") {
				// The app paints its own caption strip and buttons; see `ui::design::window_controls`.
				builder.with_decorations(false)
			} else {
				builder
			}
		},
		renderer: eframe::Renderer::Wgpu,
		persist_window: false,
		persistence_path: None,
		..Default::default()
	};
	eframe::run_native(
		"Serein",
		options,
		Box::new(move |cc| Ok(Box::new(Desktop::new(cc, demo)?))),
	)
}
/// Opt-in aggregate CPU callback timings; no payloads, per-frame logs, or repaint timer.
struct FrameMetrics {
	enabled: bool,
	started: Option<std::time::Instant>,
	frames: u64,
	inputless: u64,
	buckets: [u64; 8],
	reflows: (u64, u64),
}
impl Default for FrameMetrics {
	fn default() -> Self {
		Self {
			enabled: std::env::var_os("SEREIN_FRAME_DIAGNOSTICS").is_some_and(|v| v == "1"),
			started: None,
			frames: 0,
			inputless: 0,
			buckets: [0; 8],
			reflows: (0, 0),
		}
	}
}
impl FrameMetrics {
	fn begin(&mut self, ctx: &egui::Context) {
		if self.enabled {
			self.started = Some(std::time::Instant::now());
			self.inputless += u64::from(ctx.input(|i| i.events.is_empty()));
		}
	}
	fn finish(&mut self) {
		if let Some(started) = self.started.take() {
			let micros = started.elapsed().as_micros();
			let bucket = [1000, 2000, 4000, 8000, 16000, 32000, 64000]
				.partition_point(|limit| *limit < micros);
			self.buckets[bucket] += 1;
			self.frames += 1;
		}
	}
}
impl Drop for FrameMetrics {
	fn drop(&mut self) {
		if self.enabled {
			use std::io::Write;
			let _ = writeln!(
				std::io::stderr(),
				"[Serein frames] callbacks={} without_input={} cpu_us_buckets(1000,2000,4000,8000,16000,32000,64000,above)={:?} reflows(total,consecutive)={:?}",
				self.frames,
				self.inputless,
				self.buckets,
				self.reflows
			);
		}
	}
}
struct Desktop {
	login: Option<platform::LoginView>,
	connection: Option<connection::Connection>,
	state: State,
	messaging: ui::MessagingUi,
	downloads: downloads::Downloads,
	audio: audio::Audio,
	notifications: platform::notifications::Notifications,
	uploads: uploads::Uploads,
	clipboard: Option<clipboard::Paste>,
	download_close_pending: bool,
	window: Arc<winit::window::Window>,
	monitor_geometry: Option<(Option<egui::Rect>, Option<f32>)>,
	monitor_period: Option<Duration>,
	frame_metrics: FrameMetrics,
	avatars: Option<avatars::AvatarWorker>,
	avatar_start_failed: bool,
	avatar_clear_account: Option<model::Id>,
	avatar_cleanup: Option<std::sync::mpsc::Receiver<Result<(), &'static str>>>,
	#[cfg(feature = "voice")]
	voice: voice::Voice,
	runtime: tokio::runtime::Runtime,
	store: Option<credentials::Store>,
	cache: Option<cache::Cache>,
	cache_pending: usize,
	cache_clears: cache::HistoryClears,
	cache_error: bool,
	cache_status: &'static str,
	appearance: egui::ThemePreference,
	appearance_changed: bool,
	reading: reading_settings::ReadingSettings,
	game_activity: game_activity::Settings,
	variant_changed: bool,
	pending_save: Option<Arc<SessionSecret>>,
	credential_status: &'static str,
	forgetting: bool,
	confirming_close: bool,
	confirming_logout: bool,
	close_approved: bool,
	fixture_only: bool,
	authorized: bool,
	synthetic_id: u64,
	#[cfg(feature = "developer-session")]
	token_input: Zeroizing<String>,
}
/// Check only navigation whose effective access can change with this event.
fn access_candidates(state: &State, event: &Event) -> Vec<model::Id> {
	use client_core::permissions::Event as Permission;
	let (guilds, channel): (Option<Vec<model::Id>>, Option<model::Id>) = match event {
		Event::Ready { .. } | Event::Permissions(Permission::Snapshot(_)) => (None, None),
		Event::Permissions(permission) => match permission {
			Permission::Guild(guild) => (Some(vec![guild.id]), None),
			Permission::Role { guild, .. }
			| Permission::RoleRemoved { guild, .. }
			| Permission::Member { guild, .. }
			| Permission::Owner { guild, .. }
			| Permission::UnavailableGuild(guild) => (Some(vec![*guild]), None),
			Permission::Members(members) => (Some(members.iter().map(|m| m.0).collect()), None),
			Permission::Channel { channel, .. } => (None, Some(*channel)),
			Permission::Snapshot(_) => unreachable!(),
		},
		Event::ChannelCreated(channel) | Event::ChannelRestored(channel) => {
			// A new channel cannot revoke existing access. Duplicate IDs can replace metadata.
			if state.channel(channel.id).is_none() {
				return Vec::new();
			}
			(None, Some(channel.id))
		}
		Event::ChannelChanged(patch) | Event::ThreadChanged { patch, .. } => (None, Some(patch.id)),
		Event::ThreadRemoved { id, .. } => (None, Some(*id)),
		Event::UserAction(client_core::user_actions::Event::Written {
			action: client_core::user_actions::Action::CloseDm(channel),
			result: Ok(()),
			..
		}) => (None, Some(*channel)),
		Event::ThreadsSync { guild, .. } => (Some(vec![*guild]), None),
		_ => return Vec::new(),
	};
	state
		.channels
		.iter()
		.filter(|c| {
			c.supports_text()
				&& channel.is_none_or(|id| c.id == id || c.parent_id == Some(id))
				&& guilds
					.as_ref()
					.is_none_or(|ids| c.guild.is_some_and(|id| ids.contains(&id)))
		})
		.map(|c| c.id)
		.collect()
}
fn wants_cached_history(state: &State, channel: model::Id, request: u64) -> bool {
	state.selected == Some(channel)
		&& state.request == request
		&& state.history_pending
		&& state.freshness == model::Freshness::Loading
		&& state.timeline.row_count() == 0
		&& state.can_read_history(channel)
		&& state
			.channels
			.iter()
			.any(|c| c.id == channel && c.supports_text())
}
fn hydrate_cached_history(
	state: &mut State,
	channel: model::Id,
	request: u64,
	messages: Vec<model::Message>,
) {
	if wants_cached_history(state, channel, request)
		&& messages.iter().all(|message| message.channel == channel)
		&& state.timeline.seed_cache(messages).is_ok()
	{
		state.revision += 1;
	}
	state.enforce_resident_budget();
}
fn hydrate_cache_result(state: &mut State, safety: &cache::HistorySafety, outcome: cache::Outcome) {
	if let cache::Outcome::Channel {
		channel,
		request,
		messages,
		epoch,
	} = outcome
		&& safety.allows(epoch)
	{
		hydrate_cached_history(state, channel, request, messages);
	}
}

fn recovery_draft(state: &State, channel: model::Id) -> String {
	state
		.drafts
		.get(&channel)
		.filter(|text| !text.is_empty())
		.cloned()
		.or_else(|| {
			state
				.pending
				.iter()
				.rev()
				.find(|pending| {
					pending.channel == channel && pending.delivery != Delivery::Confirmed
				})
				.map(|pending| pending.content.clone())
		})
		.unwrap_or_default()
}
fn confirmed_recovery_channel(state: &State, event: &Event) -> Option<model::Id> {
	let (message, nonce) = match event {
		Event::SendResult {
			result: Ok(message),
			nonce,
		} => (message, nonce.as_str()),
		Event::Message(message) => (message, message.nonce.as_deref()?),
		_ => return None,
	};
	if state.user.as_ref().map(|user| user.id) != Some(message.author.id) {
		return None;
	}
	state
		.pending
		.iter()
		.any(|pending| pending.channel == message.channel && pending.nonce == nonce)
		.then_some(message.channel)
}
fn changes_active_history(state: &State, event: &Event) -> bool {
	let channel = match event {
		Event::History {
			channel, request, ..
		} if *request == state.request && state.history_pending => channel,
		Event::Message(message)
		| Event::SendResult {
			result: Ok(message),
			..
		} => &message.channel,
		Event::Patch(patch) => &patch.channel,
		Event::Edited { channel, .. }
		| Event::Delete { channel, .. }
		| Event::DeleteBulk { channel, .. } => channel,
		_ => return false,
	};
	state.selected == Some(*channel)
}
/// Synthetic People rows with presence; never a Discord member directory.
fn demo_members(guild: Option<model::Id>, channel: model::Id, request: u64) -> model::MemberList {
	let mut members = vec![
		model::Member {
			user: test_support::message(2, channel).author,
			nick: None,
			roles: if guild.is_some() {
				vec![model::Id(9001)]
			} else {
				vec![]
			},
			status: Some("idle".into()),
			custom_status: None,
			activities: vec![],
		},
		model::Member {
			user: test_support::message(1, channel).author,
			nick: None,
			roles: if guild.is_some() {
				vec![model::Id(9002)]
			} else {
				vec![]
			},
			status: Some("online".into()),
			custom_status: Some("🌙 semifluent in synthetic data".into()),
			activities: vec![model::RichActivity {
				kind: 0,
				name: "Stardew Valley".into(),
				details: Some("Tending the synthetic farm".into()),
				state: Some("Spring - Day 12".into()),
				image: Some(model::ActivityImage::Asset {
					application: model::Id(9001),
					asset: model::Id(9002),
				}),
			}],
		},
	];
	if guild.is_some() {
		for (id, name, status) in [
			(9003, "Alex (synthetic)", "online"),
			(9004, "Sam (synthetic)", "offline"),
		] {
			let mut member = members[0].clone();
			member.user.id = model::Id(id);
			member.user.name = name.into();
			member.roles.clear();
			member.status = Some(status.into());
			members.push(member);
		}
	}
	model::MemberList {
		guild,
		channel,
		request,
		total: members.len() as u64,
		rows: members.into_iter().map(Some).collect(),
		freshness: model::Freshness::Fresh,
	}
}

impl Desktop {
	fn new(
		cc: &eframe::CreationContext<'_>,
		demo: bool,
	) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
		ui::fonts::install(&cc.egui_ctx);
		ui::emoji::install_async(&cc.egui_ctx)?;
		ui::icons::install(&cc.egui_ctx);
		if demo {
			// Fixture-only preset preview, e.g. `--demo --demo-theme=onyx --demo-light`.
			if let Some(variant) = std::env::args()
				.find_map(|arg| arg.strip_prefix("--demo-theme=").map(str::to_owned))
				.and_then(|key| ui::design::Variant::from_key(&key))
			{
				ui::design::set_variant(variant);
			}
		}
		ui::design::apply(&cc.egui_ctx);
		cc.egui_ctx.set_theme(
			if demo && std::env::args().any(|arg| arg == "--demo-light") {
				egui::ThemePreference::Light
			} else {
				egui::ThemePreference::System
			},
		);
		let runtime = tokio::runtime::Builder::new_multi_thread()
			.worker_threads(2)
			.enable_all()
			.build()?;
		let mut store = (!demo).then(|| credentials::Store::start(cc.egui_ctx.clone()));
		let cache = (!demo).then(|| cache::Cache::start(cc.egui_ctx.clone()));
		let mut state = if demo {
			if std::env::args().any(|arg| arg == "--demo-audio") {
				test_support::audio_demo_state()
			} else if std::env::args().any(|arg| arg == "--demo-system-messages") {
				test_support::system_demo_state()
			} else if std::env::args().any(|arg| arg == "--demo-notifications") {
				test_support::notification_demo_state()
			} else if std::env::args().any(|arg| arg == "--demo-voice") {
				test_support::voice_demo_state()
			} else if std::env::args().any(|arg| arg == "--demo-call") {
				test_support::call_demo_state()
			} else if std::env::args().any(|arg| arg == "--demo-chat") {
				test_support::chat_demo_state()
			} else {
				test_support::demo_state()
			}
		} else {
			State::default()
		};
		if demo {
			let fixture = demo_members(None, model::Id(22), 0);
			state.direct_presences = fixture
				.rows
				.into_iter()
				.flatten()
				.filter(|member| member.user.id != model::Id(1))
				.map(|member| model::MemberPresence {
					user: member.user.id,
					status: member.status,
					custom_status: member.custom_status,
					activities: member.activities,
				})
				.collect();
			// Synthetic role metadata exercises the same bounded permission mirror as live events.
			for guild in state.permissions.guilds.values_mut() {
				if let Some(roles) = &mut guild.roles {
					roles.extend([
						model::permissions::Role {
							id: model::Id(9001),
							bits: 0,
							name: "Founders".into(),
							color: 0xe78284,
							position: 2,
							hoist: true,
						},
						model::permissions::Role {
							id: model::Id(9002),
							bits: 0,
							name: "Community".into(),
							color: 0xe5c769,
							position: 1,
							hoist: true,
						},
					]);
				}
			}
		}
		let loading_saved = store
			.as_mut()
			.is_some_and(|store| store.load(state.generation, std::time::Instant::now()));
		let mut cache_pending = usize::from(cache.as_ref().is_some_and(|cache| {
			// Appearance has its own singleton table; this account ID is unused.
			cache.queue(
				state.generation,
				model::Id(0),
				cache::Operation::LoadAppearance,
			)
		}));
		let mut reading = reading_settings::ReadingSettings::default();
		let mut game_activity = game_activity::Settings::default();
		if cache.as_ref().is_some_and(|cache| {
			cache.queue(
				state.generation,
				model::Id(0),
				cache::Operation::LoadGameActivity,
			)
		}) {
			cache_pending += 1;
		} else if !demo {
			game_activity.failed = true;
		}
		if let Some(cache) = &cache {
			if cache.queue(
				state.generation,
				model::Id(0),
				cache::Operation::LoadReadingPreferences,
			) {
				cache_pending += 1;
			} else {
				reading.restore(Err(local_store::StoreError::Unavailable));
			}
		}
		let synthetic_id = state
			.timeline
			.iter()
			.last()
			.map_or(10_000, |m| m.id.0.max(10_000));
		let mut messaging = ui::MessagingUi::default();
		if demo && std::env::args().any(|arg| arg == "--demo-game-activity") {
			messaging.share_game_activity = true;
			messaging.own_game = Some("Playing osu!".into());
		}
		messaging.build = ui::design::Build {
			channel: if cfg!(debug_assertions) {
				ui::design::Channel::Dev
			} else if option_env!("SEREIN_CHANNEL") == Some("nightly") {
				ui::design::Channel::Nightly
			} else {
				ui::design::Channel::Stable
			},
			version: env!("CARGO_PKG_VERSION"),
		};
		messaging.notification_test_available =
			demo && std::env::args().any(|arg| arg == "--demo-system-notifications");
		if messaging.notification_test_available {
			state.status = "Offline fixture · explicit system notification test";
		}
		if demo
			&& let Some(page) = std::env::args().find_map(|arg| {
				arg.strip_prefix("--demo-settings")
					.map(|rest| rest.trim_start_matches('=').to_lowercase())
			}) {
			// `--demo-settings` or `--demo-settings=account` etc.
			messaging.preview_settings(&page);
		}
		if demo && std::env::args().any(|arg| arg == "--demo-profile") {
			// Presence for the fixture card comes from the same synthetic People rows.
			let _ = state.request_members();
			if let Some(list) = &state.members {
				state.members = Some(demo_members(list.guild, list.channel, list.request));
			}
			let user = if messaging.share_game_activity {
				state.user.clone().expect("demo has a current user")
			} else {
				test_support::message(1, model::Id(20)).author
			};
			messaging.preview_profile(user);
			state.status = "Offline fixture · synthetic profile card opened at startup";
		}
		if demo && std::env::args().any(|arg| arg == "--demo-pins") {
			messaging.preview_pins();
			state.status = "Offline fixture · pinned messages popout opened at startup";
		}
		if demo && std::env::args().any(|arg| arg == "--demo-emoji") {
			messaging.preview_emoji_picker();
			state.status = "Offline fixture · emoji popout opened at startup";
		}
		if demo && std::env::args().any(|arg| arg == "--demo-viewer") {
			// Opens the full-window media viewer on the fixture gallery message.
			messaging.preview_image_viewer(model::Id(500), model::Id(700));
			state.status = "Offline fixture · media viewer opened at startup";
		}
		if demo && std::env::args().any(|arg| arg == "--demo-attachment=file") {
			// Non-image variant: exercises the file-kind glyph and extension badge.
			messaging.preview_attachment("quarterly-report.pdf", 1_482_311, None);
			state.status = "Offline fixture · synthetic file attachment staged in the composer";
		} else if demo
			&& std::env::args()
				.any(|arg| arg == "--demo-attachment" || arg == "--demo-attachment=multi")
		{
			// Synthetic gradients stand in for decoded photos; no file is read or uploaded.
			let gradient = |width: usize, height: usize, hue: f32| {
				let pixels = (0..width * height)
					.map(|index| {
						let (x, y) = (
							(index % width) as f32 / width as f32,
							(index / width) as f32 / height as f32,
						);
						let ring = ((x - 0.65).powi(2) + (y - 0.4).powi(2)).sqrt();
						if ring < 0.12 {
							egui::Color32::from_rgb(255, 214, 102)
						} else {
							egui::Color32::from_rgb(
								(40.0 + 120.0 * y * hue) as u8,
								(110.0 + 90.0 * x) as u8,
								(190.0 - 60.0 * y / hue) as u8,
							)
						}
					})
					.collect();
				egui::ColorImage {
					size: [width, height],
					source_size: egui::vec2(width as f32, height as f32),
					pixels,
				}
			};
			messaging.preview_attachment(
				"synthetic-holiday.png",
				2_437_120,
				Some(gradient(320, 200, 1.0)),
			);
			if std::env::args().any(|arg| arg == "--demo-attachment=multi") {
				// Batch variant: a portrait photo plus a document, like dropping three files.
				messaging.preview_attachment(
					"synthetic-portrait.png",
					1_106_944,
					Some(gradient(180, 320, 1.6)),
				);
				messaging.preview_attachment("synthetic-agenda.pdf", 482_311, None);
				state.status =
					"Offline fixture · three synthetic attachments staged in the composer";
			} else {
				state.status = "Offline fixture · synthetic attachment staged in the composer";
			}
		}
		if demo && std::env::args().any(|arg| arg == "--demo-sending") {
			messaging.preview_sending(&cc.egui_ctx, &mut state);
			state.status = "Offline fixture · synthetic pending message; no upload or send";
		}
		// `--demo-gifs`, `--demo-gifs=favorites`, `--demo-gifs=trending` or `--demo-gifs=<query>`.
		if demo
			&& let Some(section) = std::env::args().find_map(|arg| {
				arg.strip_prefix("--demo-gifs")
					.map(|rest| rest.strip_prefix('=').unwrap_or("").to_owned())
			}) {
			let favorites: Vec<_> = test_support::gif_page(None)
				.gifs
				.into_iter()
				.skip(2)
				.take(5)
				.collect();
			state.restore_gif_favorites(favorites);
			messaging.preview_gif_picker(&section);
			state.status = "Offline fixture · GIF popout opened at startup";
		}
		if demo
			&& let Some(query) = std::env::args()
				.find_map(|arg| arg.strip_prefix("--demo-search=").map(str::to_owned))
		{
			messaging.preview_search(&query);
			state.status = "Offline fixture · synthetic search opened at startup";
		}
		if demo
			&& std::env::args().any(|arg| arg == "--demo-browsing")
			&& let Some(channel) = state.selected
		{
			// Fixture-only: an unread marker on the oldest loaded message plus a targeted history
			// page, so both timeline overlays render without pointer input.
			let oldest = state.timeline.iter().next().map(|message| message.id);
			let _ = state.apply_read_state(client_core::read_state::Event::Snapshot {
				entries: Some(vec![(channel, oldest, 0)]),
				version: Some(1),
				partial: false,
			});
			state.history_targeted = true;
			state.status = "Offline fixture · unread strip and older-messages bar shown";
		}
		if demo && std::env::args().any(|arg| arg == "--demo-login") {
			// Fixture-only: render the sign-in screen without a session.
			state.user = None;
			state.status = "Disconnected";
		}
		Ok(Self {
			login: None,
			connection: None,
			state,
			messaging,
			downloads: downloads::Downloads::default(),
			audio: audio::Audio::default(),
			notifications: {
				let wake = cc.egui_ctx.clone();
				platform::notifications::Notifications::new(move || wake.request_repaint())
			},
			uploads: uploads::Uploads::default(),
			clipboard: None,
			download_close_pending: false,
			window: cc
				.winit_window()
				.ok_or("Native window unavailable")?
				.clone(),
			monitor_geometry: None,
			monitor_period: None,
			frame_metrics: FrameMetrics::default(),
			avatars: None,
			avatar_cleanup: None,
			avatar_start_failed: false,
			avatar_clear_account: None,
			#[cfg(feature = "voice")]
			voice: voice::Voice::default(),
			runtime,
			store,
			cache,
			cache_pending,
			cache_clears: Default::default(),
			cache_error: false,
			cache_status: "Loading local appearance…",
			appearance: egui::ThemePreference::System,
			appearance_changed: false,
			reading,
			game_activity,
			variant_changed: false,
			pending_save: None,
			credential_status: if demo {
				"Fixture mode never opens the credential store or network"
			} else if loading_saved {
				"Checking saved login…"
			} else {
				"Saved login unavailable; could not start credential lookup"
			},
			forgetting: false,
			confirming_close: false,
			confirming_logout: false,
			close_approved: false,
			fixture_only: demo,
			authorized: false,
			synthetic_id,
			#[cfg(feature = "developer-session")]
			token_input: Zeroizing::new(String::new()),
		})
	}
	fn connect(&mut self, secret: SessionSecret, save: bool, ctx: &egui::Context) {
		if let Some(store) = &mut self.store {
			store.cancel_load();
		}
		self.credential_status = if save {
			"Login will be saved after Discord connects"
		} else {
			"Connecting with the supplied session; saved login unchanged"
		};
		#[cfg(feature = "voice")]
		self.voice.stop();
		self.state.disconnect_voice();
		self.uploads.cancel();
		self.login = None;
		self.connection = None;
		if let Some(worker) = self.avatars.take() {
			self.avatar_cleanup = Some(worker.shutdown());
		}
		self.avatar_start_failed = false;
		self.messaging.clear_avatars();
		self.state.generation += 1;
		self.messaging.draft_restore_pending = false;
		self.state.auth = AuthState::Authenticating;
		self.state.status = "Connecting to Discord…";
		let secret = Arc::new(secret);
		self.pending_save = save.then(|| secret.clone());
		self.connection = Some(connection::Connection::start(
			self.runtime.handle(),
			secret,
			self.state.generation,
			self.state.user.as_ref().map(|u| u.id),
			ctx.clone(),
		));
	}
	fn logout(&mut self, ctx: &egui::Context) {
		self.notifications.clear();
		self.uploads.cancel();
		if let Some(store) = &mut self.store {
			store.cancel_load();
		}
		self.downloads.cancel();
		self.audio.stop();
		#[cfg(feature = "voice")]
		self.voice.stop();
		let was_demo = self.state.demo;
		self.clear_avatars(ctx);
		self.login = None;
		self.connection = None;
		self.pending_save = None;
		let old_account = self.state.user.as_ref().filter(|_| !was_demo).map(|u| u.id);
		self.state.logout();
		if let (Some(cache), Some(account)) = (&self.cache, old_account) {
			if cache.queue(self.state.generation, account, cache::Operation::Forget) {
				self.cache_pending += 1;
			} else {
				self.cache_error = true;
				self.cache_status = "Could not queue local account data removal";
			}
		}
		self.messaging.clear();
		self.messaging.share_game_activity = self.game_activity.enabled;
		ctx.memory_mut(|m| *m = egui::Memory::default());
		ui::design::apply(ctx);
		ctx.set_theme(self.appearance);
		self.messaging
			.apply_reading_preferences(ctx, self.reading.current);
		ctx.clear_animations();
		#[cfg(feature = "developer-session")]
		{
			self.token_input = Zeroizing::new(String::new());
		}
		if !was_demo && let Some(store) = &self.store {
			self.forgetting = store
				.send
				.try_send((self.state.generation, credentials::Operation::Forget))
				.is_ok();
			self.credential_status = if self.forgetting {
				"Removing saved login…"
			} else {
				"Credential queue unavailable; saved login may remain"
			};
		}
		self.confirming_logout = false;
	}
	fn queue_cache(&mut self, operation: cache::Operation) -> bool {
		if let Some(user) = &self.state.user {
			if matches!(operation, cache::Operation::ClearHistory) {
				self.request_history_clear(user.id);
				return true;
			}
			self.queue_cache_for(user.id, operation)
		} else {
			false
		}
	}
	fn queue_cache_for(&mut self, account: model::Id, operation: cache::Operation) -> bool {
		if self.state.demo || self.fixture_only {
			return false;
		}
		if let Some(cache) = &self.cache {
			if matches!(
				operation,
				cache::Operation::LoadChannel { .. }
					| cache::Operation::SaveChannel { .. }
					| cache::Operation::SaveChanges { .. }
			) && !cache.history.allows(cache.history.epoch())
			{
				return false;
			}
			if cache.queue(self.state.generation, account, operation) {
				self.cache_pending += 1;
				if !self.cache_error {
					self.cache_status = "Saving local changes…";
				}
				return true;
			} else {
				self.cache_error = true;
				self.cache_status = "Local storage queue full; some changes are not saved";
			}
		}
		false
	}
	fn save_reading_preferences(&mut self, ctx: &egui::Context) {
		if self.fixture_only {
			return;
		}
		let now = std::time::Instant::now();
		// Finish changes made before entering preview; never persist preview controls.
		if !self.state.demo {
			self.reading
				.observe(self.messaging.reading_preferences, now);
			if std::mem::take(&mut self.messaging.reading_save_requested) {
				self.reading.request_save(now);
			}
		}
		if self.reading.ready(now) {
			let accepted = self.cache.as_ref().is_some_and(|cache| {
				cache.queue(
					self.state.generation,
					model::Id(0),
					cache::Operation::SaveReadingPreferences(self.reading.current),
				)
			});
			self.reading.queued(accepted);
			self.cache_pending += usize::from(accepted);
		}
		if let Some(delay) = self.reading.remaining(now) {
			ctx.request_repaint_after(delay);
		}
		self.messaging.reading_status = self.reading.status();
	}
	fn sync_game_activity(&mut self, ctx: &egui::Context) {
		let previous = (
			self.messaging.own_game.clone(),
			self.messaging.game_activity_status,
		);
		if self.state.demo {
			let activity = self
				.messaging
				.share_game_activity
				.then(game_activity::demo_activity);
			self.messaging.own_game = activity.as_ref().map(model::RichActivity::summary);
			let changed = self.state.set_local_game_activity(activity);
			self.messaging.game_activity_status =
				"Offline preview: synthetic activity, never shared or saved.";
			if changed
				|| previous
					!= (
						self.messaging.own_game.clone(),
						self.messaging.game_activity_status,
					) {
				ctx.request_repaint();
			}
			return;
		}
		if self.fixture_only {
			return;
		}
		self.game_activity
			.observe(self.messaging.share_game_activity);
		if self.game_activity.dirty && !self.game_activity.saving {
			let accepted = self.cache.as_ref().is_some_and(|cache| {
				cache.queue(
					self.state.generation,
					model::Id(0),
					cache::Operation::SaveGameActivity(self.game_activity.enabled),
				)
			});
			self.game_activity.dirty = false;
			self.game_activity.saving = accepted;
			self.game_activity.failed = !accepted;
			self.cache_pending += usize::from(accepted);
		}
		self.messaging.own_game = None;
		let mut own_activity = None;
		self.messaging.game_activity_status = self.game_activity.status();
		if let Some(connection) = &self.connection {
			connection.share_activity.send_if_modified(|enabled| {
				if *enabled == self.game_activity.enabled {
					return false;
				}
				*enabled = self.game_activity.enabled;
				true
			});
			if self.game_activity.enabled && self.state.gateway_connected {
				match &*connection.game_activity.borrow() {
					Ok(game) => {
						self.messaging.own_game = game.as_ref().map(model::RichActivity::summary);
						own_activity = game.clone();
					}
					Err(error) => self.messaging.game_activity_status = error,
				}
			}
		}
		let changed = self.state.set_local_game_activity(own_activity);
		if changed
			|| previous
				!= (
					self.messaging.own_game.clone(),
					self.messaging.game_activity_status,
				) {
			ctx.request_repaint();
		}
	}
	fn request_history_clear(&mut self, account: model::Id) {
		if self.state.demo || self.fixture_only {
			return;
		}
		let Some(cache) = &self.cache else {
			return;
		};
		cache.history.invalidate();
		cache.history.block();
		if !self.cache_clears.request(account) {
			cache.history.fail();
			self.cache_error = true;
			self.cache_status = "Cache cleanup backlog exceeded; history cache disabled until restart; deleted messages may remain on disk";
			return;
		}
		if !self.cache_error {
			self.cache_status =
				"Waiting to clear cached history; cached history temporarily disabled";
		}
		self.retry_history_clears();
	}
	fn retry_history_clears(&mut self) {
		let Some(cache) = &self.cache else {
			return;
		};
		for _ in 0..16 {
			let Some(account) = self.cache_clears.next() else {
				break;
			};
			if !cache.queue(
				self.state.generation,
				account,
				cache::Operation::ClearHistory,
			) {
				break;
			}
			self.cache_clears.queued(account);
			self.cache_pending += 1;
		}
	}
	fn delete_cached_messages(&mut self, event: &Event) {
		if self.state.demo || self.fixture_only {
			return;
		}
		let Some(account) = self.state.user.as_ref().map(|user| user.id) else {
			return;
		};
		let (channel, ids) = match event {
			Event::Delete { channel, id } => (*channel, vec![*id]),
			Event::DeleteBulk { channel, ids } if !ids.is_empty() && ids.len() <= 100 => {
				(*channel, ids.clone())
			}
			Event::DeleteBulk { ids, .. } if ids.len() > 100 => {
				self.request_history_clear(account);
				return;
			}
			_ => return,
		};
		self.delete_cached_ids(channel, ids);
	}
	fn delete_cached_ids(&mut self, channel: model::Id, ids: Vec<model::Id>) {
		if self.state.demo || self.fixture_only || ids.is_empty() {
			return;
		}
		let Some(account) = self.state.user.as_ref().map(|user| user.id) else {
			return;
		};
		let Some(cache) = &self.cache else {
			return;
		};
		if cache.delete_messages(self.state.generation, account, channel, ids) {
			self.cache_pending += 1;
		} else {
			self.request_history_clear(account);
		}
	}
	fn command(&mut self, command: Command) {
		if let Command::Send { channel, nonce, .. } = &command
			&& self
				.state
				.pending
				.iter()
				.any(|p| p.nonce == *nonce && !p.attachments.is_empty())
		{
			let (channel, nonce) = (*channel, nonce.clone());
			let available = !self.state.demo
				&& self.state.can_attach(channel)
				&& !self.fixture_only
				&& self.state.auth == AuthState::Authenticated
				&& self.state.gateway_connected
				&& self.state.freshness == model::Freshness::Fresh
				&& self.state.selected == Some(channel)
				&& self.connection.is_some();
			if available
				&& let Some(source) = self.uploads.take_source(self.state.generation, channel)
			{
				let (progress, receive) =
					tokio::sync::watch::channel(discord_api::upload::Status::Preparing);
				let (cancel, _) = tokio::sync::watch::channel(false);
				if self.uploads.begin_upload(receive, cancel.clone()).is_ok() {
					let request = uploads::UploadRequest {
						command,
						source,
						progress,
						cancel,
					};
					self.messaging.attachment = None;
					if let Err(error) = self.connection.as_ref().unwrap().uploads.try_send(request)
					{
						let request = error.into_inner();
						request
							.progress
							.send_replace(discord_api::upload::Status::Failed(
								"Upload queue full; reselect the file",
							));
						self.state.command_rejected(request.command);
					}
					return;
				}
			}
			self.state.apply(Envelope {
				generation: self.state.generation,
				event: Event::SendResult {
					nonce,
					result: Err(Failure::ProtocolAt(
						"File not sent; reconnect and reselect the attachment",
					)),
				},
			});
			return;
		}
		if let Command::Voice(control) = &command {
			if self.state.demo || self.fixture_only {
				self.state.status = "Voice calls are unavailable in the offline preview";
				return;
			}
			if let client_core::voice::Command::Join {
				channel,
				request,
				ring,
			} = control
			{
				#[cfg(feature = "voice")]
				let result = self.voice.begin(&self.state, *ring);
				#[cfg(not(feature = "voice"))]
				let _ = ring;
				#[cfg(not(feature = "voice"))]
				let result: Result<(), &'static str> =
					Err("This is the text-only build; use the voice build to call");
				if let Err(message) = result {
					self.state.apply_voice(client_core::voice::Event::Failed {
						channel: *channel,
						request: *request,
						message,
					});
					return;
				}
			}
			#[cfg(feature = "voice")]
			if matches!(
				control,
				client_core::voice::Command::SetCamera { enabled: false, .. }
			) {
				self.voice.stop_camera();
				self.messaging.voice_camera_preview = None;
			}
			#[cfg(feature = "voice")]
			if matches!(control, client_core::voice::Command::Leave { .. }) {
				self.voice.stop();
			}
		}
		if let Command::History {
			channel,
			before: None,
			after: None,
			request,
		} = &command
			&& wants_cached_history(&self.state, *channel, *request)
		{
			self.queue_cache(cache::Operation::LoadChannel {
				channel: *channel,
				request: *request,
			});
		}
		if self.state.demo {
			let event = match command {
				Command::GuildFolders(settings) => {
					Event::GuildFolders(Ok(settings.unwrap_or_default()))
				}
				Command::UserAction { action, request } => {
					Event::UserAction(client_core::user_actions::Event::Written {
						action,
						request,
						result: Ok(()),
					})
				}
				Command::MarkRead {
					channel,
					message,
					request,
				} => Event::ReadState(client_core::read_state::Event::Result {
					channel,
					message,
					request,
					result: Ok(()),
				}),
				Command::Reactions(command) => {
					use client_core::reactions::{Command as R, Event as E};
					Event::Reactions(match command {
						R::Read {
							channel,
							message,
							request,
						} => E::Read {
							channel,
							message,
							request,
							result: Ok(vec![]),
						},
						R::Set {
							channel,
							message,
							emoji,
							add,
							request,
						} => {
							let mut reactions = self
								.state
								.timeline
								.get(message)
								.and_then(|m| m.reactions.clone())
								.unwrap_or_default();
							if let Some(r) = reactions.iter_mut().find(|r| r.emoji.same(&emoji)) {
								if r.me != add {
									r.count = if add {
										r.count + 1
									} else {
										r.count.saturating_sub(1)
									};
									r.me = add;
								}
							} else if add {
								reactions.push(model::Reaction {
									emoji,
									count: 1,
									me: true,
									me_burst: false,
								});
							}
							reactions.retain(|r| r.count > 0);
							self.state.reactions.reset();
							let _ = self.state.timeline.set_reactions(message, Some(reactions));
							// The fixture has no service readback; it updates synthetic RAM only.
							E::Written {
								channel,
								message,
								request,
								result: Ok(()),
							}
						}
					})
				}
				Command::Voice(_) | Command::CancelProfile | Command::CancelSearch => return,
				Command::CreatePost {
					parent,
					guild,
					title,
					request,
					..
				} => {
					self.synthetic_id += 1;
					Event::PostCreated {
						parent,
						request,
						result: Ok(model::Channel {
							id: model::Id(self.synthetic_id),
							guild: Some(guild),
							parent_id: Some(parent),
							position: 0,
							name: title,
							kind: 11,
							recipients: vec![],
							last_message: None,
							member_list_id: None,
							message_count: Some(0),
						}),
					}
				}
				Command::Archives {
					parent,
					guild,
					kind,
					before,
					request,
				} => {
					use model::archives::{Cursor, Kind, Page};
					let offset = parent.0.saturating_mul(10_000).saturating_add(match kind {
						Kind::Public => 0,
						Kind::Private => 1_000,
						Kind::JoinedPrivate => 2_000,
					});
					let ids = (if before.is_none() {
						[900, 850, 800]
					} else {
						[700, 650, 600]
					})
					.map(|id| offset.saturating_add(id));
					let public_kind = if self
						.state
						.channels
						.iter()
						.any(|c| c.id == parent && c.kind == 5)
					{
						10
					} else {
						11
					};
					let threads = ids
						.into_iter()
						.map(|id| model::Channel {
							id: model::Id(id),
							guild: Some(guild),
							parent_id: Some(parent),
							position: 0,
							name: format!("Synthetic archived thread {id}"),
							kind: if kind == Kind::Public {
								public_kind
							} else {
								12
							},
							recipients: vec![],
							last_message: None,
							member_list_id: None,
							message_count: None,
						})
						.collect();
					Event::Archives {
						parent,
						request,
						result: Ok(Page {
							threads,
							next: before.is_none().then_some(if kind == Kind::JoinedPrivate {
								Cursor::Id(model::Id(ids[2]))
							} else {
								Cursor::Time(1_788_998_400_000_000_000)
							}),
						}),
					}
				}
				Command::Pins {
					channel,
					before,
					request,
				} => {
					// Explicit synthetic pins, independent of message creation order.
					let hits = if before.is_none() {
						[480, 499, 470]
					} else {
						[420, 455, 430]
					}
					.into_iter()
					.map(|id| {
						let message = test_support::message(id, channel);
						model::SearchHit {
							id: message.id,
							channel,
							author: message.author.name,
							excerpt: format!(
								"Synthetic pinned message: {}",
								message.content.chars().take(200).collect::<String>()
							),
						}
					})
					.collect();
					Event::Search {
						channel,
						request,
						result: Ok(client_core::search::Outcome::Pins(model::SearchPage {
							hits,
							total: 0,
							partial: before.is_none(),
							pin_cursor: before.is_none().then_some(1_788_998_400_000_000_000),
						})),
					}
				}
				Command::Search {
					channel,
					query,
					before,
					request,
					..
				} => {
					let mut hits = Vec::new();
					let mut total = 0;
					for id in (1..=500)
						.rev()
						.filter(|id| before.is_none_or(|b| *id < b.0))
					{
						let message = test_support::message(id, channel);
						if message
							.content
							.to_lowercase()
							.contains(&query.to_lowercase())
						{
							total += 1;
							if hits.len() < model::SEARCH_PAGE_SIZE {
								hits.push(model::SearchHit {
									id: message.id,
									channel,
									author: message.author.name,
									excerpt: message.content.chars().take(256).collect(),
								});
							}
						}
					}
					Event::Search {
						channel,
						request,
						result: Ok(client_core::search::Outcome::Page(model::SearchPage {
							hits,
							total,
							partial: false,
							pin_cursor: None,
						})),
					}
				}
				Command::Gifs { query, request } => Event::Gifs {
					request,
					result: Ok(test_support::gif_page(query.as_deref())),
				},
				Command::CancelGifs => return,
				Command::JoinInvite { request, .. } => Event::JoinInvite {
					request,
					result: Err(Failure::ProtocolAt("Server joining unavailable offline")),
				},
				Command::Invite { code } => Event::Invite {
					code,
					result: Err(Failure::Protocol),
				},
				Command::Profile {
					user,
					guild,
					request,
				} => Event::Profile {
					user,
					guild,
					request,
					result: Err(Failure::Protocol),
				},
				Command::Members {
					guild,
					channel,
					request,
					..
				} => {
					let Some(channel) = channel else {
						return;
					};
					Event::Members(demo_members(guild, channel, request))
				}
				Command::History { before, after, .. } => {
					test_support::load_page_with_cursors(&mut self.state, before, after);
					return;
				}
				Command::Send {
					channel,
					content,
					nonce,
					reply,
				} => {
					self.synthetic_id += 1;
					let mut message = test_support::message(self.synthetic_id, channel);
					message.author = self.state.user.clone().unwrap();
					message.content = content;
					message.nonce = Some(nonce.clone());
					message.reply_to = reply;
					Event::SendResult {
						nonce,
						result: Ok(message),
					}
				}
				Command::Edit {
					request,
					channel,
					message,
					content,
				} => {
					let result = self
						.state
						.timeline
						.get(message)
						.cloned()
						.ok_or(Failure::Protocol)
						.map(|mut updated| {
							updated.content = content;
							updated.edited = true;
							updated.edited_at =
								Some(updated.edited_at.unwrap_or(0).saturating_add(1));
							updated
						});
					Event::Edited {
						request,
						channel,
						message,
						result,
					}
				}
				Command::Delete { channel, message } => Event::Delete {
					channel,
					id: message,
				},
				Command::Pin {
					request,
					channel,
					message,
					pinned,
				} => Event::Pinned {
					request,
					channel,
					message,
					pinned,
					result: Ok(()),
				},
			};
			self.state.apply(Envelope {
				generation: self.state.generation,
				event,
			});
			self.state.status = "Offline fixture · action affected synthetic RAM only";
		} else if let Some(connection) = &self.connection {
			if let Err(error) = connection.commands.try_send(command) {
				self.state.command_rejected(error.into_inner());
			}
		} else {
			self.state.command_rejected(command);
		}
	}
	fn sign_in_screen(&mut self, ui: &mut egui::Ui) {
		let p = ui::design::palette(ui);
		egui::CentralPanel::default()
			.frame(egui::Frame::NONE.fill(p.canvas))
			.show(ui, |ui| {
				// Soft radial accent glow behind the card instead of a flat canvas.
				let rect = ui.max_rect();
				let glow = rect.center() - egui::vec2(0.0, rect.height() * 0.1);
				let radius = rect.width().max(rect.height()) * 0.55;
				let mut mesh = egui::Mesh::default();
				let alpha = if ui.visuals().dark_mode { 0.16 } else { 0.10 };
				mesh.colored_vertex(glow, p.accent.gamma_multiply(alpha));
				const SEGMENTS: u32 = 48;
				for i in 0..=SEGMENTS {
					let angle = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
					mesh.colored_vertex(
						glow + egui::vec2(angle.cos(), angle.sin()) * radius,
						egui::Color32::TRANSPARENT,
					);
				}
				for i in 1..=SEGMENTS {
					mesh.add_triangle(0, i, i + 1);
				}
				ui.painter().add(egui::Shape::mesh(mesh));
				let header = egui::Frame::NONE
					.inner_margin(egui::Margin {
						left: (16.0 + ui::design::TRAFFIC_LIGHT_INSET) as i8,
						right: if ui::design::WINDOW_CONTROLS_WIDTH > 0.0 {
							0
						} else {
							24
						},
						top: if ui::design::WINDOW_CONTROLS_WIDTH > 0.0 {
							0
						} else {
							18
						},
						bottom: 12,
					})
					.show(ui, |ui| {
						ui.horizontal(|ui| {
							ui.label(ui::design::semibold(ui, "Serein", 20.0).color(p.text_strong));
							ui.add_space(6.0);
							egui::Frame::NONE
								.fill(p.accent.gamma_multiply(0.16))
								.corner_radius(4)
								.inner_margin(egui::Margin::symmetric(6, 2))
								.show(ui, |ui| {
									ui.label(
										ui::design::semibold(ui, "EARLY PREVIEW", 10.0)
											.color(p.accent),
									);
								});
							ui.with_layout(
								egui::Layout::right_to_left(egui::Align::Center),
								|ui| {
									ui::design::window_controls(ui);
									ui.add_space(12.0);
									ui.menu_button("Appearance", |ui| {
										egui::widgets::global_theme_preference_buttons(ui);
										self.messaging.reading_settings(
											ui,
											self.fixture_only || self.state.demo,
										);
									});
								},
							);
						});
					});
				ui::design::window_drag(ui, header.response.rect);
				egui::ScrollArea::vertical()
					.id_salt("sign-in-scroll")
					.show(ui, |ui| {
						ui.add_space(((ui.available_height() - 560.0) * 0.4).max(8.0));
						ui.vertical_centered(|ui| {
							ui.allocate_ui_with_layout(
								egui::vec2(ui.available_width().min(460.0), 0.0),
								egui::Layout::top_down(egui::Align::Min),
								|ui| self.sign_in_card(ui),
							);
							ui.add_space(20.0);
							ui.label(
								egui::RichText::new(
									"Independent and open source. Not affiliated with Discord.",
								)
								.size(12.0)
								.color(p.muted),
							);
							ui.add_space(24.0);
						});
					});
			});
	}
	fn sign_in_card(&mut self, ui: &mut egui::Ui) {
		let ctx = ui.ctx().clone();
		let p = ui::design::palette(ui);
		let shadow = egui::epaint::Shadow {
			offset: [0, 8],
			blur: 24,
			spread: 0,
			color: egui::Color32::from_black_alpha(if ui.visuals().dark_mode { 90 } else { 30 }),
		};
		egui::Frame::NONE
			.fill(p.surface)
			.stroke(egui::Stroke::new(1.0, p.border))
			.shadow(shadow)
			.corner_radius(12)
			.inner_margin(32)
			.show(ui, |ui| {
				ui.vertical_centered(|ui| {
					let (rect, _) = ui.allocate_exact_size(egui::vec2(56.0, 56.0), egui::Sense::hover());
					ui.painter().rect_filled(rect, 14, p.accent);
					ui::icons::paint(ui.painter(), ui::icons::Icon::Discord, rect.shrink(13.0), p.accent_text);
					ui.add_space(16.0);
					ui.label(ui::design::semibold(ui, "Welcome to Serein", 24.0).color(p.text_strong));
					ui.add_space(6.0);
					ui.label(egui::RichText::new("Sign in with your Discord account to pick up where you left off.").size(15.0).color(p.muted));
				});
				ui.add_space(24.0);
				let can_sign_in = !self.fixture_only && self.authorized && !self.forgetting && self.state.auth != AuthState::Authenticating;
				let label = if self.state.auth == AuthState::Authenticating { "Waiting for Discord…" } else { "Continue with Discord" };
				let button = ui
					.add_enabled_ui(can_sign_in, |ui| {
						ui::design::primary_icon_button(ui, ui::icons::Icon::Discord, label)
					})
					.inner;
				if button.clicked() {
					if let Some(store) = &mut self.store { store.cancel_load(); }
					self.credential_status = "Sign in through Discord; saved-login lookup stopped";
					let wake = ctx.clone();
					match platform::LoginView::open(self.window.clone(), move || wake.request_repaint()) {
						Ok(login) => { self.login = Some(login); self.state.auth = AuthState::Authenticating; self.state.status = "Waiting for Discord login"; }
						Err(_) => { self.state.auth = AuthState::Failed; self.state.status = "Platform login webview unavailable; see platform-support.md"; }
					}
				}
				ui.add_space(12.0);
				ui.add_enabled_ui(!self.fixture_only, |ui| {
					ui.horizontal_wrapped(|ui| {
						ui.spacing_mut().item_spacing.x = 8.0;
						ui.checkbox(&mut self.authorized, "");
						ui.label(egui::RichText::new("I own this account and authorize this session.").size(13.0).color(p.text));
					});
				});
				ui.add_space(6.0);
				ui.label(egui::RichText::new("Discord's own login page opens inside Serein. Passwords and 2FA stay there; only the session token is kept, in your OS credential store.").size(12.0).color(p.muted));
				// Status: one banner, only when something is happening or went wrong.
				let attention = matches!(self.state.auth, AuthState::Failed | AuthState::Expired | AuthState::Challenged);
				let busy = self.state.auth == AuthState::Authenticating || self.forgetting || self.cache_pending > 0 || self.cache_clears.pending();
				let show_status = attention || busy || self.state.status != "Disconnected" || (!self.fixture_only && self.credential_status != "Checking saved login…" && !self.credential_status.is_empty());
				if show_status && !(self.fixture_only && self.state.status == "Disconnected") {
					ui.add_space(14.0);
					let (fill, color) = if attention { (p.warning.gamma_multiply(0.14), p.warning) } else { (p.raised, p.text) };
					egui::Frame::NONE.fill(fill).corner_radius(8).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
						ui.set_width(ui.available_width());
						if self.state.status != "Disconnected" || attention {
							ui.label(egui::RichText::new(self.state.status).size(13.0).color(color));
						}
						if !self.fixture_only && !self.credential_status.is_empty() {
							ui.label(egui::RichText::new(self.credential_status).size(12.0).color(p.muted));
						}
						if self.cache_error || self.cache_pending > 0 || self.cache_clears.pending() {
							ui.label(egui::RichText::new(self.cache_status).size(12.0).color(p.muted));
						}
					});
				}
				ui.add_space(22.0);
				ui.horizontal(|ui| {
					let y = ui.cursor().top() + 8.0;
					let left = ui.cursor().left();
					let width = ui.available_width();
					let galley = ui.painter().layout_no_wrap("or".into(), egui::FontId::proportional(12.0), p.muted);
					let text_w = galley.size().x + 20.0;
					ui.painter().hline(left..=left + (width - text_w) * 0.5, y, egui::Stroke::new(1.0, p.border));
					ui.painter().galley(egui::pos2(left + (width - galley.size().x) * 0.5, y - galley.size().y * 0.5), galley, p.muted);
					ui.painter().hline(left + (width + text_w) * 0.5..=left + width, y, egui::Stroke::new(1.0, p.border));
					ui.allocate_space(egui::vec2(width, 16.0));
				});
				ui.add_space(14.0);
				if ui::design::secondary_button(ui, "Explore the offline preview").clicked() {
					if let Some(store) = &mut self.store { store.cancel_load(); }
					self.connection = None;
					self.pending_save = None;
					let generation = self.state.generation + 1;
					self.state = test_support::demo_state();
					self.state.generation = generation;
					self.messaging.clear();
				}
				ui.add_space(8.0);
				ui.vertical_centered(|ui| {
					ui.label(egui::RichText::new("Sample conversations. No Discord connection.").size(12.0).color(p.muted));
				});
				ui.add_space(16.0);
				ui.collapsing("About this preview", |ui| {
						ui.small("Messaging, reactions, search and read markers have offline tests. Real Discord interoperability is still unverified; attachment uploads and advanced search remain incomplete.");
						ui.small("Messages and drafts are cached locally. Login tokens use the operating system credential store.");
						ui.small("Unofficial clients may put your Discord account at risk.");
						if !self.fixture_only {
							ui.small(self.credential_status);
							if ui.button("Forget saved login").clicked() { self.logout(&ctx); }
						}
					});
				#[cfg(feature = "developer-session")]
				if !self.fixture_only {
					ui.collapsing("Developer session", |ui| {
						ui.add(egui::TextEdit::singleline(&mut *self.token_input).password(true).char_limit(2048).hint_text("Owner-supplied test credential"));
						if ui.add_enabled(self.authorized, egui::Button::new("Connect imported session (RAM only)")).clicked() {
							let input = std::mem::take(&mut *self.token_input);
							match SessionSecret::from_owner_input(input) { Ok(secret) => self.connect(secret, false, &ctx), Err(f) => self.state.status = f.label() }
						}
					});
				}
			});
	}
	fn clear_avatars(&mut self, ctx: &egui::Context) {
		self.avatar_start_failed = false;
		if self.avatar_cleanup.is_some() && !self.fixture_only && !self.state.demo {
			self.avatar_clear_account = self.state.user.as_ref().map(|user| user.id);
		}
		// Logout may follow expiry, when the downloading worker was already stopped.
		if self.avatars.is_none()
			&& self.avatar_cleanup.is_none()
			&& !self.fixture_only
			&& !self.state.demo
			&& let Some(user) = &self.state.user
		{
			match avatars::AvatarWorker::start(&self.runtime, user.id, ctx.clone()) {
				Ok(worker) => self.avatars = Some(worker),
				Err(error) => {
					self.cache_error = true;
					self.cache_status = error;
				}
			}
		}
		if let Some(worker) = self.avatars.take() {
			self.avatar_cleanup = Some(worker.shutdown_and_clear());
		}
		self.messaging.clear_avatars();
	}
	fn poll_avatars(&mut self, ctx: &egui::Context) {
		if let Some(cleanup) = &self.avatar_cleanup {
			match cleanup.try_recv() {
				Ok(result) => {
					self.avatar_cleanup = None;
					if let Err(error) = result {
						self.cache_error = true;
						self.cache_status = error;
					}
				}
				Err(std::sync::mpsc::TryRecvError::Disconnected) => {
					self.avatar_cleanup = None;
					self.cache_error = true;
					self.cache_status =
						"Avatar cache cleanup failed; cached pictures may remain on disk";
				}
				Err(std::sync::mpsc::TryRecvError::Empty) => {}
			}
		}
		if self.avatar_cleanup.is_none()
			&& let Some(account) = self.avatar_clear_account.take()
		{
			match avatars::AvatarWorker::start(&self.runtime, account, ctx.clone()) {
				Ok(worker) => self.avatar_cleanup = Some(worker.shutdown_and_clear()),
				Err(error) => {
					self.cache_error = true;
					self.cache_status = error;
				}
			}
		}
		if self.fixture_only || self.state.demo || self.state.auth != AuthState::Authenticated {
			if let Some(worker) = self.avatars.take() {
				self.avatar_cleanup = Some(worker.shutdown());
			}
			return;
		}
		if !self.avatar_start_failed
			&& self.avatars.is_none()
			&& self.avatar_cleanup.is_none()
			&& let Some(user) = &self.state.user
		{
			match avatars::AvatarWorker::start(&self.runtime, user.id, ctx.clone()) {
				Ok(worker) => {
					self.messaging.clear_avatars();
					self.avatars = Some(worker);
				}
				Err(error) => {
					self.avatar_start_failed = true;
					self.cache_error = true;
					self.cache_status = error;
				}
			}
		}
		if let Some(worker) = &mut self.avatars {
			for _ in 0..8 {
				let Some(result) = worker.poll() else {
					break;
				};
				if let Some(error) = result.error {
					self.cache_error = true;
					self.cache_status = error;
				}
				self.messaging
					.accept_avatar(ctx, result.key.clone(), result.image);
				self.messaging
					.accept_gif_animation(result.key, result.frames);
			}
		}
	}
	#[cfg(feature = "voice")]
	fn poll_voice(&mut self, ctx: &egui::Context) {
		if let Some(command) =
			self.voice
				.poll(&self.runtime, &mut self.state, &mut self.messaging, ctx)
		{
			self.command(command);
		}
	}
	fn poll(&mut self, ctx: &egui::Context) {
		let mut cached = Vec::new();
		if let Some(cache) = &self.cache {
			for _ in 0..16 {
				match cache.receive.try_recv() {
					Ok(value) => cached.push(value),
					Err(_) => break,
				}
			}
		}
		for (generation, outcome) in cached {
			self.cache_pending = self.cache_pending.saturating_sub(1);
			// Settings are global; account removal/write failures still matter after logout.
			match &outcome {
				cache::Outcome::GameActivity(result) => {
					self.game_activity.restore(*result);
					if !self.state.demo && !self.fixture_only {
						self.messaging.share_game_activity = self.game_activity.enabled;
					}
					continue;
				}
				cache::Outcome::GameActivitySaved(result) => {
					self.game_activity.saving = false;
					self.game_activity.failed = result.is_err();
					continue;
				}
				cache::Outcome::ReadingPreferences(result) => {
					if let Some(value) = self.reading.restore(*result)
						&& !self.state.demo
						&& !self.fixture_only
					{
						self.messaging.apply_reading_preferences(ctx, value);
					}
					continue;
				}
				cache::Outcome::ReadingPreferencesSaved(result) => {
					self.reading.saved(*result);
					continue;
				}
				cache::Outcome::Appearance(appearance, variant) => {
					if !self.state.demo && !self.appearance_changed {
						self.appearance = match appearance {
							local_store::Appearance::System => egui::ThemePreference::System,
							local_store::Appearance::Light => egui::ThemePreference::Light,
							local_store::Appearance::Dark => egui::ThemePreference::Dark,
						};
						ctx.set_theme(self.appearance);
					}
					if !self.state.demo && !self.variant_changed {
						// Unknown keys from a newer build fall back to the default preset.
						let variant = variant
							.as_deref()
							.and_then(ui::design::Variant::from_key)
							.unwrap_or_default();
						ui::design::set_variant(variant);
						ui::design::apply(ctx);
					}
					continue;
				}
				cache::Outcome::Failed {
					error,
					message,
					draft_restore,
					history_cleanup,
				} => {
					if *history_cleanup && let Some(cache) = &self.cache {
						self.cache_clears.acknowledge(&cache.history);
					}
					let _ = error;
					self.cache_error = true;
					self.cache_status = message;
					if *draft_restore && generation == self.state.generation {
						self.messaging.draft_restore_pending = false;
						self.state.drafts.retain(|_, content| !content.is_empty());
					}
					continue;
				}
				cache::Outcome::HistoryCleared => {
					if let Some(cache) = &self.cache
						&& self.cache_clears.acknowledge(&cache.history)
						&& !self.cache_error
					{
						if cache.history.allows(cache.history.epoch()) {
							self.cache_status = "Cached history cleared; saved drafts preserved";
						} else {
							self.cache_status = "Requested history cleanup completed; history cache remains disabled until restart after a storage failure";
						}
					}
					continue;
				}
				_ => {}
			}
			if generation != self.state.generation {
				continue;
			}
			match outcome {
				cache::Outcome::GifFavorites(favorites) => {
					self.state.restore_gif_favorites(favorites);
				}
				cache::Outcome::Drafts(drafts) => {
					for (channel, content) in drafts {
						if !self
							.state
							.pending
							.iter()
							.any(|pending| pending.channel == channel)
						{
							self.state.drafts.entry(channel).or_insert(content);
						}
					}
					self.messaging.draft_restore_pending = false;
					self.state.drafts.retain(|_, content| !content.is_empty());
					if !self.cache_error {
						self.cache_status = "Saved drafts restored; check the conversation before resending recovered text";
					}
				}
				outcome @ cache::Outcome::Channel { .. } => {
					if let Some(cache) = &self.cache {
						hydrate_cache_result(&mut self.state, &cache.history, outcome);
					}
				}
				cache::Outcome::Saved => {
					if !self.cache_error {
						self.cache_status = "Local changes saved";
					}
				}
				cache::Outcome::Appearance(..)
				| cache::Outcome::GameActivity(_)
				| cache::Outcome::GameActivitySaved(_)
				| cache::Outcome::ReadingPreferences(_)
				| cache::Outcome::ReadingPreferencesSaved(_)
				| cache::Outcome::HistoryCleared
				| cache::Outcome::Failed { .. } => unreachable!(),
			}
		}
		self.retry_history_clears();
		let mut results = Vec::new();
		if let Some(store) = &mut self.store {
			for _ in 0..4 {
				match store.poll(std::time::Instant::now()) {
					Some(result) => results.push(result),
					None => break,
				}
			}
			if let Some(remaining) = store.remaining(std::time::Instant::now()) {
				ctx.request_repaint_after(remaining);
			}
		}
		for (generation, outcome) in results {
			if generation != self.state.generation {
				continue;
			}
			match outcome {
				credentials::Outcome::Loaded(result) => {
					let status = credentials::loaded_status(&result);
					if let Ok(Some(secret)) = result {
						self.connect(secret, false, ctx);
					}
					self.credential_status = status;
				}
				credentials::Outcome::Saved(Ok(())) => {
					self.credential_status = "Login saved in the OS credential store"
				}
				credentials::Outcome::Saved(Err(_)) => {
					self.credential_status =
						"Could not save login; this session will not restore automatically"
				}
				credentials::Outcome::Forgotten(result) => {
					self.forgetting = false;
					self.credential_status = if result.is_ok() {
						"Saved login removed"
					} else {
						"Could not remove saved login; remove org.serein.desktop / discord-session in your OS credential manager"
					};
				}
			}
		}
		let mut events = Vec::new();
		let mut terminal = None;
		if let Some(connection) = &mut self.connection {
			for _ in 0..client_core::EVENT_SLOTS {
				match connection.events.try_recv() {
					Ok(event) => events.push(event),
					Err(_) => break,
				}
			}
			// Collect reliable events first: their preceding typing signals are now queued.
			// Apply typing first so messages/access changes retire those older signals.
			let reliable_count = events.len();
			for _ in 0..8 {
				match connection.typing.try_recv() {
					Ok(event) => events.push(event),
					Err(_) => break,
				}
			}
			let typing_count = events.len() - reliable_count;
			events.rotate_right(typing_count);
			terminal = *connection.terminal.borrow();
		}
		let mut persist_timeline = false;
		let mut full_window = false;
		let mut changed_messages = std::collections::BTreeSet::new();
		for mut event in events {
			if event.generation != self.state.generation {
				continue;
			}
			self.delete_cached_messages(&event.event);
			match &event.event {
				Event::Delete { channel, id } => {
					self.messaging
						.messages_deleted(ctx, *channel, std::slice::from_ref(id));
				}
				Event::DeleteBulk { channel, ids } if ids.len() <= 100 => {
					self.messaging.messages_deleted(ctx, *channel, ids);
				}
				_ => {}
			}
			#[cfg(feature = "voice")]
			let voice_failure = self.voice.observe(&self.state, &mut event.event);
			#[cfg(not(feature = "voice"))]
			let _ = &mut event;
			let ready = matches!(event.event, Event::Ready { .. });
			let resumed = matches!(event.event, Event::Resumed);
			let confirmed_channel = confirmed_recovery_channel(&self.state, &event.event);
			let mut removed_channels = access_candidates(&self.state, &event.event);
			removed_channels.retain(|id| self.state.can_read_history(*id));
			let invalidate = matches!(
				event.event,
				Event::Resync | Event::PermissionsChanged | Event::Unavailable(_)
			) || matches!(&event.event, Event::RecipientRemoved { user, .. } if self.state.user.as_ref().is_some_and(|owner| owner.id == *user))
				|| matches!(&event.event, Event::Reactions(client_core::reactions::Event::Read {channel,result:Err(Failure::Forbidden),..}) if self.state.selected==Some(*channel))
				|| matches!(&event.event, Event::HistoryFailed { channel, request, failure: Failure::Forbidden }
                if self.state.selected == Some(*channel) && self.state.request == *request && self.state.history_pending);
			let history_changed = changes_active_history(&self.state, &event.event);
			if history_changed {
				match &event.event {
					Event::Message(message)
					| Event::SendResult {
						result: Ok(message),
						..
					} => {
						changed_messages.insert(message.id);
					}
					Event::Edited { message, .. } => {
						changed_messages.insert(*message);
					}
					Event::Patch(patch) => {
						changed_messages.insert(patch.id);
					}
					_ => full_window = true,
				}
			}
			if event.generation == self.state.generation
				&& (invalidate
					|| event.event.changes_access()
					|| matches!(
						&event.event,
						Event::NotificationPreferences(_)
							| Event::UserAction(_)
							| Event::Disconnected | Event::ReadState(
							client_core::read_state::Event::Ack { .. }
						) | Event::ReadState(client_core::read_state::Event::Result {
							result: Ok(()),
							..
						})
					)) {
				self.notifications.dismiss();
			}
			self.state.apply(event);
			// Only admitted service messages can establish a deleted reply target.
			// Fence pending disk writes before the post-drain timeline snapshot is saved.
			let deleted_replies = self.state.take_reply_deletions();
			if let Some(&(channel, _)) = deleted_replies.first() {
				full_window = true;
				let ids: Vec<_> = deleted_replies.into_iter().map(|(_, id)| id).collect();
				self.messaging.messages_deleted(ctx, channel, &ids);
				self.delete_cached_ids(channel, ids);
			}
			removed_channels.retain(|id| !self.state.can_read_history(*id));
			#[cfg(feature = "voice")]
			if let Some(error) = voice_failure
				&& let Some(command) = self.voice.fail(&mut self.state, error)
			{
				self.command(command);
			}
			// ponytail: accepted navigation removals clear account-wide history;
			// add scoped disk deletion if channel churn makes refetch cost significant.
			if invalidate || !removed_channels.is_empty() {
				self.queue_cache(cache::Operation::ClearHistory);
			}
			persist_timeline |= history_changed;
			if let Some(channel) = confirmed_channel {
				let content = recovery_draft(&self.state, channel);
				self.queue_cache(cache::Operation::SaveDraft { channel, content });
			}
			if ready && self.state.auth == AuthState::Authenticated {
				// The worker survives logout; each accepted account READY restores its own drafts.
				if !self.messaging.draft_restore_pending {
					self.messaging.draft_restore_pending =
						self.queue_cache(cache::Operation::LoadDrafts);
					self.queue_cache(cache::Operation::LoadGifFavorites);
				}
				if let Some(secret) = self.pending_save.take()
					&& let Some(store) = &self.store
					&& store
						.send
						.try_send((self.state.generation, credentials::Operation::Save(secret)))
						.is_err()
				{
					self.credential_status = "Could not queue saved login; session only";
				}
			}
			if (ready || resumed)
				&& self.state.auth == AuthState::Authenticated
				&& self.state.selected.is_some_and(|selected| {
					self.state
						.channels
						.iter()
						.any(|channel| channel.id == selected && channel.supports_text())
				}) {
				let command = self.state.history(None);
				self.command(command);
			}
		}
		if persist_timeline
			&& self.state.freshness == model::Freshness::Fresh
			&& let Some(channel) = self.state.selected
			&& self.state.can_read_history(channel)
		{
			let messages = self
				.state
				.timeline
				.iter()
				.filter(|m| full_window || changed_messages.contains(&m.id))
				.cloned()
				.collect();
			let operation = if full_window {
				cache::Operation::SaveChannel { channel, messages }
			} else {
				cache::Operation::SaveChanges {
					channel,
					messages,
					retained: self.state.timeline.iter().map(|m| m.id).collect(),
				}
			};
			self.queue_cache(operation);
		}
		if let Some(failure) = terminal {
			for (setting, scope) in [
				("SEREIN_MEMBER_DIAGNOSTICS", "members"),
				("SEREIN_GATEWAY_DIAGNOSTICS", "gateway"),
			] {
				if std::env::var_os(setting).as_deref() == Some(std::ffi::OsStr::new("1")) {
					use std::io::Write;
					// One extra fixed-label terminal line per enabled scope; closed stderr is OK.
					let _ = writeln!(
						std::io::stderr(),
						"[Serein {scope}] Session stopped: {}",
						failure.label()
					);
				}
			}
			self.connection = None;
			self.pending_save = None;
			self.state.apply(Envelope {
				generation: self.state.generation,
				event: Event::Failure(failure),
			});
			if !matches!(self.state.auth, AuthState::Expired | AuthState::Challenged) {
				self.state.auth = AuthState::Failed;
			}
			for pending in &mut self.state.pending {
				if pending.delivery == Delivery::Sending {
					pending.delivery = Delivery::Ambiguous;
				}
			}
			if failure == Failure::Expired
				&& let Some(store) = &self.store
			{
				let _ = store
					.send
					.try_send((self.state.generation, credentials::Operation::Forget));
			}
		}
		if let Some(login) = &self.login {
			login.pump();
			if let Some(secret) = login.token() {
				self.connect(secret, true, ctx);
			} else if login.expired() {
				self.login = None;
				self.state.auth = AuthState::Challenged;
				self.state.status =
					"Login timed out or token handoff unavailable; no session accepted";
			}
		}
		// Network and store workers request repaint only when their outcomes change.
		if let Some(connection) = &self.connection {
			connection.set_typing_channel(self.state.typing_scope());
		}
		if !self.state.demo
			&& let Some(command) = self.state.next_reaction_read()
		{
			self.command(command);
		}

		#[cfg(target_os = "linux")]
		if self.login.is_some() {
			ctx.request_repaint_after(self.frame_period().unwrap_or(Duration::from_millis(16)));
		}
		if self.login.is_some() {
			ctx.request_repaint_after(Duration::from_secs(1));
		}
	}
}
impl Desktop {
	/// Frame period of the display the window is on; egui otherwise assumes 60 Hz.
	fn frame_period(&self) -> Option<Duration> {
		self.monitor_period
	}
	fn refresh_frame_period(&mut self) -> Option<Duration> {
		let millihertz = self.window.current_monitor()?.refresh_rate_millihertz()?;
		(1_000..=1_000_000)
			.contains(&millihertz)
			.then(|| Duration::from_secs_f64(1000.0 / f64::from(millihertz)))
	}
}
impl eframe::App for Desktop {
	fn persist_egui_memory(&self) -> bool {
		false
	}
	fn raw_input_hook(&mut self, _: &egui::Context, raw_input: &mut egui::RawInput) {
		// Viewport position/scale comes from native events; avoid an OS monitor query on paints.
		if let Some(viewport) = raw_input.viewports.get(&raw_input.viewport_id) {
			let geometry = (viewport.outer_rect, viewport.native_pixels_per_point);
			if self.monitor_geometry != Some(geometry) {
				self.monitor_geometry = Some(geometry);
				self.monitor_period = self.refresh_frame_period();
			}
		}
		if let Some(period) = self.frame_period() {
			raw_input.predicted_dt = period.as_secs_f32();
		}
	}
	fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
		self.frame_metrics.begin(ctx);
		self.messaging.sync_reading_zoom(ctx);
		self.poll(ctx);
		let (focused, hidden_or_closing, ptt_down) = ctx.input(|input| {
			(
				input.focused,
				input.viewport().visible() == Some(false) || input.viewport().close_requested(),
				input.key_down(egui::Key::V),
			)
		});
		#[cfg(not(feature = "voice"))]
		let _ = ptt_down;
		if self.state.user.is_none()
			|| (!self.state.demo && self.state.auth != AuthState::Authenticated)
			|| hidden_or_closing
		{
			self.audio.stop();
			self.messaging.audio().stop();
		}
		let audio = self.audio.poll();
		let player = self.messaging.audio();
		player.position = audio.position.as_secs_f64();
		player.duration = audio.duration.as_secs_f64();
		player.state = match audio.state {
			audio::State::Idle => ui::AudioState::Idle,
			audio::State::Loading => ui::AudioState::Loading,
			audio::State::Playing => ui::AudioState::Playing,
			audio::State::Paused => ui::AudioState::Paused,
			audio::State::Ended => ui::AudioState::Ended,
			audio::State::Failed(error) => ui::AudioState::Failed(error),
		};
		if self.state.auth != AuthState::Authenticated && !self.state.demo {
			self.notifications.clear();
			self.messaging.notifications_enabled = false;
		}
		while let Some(notification) = self.state.take_notification() {
			if !self.fixture_only
				&& self.messaging.notifications_enabled
				&& !(focused && self.messaging.viewing_latest(notification.channel))
			{
				self.notifications.notify();
			}
		}
		#[cfg(feature = "voice")]
		{
			self.messaging.voice_ptt_active =
				focused && ptt_down && !ctx.egui_wants_keyboard_input();
		}
	}
	fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
		let ctx = ui.ctx().clone();
		let (close_requested, dropped) = ctx.input_mut(|input| {
			(
				input.viewport().close_requested(),
				std::mem::take(&mut input.raw.dropped_files),
			)
		});
		ui::design::paint_backdrop(&ctx);
		let upload_allowed = self.state.user.is_some()
			&& self.state.gateway_connected
			&& self.state.freshness == model::Freshness::Fresh;
		let can_attach = self
			.state
			.selected
			.is_some_and(|channel| self.state.can_attach(channel));
		if let Some(paste) = &self.clipboard
			&& let Some(result) = paste.poll()
		{
			if paste.generation == self.state.generation
				&& Some(paste.channel) == self.state.selected
				&& self.state.user.is_some()
				&& !self.messaging.has_edit()
			{
				match result {
					Ok(clipboard::Content::Text(text)) => {
						self.messaging.pasted_text = Some((paste.channel, paste.target, text));
					}
					Ok(clipboard::Content::File(source)) if upload_allowed && can_attach => {
						if let Err(error) = self.uploads.select_pasted(
							paste.generation,
							paste.channel,
							source,
							self.runtime.handle(),
							&ctx,
						) {
							self.state.status = error;
						}
					}
					Ok(clipboard::Content::File(..)) => {
						self.state.status = "Attaching files is unavailable here"
					}
					Err(error) => self.state.status = error,
				}
			}
			self.clipboard = None;
		}
		if !can_attach {
			self.uploads.cancel();
		}
		self.uploads.poll(
			self.state.generation,
			self.state.selected,
			self.state.user.is_some() && self.state.gateway_connected,
			&ctx,
		);
		// Move native handles once; never load dropped bytes on the rendering thread.
		if !dropped.is_empty() {
			if upload_allowed
				&& can_attach
				&& self.login.is_none()
				&& !self.confirming_close
				&& !self.confirming_logout
				&& !self.messaging.has_edit()
				&& !self.downloads.is_active()
				&& let Some(channel) = self.state.selected
			{
				if let Err(error) = self.uploads.start_drop(
					self.state.generation,
					channel,
					self.runtime.handle(),
					&ctx,
					dropped,
				) {
					self.state.status = error;
				}
			} else {
				self.state.status =
					"File not attached; return to a connected conversation and drop it again";
			}
		}
		// Offline fixtures may stage a synthetic attachment without any upload selection.
		if !self.state.demo || self.uploads.selection().is_some() {
			self.messaging.attachment = self
				.uploads
				.selection()
				.map(|(name, size)| (name.to_owned(), size));
			self.messaging.attachment_previews = self.uploads.previews();
			self.messaging.attachment_files = self.uploads.files();
		}
		self.messaging.upload_busy = self.uploads.busy() || self.clipboard.is_some();
		self.messaging.upload_status = self.uploads.status();
		if !self.state.demo {
			let (progress, sending) = self.uploads.transfer_progress();
			self.messaging.update_upload_progress(progress, sending);
		}
		if self.state.user.is_none() {
			self.downloads.cancel();
		}

		let download_status = match self.downloads.poll() {
			downloads::Status::Idle => String::new(),
			downloads::Status::Choosing => "Choose where to save the attachment…".into(),
			downloads::Status::Downloading { received, total } => {
				format!("Downloading: {} / {} KiB", received / 1024, total / 1024)
			}
			downloads::Status::Saved | downloads::Status::Cancelled => String::new(),
			downloads::Status::Failed(error) => (*error).into(),
		};
		self.messaging.downloads().active = self.downloads.is_active();
		self.messaging.downloads().status = download_status;
		if close_requested && self.downloads.is_active() {
			self.downloads.cancel();
			self.download_close_pending = true;
			ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
		}
		if self.download_close_pending
			&& !self.downloads.is_active()
			&& (!self.confirming_close || self.close_approved)
		{
			self.download_close_pending = false;
			ctx.send_viewport_cmd(egui::ViewportCommand::Close);
		}
		self.poll_avatars(&ctx);
		self.messaging.voice_available = cfg!(feature = "voice");
		if close_requested
			&& !self.close_approved
			&& ((self.state.demo && self.state.has_unsent())
				|| self
					.state
					.pending
					.iter()
					.any(|p| p.delivery != Delivery::Confirmed)
				|| self.messaging.has_edit()
				|| self.uploads.has_unsent()
				|| self.forgetting
				|| self.avatar_cleanup.is_some()
				|| self.cache_pending > 0
				|| self.cache_clears.pending()
				|| (!self.fixture_only && self.reading.needs_attention())
				|| (!self.fixture_only && self.game_activity.needs_attention())
				|| self.cache_error)
		{
			ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
			self.confirming_close = true;
		}
		if self.login.is_some() {
			let p = ui::design::palette(ui);
			egui::Panel::top("login-header")
				.exact_size(platform::LOGIN_HEADER_HEIGHT)
				.show_separator_line(false)
				.frame(
					egui::Frame::NONE
						.fill(p.surface)
						.stroke(egui::Stroke::new(1.0, p.border))
						.inner_margin(egui::Margin::symmetric(16, 0)),
				)
				.show(ui, |ui| {
					ui::design::window_drag(ui, ui.max_rect());
					ui.horizontal_centered(|ui| {
						ui.add_space(ui::design::TRAFFIC_LIGHT_INSET);
						let (rect, _) =
							ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::hover());
						ui.painter().rect_filled(rect, 8, p.accent);
						ui::icons::paint(
							ui.painter(),
							ui::icons::Icon::Discord,
							rect.shrink(7.0),
							p.accent_text,
						);
						ui.add_space(4.0);
						ui.vertical(|ui| {
							ui.spacing_mut().item_spacing.y = 1.0;
							ui.label(
								ui::design::semibold(ui, "Sign in to Discord", 15.0)
									.color(p.text_strong),
							);
							ui.label(
								egui::RichText::new(
									"discord.com · temporary login window · passwords and 2FA never leave the page",
								)
								.size(12.0)
								.color(p.muted),
							);
						});
						ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
							ui::design::window_controls(ui);
							if ui
								.add(
									egui::Button::new(
										ui::design::medium(ui, "Cancel", 13.0).color(p.text_strong),
									)
									.fill(p.raised)
									.stroke(egui::Stroke::new(1.0, p.border))
									.corner_radius(6)
									.min_size(egui::vec2(0.0, 32.0)),
								)
								.clicked()
							{
								self.login = None;
								self.state.auth = AuthState::Unauthenticated;
							}
						});
					});
				});
			egui::CentralPanel::default()
				.frame(egui::Frame::NONE.fill(p.canvas))
				.show(ui, |ui| {
					ui.centered_and_justified(|ui| {
						ui.label(egui::RichText::new("Loading discord.com…").color(p.muted));
					});
				});
			if let Some(login) = &self.login {
				login.resize(&self.window);
			}
		} else if self.state.user.is_some() {
			self.messaging.storage_status = self.cache_status;
			self.messaging.notification_status = self.notifications.status().label();
			let commands = self.messaging.show(ui, &mut self.state);
			let player = self.messaging.audio();
			if !player.seen
				|| player.active.is_none()
				|| (!self.state.demo && self.state.auth != AuthState::Authenticated)
			{
				self.audio.stop();
				player.stop();
			}
			if let Some(command) = player.command.take() {
				match command {
					ui::AudioCommand::Play(attachment) => {
						self.audio.volume(player.volume);
						if let Err(error) = self.audio.start(
							attachment,
							self.runtime.handle(),
							&ctx,
							self.fixture_only || self.state.demo,
						) {
							player.state = ui::AudioState::Failed(error);
						}
					}
					ui::AudioCommand::Pause(paused) => self.audio.pause(paused),
					ui::AudioCommand::Seek(seconds) => {
						self.audio.seek(Duration::from_secs_f64(seconds))
					}
					ui::AudioCommand::Volume(volume) => self.audio.volume(volume),
					ui::AudioCommand::Stop => self.audio.stop(),
				}
			}
			self.notifications.set_enabled(
				self.messaging.notifications_enabled
					&& (!self.fixture_only || self.messaging.notification_test_available),
			);
			if std::mem::take(&mut self.messaging.notification_test_requested)
				&& self.messaging.notification_test_available
			{
				self.notifications.notify();
			}
			// Revalidate scope after navigation without polling workers a second time.
			self.uploads.revalidate_scope(
				self.state.generation,
				self.state.selected,
				self.state.user.is_some() && self.state.gateway_connected,
			);
			if !self
				.state
				.selected
				.is_some_and(|channel| self.state.can_attach(channel))
			{
				self.uploads.cancel();
			}
			if let Some(index) = self.messaging.remove_attachment_index.take() {
				self.uploads.remove_at(index);
				if self.state.demo {
					self.messaging.attachment = None;
					self.messaging.attachment_files.clear();
					self.messaging.attachment_previews.clear();
				}
			}
			if std::mem::take(&mut self.messaging.remove_attachment_requested) {
				self.uploads.remove();
				self.messaging.attachment = None;
				self.messaging.attachment_files.clear();
				self.messaging.attachment_previews.clear();
			}
			if std::mem::take(&mut self.messaging.cancel_upload_requested) {
				self.uploads.cancel();
			}
			if let Some(request) = self.messaging.attachment_paste_requested.take()
				&& let Some(channel) = self.state.selected
			{
				if self.clipboard.is_none() {
					self.clipboard = Some(clipboard::Paste::start(
						self.state.generation,
						channel,
						request,
						self.runtime.handle(),
						&ctx,
					));
				} else {
					self.state.status = "Wait for the current paste to finish";
				}
			}
			if std::mem::take(&mut self.messaging.attach_requested)
				&& let Some(channel) = self.state.selected
				&& self.state.can_attach(channel)
				&& let Err(error) = self.uploads.start_choose(
					self.state.generation,
					channel,
					self.runtime.handle(),
					&ctx,
					self.window.clone(),
				) {
				self.state.status = error;
			}
			if std::mem::take(&mut self.messaging.downloads().cancel_requested) {
				self.downloads.cancel();
			}
			if let Some(attachment) = self.messaging.downloads().request.take()
				&& !self.state.demo
				&& !self.fixture_only
				&& let Err(error) = self.downloads.start(
					attachment,
					self.runtime.handle(),
					&ctx,
					self.window.clone(),
				) {
				self.state.status = error;
			}

			for key in self.messaging.take_avatar_requests() {
				if !self
					.avatars
					.as_ref()
					.is_some_and(|worker| worker.request(key.clone()))
				{
					self.messaging.accept_avatar(&ctx, key, None);
				}
			}
			if self.messaging.reconnect_requested {
				if let Some(store) = &mut self.store {
					store.cancel_load();
				}
				self.messaging.reconnect_requested = false;
				let wake = ctx.clone();
				match platform::LoginView::open(self.window.clone(), move || wake.request_repaint())
				{
					Ok(login) => self.login = Some(login),
					Err(_) => self.state.status = "Platform login webview unavailable",
				}
			}
			let draft_changes = std::mem::take(&mut self.messaging.draft_changes);
			for channel in draft_changes {
				let content = recovery_draft(&self.state, channel);
				self.queue_cache(cache::Operation::SaveDraft { channel, content });
			}
			if self.messaging.clear_cache_requested {
				self.messaging.clear_cache_requested = false;
				self.state.clear_cached_history();
				self.clear_avatars(&ctx);
				self.queue_cache(cache::Operation::ClearHistory);
			}
			for command in commands {
				self.command(command);
			}
			#[cfg(feature = "voice")]
			self.poll_voice(&ctx);
			if self.messaging.logout_requested {
				self.messaging.logout_requested = false;
				if self.state.has_unsent() || self.messaging.has_edit() || self.uploads.has_unsent()
				{
					self.confirming_logout = true;
				} else {
					self.logout(&ctx);
				}
			}
		} else {
			self.sign_in_screen(ui);
		}
		let appearance = ctx.options(|options| options.theme_preference);
		self.save_reading_preferences(&ctx);
		self.sync_game_activity(&ctx);
		if appearance != self.appearance {
			self.appearance = appearance;
			self.appearance_changed = true;
			let preference = match appearance {
				egui::ThemePreference::System => local_store::Appearance::System,
				egui::ThemePreference::Light => local_store::Appearance::Light,
				egui::ThemePreference::Dark => local_store::Appearance::Dark,
			};
			self.queue_cache_for(model::Id(0), cache::Operation::SaveAppearance(preference));
		}
		if std::mem::take(&mut self.state.gifs.favorites_changed) {
			let favorites = self.state.gifs.favorites.clone();
			self.queue_cache(cache::Operation::SaveGifFavorites(favorites));
		}
		if let Some(variant) = self.messaging.theme_variant_changed.take() {
			self.variant_changed = true;
			let key = (variant != ui::design::Variant::Standard).then(|| variant.key().to_owned());
			self.queue_cache_for(model::Id(0), cache::Operation::SaveThemeVariant(key));
		}
		if self.confirming_close || self.confirming_logout {
			egui::Window::new("Leave this session?").collapsible(false).show(&ctx,|ui|{
                ui.label("Saved text drafts survive exit; selected files must be reselected. Logout removes local account data. Edits and uncertain sends need your attention.");
                if self.forgetting{ui.label("Wait for saved-login removal to finish.");}
                if self.cache_clears.pending(){ui.label("Cached history cleanup is pending; closing now may leave deleted messages on disk.");}
                if !self.fixture_only && self.reading.needs_attention(){ui.label(self.reading.status());}
				if !self.fixture_only && self.game_activity.needs_attention(){ui.label(self.game_activity.status());}
                ui.horizontal(|ui|{
                    if ui.button("Keep working").clicked(){self.confirming_close=false;self.confirming_logout=false;self.download_close_pending=false;}
                    if ui.add_enabled(!self.forgetting,egui::Button::new("Discard and continue")).clicked(){
                        if self.confirming_close{self.close_approved=true;self.uploads.cancel();if self.downloads.is_active(){self.downloads.cancel();self.download_close_pending=true;}else{ctx.send_viewport_cmd(egui::ViewportCommand::Close);}}else{self.logout(&ctx);}
                    }
                });
            });
		}
		self.frame_metrics.reflows = self.messaging.timeline_reflows();
		self.frame_metrics.finish();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn disk_cache_does_not_replace_resident_previews_or_deleted_positions() {
		for deleted in [false, true] {
			let mut state = test_support::demo_state();
			let alpha = state.selected.unwrap();
			let beta = model::Id(900);
			let mut other = state
				.channels
				.iter()
				.find(|c| c.id == alpha)
				.unwrap()
				.clone();
			other.id = beta;
			other.guild = None;
			other.kind = 1;
			state.channels.push(other);
			state.timeline.clear();
			state
				.timeline
				.insert(test_support::message(1001, alpha), false, false)
				.unwrap();
			if deleted {
				state.timeline.delete(model::Id(1001)).unwrap();
			}
			state.select(beta).unwrap();
			state.apply(Envelope {
				generation: state.generation,
				event: Event::History {
					channel: beta,
					request: state.request,
					older: false,
					messages: vec![test_support::message(1002, beta)],
				},
			});
			state.select(alpha).unwrap();
			let request = state.request;
			assert_eq!(state.timeline.row_count(), 1);
			assert!(!wants_cached_history(&state, alpha, request));
			hydrate_cached_history(
				&mut state,
				alpha,
				request,
				vec![test_support::message(1003, alpha)],
			);
			assert_eq!(
				state.timeline.row_ids().collect::<Vec<_>>(),
				[model::Id(1001)]
			);
			assert_eq!(state.timeline.is_empty(), deleted);
			assert_eq!(state.freshness, model::Freshness::Loading);
			state.clear_cached_history();
			assert_eq!(
				state.timeline.row_count(),
				1,
				"Clear keeps the displayed conversation"
			);
			state.select(beta).unwrap();
			assert_eq!(
				state.timeline.row_count(),
				0,
				"Cleared dormant windows cannot return"
			);
			assert!(wants_cached_history(&state, beta, state.request));
		}
	}
	#[test]
	fn delayed_cache_results_cannot_hydrate_after_a_known_deletion() {
		let mut state = test_support::demo_state();
		let channel = state.selected.unwrap();
		state.timeline.clear();
		state.history(None);
		let request = state.request;
		let safety = cache::HistorySafety::default();
		let outcome = |epoch| cache::Outcome::Channel {
			channel,
			request,
			epoch,
			messages: vec![test_support::message(1000, channel)],
		};
		let old_epoch = safety.epoch();
		safety.invalidate();
		hydrate_cache_result(&mut state, &safety, outcome(old_epoch));
		assert!(state.timeline.is_empty());
		safety.block();
		hydrate_cache_result(&mut state, &safety, outcome(safety.epoch()));
		assert!(state.timeline.is_empty());
		safety.cleared();
		hydrate_cache_result(&mut state, &safety, outcome(safety.epoch()));
		assert_eq!(state.timeline.len(), 1);
	}

	#[test]
	fn cached_history_requires_current_readable_navigation_and_pending_request() {
		let mut state = test_support::demo_state();
		let channel = state.selected.unwrap();
		state.timeline.clear();
		state.history(None);
		let request = state.request;
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert_eq!(state.timeline.len(), 1);
		assert_eq!(state.freshness, model::Freshness::Loading);
		state.timeline.clear();
		let permissions = state.permissions.clone();
		state.permissions.channels.remove(&channel);
		state.permissions.clear_cache();
		assert!(!state.can_read_history(channel));
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert!(
			state.timeline.is_empty(),
			"Loaded navigation alone does not authorize cached history"
		);
		state.permissions = permissions;
		for invalid in [
			vec![test_support::message(1001, model::Id(999))],
			vec![test_support::message(1002, channel)],
		] {
			hydrate_cached_history(&mut state, channel, request.wrapping_sub(1), invalid);
			assert!(state.timeline.is_empty());
		}
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1001, model::Id(999))],
		);
		assert!(state.timeline.is_empty());
		state.history_pending = false;
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert!(state.timeline.is_empty());
		state.history_pending = true;
		let mut channels = state.channels.clone();
		channels.retain(|c| c.id != channel);
		state.apply(Envelope {
			generation: state.generation,
			event: Event::Ready {
				user: state.user.clone().unwrap(),
				guilds: state.guilds.clone(),
				permissions: test_support::permission_snapshot(&state),
				channels,
			},
		});
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert!(state.timeline.is_empty());
		// Even an inconsistent queued-cache admission state cannot bypass current navigation.
		state.selected = Some(channel);
		state.request = request;
		state.freshness = model::Freshness::Loading;
		state.history_pending = true;
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert!(state.timeline.is_empty());
		let mut restored = test_support::demo_state()
			.channels
			.into_iter()
			.find(|c| c.id == channel)
			.unwrap();
		restored.kind = 2;
		state.channels.push(restored);
		hydrate_cached_history(
			&mut state,
			channel,
			request,
			vec![test_support::message(1000, channel)],
		);
		assert!(state.timeline.is_empty());
	}
	#[test]
	fn correlated_confirmation_preserves_other_pending_text_and_ignores_unrelated_events() {
		let mut state = test_support::demo_state();
		let channel = state.selected.unwrap();
		state.drafts.insert(channel, "first pending draft".into());
		let Command::Send { nonce, .. } = state.prepare_send().unwrap() else {
			panic!()
		};
		state.drafts.insert(channel, "second pending draft".into());
		state.prepare_send().unwrap();
		let mut message = test_support::message(1000, channel);
		message.author = state.user.clone().unwrap();
		message.nonce = Some(nonce.clone());
		let mut foreign = message.clone();
		foreign.author.id = model::Id(999);
		assert!(confirmed_recovery_channel(&state, &Event::Message(foreign)).is_none());
		let confirmation = Event::SendResult {
			nonce,
			result: Ok(message),
		};
		assert_eq!(
			confirmed_recovery_channel(&state, &confirmation),
			Some(channel)
		);
		state.apply(Envelope {
			generation: state.generation,
			event: confirmation,
		});
		assert_eq!(recovery_draft(&state, channel), "second pending draft");
		state.drafts.insert(channel, String::new());
		assert_eq!(recovery_draft(&state, channel), "second pending draft");
		state.drafts.insert(channel, "new unsent edit".into());
		assert_eq!(recovery_draft(&state, channel), "new unsent edit");
		assert!(!changes_active_history(
			&state,
			&Event::Message(test_support::message(1001, model::Id(999)))
		));
		assert!(!changes_active_history(
			&state,
			&Event::History {
				channel,
				request: state.request.wrapping_sub(1),
				older: false,
				messages: vec![]
			}
		));
		assert!(changes_active_history(
			&state,
			&Event::DeleteBulk {
				channel,
				ids: vec![model::Id(1000)]
			}
		));
	}
}
