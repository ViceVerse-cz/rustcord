//! UI-neutral session entities. No filesystem or network dependencies.
pub mod archives;
pub mod permissions;
mod reading_preferences;
pub use reading_preferences::ReadingPreferences;
mod profile;
mod system_messages;
pub use profile::*;
mod attachments;
pub use attachments::*;
mod embeds;
pub use embeds::*;
mod extra_content;
pub use extra_content::{ExtraContent, ExtraContentPatch};
mod mentions;
pub use mentions::*;
mod reactions;
pub use reactions::*;
mod search;
pub use search::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub u64);
impl fmt::Display for Id {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}
impl FromStr for Id {
	type Err = &'static str;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if s.is_empty() || s.len() > 20 || !s.bytes().all(|b| b.is_ascii_digit()) {
			return Err("Invalid Discord ID");
		}
		s.parse::<u64>()
			.ok()
			.filter(|v| *v != 0)
			.map(Self)
			.ok_or("Invalid Discord ID")
	}
}
impl Serialize for Id {
	fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
		s.serialize_str(&self.to_string())
	}
}
impl<'de> Deserialize<'de> for Id {
	fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		String::deserialize(d)?
			.parse()
			.map_err(serde::de::Error::custom)
	}
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
	pub id: Id,
	pub name: String,
	pub avatar: Option<String>,
	pub discriminator: u16,
}
impl User {
	pub fn heap_bytes(&self) -> usize {
		self.name.capacity() + self.avatar.as_ref().map_or(0, String::capacity)
	}
	pub fn avatar_key(&self) -> String {
		if let Some(hash) = self
			.avatar
			.as_deref()
			.filter(|hash| valid_avatar_hash(hash))
		{
			format!("{}-{hash}", self.id)
		} else {
			let index = if self.discriminator == 0 {
				(self.id.0 >> 22) % 6
			} else {
				u64::from(self.discriminator % 5)
			};
			format!("default-{index}")
		}
	}
	pub fn avatar_url(&self) -> String {
		let key = self.avatar_key();
		if let Some(index) = key.strip_prefix("default-") {
			format!("https://cdn.discordapp.com/embed/avatars/{index}.png")
		} else {
			let (_, hash) = key.split_once('-').expect("avatar key");
			format!(
				"https://cdn.discordapp.com/avatars/{}/{hash}.png?size=128",
				self.id
			)
		}
	}
}
pub fn valid_avatar_hash(hash: &str) -> bool {
	let hash = hash.strip_prefix("a_").unwrap_or(hash);
	hash.len() == 32 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}
#[derive(Clone, PartialEq, Eq)]
pub struct Guild {
	pub emojis: Option<Vec<CustomEmoji>>,
	pub id: Id,
	pub name: String,
	pub icon: Option<String>,
}
impl Guild {
	pub fn bytes(&self) -> usize {
		std::mem::size_of::<Self>()
			+ self.name.capacity()
			+ self.icon.as_ref().map_or(0, String::capacity)
			+ self.emojis.as_ref().map_or(0, custom_emoji_bytes)
	}
	pub fn icon_key(&self) -> Option<String> {
		self.icon
			.as_deref()
			.filter(|hash| valid_avatar_hash(hash))
			.map(|hash| format!("guild-{}-{hash}", self.id))
	}
}
#[derive(Clone)]
pub struct GuildPatch {
	pub id: Id,
	pub name: Patch<String>,
	pub icon: Patch<String>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Channel {
	pub last_message: Option<Id>,
	pub id: Id,
	pub guild: Option<Id>,
	pub parent_id: Option<Id>,
	pub position: i32,
	pub name: String,
	pub kind: u8,
	pub recipients: Vec<User>,
	/// Unofficial service member-list identity; absent when permission metadata is missing.
	pub member_list_id: Option<String>,
	/// Thread reply count reported by the service; None for non-threads or unknown.
	pub message_count: Option<u32>,
}
impl Channel {
	pub fn bytes(&self) -> usize {
		size_of::<Self>()
			+ self.name.capacity()
			+ self.member_list_id.as_ref().map_or(0, String::capacity)
			+ self.recipients.capacity() * size_of::<User>()
			+ self.recipients.iter().map(User::heap_bytes).sum::<usize>()
	}
	pub fn supports_text(&self) -> bool {
		matches!(self.kind, 0 | 1 | 3 | 5 | 10..=12)
	}
}
#[derive(Clone)]
pub struct ChannelPatch {
	pub last_message: Patch<Id>,
	pub id: Id,
	pub name: Patch<String>,
	pub parent_id: Patch<Id>,
	pub position: Patch<i32>,
	pub kind: Patch<u8>,
	pub message_count: Patch<u32>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Message {
	/// Session-only counts; None means a service refresh is needed.
	pub reactions: Option<Vec<Reaction>>,
	pub id: Id,
	pub channel: Id,
	pub author: User,
	pub content: String,
	pub mentions: Vec<User>,
	/// Session-only service notification metadata; never inferred from message text.
	pub mention_roles: Vec<Id>,
	pub mention_everyone: bool,
	pub suppress_notifications: bool,
	pub edited: bool,
	pub edited_at: Option<i128>,
	pub revision: u64,
	pub nonce: Option<String>,
	pub reply_to: Option<Id>,
	/// Discord message type; 255 denotes an unknown legacy cached type.
	pub kind: u8,
	/// The service explicitly returned a null referenced message, not an unresolved preview.
	pub reply_deleted: bool,
	pub unsupported: bool,
	pub extra_content: ExtraContent,
	pub embeds: Vec<Embed>,
	pub embeds_suppressed: bool,
	pub attachments: Vec<Attachment>,
}
impl Message {
	/// A plain-text description, separate from the original service content.
	pub fn system_summary(&self) -> Option<String> {
		system_messages::summary(self)
	}

