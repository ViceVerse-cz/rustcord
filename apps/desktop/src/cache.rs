use eframe::egui;
use local_store::{Appearance, LocalStore, StoreError};
use model::{Id, Message};
use std::{
	collections::{BTreeMap, BTreeSet},
	sync::{
		Arc,
		atomic::{AtomicBool, AtomicU64, Ordering},
		mpsc::{self, Receiver, SyncSender},
	},
};
pub enum Operation {
	LoadAppearance,
	SaveAppearance(Appearance),
	SaveThemeVariant(Option<String>),
	LoadReadingPreferences,
	SaveReadingPreferences(model::ReadingPreferences),
	LoadGameActivity,
	SaveGameActivity(bool),
	LoadDrafts,
	LoadGifFavorites,
	SaveGifFavorites(Vec<model::Gif>),
	LoadChannel {
		channel: Id,
		request: u64,
	},
	SaveDraft {
		channel: Id,
		content: String,
	},
	SaveChannel {
		channel: Id,
		messages: Vec<Message>,
	},
	SaveChanges {
		channel: Id,
		messages: Vec<Message>,
		retained: Vec<Id>,
	},
	DeleteMessages {
		channel: Id,
		ids: Vec<Id>,
	},
	ClearHistory,
	Forget,
}
pub enum Outcome {
	/// Saved appearance plus the saved theme preset key, if any.
	Appearance(Appearance, Option<String>),
	ReadingPreferences(Result<model::ReadingPreferences, StoreError>),
	ReadingPreferencesSaved(Result<(), StoreError>),
	GameActivity(Result<bool, StoreError>),
	GameActivitySaved(Result<(), StoreError>),
	Drafts(BTreeMap<Id, String>),
	GifFavorites(Vec<model::Gif>),
	Channel {
		channel: Id,
		request: u64,
		messages: Vec<Message>,
		epoch: u64,
	},
	Saved,
	HistoryCleared,
	Failed {
		error: StoreError,
		message: &'static str,
		draft_restore: bool,
		history_cleanup: bool,
	},
}
pub struct Cache {
	send: SyncSender<(u64, Id, u64, Operation)>,
	pub receive: Receiver<(u64, Outcome)>,
	pub history: Arc<HistorySafety>,
}
#[derive(Default)]
pub struct HistorySafety {
	epoch: AtomicU64,
	blocked: AtomicBool,
	failed: AtomicBool,
}
impl HistorySafety {
	pub fn invalidate(&self) {
		self.epoch.fetch_add(1, Ordering::SeqCst);
	}
	pub fn block(&self) {
		self.blocked.store(true, Ordering::SeqCst);
	}
	pub fn fail(&self) {
		self.failed.store(true, Ordering::SeqCst);
		self.block();
	}
	pub fn cleared(&self) {
		if !self.failed.load(Ordering::SeqCst) {
			self.blocked.store(false, Ordering::SeqCst);
		}
	}
	pub fn epoch(&self) -> u64 {
		self.epoch.load(Ordering::SeqCst)
	}
	pub fn allows(&self, epoch: u64) -> bool {
		!self.failed.load(Ordering::SeqCst)
			&& !self.blocked.load(Ordering::SeqCst)
			&& self.epoch() == epoch
	}
}
/// At most sixteen distinct account cleanups await queue space; drafts are unaffected.
#[derive(Default)]
pub struct HistoryClears {
	accounts: BTreeSet<Id>,
	queued: usize,
}
impl HistoryClears {
	pub fn pending(&self) -> bool {
		!self.accounts.is_empty() || self.queued != 0
	}
	pub fn request(&mut self, account: Id) -> bool {
		if self.accounts.len() == 16 && !self.accounts.contains(&account) {
			return false;
		}
		self.accounts.insert(account);
		true
	}
	pub fn next(&self) -> Option<Id> {
		self.accounts.first().copied()
	}
	pub fn queued(&mut self, account: Id) {
		if self.accounts.remove(&account) {
			self.queued += 1;
		}
	}
	pub fn acknowledge(&mut self, safety: &HistorySafety) -> bool {
		self.queued = self.queued.saturating_sub(1);
		let complete = self.accounts.is_empty() && self.queued == 0;
		if complete {
			safety.cleared();
		}
		complete
	}
}
impl Cache {
	pub fn delete_messages(&self, generation: u64, account: Id, channel: Id, ids: Vec<Id>) -> bool {
		let blocked = !self.history.allows(self.history.epoch());
		self.history.invalidate();
		if blocked
			|| ids.len() > 100
			|| !self.queue(
				generation,
				account,
				Operation::DeleteMessages { channel, ids },
			) {
			self.history.block();
			false
		} else {
			true
		}
	}
	pub fn queue(&self, generation: u64, account: Id, operation: Operation) -> bool {
		let epoch = self.history.epoch();
		if matches!(
			operation,
			Operation::LoadChannel { .. }
				| Operation::SaveChannel { .. }
				| Operation::SaveChanges { .. }
		) && !self.history.allows(epoch)
		{
			return false;
		}
		self.send
			.try_send((generation, account, epoch, operation))
			.is_ok()
	}
	pub fn start(ctx: egui::Context) -> Self {
		let (send, commands) = mpsc::sync_channel::<(u64, Id, u64, Operation)>(16);
		let (events, receive) = mpsc::sync_channel(16);
		let history = Arc::new(HistorySafety::default());
		let worker_history = history.clone();
		std::thread::spawn(move || {
			let mut store = LocalStore::open_default();
			while let Ok((generation, account, epoch, operation)) = commands.recv() {
				let outcome = execute(&mut store, &worker_history, account, epoch, operation);
				if events.send((generation, outcome)).is_err() {
					break;
				}
				ctx.request_repaint();
			}
		});
		Self {
			send,
			receive,
			history,
		}
	}
}

