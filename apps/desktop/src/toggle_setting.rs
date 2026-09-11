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
			"Setting could not be saved or loaded. Toggle it to retry saving."
		} else if self.dirty || self.saving {
			"Saving setting…"
		} else {
			""
		}
	}
	pub fn needs_attention(&self) -> bool {
		self.dirty || self.saving || self.failed
	}
}
