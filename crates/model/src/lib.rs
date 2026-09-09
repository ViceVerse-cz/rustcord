//! UI-neutral session entities. No filesystem or network dependencies.
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
#[derive(Clone, PartialEq, Eq)]
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
    pub id: Id,
    pub name: String,
}
#[derive(Clone, PartialEq, Eq)]
pub struct Channel {
    pub id: Id,
    pub guild: Option<Id>,
    pub name: String,
    pub kind: u8,
    pub recipients: Vec<User>,
    /// Unofficial service member-list identity; absent when permission metadata is missing.
    pub member_list_id: Option<String>,
}
impl Channel {
    pub fn supports_text(&self) -> bool {
        matches!(self.kind, 0 | 1 | 3 | 5 | 10..=12)
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Message {
    pub id: Id,
    pub channel: Id,
    pub author: User,
    pub content: String,
    pub edited: bool,
    pub edited_at: Option<i128>,
    pub revision: u64,
    pub nonce: Option<String>,
    pub reply_to: Option<Id>,
    pub unsupported: bool,
}
impl Message {
    pub fn bytes(&self) -> usize {
        size_of::<Self>()
            + self.content.capacity()
            + self.author.heap_bytes()
            + self.nonce.as_ref().map_or(0, String::capacity)
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
    pub id: Id,
    pub channel: Id,
    pub content: Patch<String>,
    pub edited: Patch<i128>,
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
}
impl Member {
    pub fn bytes(&self) -> usize {
        size_of::<Self>()
            + self.user.heap_bytes()
            + self.nick.as_ref().map_or(0, String::capacity)
            + self.status.as_ref().map_or(0, String::capacity)
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