	pub fn display_text(&self) -> std::borrow::Cow<'_, str> {
		match self.system_summary() {
			Some(summary) if self.content.is_empty() => summary.into(),
			Some(summary) => format!("{summary}\n{}", self.content).into(),
			None => self.content.as_str().into(),
		}
	}

	pub fn bytes(&self) -> usize {
		size_of::<Self>()
			+ self.reactions.as_ref().map_or(0, |r| {
				reaction_bytes(r) + r.capacity().saturating_sub(r.len()) * size_of::<Reaction>()
			}) + self.content.capacity()
			+ self.author.heap_bytes()
			+ mention_bytes(&self.mentions)
			+ self.mention_roles.capacity() * size_of::<Id>()
			+ self.nonce.as_ref().map_or(0, String::capacity)
			+ attachment_bytes(&self.attachments)
			+ self
				.attachments
				.capacity()
				.saturating_sub(self.attachments.len())
				* size_of::<Attachment>()
			+ embed_bytes(&self.embeds)
			+ self.embeds.capacity().saturating_sub(self.embeds.len()) * size_of::<Embed>()
	}
}
pub const MAX_MENTION_ROLES: usize = 100;
pub fn valid_mention_roles(roles: &Vec<Id>) -> bool {
	roles.len() <= MAX_MENTION_ROLES
		&& roles.capacity() <= MAX_MENTION_ROLES
		&& roles
			.iter()
			.enumerate()
			.all(|(index, id)| id.0 != 0 && !roles[..index].contains(id))
}
/// Missing differs from explicit null in partial service updates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Patch<T> {
	#[default]
	Absent,
	Null,
	Value(T),
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Patch<T> {
	fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
		Ok(Option::<T>::deserialize(d)?.map_or(Self::Null, Self::Value))
	}
}
#[derive(Clone)]
pub struct MessagePatch {
	pub extra_content: ExtraContentPatch,
	pub reactions: Patch<Vec<Reaction>>,
	pub id: Id,
	pub channel: Id,
	pub content: Patch<String>,
	pub mentions: Patch<Vec<User>>,
	pub edited: Patch<i128>,
	pub embeds: Patch<Vec<Embed>>,
	pub embeds_suppressed: Patch<bool>,
	pub attachments: Patch<Vec<Attachment>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
	Loading,
	Fresh,
	Stale,
	Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
	Sending,
	Confirmed,
	Rejected,
	Ambiguous,
}

pub const MAX_RICH_ACTIVITIES: usize = 4;

