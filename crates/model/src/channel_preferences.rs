use crate::Id;

/// Device-local channel shortcuts, isolated by account; never synchronized to Discord.
/// At most 256 IDs (2 KiB of ID payload) across both lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ChannelPreferences {
	pub favorites: Vec<Id>,
	pub pinned: Vec<Id>,
}

impl ChannelPreferences {
	pub const MAX_ENTRIES: usize = 256;
	pub const MAX_JSON_BYTES: usize = 8192;

	pub fn is_valid(&self) -> bool {
		// ponytail: duplicate scans are capped at 256 IDs; use a set if this limit grows.
		self.favorites.len().saturating_add(self.pinned.len()) <= Self::MAX_ENTRIES
			&& self
				.favorites
				.capacity()
				.saturating_add(self.pinned.capacity())
				<= Self::MAX_ENTRIES * 2
			&& [&self.favorites, &self.pinned].into_iter().all(|ids| {
				ids.iter()
					.enumerate()
					.all(|(index, id)| id.0 != 0 && !ids[..index].contains(id))
			})
	}

	pub fn is_favorite(&self, channel: Id) -> bool {
		self.favorites.contains(&channel)
	}

	pub fn is_pinned(&self, channel: Id) -> bool {
		self.pinned.contains(&channel)
	}

	/// Returns false without changing the list when the channel or capacity is invalid.
	pub fn toggle_favorite(&mut self, channel: Id) -> bool {
		self.toggle(channel, true)
	}

	pub fn toggle_pinned(&mut self, channel: Id) -> bool {
		self.toggle(channel, false)
	}

	fn toggle(&mut self, channel: Id, favorite: bool) -> bool {
		if channel.0 == 0 || !self.is_valid() {
			return false;
		}
		let full = self.favorites.len() + self.pinned.len() == Self::MAX_ENTRIES;
		let ids = if favorite {
			&mut self.favorites
		} else {
			&mut self.pinned
		};
		if let Some(index) = ids.iter().position(|id| *id == channel) {
			ids.remove(index);
		} else if full {
			return false;
		} else {
			ids.push(channel);
		}
		true
	}
}
