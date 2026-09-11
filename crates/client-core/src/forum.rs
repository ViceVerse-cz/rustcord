//! Forum containers list their posts (threads) and create one post at a time.
use crate::{
	Command, MAX_CONTENT, MAX_EVENT_BYTES, MAX_NAV, State, auth::AuthState, auth::Failure,
};
use model::{Channel, Id, permissions as p};

pub const MAX_TITLE: usize = 100;

#[derive(Default)]
pub struct Posting {
	pub request: u64,
	pub pending: Option<(Id, u64)>,
	pub error: Option<&'static str>,
	/// A freshly created post the UI should open on its next frame.
	pub created: Option<Id>,
}

impl State {
	pub fn is_forum(&self, channel: Id) -> bool {
		self.channels
			.iter()
			.any(|c| c.id == channel && c.guild.is_some() && matches!(c.kind, 15 | 16))
	}

	/// Loaded posts of a forum: active threads the gateway delivered, newest activity first.
	pub fn forum_posts(&self, parent: Id) -> Vec<&Channel> {
		let mut posts: Vec<_> = self
			.channels
			.iter()
			.filter(|c| c.parent_id == Some(parent) && matches!(c.kind, 11 | 12))
			.collect();
		posts.sort_by_key(|c| std::cmp::Reverse(c.last_message.unwrap_or(c.id)));
		posts
	}

	pub fn can_create_post(&self, parent: Id) -> bool {
		self.auth == AuthState::Authenticated
			&& self.gateway_connected
			&& self.is_forum(parent)
			&& self.posting.pending.is_none()
			&& self.can_view(parent)
			&& self.permission(parent, p::SEND_MESSAGES) == Some(true)
	}

	pub fn create_post(&mut self, parent: Id, title: &str, content: &str) -> Option<Command> {
		let title = title.trim();
		let content = content.trim();
		if !self.can_create_post(parent)
			|| title.is_empty()
			|| title.chars().count() > MAX_TITLE
			|| content.is_empty()
			|| content.chars().count() > MAX_CONTENT
		{
			return None;
		}
		let guild = self.channels.iter().find(|c| c.id == parent)?.guild?;
		self.posting.request = self.posting.request.wrapping_add(1);
		self.posting.pending = Some((parent, self.posting.request));
		self.posting.error = None;
		Some(Command::CreatePost {
			parent,
			guild,
			title: title.to_owned(),
			content: content.to_owned(),
			request: self.posting.request,
		})
	}