/// A service image reference, never permission to fetch an arbitrary external URL.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivityImage {
	Asset { application: Id, asset: Id },
	Proxy(String),
	Application(Id),
}
impl ActivityImage {
	pub fn valid(&self) -> bool {
		match self {
			Self::Asset { application, asset } => application.0 != 0 && asset.0 != 0,
			Self::Application(id) => id.0 != 0,
			Self::Proxy(path) => {
				!path.is_empty()
					&& path.len() <= 1024
					&& !path.starts_with('/')
					&& !path
						.chars()
						.any(|c| c.is_control() || c.is_whitespace() || c == '\\')
					&& !path.split('/').any(|part| matches!(part, "." | ".."))
					&& !path.as_bytes().windows(3).any(|part| {
						part.eq_ignore_ascii_case(b"%2e")
							|| part.eq_ignore_ascii_case(b"%2f")
							|| part.eq_ignore_ascii_case(b"%5c")
					})
			}
		}
	}
	pub fn heap_bytes(&self) -> usize {
		match self {
			Self::Proxy(path) => path.capacity(),
			_ => 0,
		}
	}
	pub fn key(&self) -> String {
		match self {
			Self::Asset { application, asset } => format!("activity-{application}-{asset}"),
			Self::Proxy(path) => format!("embed:https://media.discordapp.net/{path}"),
			Self::Application(id) => format!("app-icon-{id}"),
		}
	}
}

/// Bounded activity metadata. Secrets and actions are never retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RichActivity {
	pub kind: u8,
	pub name: String,
	pub details: Option<String>,
	pub state: Option<String>,
	pub image: Option<ActivityImage>,
}
impl RichActivity {
	pub fn valid(&self) -> bool {
		matches!(self.kind, 0..=3 | 5)
			&& valid_presence_text(&self.name)
			&& self.details.as_deref().is_none_or(valid_presence_text)
			&& self.state.as_deref().is_none_or(valid_presence_text)
			&& self.image.as_ref().is_none_or(ActivityImage::valid)
	}
	pub fn heap_bytes(&self) -> usize {
		self.name.capacity()
			+ self.details.as_ref().map_or(0, String::capacity)
			+ self.state.as_ref().map_or(0, String::capacity)
			+ self.image.as_ref().map_or(0, ActivityImage::heap_bytes)
	}
	pub fn summary(&self) -> String {
		let verb = match self.kind {
			0 => "Playing",
			1 => "Streaming",
			2 => "Listening to",
			3 => "Watching",
			5 => "Competing in",
			_ => return self.name.clone(),
		};
		format!("{verb} {}", self.name)
	}
}

fn valid_presence_text(text: &str) -> bool {
	!text.is_empty()
		&& text.len() <= 512
		&& text.chars().count() <= 128
		&& text.trim() == text
		&& !text.chars().any(char::is_control)
}

/// Complete, bounded presence values for an already-loaded user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberPresence {
	pub user: Id,
	pub status: Option<String>,
	pub custom_status: Option<String>,
	pub activities: Vec<RichActivity>,
}

impl MemberPresence {
	pub fn valid(&self) -> bool {
		self.user.0 != 0
			&& self
				.status
				.as_deref()
				.is_none_or(|status| matches!(status, "online" | "idle" | "dnd" | "offline"))
			&& self
				.custom_status
				.as_deref()
				.is_none_or(valid_presence_text)
			&& self.activities.len() <= MAX_RICH_ACTIVITIES
			&& self.activities.iter().all(RichActivity::valid)
	}
	pub fn heap_bytes(&self) -> usize {
		self.status.as_ref().map_or(0, String::capacity)
			+ self.custom_status.as_ref().map_or(0, String::capacity)
			+ self.activities.capacity() * size_of::<RichActivity>()
			+ self
				.activities
				.iter()
				.map(RichActivity::heap_bytes)
				.sum::<usize>()
	}
}

/// Only the active member pane is retained; group rows and unloaded slots remain None.
#[derive(Clone)]
pub struct Member {
	pub roles: Vec<Id>,
	pub user: User,
	pub nick: Option<String>,
	pub status: Option<String>,
	/// Custom status text with any unicode emoji; bounded, never a rich activity.
	pub custom_status: Option<String>,
	pub activities: Vec<RichActivity>,
}
impl Member {
	pub fn valid(&self) -> bool {
		self.user.id.0 != 0
			&& self
				.status
				.as_deref()
				.is_none_or(|status| matches!(status, "online" | "idle" | "dnd" | "offline"))
			&& self
				.custom_status
				.as_deref()
				.is_none_or(valid_presence_text)
			&& self.activities.len() <= MAX_RICH_ACTIVITIES
			&& self.activities.iter().all(RichActivity::valid)
	}
	pub fn bytes(&self) -> usize {
		size_of::<Self>()
			+ self.roles.capacity() * size_of::<Id>()
			+ self.user.heap_bytes()
			+ self.nick.as_ref().map_or(0, String::capacity)
			+ self.status.as_ref().map_or(0, String::capacity)
			+ self.custom_status.as_ref().map_or(0, String::capacity)
			+ self.activities.capacity() * size_of::<RichActivity>()
			+ self
				.activities
				.iter()
				.map(RichActivity::heap_bytes)
				.sum::<usize>()
	}
}
#[derive(Clone)]
pub struct MemberList {
	pub guild: Option<Id>,
	pub channel: Id,
	pub request: u64,
	pub rows: Vec<Option<Member>>,
	pub total: u64,
	pub freshness: Freshness,
}

