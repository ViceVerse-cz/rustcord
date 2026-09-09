use eframe::egui;
use local_store::{Appearance, LocalStore, StoreError};
use model::{Id, Message};
use std::{
    collections::BTreeMap,
    sync::mpsc::{self, Receiver, SyncSender},
};
pub enum Operation {
    LoadAppearance,
    SaveAppearance(Appearance),
    LoadDrafts,
    LoadChannel { channel: Id, request: u64 },
    SaveDraft { channel: Id, content: String },
    SaveChannel { channel: Id, messages: Vec<Message> },
    ClearHistory,
    Forget,
}
pub enum Outcome {
    Appearance(Appearance),
    Drafts(BTreeMap<Id, String>),
    Channel {
        channel: Id,
        request: u64,
        messages: Vec<Message>,
    },
    Saved,
    Failed {
        error: StoreError,
        message: &'static str,
        draft_restore: bool,
    },
}
pub struct Cache {
    pub send: SyncSender<(u64, Id, Operation)>,
    pub receive: Receiver<(u64, Outcome)>,
}
impl Cache {
    pub fn start(ctx: egui::Context) -> Self {
        let (send, commands) = mpsc::sync_channel::<(u64, Id, Operation)>(16);
        let (events, receive) = mpsc::sync_channel(16);
        std::thread::spawn(move || {
            let mut store = LocalStore::open_default();
            while let Ok((generation, account, operation)) = commands.recv() {
                let draft_restore = matches!(operation, Operation::LoadDrafts);
                let failure_message = match &operation {
                    Operation::LoadAppearance => {
                        "Could not load saved appearance; using system theme"
                    }
                    Operation::SaveAppearance(_) => {
                        "Could not save appearance; change exists only in this session"
                    }
                    Operation::Forget => {
                        "Could not remove local account data; history and drafts may remain on disk"
                    }
                    Operation::ClearHistory => {
                        "Could not clear cached history; messages may remain on disk"
                    }
                    Operation::SaveDraft { .. } => {
                        "Could not save a draft; latest text may exist only in memory"
                    }
                    Operation::SaveChannel { .. } => "Could not save cached history",
                    Operation::LoadDrafts => "Could not restore drafts from local storage",
                    Operation::LoadChannel { .. } => "Could not read cached history",
                };
                let result = match &mut store {
                    Ok(store) => match operation {
                        Operation::LoadAppearance => store.appearance().map(Outcome::Appearance),
                        Operation::SaveAppearance(appearance) => {
                            store.save_appearance(appearance).map(|_| Outcome::Saved)
                        }
                        Operation::LoadDrafts => store.load_drafts(account).map(Outcome::Drafts),
                        Operation::LoadChannel { channel, request } => store
                            .load_channel(account, channel)
                            .map(|messages| Outcome::Channel {
                                channel,
                                request,
                                messages,
                            }),
                        Operation::SaveDraft { channel, content } => store
                            .save_draft(account, channel, &content)
                            .map(|_| Outcome::Saved),
                        Operation::SaveChannel { channel, messages } => store
                            .save_channel(account, channel, &messages)
                            .map(|_| Outcome::Saved),
                        Operation::ClearHistory => {
                            store.clear_history(account).map(|_| Outcome::Saved)
                        }
                        Operation::Forget => store.forget_account(account).map(|_| Outcome::Saved),
                    },
                    Err(error) => Err(*error),
                };
                if events
                    .send((
                        generation,
                        result.unwrap_or_else(|error| Outcome::Failed {
                            error,
                            message: failure_message,
                            draft_restore,
                        }),
                    ))
                    .is_err()
                {
                    break;
                }
                ctx.request_repaint();
            }
        });
        Self { send, receive }
    }
}
