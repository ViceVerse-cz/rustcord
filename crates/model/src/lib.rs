//! UI-neutral session entities. No filesystem or network dependencies.
pub mod archives;
mod profile;
pub use profile::*;
mod attachments;
pub use attachments::*;
mod embeds;
pub use embeds::*;
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
    pub edited: bool,
    pub edited_at: Option<i128>,
    pub revision: u64,
    pub nonce: Option<String>,
    pub reply_to: Option<Id>,
    pub unsupported: bool,
    pub embeds: Vec<Embed>,
    pub embeds_suppressed: bool,
    pub attachments: Vec<Attachment>,
}
impl Message {
    pub fn bytes(&self) -> usize {
        size_of::<Self>()
            + self.reactions.as_ref().map_or(0, |r| {
                reaction_bytes(r) + r.capacity().saturating_sub(r.len()) * size_of::<Reaction>()
            })
            + self.content.capacity()
            + self.author.heap_bytes()
            + mention_bytes(&self.mentions)
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

/// Only the active member pane is retained; group rows and unloaded slots remain None.
#[derive(Clone)]
pub struct Member {
    pub user: User,
    pub nick: Option<String>,
    pub status: Option<String>,
    /// Custom status text with any unicode emoji; bounded, never a rich activity.
    pub custom_status: Option<String>,
}
impl Member {
    pub fn bytes(&self) -> usize {
        size_of::<Self>()
            + self.user.heap_bytes()
            + self.nick.as_ref().map_or(0, String::capacity)
            + self.status.as_ref().map_or(0, String::capacity)
            + self.custom_status.as_ref().map_or(0, String::capacity)
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
