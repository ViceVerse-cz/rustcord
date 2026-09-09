#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod avatars;
mod cache;
mod connection;
mod credentials;
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
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([760.0, 520.0])
            .with_app_id("org.serein.desktop"),
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
    cache_error: bool,
    cache_status: &'static str,
    appearance: egui::ThemePreference,
    appearance_changed: bool,
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
impl Desktop {
    fn new(
        cc: &eframe::CreationContext<'_>,
        demo: bool,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        ui::fonts::install(&cc.egui_ctx);
        ui::design::apply(&cc.egui_ctx);
        cc.egui_ctx.set_theme(egui::ThemePreference::System);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;
        let store = (!demo).then(|| credentials::Store::start(cc.egui_ctx.clone()));
        let cache = (!demo).then(|| cache::Cache::start(cc.egui_ctx.clone()));
        let state = if demo {
            test_support::demo_state()
        } else {
            State::default()
        };
        if let Some(store) = &store {
            let _ = store
                .send
                .try_send((state.generation, credentials::Operation::Load));
        }
        let cache_pending = usize::from(cache.as_ref().is_some_and(|cache| {
            // Appearance has its own singleton table; this account ID is unused.
            cache
                .send
                .try_send((
                    state.generation,
                    model::Id(0),
                    cache::Operation::LoadAppearance,
                ))
                .is_ok()
        }));
        Ok(Self {
            login: None,
            connection: None,
            state,
            messaging: ui::MessagingUi::default(),
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
            cache_error: false,
            cache_status: "Loading local appearance…",
            appearance: egui::ThemePreference::System,
            appearance_changed: false,
            pending_save: None,
            credential_status: if demo {
                "Fixture mode never opens the credential store or network"
            } else {
                "Checking saved login…"
            },
            forgetting: false,
            confirming_close: false,
            confirming_logout: false,
            close_approved: false,
            fixture_only: demo,
            authorized: false,
            synthetic_id: 10000,
            #[cfg(feature = "developer-session")]
            token_input: Zeroizing::new(String::new()),
        })
    }
    fn connect(&mut self, secret: SessionSecret, save: bool, ctx: &egui::Context) {
        #[cfg(feature = "voice")]
        self.voice.stop();
        self.state.disconnect_voice();
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
            if cache
                .send
                .try_send((self.state.generation, account, cache::Operation::Forget))
                .is_ok()
            {
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
            if cache
                .send
                .try_send((self.state.generation, account, operation))
                .is_ok()
            {
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
    fn command(&mut self, command: Command) {
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
            request,
        } = &command
        {
            self.queue_cache(cache::Operation::LoadChannel {
                channel: *channel,
                request: *request,
            });
        }
        if self.state.demo {
            let event = match command {
                Command::Voice(_) => return,
                Command::Members {
                    guild,
                    channel,
                    request,
                    ..
                } => {
                    let Some(channel) = channel else {
                        return;
                    };
                    Event::Members(model::MemberList {
                        guild,
                        channel,
                        request,
                        rows: vec![
                            Some(model::Member {
                                user: test_support::message(2, channel).author,
                                nick: None,
                                status: None,
                            }),
                            Some(model::Member {
                                user: test_support::message(1, channel).author,
                                nick: None,
                                status: None,
                            }),
                        ],
                        total: 2,
                        freshness: model::Freshness::Fresh,
                    })
                }
                Command::History { before, .. } => {
                    test_support::load_page(&mut self.state, before);
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
                    if self.state.auth != AuthState::Unauthenticated || self.state.status != "Disconnected" || self.forgetting || self.credential_status.contains("unavailable") || self.credential_status.contains("Could not") {
                        ui.label(self.state.status);
                        ui.label(egui::RichText::new(self.credential_status).size(12.0));
                    }
                    if self.cache_error || self.cache_pending > 0 { ui.small(self.cache_status); }
                    ui.label(egui::RichText::new("Unofficial clients may put your Discord account at risk.").size(12.0).color(p.muted));
                }
                ui.add_space(18.0);
                ui.separator();
                ui.add_space(10.0);
                if ui.add(egui::Button::new("Preview the interface  →").frame(false)).clicked() {
                    self.connection = None; self.pending_save = None;
                    let generation = self.state.generation + 1;
                    self.state = test_support::demo_state(); self.state.generation = generation; self.messaging.clear();
                }
                ui.label(egui::RichText::new("Sample conversations. No Discord connection.").size(12.0).color(p.muted));
                ui.add_space(12.0);
                ui.collapsing("About this preview", |ui| {
                    ui.small("Login and messaging compatibility are still being tested. Voice, attachments, reactions, search and read markers are not available yet.");
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
                cache::Outcome::Appearance(appearance) => {
                    if !self.state.demo && !self.appearance_changed {
                        self.appearance = match appearance {
                            local_store::Appearance::System => egui::ThemePreference::System,
                            local_store::Appearance::Light => egui::ThemePreference::Light,
                            local_store::Appearance::Dark => egui::ThemePreference::Dark,
                        };
                        ctx.set_theme(self.appearance);
                    }
                    continue;
                }
                cache::Outcome::Failed {
                    error,
                    message,
                    draft_restore,
                } => {
                    let _ = error;
                    self.cache_error = true;
                    self.cache_status = message;
                    if *draft_restore && generation == self.state.generation {
                        self.messaging.draft_restore_pending = false;
                        self.state.drafts.retain(|_, content| !content.is_empty());
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
                cache::Outcome::Channel {
                    channel,
                    request,
                    messages,
                } => {
                    if self.state.selected == Some(channel)
                        && self.state.request == request
                        && self.state.freshness == model::Freshness::Loading
                        && self.state.timeline.is_empty()
                    {
                        let _ = self.state.timeline.seed_cache(messages);
                        self.state.revision += 1;
                        self.state.status =
                            "Showing cached history · waiting for Discord revalidation";
                    }
                }
                cache::Outcome::Saved => {
                    if !self.cache_error {
                        self.cache_status = "Local changes saved";
                    }
                }
                cache::Outcome::Appearance(_) | cache::Outcome::Failed { .. } => unreachable!(),
            }
        }
        let mut results = Vec::new();
        if let Some(store) = &self.store {
            for _ in 0..4 {
                match store.receive.try_recv() {
                    Ok(result) => results.push(result),
                    Err(_) => break,
                }
            }
        }
        for (generation, outcome) in results {
            if generation != self.state.generation {
                continue;
            }
            match outcome {
                credentials::Outcome::Loaded(Ok(Some(secret))) => self.connect(secret, false, ctx),
                credentials::Outcome::Loaded(Ok(None)) => {
                    self.credential_status =
                        "Sign in once; the session token will be saved in your OS credential store"
                }
                credentials::Outcome::Loaded(Err(_)) => {
                    self.credential_status = "Saved login unavailable; no plaintext fallback"
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
            terminal = *connection.terminal.borrow();
        }
        let mut persist_timeline = false;
        for mut event in events {
            if event.generation != self.state.generation {
                continue;
            }
            #[cfg(feature = "voice")]
            let voice_failure = self.voice.observe(&self.state, &mut event.event);
            #[cfg(not(feature = "voice"))]
            let _ = &mut event;
            let ready = matches!(event.event, Event::Ready { .. });
            let resumed = matches!(event.event, Event::Resumed);
            let confirmed_channel = confirmed_recovery_channel(&self.state, &event.event);
            let invalidate = matches!(
                event.event,
                Event::Resync | Event::PermissionsChanged | Event::Unavailable(_)
            ) || matches!(&event.event, Event::RecipientRemoved { user, .. } if self.state.user.as_ref().is_some_and(|owner| owner.id == *user))
                || matches!(&event.event, Event::HistoryFailed { channel, request, failure: Failure::Forbidden }
                if self.state.selected == Some(*channel) && self.state.request == *request && self.state.history_pending);
            let history_changed = changes_active_history(&self.state, &event.event);
            self.state.apply(event);
            #[cfg(feature = "voice")]
            if let Some(error) = voice_failure
                && let Some(command) = self.voice.fail(&mut self.state, error)
            {
                self.command(command);
            }
            if invalidate {
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
        {
            self.queue_cache(cache::Operation::SaveChannel {
                channel,
                messages: self.state.timeline.iter().cloned().collect(),
            });
        }
        if let Some(failure) = terminal {
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
        self.poll(ctx);
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
                || self.forgetting
                || self.avatar_cleanup.is_some()
                || self.cache_pending > 0
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
            let commands = self.messaging.show(ui, &mut self.state);
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
                if self.state.has_unsent() || self.messaging.has_edit() {
                    self.confirming_logout = true;
                } else {
                    self.logout(&ctx);
                }
            }
        } else {
            self.sign_in_screen(ui);
        }
        let appearance = ctx.options(|options| options.theme_preference);
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
        if self.confirming_close || self.confirming_logout {
            egui::Window::new("Leave this session?").collapsible(false).show(&ctx,|ui|{
                ui.label("Saved drafts survive exit. Logout removes local account data. Edits and uncertain sends need your attention.");
                if self.forgetting{ui.label("Wait for saved-login removal to finish.");}
                ui.horizontal(|ui|{
                    if ui.button("Keep working").clicked(){self.confirming_close=false;self.confirming_logout=false;}
                    if ui.add_enabled(!self.forgetting,egui::Button::new("Discard and continue")).clicked(){
                        if self.confirming_close{self.close_approved=true;ctx.send_viewport_cmd(egui::ViewportCommand::Close);}else{self.logout(&ctx);}
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