fn execute(
	store: &mut Result<LocalStore, StoreError>,
	history: &HistorySafety,
	account: Id,
	epoch: u64,
	operation: Operation,
) -> Outcome {
	// Settings completions are account-independent and have their own pending/error state.
	match &operation {
		Operation::LoadGameActivity => {
			return Outcome::GameActivity(match store {
				Ok(store) => store.game_activity_enabled(),
				Err(error) => Err(*error),
			});
		}
		Operation::SaveGameActivity(value) => {
			return Outcome::GameActivitySaved(match store {
				Ok(store) => store.save_game_activity_enabled(*value),
				Err(error) => Err(*error),
			});
		}
		Operation::LoadReadingPreferences => {
			return Outcome::ReadingPreferences(match store {
				Ok(store) => store.reading_preferences(),
				Err(error) => Err(*error),
			});
		}
		Operation::SaveReadingPreferences(value) => {
			return Outcome::ReadingPreferencesSaved(match store {
				Ok(store) => store.save_reading_preferences(*value),
				Err(error) => Err(*error),
			});
		}
		_ => {}
	}
	if matches!(
		operation,
		Operation::LoadChannel { .. }
			| Operation::SaveChannel { .. }
			| Operation::SaveChanges { .. }
	) && !history.allows(epoch)
	{
		return Outcome::Saved;
	}
	let draft_restore = matches!(operation, Operation::LoadDrafts);
	let history_cleanup = matches!(operation, Operation::ClearHistory);
	let history_unsafe = matches!(
		operation,
		Operation::DeleteMessages { .. } | Operation::ClearHistory | Operation::Forget
	);
	let failure_message = match &operation {
		Operation::LoadAppearance => "Could not load saved appearance; using system theme",
		Operation::SaveAppearance(_) | Operation::SaveThemeVariant(_) => {
			"Could not save appearance; change exists only in this session"
		}
		Operation::Forget => {
			"Could not remove local account data; history and drafts may remain on disk"
		}
		Operation::ClearHistory => {
			"Could not clear cached history; history cache disabled until restart; messages may remain on disk"
		}
		Operation::DeleteMessages { .. } => {
			"Could not remove deleted cached messages; history cache disabled until restart; messages may remain on disk"
		}
		Operation::SaveDraft { .. } => {
			"Could not save a draft; latest text may exist only in memory"
		}
		Operation::SaveChannel { .. } | Operation::SaveChanges { .. } => {
			"Could not save cached history"
		}
		Operation::LoadDrafts => "Could not restore drafts from local storage",
		Operation::LoadGifFavorites => "Could not restore GIF favorites from local storage",
		Operation::SaveGifFavorites(_) => {
			"Could not save GIF favorites; the change exists only in this session"
		}
		Operation::LoadChannel { .. } => "Could not read cached history",
		Operation::LoadReadingPreferences
		| Operation::SaveReadingPreferences(_)
		| Operation::LoadGameActivity
		| Operation::SaveGameActivity(_) => unreachable!(),
	};
	let result = match store {
		Ok(store) => match operation {
			Operation::LoadReadingPreferences
			| Operation::SaveReadingPreferences(_)
			| Operation::LoadGameActivity
			| Operation::SaveGameActivity(_) => {
				unreachable!()
			}
			Operation::LoadAppearance => store
				.appearance()
				.and_then(|appearance| Ok(Outcome::Appearance(appearance, store.theme_variant()?))),
			Operation::SaveAppearance(appearance) => {
				store.save_appearance(appearance).map(|_| Outcome::Saved)
			}
			Operation::SaveThemeVariant(variant) => store
				.save_theme_variant(variant.as_deref())
				.map(|_| Outcome::Saved),
			Operation::LoadDrafts => store.load_drafts(account).map(Outcome::Drafts),
			Operation::LoadGifFavorites => store.gif_favorites(account).map(Outcome::GifFavorites),
			Operation::SaveGifFavorites(favorites) => store
				.save_gif_favorites(account, &favorites)
				.map(|_| Outcome::Saved),
			Operation::LoadChannel { channel, request } => store
				.load_channel(account, channel)
				.map(|messages| Outcome::Channel {
					channel,
					request,
					messages,
					epoch,
				}),
			Operation::SaveDraft { channel, content } => store
				.save_draft(account, channel, &content)
				.map(|_| Outcome::Saved),
			Operation::SaveChanges {
				channel,
				messages,
				retained,
			} => store
				.save_changes(account, channel, &messages, &retained)
				.map(|_| Outcome::Saved),
			Operation::SaveChannel { channel, messages } => store
				.save_channel(account, channel, &messages)
				.map(|_| Outcome::Saved),
			Operation::DeleteMessages { channel, ids } => store
				.delete_messages(account, channel, &ids)
				.map(|_| Outcome::Saved),
			Operation::ClearHistory => store
				.clear_history(account)
				.map(|_| Outcome::HistoryCleared),
			Operation::Forget => store.forget_account(account).map(|_| Outcome::Saved),
		},
		Err(error) => Err(*error),
	};
	result.unwrap_or_else(|error| {
		if history_unsafe {
			history.fail();
		}
		Outcome::Failed {
			error,
			message: failure_message,
			draft_restore,
			history_cleanup,
		}
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn game_activity_operations_keep_their_own_results_even_when_history_is_blocked() {
		let safety = HistorySafety::default();
		safety.block();
		let mut store = Ok(LocalStore::open(std::path::Path::new(":memory:")).unwrap());
		assert!(matches!(
			execute(&mut store, &safety, Id(0), 0, Operation::LoadGameActivity),
			Outcome::GameActivity(Ok(false))
		));
		assert!(matches!(
			execute(
				&mut store,
				&safety,
				Id(0),
				0,
				Operation::SaveGameActivity(true)
			),
			Outcome::GameActivitySaved(Ok(()))
		));
		assert!(matches!(
			execute(&mut store, &safety, Id(9), 0, Operation::LoadGameActivity),
			Outcome::GameActivity(Ok(true))
		));
		let mut unavailable = Err(StoreError::Unavailable);
		assert!(matches!(
			execute(
				&mut unavailable,
				&safety,
				Id(0),
				0,
				Operation::LoadGameActivity
			),
			Outcome::GameActivity(Err(StoreError::Unavailable))
		));
		assert!(matches!(
			execute(
				&mut unavailable,
				&safety,
				Id(0),
				0,
				Operation::SaveGameActivity(false)
			),
			Outcome::GameActivitySaved(Err(StoreError::Unavailable))
		));
	}

	#[test]
	fn reading_operations_report_their_own_results_without_touching_account_history() {
		let safety = HistorySafety::default();
		let mut store = Ok(LocalStore::open(std::path::Path::new(":memory:")).unwrap());
		let value = model::ReadingPreferences {
			zoom_percent: 125,
			sidebar_width: 300,
			show_members: false,
			animate_gifs: false,
			hide_media_links: true,
		};
		store
			.as_mut()
			.unwrap()
			.save_draft(Id(1), Id(2), "Synthetic draft")
			.unwrap();
		safety.block(); // History cleanup does not prohibit application settings.
		assert!(matches!(
			execute(
				&mut store,
				&safety,
				Id(0),
				0,
				Operation::SaveReadingPreferences(value)
			),
			Outcome::ReadingPreferencesSaved(Ok(()))
		));
		assert!(matches!(execute(&mut store, &safety, Id(9), 0,
            Operation::LoadReadingPreferences), Outcome::ReadingPreferences(Ok(stored)) if stored == value));
		assert_eq!(
			store.as_ref().unwrap().load_drafts(Id(1)).unwrap()[&Id(2)],
			"Synthetic draft"
		);
		let mut unavailable = Err(StoreError::Unavailable);
		assert!(matches!(
			execute(
				&mut unavailable,
				&safety,
				Id(0),
				0,
				Operation::LoadReadingPreferences
			),
			Outcome::ReadingPreferences(Err(StoreError::Unavailable))
		));
		assert!(matches!(
			execute(
				&mut unavailable,
				&safety,
				Id(0),
				0,
				Operation::SaveReadingPreferences(value)
			),
			Outcome::ReadingPreferencesSaved(Err(StoreError::Unavailable))
		));
	}

	#[test]
	fn cleanup_acknowledgements_cross_generations_without_reopening_early() {
		let safety = HistorySafety::default();
		let mut clears = HistoryClears::default();
		safety.block();
		assert!(clears.request(Id(1)));
		clears.queued(Id(1)); // Cleanup queued before logout.
		assert!(clears.request(Id(9))); // Another account needs cleanup after login.
		assert!(!clears.acknowledge(&safety)); // Old-generation completion still counts.
		assert!(!safety.allows(safety.epoch()));
		clears.queued(Id(9));
		assert!(clears.request(Id(9))); // A second deletion races that account's first clear.
		assert!(!clears.acknowledge(&safety));
		clears.queued(Id(9));
		assert!(clears.acknowledge(&safety));
		assert!(safety.allows(safety.epoch()));
		assert!(!clears.pending());
		safety.block();
		for account in 1..=16 {
			assert!(clears.request(Id(account)));
		}
		assert!(!clears.request(Id(17)));
		for account in 1..=16 {
			assert_eq!(clears.next(), Some(Id(account)));
			clears.queued(Id(account));
		}
		safety.fail();
		for _ in 0..16 {
			clears.acknowledge(&safety);
		}
		// Even a racing clear of the separate blocked atomic cannot override failure.
		safety.blocked.store(false, Ordering::SeqCst);
		assert!(!safety.allows(safety.epoch()));
	}

	#[test]
	fn deletion_epoch_rejects_queued_saves_and_returning_loads() {
		let mut store = Ok(LocalStore::open(std::path::Path::new(":memory:")).unwrap());
		let safety = HistorySafety::default();
		let account = Id(1);
		let channel = Id(2);
		let snapshot = vec![
			test_support::message(10, channel),
			test_support::message(11, channel),
		];
		for owner in [account, Id(9)] {
			assert!(matches!(
				execute(
					&mut store,
					&safety,
					owner,
					0,
					Operation::SaveChannel {
						channel,
						messages: snapshot.clone(),
					}
				),
				Outcome::Saved
			));
		}
		let loaded = execute(
			&mut store,
			&safety,
			account,
			0,
			Operation::LoadChannel {
				channel,
				request: 5,
			},
		);
		let Outcome::Channel {
			epoch, messages, ..
		} = loaded
		else {
			panic!()
		};
		assert_eq!(messages.len(), 2);
		let mut state = client_core::State {
			user: Some(test_support::message(1, channel).author),
			auth: client_core::auth::AuthState::Authenticated,
			gateway_connected: true,
			selected: Some(channel),
			freshness: model::Freshness::Fresh,
			channels: vec![model::Channel {
				id: channel,
				guild: None,
				parent_id: None,
				kind: 1,
				name: "Synthetic DM".into(),
				position: 0,
				recipients: vec![],
				last_message: None,
				member_list_id: None,
				message_count: None,
			}],
			..Default::default()
		};
		let mut reply = test_support::message(12, channel);
		reply.kind = 19;
		reply.reply_to = Some(Id(10));
		reply.reply_deleted = true;
		state.apply(client_core::Envelope {
			generation: state.generation,
			event: client_core::Event::Message(reply),
		});
		let deletions = state.take_reply_deletions();
		assert_eq!(deletions, vec![(channel, Id(10))]);
		safety.invalidate();
		assert!(!safety.allows(epoch));
		assert!(matches!(
			execute(
				&mut store,
				&safety,
				account,
				0,
				Operation::LoadChannel {
					channel,
					request: 5
				}
			),
			Outcome::Saved
		));
		execute(
			&mut store,
			&safety,
			account,
			safety.epoch(),
			Operation::DeleteMessages {
				channel,
				ids: deletions.into_iter().map(|(_, id)| id).collect(),
			},
		);
		execute(
			&mut store,
			&safety,
			account,
			0,
			Operation::SaveChannel {
				channel,
				messages: snapshot,
			},
		);
		let current = store
			.as_ref()
			.unwrap()
			.load_channel(account, channel)
			.unwrap();
		assert_eq!(
			current.iter().map(|m| m.id).collect::<Vec<_>>(),
			vec![Id(11)]
		);
		assert_eq!(
			store
				.as_ref()
				.unwrap()
				.load_channel(Id(9), channel)
				.unwrap()
				.len(),
			2
		);
	}

	#[test]
	fn full_queue_blocks_history_until_scoped_cleanup_and_io_failure_stays_closed() {
		let (send, commands) = mpsc::sync_channel(16);
		let (_, receive) = mpsc::sync_channel(16);
		let cache = Cache {
			send,
			receive,
			history: Arc::new(HistorySafety::default()),
		};
		let account = Id(1);
		let channel = Id(2);
		for _ in 0..16 {
			assert!(cache.queue(0, account, Operation::LoadDrafts));
		}
		assert!(!cache.delete_messages(0, account, channel, vec![Id(10)]));
		assert!(!cache.history.allows(cache.history.epoch()));
		assert!(!cache.queue(
			0,
			account,
			Operation::LoadChannel {
				channel,
				request: 1
			}
		));
		assert!(!cache.queue(
			0,
			account,
			Operation::SaveChannel {
				channel,
				messages: vec![]
			}
		));
		for _ in 0..16 {
			commands.try_recv().unwrap();
		}
		assert!(cache.queue(1, account, Operation::ClearHistory));
		let (generation, owner, epoch, operation) = commands.try_recv().unwrap();
		assert_eq!((generation, owner), (1, account));
		let mut store = Ok(LocalStore::open(std::path::Path::new(":memory:")).unwrap());
		for owner in [account, Id(9)] {
			store
				.as_mut()
				.unwrap()
				.save_channel(owner, channel, &[test_support::message(10, channel)])
				.unwrap();
			store
				.as_mut()
				.unwrap()
				.save_draft(owner, channel, "preserved draft")
				.unwrap();
		}
		assert!(matches!(
			execute(&mut store, &cache.history, owner, epoch, operation),
			Outcome::HistoryCleared
		));
		cache.history.cleared();
		assert!(cache.history.allows(cache.history.epoch()));
		assert!(
			store
				.as_ref()
				.unwrap()
				.load_channel(account, channel)
				.unwrap()
				.is_empty()
		);
		assert_eq!(
			store
				.as_ref()
				.unwrap()
				.load_channel(Id(9), channel)
				.unwrap()
				.len(),
			1
		);
		assert_eq!(
			store.as_ref().unwrap().load_drafts(account).unwrap()[&channel],
			"preserved draft"
		);
		let mut failed = Err(StoreError::Unavailable);
		assert!(matches!(
			execute(
				&mut failed,
				&cache.history,
				account,
				epoch,
				Operation::DeleteMessages {
					channel,
					ids: vec![Id(10)]
				}
			),
			Outcome::Failed { .. }
		));
		cache.history.cleared();
		assert!(!cache.history.allows(cache.history.epoch()));
		assert!(cache.queue(
			2,
			account,
			Operation::SaveDraft {
				channel,
				content: "still usable".into()
			}
		));
	}
}
