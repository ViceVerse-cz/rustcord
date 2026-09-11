use eframe::egui;
use tokio::sync::watch;

pub type Detection = Result<Option<&'static str>, &'static str>;

/// One replaceable result; detection runs off the UI and only while sharing is enabled.
pub async fn run(
	enabled: watch::Receiver<bool>,
	activity: watch::Sender<Option<String>>,
	report: watch::Sender<Detection>,
	ctx: egui::Context,
) {
	run_with_detector(
		enabled,
		activity,
		report,
		ctx,
		platform::game_activity::detect_game,
	)
	.await;
}

async fn run_with_detector(
	mut enabled: watch::Receiver<bool>,
	activity: watch::Sender<Option<String>>,
	report: watch::Sender<Detection>,
	ctx: egui::Context,
	detect: fn() -> Detection,
) {
	loop {
		let sharing = *enabled.borrow_and_update();
		let detected = if sharing {
			tokio::task::spawn_blocking(detect)
				.await
				.unwrap_or(Err("Game detection unavailable"))
		} else {
			Ok(None)
		};
		// Disabling while a scan is running must never publish that stale result.
		{
			let latest = enabled.borrow();
			if *latest != sharing {
				continue;
			}
			let game = detected.ok().flatten().map(str::to_owned);
			activity.send_if_modified(|current| {
				if *current == game {
					return false;
				}
				*current = game;
				true
			});
			if report.send_if_modified(|current| {
				if *current == detected {
					return false;
				}
				*current = detected;
				true
			}) {
				ctx.request_repaint();
			}
		}
		if sharing {
			tokio::select! {
				changed = enabled.changed() => if changed.is_err() { break; },
				_ = tokio::time::sleep(std::time::Duration::from_secs(15)) => {},
			}
		} else if enabled.changed().await.is_err() {
			break;
		}
	}
}

/// Coalesce toggles behind one SQLite write and protect a choice from a late load.
#[derive(Default)]
pub struct Settings {
	pub enabled: bool,
	pub touched: bool,
	pub dirty: bool,
	pub saving: bool,
	pub failed: bool,
}
impl Settings {
	pub fn observe(&mut self, enabled: bool) {
		if self.enabled != enabled {
			self.enabled = enabled;
			self.touched = true;
			self.dirty = true;
			self.failed = false;
		}
	}
	pub fn restore(&mut self, result: Result<bool, local_store::StoreError>) {
		if !self.touched {
			self.enabled = result.unwrap_or(false);
			self.failed = result.is_err();
		}
	}
	pub fn status(&self) -> &'static str {
		if self.failed {
			"Activity setting could not be saved or loaded. Toggle it to retry saving."
		} else if self.dirty || self.saving {
			"Saving activity setting…"
		} else {
			"Saved on this device. Applies to accounts used here."
		}
	}
	pub fn needs_attention(&self) -> bool {
		self.dirty || self.saving || self.failed
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[tokio::test]
	async fn sharing_starts_disabled_detects_and_clears_without_waiting_for_next_scan() {
		let (enabled, receive) = watch::channel(false);
		let (activity, mut games) = watch::channel(None);
		let (report, _) = watch::channel(Ok(None));
		let worker = tokio::spawn(run_with_detector(
			receive,
			activity,
			report,
			egui::Context::default(),
			|| Ok(Some("osu!")),
		));
		assert!(
			tokio::time::timeout(std::time::Duration::from_millis(30), games.changed())
				.await
				.is_err()
		);
		enabled.send(true).unwrap();
		tokio::time::timeout(std::time::Duration::from_secs(2), games.changed())
			.await
			.unwrap()
			.unwrap();
		assert_eq!(games.borrow_and_update().as_deref(), Some("osu!"));
		enabled.send(false).unwrap();
		tokio::time::timeout(std::time::Duration::from_secs(2), games.changed())
			.await
			.unwrap()
			.unwrap();
		assert!(games.borrow_and_update().is_none());
		drop(enabled);
		worker.await.unwrap();
	}
	#[test]
	fn choice_survives_late_load_and_failed_load_never_enables_sharing() {
		let mut settings = Settings::default();
		settings.restore(Err(local_store::StoreError::Unavailable));
		assert!(!settings.enabled);
		assert!(settings.failed);
		settings.observe(true);
		settings.restore(Ok(false));
		assert!(settings.enabled && settings.dirty && !settings.failed);
		settings.saving = true;
		settings.dirty = false;
		settings.observe(false);
		assert!(settings.dirty && settings.saving && !settings.enabled);
	}
}
