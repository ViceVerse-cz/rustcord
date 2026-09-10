#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod avatars;
mod cache;
mod connection;
mod credentials;
mod downloads;
mod reading_settings;
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
struct Desktop {
    login: Option<platform::LoginView>,
    connection: Option<connection::Connection>,
    state: State,
    messaging: ui::MessagingUi,
    downloads: downloads::Downloads,
    notifications: platform::notifications::Notifications,
    uploads: uploads::Uploads,
    download_close_pending: bool,
    window: Arc<winit::window::Window>,
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
        state.status = "Showing cached history · waiting for Discord revalidation";
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
        Event::Delete { channel, .. } | Event::DeleteBulk { channel, .. } => channel,
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
        ui::emoji::install(&cc.egui_ctx)?;
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
            if std::env::args().any(|arg| arg == "--demo-system-messages") {
                test_support::system_demo_state()
            } else if std::env::args().any(|arg| arg == "--demo-notifications") {
                test_support::notification_demo_state()
            } else if std::env::args().any(|arg| arg == "--demo-voice") {
                test_support::voice_demo_state()
            } else if std::env::args().any(|arg| arg == "--demo-chat") {
                test_support::chat_demo_state()
            } else {
                test_support::demo_state()
            }
        } else {
            State::default()
        };
        if demo {
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
        if demo && std::env::args().any(|arg| arg == "--demo-emoji-completion") {
            if let Some(channel) = state.selected {
                state.drafts.insert(channel, "<#20> :hea".into());
            }
            messaging.preview_composer();
        }
        messaging.notification_test_available =
            demo && std::env::args().any(|arg| arg == "--demo-system-notifications");
        if messaging.notification_test_available {
            state.status = "Offline fixture · explicit system notification test";
        }
        if demo && std::env::args().any(|arg| arg == "--demo-profile") {
            // Presence for the fixture card comes from the same synthetic People rows.
            let _ = state.request_members();
            if let Some(list) = &state.members {
                state.members = Some(demo_members(list.guild, list.channel, list.request));
            }
            messaging.preview_profile(test_support::message(1, model::Id(20)).author);
            state.status = "Offline fixture · synthetic profile card opened at startup";
        }
        Ok(Self {
            login: None,
            connection: None,
            state,
            messaging,
            downloads: downloads::Downloads::default(),
            notifications: {
                let wake = cc.egui_ctx.clone();
                platform::notifications::Notifications::new(move || wake.request_repaint())
            },
            uploads: uploads::Uploads::default(),
            download_close_pending: false,
            window: cc
                .winit_window()
                .ok_or("Native window unavailable")?
                .clone(),
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
        self.state.status = "Connecting to Discord · experimental normal-user adapter";
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
                cache::Operation::LoadChannel { .. } | cache::Operation::SaveChannel { .. }
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
                .any(|p| p.nonce == *nonce && p.attachment.is_some())
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
                            self.state.reactions.writing = None;
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
                    channel,
                    message,
                    content,
                } => Event::Patch(model::MessagePatch {
                    extra_content: Default::default(),
                    reactions: model::Patch::Absent,
                    embeds: model::Patch::Absent,
                    attachments: model::Patch::Absent,
                    mentions: model::Patch::Absent,
                    embeds_suppressed: model::Patch::Absent,
                    id: message,
                    channel,
                    content: model::Patch::Value(content),
                    edited: model::Patch::Value(1),
                }),
                Command::Delete { channel, message } => Event::Delete {
                    channel,
                    id: message,
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
            .frame(egui::Frame::NONE.fill(p.canvas).inner_margin(32))
            .show(ui, |ui| {
                egui::ScrollArea::vertical().id_salt("sign-in-scroll").show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Serein").size(24.0).strong());
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("EARLY PREVIEW").size(10.0).color(p.accent));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.menu_button("Appearance", |ui| {
                                egui::widgets::global_theme_preference_buttons(ui);
                                self.messaging.reading_settings(ui, self.fixture_only || self.state.demo);
                            });
                        });
                    });
                    let wide = ui.available_width() >= 900.0;
                    ui.add_space(if wide { ((ui.available_height() - 460.0) * 0.4).max(30.0) } else { 24.0 });
                    if wide {
                        let left = (ui.available_width() - 438.0).min(530.0);
                        ui.horizontal_top(|ui| {
                            ui.allocate_ui_with_layout(egui::vec2(left, 390.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                                ui.add_space(30.0);
                                ui.label(egui::RichText::new("Your conversations.\nA little more calm.").size(42.0).color(p.text));
                                ui.add_space(16.0);
                                ui.add(egui::Label::new(egui::RichText::new("A lightweight home for the servers and people you already know.").size(18.0).color(p.muted)).wrap());
                                ui.add_space(36.0);
                                ui.label(egui::RichText::new("YOUR ACCOUNT. YOUR COMMUNITIES.").size(11.0).color(p.accent));
                                ui.add_space(4.0);
                                ui.label(egui::RichText::new("Connect to Discord, then pick up the conversation.").color(p.muted));
                            });
                            ui.add_space(40.0);
                            ui.allocate_ui_with_layout(egui::vec2(390.0, 390.0), egui::Layout::top_down(egui::Align::Min), |ui| self.sign_in_card(ui));
                        });
                    } else {
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("A little more room to talk.").size(30.0));
                            ui.add_space(18.0);
                            ui.allocate_ui_with_layout(egui::vec2(ui.available_width().min(430.0), 390.0), egui::Layout::top_down(egui::Align::Min), |ui| self.sign_in_card(ui));
                        });
                    }
                    ui.add_space(28.0);
                    ui.label(egui::RichText::new("Independent. Open source. Not affiliated with Discord.").size(12.0).color(p.muted));
                });
            });
    }
    fn sign_in_card(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let p = ui::design::palette(ui);
        egui::Frame::NONE.fill(p.surface).stroke(egui::Stroke::new(1.0, p.border))
            .corner_radius(16).inner_margin(28).show(ui, |ui| {
                ui.set_width((ui.available_width()).min(350.0));
                ui.heading("Welcome to Serein");
                ui.add_space(4.0);
                ui.label(egui::RichText::new("Continue with your Discord account.").color(p.muted));
                ui.add_space(22.0);
                {
                    ui.add_enabled(!self.fixture_only, egui::Checkbox::new(&mut self.authorized, "I own this account and authorize this test session."));
                    ui.add_space(8.0);
                    let can_sign_in = !self.fixture_only && self.authorized && !self.forgetting && self.state.auth != AuthState::Authenticating;
                    if ui.add_enabled_ui(can_sign_in, |ui| ui::design::primary_button(ui, "Sign in with Discord  →")).inner.clicked() {
                        if let Some(store)=&mut self.store {store.cancel_load();}
                        self.credential_status="Sign in through Discord; saved-login lookup stopped";
                        let wake = ctx.clone();
                        match platform::LoginView::open(self.window.clone(), move || wake.request_repaint()) {
                            Ok(login) => { self.login = Some(login); self.state.auth = AuthState::Authenticating; self.state.status = "Waiting for Discord login"; }
                            Err(_) => { self.state.auth = AuthState::Failed; self.state.status = "Platform login webview unavailable; see platform-support.md"; }
                        }
                    }
                    if self.fixture_only { ui.small("Offline preview. Launch normally to sign in."); }
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Opens Discord’s login page. Your device’s credential store remembers your login.").size(12.0).color(p.muted));
                    ui.add_space(12.0);
                    if self.state.auth != AuthState::Unauthenticated || self.state.status != "Disconnected" || self.forgetting {
                        ui.label(self.state.status);
                    }
                    ui.label(egui::RichText::new(self.credential_status).size(12.0));
                    if self.cache_error || self.cache_pending > 0 || self.cache_clears.pending() { ui.small(self.cache_status); }
                    ui.label(egui::RichText::new("Unofficial clients may put your Discord account at risk.").size(12.0).color(p.muted));
                }
                ui.add_space(18.0);
                ui.separator();
                ui.add_space(10.0);
                if ui.add(egui::Button::new("Preview the interface  →").frame(false)).clicked() {
                    if let Some(store)=&mut self.store {store.cancel_load();}
                    self.connection = None; self.pending_save = None;
                    let generation = self.state.generation + 1;
                    self.state = test_support::demo_state(); self.state.generation = generation; self.messaging.clear();
                }
                ui.label(egui::RichText::new("Sample conversations. No Discord connection.").size(12.0).color(p.muted));
                ui.add_space(12.0);
                ui.collapsing("About this preview", |ui| {
                    ui.small("Messaging, reactions, search and read markers have offline tests. Real Discord interoperability is still unverified; attachment uploads and advanced search remain incomplete.");
                    ui.small("Messages and drafts are cached locally. Login tokens use the operating system credential store.");
                    ui.small(self.credential_status);
                    if !self.fixture_only && ui.button("Forget saved login").clicked() { self.logout(&ctx); }
                });
                #[cfg(feature = "developer-session")]
                if !self.fixture_only { ui.collapsing("Developer session", |ui| {
                    ui.add(egui::TextEdit::singleline(&mut *self.token_input).password(true).char_limit(2048).hint_text("Owner-supplied test credential"));
                    if ui.add_enabled(self.authorized, egui::Button::new("Connect imported session (RAM only)")).clicked() {
                        let input = std::mem::take(&mut *self.token_input);
                        match SessionSecret::from_owner_input(input) { Ok(secret) => self.connect(secret, false, &ctx), Err(f) => self.state.status = f.label() }
                    }
                }); }
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
                self.messaging.accept_avatar(ctx, result.key, result.image);
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
            let mut removed_channels: std::collections::BTreeSet<_> = if matches!(
                &event.event,
                Event::Ready { .. }
                    | Event::Permissions(_)
                    | Event::ChannelChanged(_)
                    | Event::ThreadChanged { .. }
                    | Event::ChannelCreated(_)
                    | Event::ChannelRestored(_)
                    | Event::ThreadsSync { .. }
                    | Event::ThreadRemoved { .. }
            ) {
                self.state
                    .channels
                    .iter()
                    .filter(|channel| {
                        channel.supports_text() && self.state.can_read_history(channel.id)
                    })
                    .map(|channel| channel.id)
                    .collect()
            } else {
                Default::default()
            };
            let invalidate = matches!(
                event.event,
                Event::Resync | Event::PermissionsChanged | Event::Unavailable(_)
            ) || matches!(&event.event, Event::RecipientRemoved { user, .. } if self.state.user.as_ref().is_some_and(|owner| owner.id == *user))
                || matches!(&event.event, Event::Reactions(client_core::reactions::Event::Read {channel,result:Err(Failure::Forbidden),..}) if self.state.selected==Some(*channel))
                || matches!(&event.event, Event::HistoryFailed { channel, request, failure: Failure::Forbidden }
                if self.state.selected == Some(*channel) && self.state.request == *request && self.state.history_pending);
            let history_changed = changes_active_history(&self.state, &event.event);
            if event.generation == self.state.generation
                && (invalidate
                    || event.event.changes_access()
                    || matches!(
                        &event.event,
                        Event::NotificationPreferences(_)
                            | Event::Disconnected
                            | Event::ReadState(client_core::read_state::Event::Ack { .. })
                            | Event::ReadState(client_core::read_state::Event::Result {
                                result: Ok(()),
                                ..
                            })
                    ))
            {
                self.notifications.dismiss();
            }
            self.state.apply(event);
            // Only admitted service messages can establish a deleted reply target.
            // Fence pending disk writes before the post-drain timeline snapshot is saved.
            let deleted_replies = self.state.take_reply_deletions();
            if let Some(&(channel, _)) = deleted_replies.first() {
                let ids: Vec<_> = deleted_replies.into_iter().map(|(_, id)| id).collect();
                self.messaging.messages_deleted(ctx, channel, &ids);
                self.delete_cached_ids(channel, ids);
            }
            if !removed_channels.is_empty() {
                for channel in self
                    .state
                    .channels
                    .iter()
                    .filter(|c| c.supports_text() && self.state.can_read_history(c.id))
                {
                    removed_channels.remove(&channel.id);
                }
            }
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
                })
            {
                let command = self.state.history(None);
                self.command(command);
            }
        }
        if persist_timeline
            && self.state.freshness == model::Freshness::Fresh
            && let Some(channel) = self.state.selected
            && self.state.can_read_history(channel)
        {
            self.queue_cache(cache::Operation::SaveChannel {
                channel,
                messages: self.state.timeline.iter().cloned().collect(),
            });
        }
        if let Some(failure) = terminal {
            if std::env::var_os("SEREIN_MEMBER_DIAGNOSTICS").as_deref()
                == Some(std::ffi::OsStr::new("1"))
            {
                // One additional fixed-label diagnostic when the session terminates.
                eprintln!("[Serein members] Session stopped: {}", failure.label());
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
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if self.login.is_some() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }
}
impl eframe::App for Desktop {
    fn persist_egui_memory(&self) -> bool {
        false
    }
    fn logic(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.messaging.sync_reading_zoom(ctx);
        if !self.fixture_only && !self.state.demo {
            self.reading.observe(
                self.messaging.reading_preferences,
                std::time::Instant::now(),
            );
        }
        self.poll(ctx);
        if self.state.auth != AuthState::Authenticated && !self.state.demo {
            self.notifications.clear();
            self.messaging.notifications_enabled = false;
        }
        while let Some(notification) = self.state.take_notification() {
            if !self.fixture_only
                && self.messaging.notifications_enabled
                && !(ctx.input(|i| i.focused)
                    && self.messaging.viewing_latest(notification.channel))
            {
                self.notifications.notify();
            }
        }
        #[cfg(feature = "voice")]
        {
            self.messaging.voice_ptt_active = ctx
                .input(|input| input.focused && input.key_down(egui::Key::V))
                && !ctx.egui_wants_keyboard_input();
            self.poll_voice(ctx);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui::design::paint_backdrop(&ctx);
        let upload_allowed = self.state.user.is_some()
            && self.state.gateway_connected
            && self.state.freshness == model::Freshness::Fresh;
        let can_attach = self
            .state
            .selected
            .is_some_and(|channel| self.state.can_attach(channel));
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
        let dropped = ctx.input_mut(|input| std::mem::take(&mut input.raw.dropped_files));
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
        self.messaging.attachment = self
            .uploads
            .selection()
            .map(|(name, size)| (name.to_owned(), size));
        self.messaging.upload_busy = self.uploads.busy();
        self.messaging.upload_status = self.uploads.status();
        if self.state.user.is_none() {
            self.downloads.cancel();
        }
        let download_status = match self.downloads.poll() {
            downloads::Status::Idle => String::new(),
            downloads::Status::Choosing => "Choose where to save the attachment…".into(),
            downloads::Status::Downloading { received, total } => {
                format!("Downloading: {} / {} KiB", received / 1024, total / 1024)
            }
            downloads::Status::Saved => "Attachment downloaded".into(),
            downloads::Status::Cancelled => "Download cancelled".into(),
            downloads::Status::Failed(error) => (*error).into(),
        };
        self.messaging.downloads().active = self.downloads.is_active();
        self.messaging.downloads().status = download_status;
        if ctx.input(|i| i.viewport().close_requested()) && self.downloads.is_active() {
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
        #[cfg(feature = "voice")]
        self.poll_voice(&ctx);
        self.messaging.voice_available = cfg!(feature = "voice");
        if ctx.input(|i| i.viewport().close_requested())
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
                || self.cache_error)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirming_close = true;
        }
        if self.login.is_some() {
            egui::CentralPanel::default().show(ui,|ui|{
                ui.heading("Discord sign-in");ui.horizontal(|ui|{ui.label("Official discord.com page · temporary login webview · Serein is unofficial");if ui.button("Cancel login").clicked(){self.login=None;self.state.auth=AuthState::Unauthenticated;}});
                ui.small("Complete login here. Passwords and QR challenges stay in the webview; only the resulting token is saved.");
            });
            if let Some(login) = &self.login {
                login.resize(&self.window);
            }
        } else if self.state.user.is_some() {
            self.messaging.storage_status = self.cache_status;
            self.messaging.notification_status = self.notifications.status().label();
            let commands = self.messaging.show(ui, &mut self.state);
            self.notifications.set_enabled(
                self.messaging.notifications_enabled
                    && (!self.fixture_only || self.messaging.notification_test_available),
            );
            if std::mem::take(&mut self.messaging.notification_test_requested)
                && self.messaging.notification_test_available
            {
                self.notifications.notify();
            }
            // Selection may have changed during this frame; never reuse another channel's file.
            self.uploads.poll(
                self.state.generation,
                self.state.selected,
                self.state.user.is_some() && self.state.gateway_connected,
                &ctx,
            );
            if !self
                .state
                .selected
                .is_some_and(|channel| self.state.can_attach(channel))
            {
                self.uploads.cancel();
            }
            if std::mem::take(&mut self.messaging.remove_attachment_requested) {
                self.uploads.remove();
            }
            if std::mem::take(&mut self.messaging.cancel_upload_requested) {
                self.uploads.cancel();
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
                )
            {
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
                )
            {
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
                ui.horizontal(|ui|{
                    if ui.button("Keep working").clicked(){self.confirming_close=false;self.confirming_logout=false;self.download_close_pending=false;}
                    if ui.add_enabled(!self.forgetting,egui::Button::new("Discard and continue")).clicked(){
                        if self.confirming_close{self.close_approved=true;self.uploads.cancel();if self.downloads.is_active(){self.downloads.cancel();self.download_close_pending=true;}else{ctx.send_viewport_cmd(egui::ViewportCommand::Close);}}else{self.logout(&ctx);}
                    }
                });
            });
        }
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
