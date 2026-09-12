//! A bounded page of active forum posts fetched on demand, mirroring the archive page budget.
use crate::{Channel, Id};

pub const PAGE_SIZE: usize = 25;
pub const MAX_BYTES: usize = 64 * 1024;
/// How many posts one forum may pull in before the list stops offering more.
pub const MAX_POSTS: usize = 200;

pub struct Page {
	pub threads: Vec<Channel>,
	pub more: bool,
}
impl Page {
	pub fn bytes(&self) -> usize {
		self.threads.capacity().saturating_sub(self.threads.len()) * size_of::<Channel>()
			+ self.threads.iter().map(Channel::bytes).sum::<usize>()
	}
	pub fn valid(&self, parent: Id, guild: Id) -> bool {
		parent.0 > 0
			&& guild.0 > 0
			&& self.threads.len() <= PAGE_SIZE
			&& self.bytes() <= MAX_BYTES
			&& (!self.more || !self.threads.is_empty())
			&& self.threads.iter().enumerate().all(|(i, thread)| {
				thread.id.0 > 0
					&& thread.id != parent
					&& thread.guild == Some(guild)
					&& thread.parent_id == Some(parent)
					&& thread.name.len() <= 512
					&& matches!(thread.kind, 10 | 11)
					&& self.threads[..i].iter().all(|other| other.id != thread.id)
			})
	}
}