	pub fn apply_post(&mut self, parent: Id, request: u64, result: Result<Channel, Failure>) {
		if self.posting.pending != Some((parent, request)) {
			return;
		}
		self.posting.pending = None;
		let guild = self
			.channels
			.iter()
			.find(|c| c.id == parent)
			.and_then(|c| c.guild);
		let post = match result {
			Err(failure) if failure.ends_session() && failure != Failure::Capacity => {
				self.fail(failure);
				return;
			}
			Err(failure) => {
				self.posting.error = Some(failure.label());
				return;
			}
			Ok(post) => post,
		};
		if guild.is_none()
			|| post.id.0 == 0
			|| post.kind != 11
			|| post.parent_id != Some(parent)
			|| post.guild != guild
			|| post.name.len() > 512
		{
			self.posting.error = Some("The service returned an unexpected post");
			return;
		}
		if let Some(existing) = self.channels.iter_mut().find(|c| c.id == post.id) {
			if existing.parent_id != post.parent_id || existing.guild != post.guild {
				self.posting.error = Some("Post conflicts with current navigation");
				return;
			}
			existing.message_count = post.message_count.or(existing.message_count);
		} else {
			let bytes = self.channels.iter().map(Channel::bytes).sum::<usize>()
				+ self.guilds.iter().map(model::Guild::bytes).sum::<usize>();
			if self.channels.len() + self.guilds.len() >= MAX_NAV
				|| bytes + post.bytes() > MAX_EVENT_BYTES
			{
				self.posting.error = Some("Post exceeds the navigation budget");
				return;
			}
			self.channels.push(post.clone());
		}
		self.posting.created = Some(post.id);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{Envelope, Event};

	fn channel(id: u64, parent: Option<Id>, kind: u8) -> Channel {
		Channel {
			id: Id(id),
			guild: Some(Id(1)),
			parent_id: parent,
			kind,
			name: "Synthetic".into(),
			position: 0,
			recipients: vec![],
			last_message: None,
			member_list_id: None,
			message_count: Some(3),
		}
	}
	fn state() -> State {
		let mut state = State {
			auth: AuthState::Authenticated,
			gateway_connected: true,
			user: Some(model::User {
				id: Id(2),
				name: "Synthetic".into(),
				avatar: None,
				discriminator: 0,
			}),
			guilds: vec![model::Guild {
				emojis: None,
				id: Id(1),
				name: "Synthetic".into(),
				icon: None,
			}],
			channels: vec![
				channel(10, None, 0),
				channel(20, None, 15),
				channel(21, Some(Id(20)), 11),
				channel(22, Some(Id(20)), 11),
			],
			..State::default()
		};
		state.channels[3].last_message = Some(Id(500));
		crate::tests::grant_permissions(&mut state);
		state
	}

	#[test]
	fn forum_selection_lists_posts_without_history_and_counts_replies() {
		let mut state = state();
		assert!(state.select(Id(20)).is_none());
		assert_eq!(state.selected, Some(Id(20)));
		assert!(!state.history_pending);
		let posts: Vec<_> = state.forum_posts(Id(20)).iter().map(|c| c.id).collect();
		assert_eq!(posts, vec![Id(22), Id(21)]);
		let message = model::Message {
			reactions: Some(vec![]),
			id: Id(600),
			channel: Id(21),
			author: state.user.clone().unwrap(),
			content: "Synthetic reply".into(),
			edited: false,
			edited_at: None,
			revision: 0,
			nonce: None,
			reply_to: None,
			kind: 0,
			reply_deleted: false,
			unsupported: false,
			extra_content: Default::default(),
			embeds: vec![],
			attachments: vec![],
			mention_roles: vec![],
			mention_everyone: false,
			suppress_notifications: false,
			mentions: Vec::new(),
			embeds_suppressed: false,
		};
		state.apply(Envelope {
			generation: state.generation,
			event: Event::Message(message),
		});
		let post = state.channels.iter().find(|c| c.id == Id(21)).unwrap();
		assert_eq!(post.message_count, Some(4));
		assert_eq!(post.last_message, Some(Id(600)));
		assert_eq!(state.forum_posts(Id(20))[0].id, Id(21));
	}

	#[test]
	fn posts_are_created_once_validated_and_opened_by_the_ui() {
		let mut state = state();
		assert!(state.create_post(Id(10), "Title", "Body").is_none());
		assert!(state.create_post(Id(20), "", "Body").is_none());
		assert!(state.create_post(Id(20), "Title", " ").is_none());
		let Some(Command::CreatePost { request, .. }) =
			state.create_post(Id(20), " Title ", "Body")
		else {
			panic!("forum post command expected");
		};
		assert!(!state.can_create_post(Id(20)), "One post at a time");
		state.apply_post(
			Id(20),
			request.wrapping_add(1),
			Ok(channel(30, Some(Id(20)), 11)),
		);
		assert!(state.posting.pending.is_some());
		state.apply_post(Id(20), request, Ok(channel(30, Some(Id(10)), 11)));
		assert!(state.posting.error.is_some());
		assert!(state.channels.iter().all(|c| c.id != Id(30)));
		let Some(Command::CreatePost { request, .. }) = state.create_post(Id(20), "Title", "Body")
		else {
			panic!("forum post command expected");
		};
		state.apply_post(Id(20), request, Ok(channel(30, Some(Id(20)), 11)));
		assert_eq!(state.posting.created.take(), Some(Id(30)));
		assert!(state.channels.iter().any(|c| c.id == Id(30)));
		assert!(state.select(Id(30)).is_some());
		let command = state.create_post(Id(20), "Title", "Body").unwrap();
		state.command_rejected(command);
		assert!(state.posting.pending.is_none());
		assert!(state.posting.error.is_some());
	}
}
