//! A single worker orders credential-store reads/writes/deletions, even during logout.
use client_core::auth::SessionSecret;
use eframe::egui;
use platform::CredentialError;
use std::sync::{
    Arc,
    mpsc::{self, Receiver, SyncSender},
};

pub enum Operation {
    Load,
    Save(Arc<SessionSecret>),
    Forget,
}
pub enum Outcome {
    Loaded(Result<Option<SessionSecret>, CredentialError>),
    Saved(Result<(), CredentialError>),
    Forgotten(Result<(), CredentialError>),
}
pub struct Store {
    pub send: SyncSender<(u64, Operation)>,
    pub receive: Receiver<(u64, Outcome)>,
}
impl Store {
    pub fn start(ctx: egui::Context) -> Self {
        let (send, commands) = mpsc::sync_channel::<(u64, Operation)>(4);
        let (events, receive) = mpsc::sync_channel(4);
        std::thread::spawn(move || {
            while let Ok((generation, operation)) = commands.recv() {
                let outcome = match operation {
                    Operation::Load => Outcome::Loaded(platform::load_session()),
                    Operation::Save(secret) => Outcome::Saved(platform::save_session(&secret)),
                    Operation::Forget => Outcome::Forgotten(platform::forget_session()),
                };
                if events.send((generation, outcome)).is_err() {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self { send, receive }
    }
}
