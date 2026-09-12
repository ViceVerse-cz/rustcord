//! Read-only, bounded audit history. Change values are display text, never raw JSON state.
use crate::{Id, Patch, User};
pub const PAGE_SIZE: usize = 50;
pub const MAX_PAGE_BYTES: usize = 1024 * 1024;
pub const MAX_ENTRIES: usize = 500;
pub const MAX_USERS: usize = 1000;
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_CHANGES: usize = 64;
pub const MAX_VALUE_BYTES: usize = 4096;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Query {
	pub user: Option<Id>,
	pub action: Option<u16>,
	pub before: Option<Id>,
}
impl Query {
	pub fn valid(&self) -> bool {
		self.user.is_none_or(|id| id.0 != 0)
			&& self.before.is_none_or(|id| id.0 != 0)
			&& self.action.is_none_or(|action| action != 0)
	}
}
#[derive(Clone)]
pub struct Change {
	pub key: String,
	pub old: Patch<String>,
	pub new: Patch<String>,
}
#[derive(Clone)]
pub struct Detail {
	pub key: String,
	pub value: String,
}
#[derive(Clone)]
pub struct Entry {
	pub id: Id,
	pub user_id: Option<Id>,
	pub target_id: Option<String>,
	pub action_type: u16,
	pub reason: Option<String>,
	pub changes: Vec<Change>,
	pub options: Vec<Detail>,
}
#[derive(Clone)]
pub struct Page {
	pub guild: Id,
	pub entries: Vec<Entry>,
	pub users: Vec<User>,
	/// The service filled the requested page; another page may exist.
	pub has_more: bool,
}
impl Entry {
	pub fn bytes(&self) -> usize {
		size_of::<Self>()
			+ self.target_id.as_ref().map_or(0, String::capacity)
			+ self.reason.as_ref().map_or(0, String::capacity)
			+ self.changes.capacity() * size_of::<Change>()
			+ self
				.changes
				.iter()
				.map(|change| {
					change.key.capacity() + patch_bytes(&change.old) + patch_bytes(&change.new)
				})
				.sum::<usize>()
			+ self.options.capacity() * size_of::<Detail>()
			+ self
				.options
				.iter()
				.map(|detail| detail.key.capacity() + detail.value.capacity())
				.sum::<usize>()
	}
	pub fn valid(&self) -> bool {
		self.id.0 != 0
			&& self.action_type != 0
			&& self.user_id.is_none_or(|id| id.0 != 0)
			&& self.target_id.as_ref().is_none_or(|id| {
				!id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control)
			}) && self
			.reason
			.as_ref()
			.is_none_or(|reason| reason.len() <= 2048 && reason.chars().count() <= 512)
			&& self.changes.len() <= MAX_CHANGES
			&& self.changes.iter().all(|change| {
				key_valid(&change.key) && patch_valid(&change.old) && patch_valid(&change.new)
			}) && self.options.len() <= 16
			&& self
				.options
				.iter()
				.all(|detail| key_valid(&detail.key) && detail.value.len() <= MAX_VALUE_BYTES)
			&& self.bytes() <= 128 * 1024
	}
}
impl Page {
	pub fn bytes(&self) -> usize {
		size_of::<Self>()
			+ self.entries.iter().map(Entry::bytes).sum::<usize>()
			+ self.entries.capacity().saturating_sub(self.entries.len()) * size_of::<Entry>()
			+ self.users.capacity() * size_of::<User>()
			+ self.users.iter().map(User::heap_bytes).sum::<usize>()
	}
	pub fn valid(&self) -> bool {
		let mut users = std::collections::BTreeSet::new();
		self.guild.0 != 0
			&& self.entries.len() <= MAX_ENTRIES
			&& self.users.len() <= MAX_USERS
			&& self.bytes() <= MAX_BYTES
			&& self.entries.iter().all(Entry::valid)
			&& self.entries.windows(2).all(|pair| pair[0].id > pair[1].id)
			&& self.users.iter().all(|user| {
				user.id.0 != 0
					&& user.heap_bytes() <= 1024
					&& user.name.len() <= 512
					&& user.avatar.as_deref().is_none_or(crate::valid_avatar_hash)
					&& users.insert(user.id)
			})
	}
	pub fn valid_response(&self) -> bool {
		self.valid()
			&& self.entries.len() <= PAGE_SIZE
			&& self.users.len() <= 100
			&& self.bytes() + size_of::<crate::server_admin::Result>() <= MAX_PAGE_BYTES
			&& self.has_more == (self.entries.len() == PAGE_SIZE)
	}
	pub fn matches_query(&self, query: &Query) -> bool {
		query.valid()
			&& self.entries.iter().all(|entry| {
				query.user.is_none_or(|user| entry.user_id == Some(user))
					&& query
						.action
						.is_none_or(|action| entry.action_type == action)
					&& query.before.is_none_or(|before| entry.id < before)
			})
	}
}
fn patch_bytes(value: &Patch<String>) -> usize {
	match value {
		Patch::Value(value) => value.capacity(),
		_ => 0,
	}
}
fn patch_valid(value: &Patch<String>) -> bool {
	match value {
		Patch::Value(value) => value.len() <= MAX_VALUE_BYTES,
		_ => true,
	}
}
fn key_valid(key: &str) -> bool {
	!key.is_empty() && key.len() <= 128 && !key.chars().any(char::is_control)
}