#[cfg(test)]
mod presence_tests {
	use super::*;

	#[test]
	fn rich_presence_validates_retained_fields_and_accounts_for_allocations() {
		let activity = RichActivity {
			kind: 0,
			name: "Synthetic".into(),
			details: Some("Level 2".into()),
			state: Some("In a party".into()),
			image: None,
		};
		let mut presence = MemberPresence {
			user: Id(2),
			status: None,
			custom_status: None,
			activities: vec![activity.clone(); MAX_RICH_ACTIVITIES],
		};
		assert!(presence.valid());
		assert_eq!(
			presence.heap_bytes(),
			presence.activities.capacity() * size_of::<RichActivity>()
				+ presence
					.activities
					.iter()
					.map(RichActivity::heap_bytes)
					.sum::<usize>()
		);
		presence.activities.push(activity.clone());
		assert!(!presence.valid());
		for text in [
			"",
			" padded",
			"control\n",
			&"x".repeat(129),
			&"🌙".repeat(129),
		] {
			let mut invalid = activity.clone();
			invalid.name = text.into();
			assert!(!invalid.valid());
			invalid.name = activity.name.clone();
			invalid.details = Some(text.into());
			assert!(!invalid.valid());
			invalid.details = None;
			invalid.state = Some(text.into());
			assert!(!invalid.valid());
		}
		for kind in [4, 6, 255] {
			assert!(
				!RichActivity {
					kind,
					..activity.clone()
				}
				.valid()
			);
		}
		let mut allocated = activity;
		allocated.name.reserve(100);
		let mut path = String::from("external/synthetic-hash-01/https/example.com/art.png");
		path.reserve(2048);
		allocated.image = Some(ActivityImage::Proxy(path));
		assert!(allocated.valid());
		assert_eq!(
			allocated.heap_bytes(),
			allocated.name.capacity()
				+ allocated.details.as_ref().unwrap().capacity()
				+ allocated.state.as_ref().unwrap().capacity()
				+ allocated.image.as_ref().unwrap().heap_bytes()
		);
	}

	#[test]
	fn activity_image_keys_preserve_only_bounded_service_references() {
		for (image, key) in [
			(
				ActivityImage::Asset {
					application: Id(10),
					asset: Id(20),
				},
				"activity-10-20",
			),
			(ActivityImage::Application(Id(10)), "app-icon-10"),
			(
				ActivityImage::Proxy("external/synthetic-hash-01/https/example.com/art.png".into()),
				"embed:https://media.discordapp.net/external/synthetic-hash-01/https/example.com/art.png",
			),
		] {
			assert!(image.valid());
			assert_eq!(image.key(), key);
		}
		assert!(!ActivityImage::Application(Id(0)).valid());
		assert!(
			!ActivityImage::Asset {
				application: Id(1),
				asset: Id(0)
			}
			.valid()
		);
		assert!(!ActivityImage::Proxy("external/../secret".into()).valid());
		assert!(!ActivityImage::Proxy("x".repeat(1025)).valid());
	}
}

#[cfg(test)]
mod notification_metadata_tests {
	use super::*;
	#[test]
	fn role_mentions_bound_identity_count_and_reserved_allocation() {
		assert!(valid_mention_roles(&Vec::new()));
		assert!(valid_mention_roles(&(1..=100).map(Id).collect()));
		assert!(!valid_mention_roles(&vec![Id(0)]));
		assert!(!valid_mention_roles(&vec![Id(1), Id(1)]));
		assert!(!valid_mention_roles(&(1..=101).map(Id).collect()));
		assert!(!valid_mention_roles(&Vec::with_capacity(101)));
	}
}
