//! Discord wire DTOs. JSON values never become application state.
mod attachments;
mod embeds;
pub mod notifications;
pub mod profile;
mod reactions;
pub mod read_state;
pub mod search;
use attachments::AttachmentList;
use embeds::EmbedList;
use model::{Channel, Guild, Id, Message, MessagePatch, Patch, User};
pub use reactions::{GuildEmojisUpdate, ReactionTarget};
use serde::Deserialize;
use serde_json::value::RawValue;

#[derive(Deserialize, Debug, PartialEq, Eq)]
#[serde(try_from = "String")]
pub struct Timestamp(i128);
impl TryFrom<String> for Timestamp {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        time::OffsetDateTime::parse(&value, &time::format_description::well_known::Rfc3339)
            .map(|t| Self(t.unix_timestamp_nanos()))
            .map_err(|_| "Invalid service timestamp")
    }
}
pub const MAX_WIRE: usize = 4 * 1024 * 1024;
#[derive(Debug, thiserror::Error)]
#[error("Unsupported or oversized Discord payload")]
pub struct DecodeError;
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, DecodeError> {
    if bytes.len() > MAX_WIRE {
        return Err(DecodeError);
    }
    serde_json::from_slice(bytes).map_err(|_| DecodeError)
}
#[derive(Deserialize)]
pub struct UserDto {
    pub id: Id,
    pub username: String,
    #[serde(default)]
    pub global_name: Option<String>,
    #[serde(default)]
    pub bot: bool,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub discriminator: String,
}
impl UserDto {
    pub fn into_model(self) -> User {
        User {
            id: self.id,
            name: self
                .global_name
                .unwrap_or(self.username)
                .chars()
                .take(128)
                .collect(),
            avatar: self.avatar.filter(|hash| model::valid_avatar_hash(hash)),
            discriminator: self
                .discriminator
                .parse::<u16>()
                .ok()
                .filter(|n| *n <= 9999)
                .unwrap_or(0),
        }
    }
}
#[derive(Deserialize)]
pub struct ChannelDto {
    #[serde(default)]
    pub last_message_id: Option<Id>,
    pub id: Id,
    #[serde(default)]
    pub guild_id: Option<Id>,
    #[serde(default)]
    pub parent_id: Option<Id>,
    #[serde(default)]
    pub position: i32,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub kind: u8,
    #[serde(default)]
    pub recipients: Vec<UserDto>,
    #[serde(default)]
    pub permission_overwrites: Option<Vec<Overwrite>>,
}
impl ChannelDto {
    pub fn into_model(self) -> Channel {
        let recipients: Vec<_> = self
            .recipients
            .into_iter()
            .take(64)
            .map(UserDto::into_model)
            .collect();
        Channel {
            last_message: self.last_message_id,
            id: self.id,
            guild: self.guild_id,
            parent_id: self.parent_id,
            position: self.position,
            name: self.name.unwrap_or_else(|| {
                recipients
                    .iter()
                    .map(|u| u.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            }),
            kind: self.kind,
            recipients,
            member_list_id: None,
        }
    }
}
#[derive(Deserialize)]
pub struct ChannelPatchDto {
    #[serde(default)]
    pub last_message_id: Patch<Id>,
    pub id: Id,
    #[serde(default)]
    pub name: Patch<String>,
    #[serde(default)]
    pub parent_id: Patch<Id>,
    #[serde(default)]
    pub position: Patch<i32>,
    #[serde(rename = "type", default)]
    pub kind: Patch<u8>,
    #[serde(default)]
    pub permission_overwrites: Patch<Vec<Overwrite>>,
    #[serde(default)]
    pub flags: Patch<u64>,
}
impl ChannelPatchDto {
    pub fn into_model(self) -> model::ChannelPatch {
        model::ChannelPatch {
            last_message: self.last_message_id,
            id: self.id,
            name: self.name,
            parent_id: self.parent_id,
            position: self.position,
            kind: self.kind,
        }
    }
}
#[cfg(test)]
mod channel_tests {
    use super::*;
    #[test]
    fn category_metadata_and_partial_channel_updates() {
        let channel = decode::<ChannelDto>(
            br#"{"id":"3","guild_id":"1","type":0,"name":"general","position":12,"parent_id":"2"}"#,
        )
        .unwrap()
        .into_model();
        assert_eq!(channel.parent_id, Some(Id(2)));
        assert_eq!(channel.position, 12);
        assert!(channel.supports_text());
        let category =
            decode::<ChannelDto>(br#"{"id":"2","guild_id":"1","type":4,"name":"Category"}"#)
                .unwrap()
                .into_model();
        assert!(!category.supports_text());
        let patch = decode::<ChannelPatchDto>(br#"{"id":"3","position":0,"parent_id":null}"#)
            .unwrap()
            .into_model();
        assert_eq!(patch.parent_id, Patch::Null);
        assert_eq!(patch.position, Patch::Value(0));
        assert_eq!(patch.name, Patch::Absent);
        assert_eq!(patch.kind, Patch::Absent);
        assert_eq!(
            decode::<ChannelPatchDto>(br#"{"id":"3"}"#)
                .unwrap()
                .into_model()
                .parent_id,
            Patch::Absent
        );
        assert!(decode::<ChannelDto>(br#"{"id":"3","type":0,"position":4294967296}"#).is_err());
    }
}
#[derive(Deserialize)]
pub struct GuildDto {
    #[serde(default)]
    pub emojis: Option<reactions::CustomEmojiList>,
    pub id: Id,
    #[serde(default)]
    pub properties: Option<GuildProperties>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub channels: Vec<ChannelDto>,
    #[serde(default)]
    pub roles: Vec<RoleDto>,
    #[serde(default)]
    pub voice_states: Vec<VoiceStateDto>,
    #[serde(default)]
    pub members: Vec<VoiceMemberDto>,
}
#[derive(Deserialize)]
pub struct GuildProperties {
    #[serde(default)]
    pub name: Patch<String>,
    #[serde(default)]
    pub icon: Patch<String>,
}
#[derive(Deserialize)]
pub struct GuildPatchDto {
    pub id: Id,
    #[serde(flatten)]
    pub properties: GuildProperties,
}
impl GuildPatchDto {
    pub fn into_model(self) -> model::GuildPatch {
        model::GuildPatch {
            id: self.id,
            name: self.properties.name,
            icon: self.properties.icon,
        }
    }
}
#[derive(Deserialize)]
pub struct Ready {
    #[serde(default)]
    pub users: Vec<UserDto>,
    #[serde(default)]
    pub read_state: Option<read_state::Snapshot>,
    #[serde(default)]
    pub user_guild_settings: Option<notifications::Snapshot>,
    #[serde(default)]
    pub sessions: Option<notifications::Sessions>,
    pub user: UserDto,
    pub session_id: String,
    pub resume_gateway_url: String,
    #[serde(default)]
    pub guilds: Vec<GuildDto>,
    #[serde(default)]
    pub private_channels: Vec<ChannelDto>,
}
impl Ready {
    pub fn navigation(&mut self) -> (Vec<Guild>, Vec<Channel>) {
        let mut channels: Vec<_> = std::mem::take(&mut self.private_channels)
            .into_iter()
            .map(ChannelDto::into_model)
            .collect();
        let guilds = std::mem::take(&mut self.guilds)
            .into_iter()
            .map(|mut g| {
                // Normal-user READY may wrap guild identity in `properties`; public bot objects
                // are flat. The nested values override only fields actually present there.
                if let Some(properties) = g.properties {
                    match properties.name {
                        Patch::Value(name) => g.name = name,
                        Patch::Null => g.name.clear(),
                        Patch::Absent => {}
                    }
                    match properties.icon {
                        Patch::Value(icon) => g.icon = Some(icon),
                        Patch::Null => g.icon = None,
                        Patch::Absent => {}
                    }
                }
                let everyone = g
                    .roles
                    .iter()
                    .find(|r| r.id == g.id)
                    .and_then(|r| r.permissions.parse::<u64>().ok());
                channels.extend(g.channels.into_iter().map(|mut c| {
                    c.guild_id = Some(g.id);
                    let list_id = everyone.and_then(|permissions| {
                        c.permission_overwrites
                            .as_ref()
                            .and_then(|o| member_list_id(permissions, o))
                    });
                    let mut channel = c.into_model();
                    channel.member_list_id = list_id;
                    channel
                }));
                Guild {
                    emojis: g.emojis.map(|emojis| emojis.0),
                    id: g.id,
                    name: g.name.chars().take(128).collect(),
                    icon: g.icon.filter(|hash| model::valid_avatar_hash(hash)),
                }
            })
            .collect();
        (guilds, channels)
    }
}
#[derive(Deserialize, Default)]
pub struct MentionList(#[serde(deserialize_with = "model::deserialize_mentions")] pub Vec<UserDto>);
#[derive(Deserialize)]
pub struct MessageDto {
    #[serde(default)]
    pub reactions: reactions::ReactionList,
    pub id: Id,
    pub channel_id: Id,
    pub author: UserDto,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub mentions: MentionList,
    #[serde(default)]
    pub edited_timestamp: Option<Timestamp>,
    #[serde(default)]
    pub nonce: Option<Nonce>,
    #[serde(default)]
    pub message_reference: Option<Reference>,
    #[serde(default)]
    pub attachments: AttachmentList,
    #[serde(default)]
    pub embeds: EmbedList,
    #[serde(default)]
    pub flags: u64,
    #[serde(rename = "type", default)]
    pub kind: u8,
}
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Nonce {
    Text(String),
    Number(u64),
}
#[derive(Deserialize)]
pub struct Reference {
    pub message_id: Option<Id>,
}
impl MessageDto {
    pub fn into_model(self) -> Message {
        Message {
            reactions: Some(self.reactions.0),
            id: self.id,
            channel: self.channel_id,
            author: self.author.into_model(),
            content: self.content,
            mentions: self
                .mentions
                .0
                .into_iter()
                .map(UserDto::into_model)
                .collect(),
            edited: self.edited_timestamp.is_some(),
            edited_at: self.edited_timestamp.map(|t| t.0),
            revision: 0,
            nonce: self.nonce.map(|n| match n {
                Nonce::Text(s) => s,
                Nonce::Number(n) => n.to_string(),
            }),
            reply_to: self.message_reference.and_then(|r| r.message_id),
            unsupported: !matches!(self.kind, 0 | 19 | 20 | 23),
            attachments: self.attachments.0,
            embeds: embeds::bounded(self.embeds.0),
            embeds_suppressed: self.flags & 4 != 0,
        }
    }
}
#[derive(Deserialize)]
pub struct PatchDto {
    #[serde(default)]
    pub reactions: Patch<reactions::ReactionList>,
    pub id: Id,
    pub channel_id: Id,
    #[serde(default)]
    pub content: Patch<String>,
    #[serde(default)]
    pub mentions: Patch<MentionList>,
    #[serde(default)]
    pub edited_timestamp: Patch<Timestamp>,
    #[serde(default)]
    pub embeds: Patch<EmbedList>,
    #[serde(default)]
    pub attachments: Patch<AttachmentList>,
    #[serde(default)]
    pub flags: Patch<u64>,
}
impl PatchDto {
    pub fn into_model(self) -> MessagePatch {
        MessagePatch {
            reactions: match self.reactions {
                Patch::Absent => Patch::Absent,
                Patch::Null => Patch::Null,
                Patch::Value(list) => Patch::Value(list.0),
            },
            id: self.id,
            channel: self.channel_id,
            content: self.content,
            mentions: match self.mentions {
                Patch::Absent => Patch::Absent,
                Patch::Null => Patch::Null,
                Patch::Value(users) => {
                    Patch::Value(users.0.into_iter().map(UserDto::into_model).collect())
                }
            },
            attachments: match self.attachments {
                Patch::Absent => Patch::Absent,
                Patch::Null => Patch::Null,
                Patch::Value(values) => Patch::Value(values.0),
            },
            embeds: match self.embeds {
                Patch::Absent => Patch::Absent,
                Patch::Null => Patch::Null,
                Patch::Value(values) => Patch::Value(embeds::bounded(values.0)),
            },
            embeds_suppressed: match self.flags {
                Patch::Absent => Patch::Absent,
                Patch::Null => Patch::Null,
                Patch::Value(flags) => Patch::Value(flags & 4 != 0),
            },
            edited: match self.edited_timestamp {
                Patch::Absent => Patch::Absent,
                Patch::Null => Patch::Null,
                Patch::Value(t) => Patch::Value(t.0),
            },
        }
    }
}
#[derive(Deserialize)]
pub struct Deleted {
    pub id: Id,
    pub channel_id: Id,
}
#[derive(Deserialize)]
pub struct BulkDeleted {
    pub ids: Vec<Id>,
    pub channel_id: Id,
}
#[derive(Deserialize)]
pub struct GatewayPacket {
    pub op: u8,
    #[serde(default)]
    pub s: Option<u64>,
    #[serde(default)]
    pub t: Option<String>,
    pub d: Box<RawValue>,
}
#[derive(Deserialize)]
pub struct Hello {
    pub heartbeat_interval: u64,
}
#[derive(Deserialize)]
pub struct GatewayLocation {
    pub url: String,
}
#[derive(Deserialize, Default)]
pub struct ErrorBody {
    #[serde(default)]
    pub code: Option<u64>,
    #[serde(default)]
    pub retry_after: Option<f64>,
    #[serde(default)]
    pub global: bool,
    #[serde(default)]
    pub captcha_key: Option<Box<RawValue>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_message_mentions_and_patch_presence() {
        let message=decode::<MessageDto>(br#"{"id":"1","channel_id":"2","author":{"id":"3","username":"author"},"content":"<@4>","mentions":[{"id":"4","username":"user","global_name":"Display name"}]}"#).unwrap().into_model();
        assert_eq!(message.mentions[0].id, Id(4));
        assert_eq!(message.mentions[0].name, "Display name");
        assert!(matches!(
            decode::<PatchDto>(br#"{"id":"1","channel_id":"2"}"#)
                .unwrap()
                .into_model()
                .mentions,
            Patch::Absent
        ));
        assert!(matches!(
            decode::<PatchDto>(br#"{"id":"1","channel_id":"2","mentions":null}"#)
                .unwrap()
                .into_model()
                .mentions,
            Patch::Null
        ));
        let large = serde_json::json!({"id":"1","channel_id":"2","mentions":(1..=101).map(|id|serde_json::json!({"id":id.to_string(),"username":"synthetic"})).collect::<Vec<_>>()});
        assert!(decode::<PatchDto>(&serde_json::to_vec(&large).unwrap()).is_err());
    }
    #[test]
    fn precision_patches_and_hostile_payloads() {
        let id: Id = decode(br#""18446744073709551615""#).unwrap();
        assert_eq!(id.0, u64::MAX);
        assert!(decode::<Id>(b"123").is_err());
        let patch: PatchDto = decode(br#"{"id":"1","channel_id":"2","content":null}"#).unwrap();
        assert_eq!(patch.content, Patch::Null);
        assert_eq!(patch.edited_timestamp, Patch::Absent);
        assert!(decode::<GatewayPacket>(b"{").is_err());
        assert!(decode::<GatewayPacket>(&vec![b' '; MAX_WIRE + 1]).is_err());
        assert!(decode::<serde_json::Value>(&[vec![b'['; 200], vec![b']'; 200]].concat()).is_err());
    }
}

#[derive(Deserialize)]
pub struct RoleDto {
    pub id: Id,
    pub permissions: String,
}
#[derive(Deserialize)]
pub struct Overwrite {
    pub id: Id,
    pub allow: String,
    pub deny: String,
}
// Unofficial list identity observed by discord.py-self (abc.GuildChannel.member_list_id).
// This hash selects a server list; it never grants permissions.
fn member_list_id(everyone: u64, overwrites: &[Overwrite]) -> Option<String> {
    if overwrites.len() > 1000
        || overwrites
            .iter()
            .any(|o| o.allow.parse::<u64>().is_err() || o.deny.parse::<u64>().is_err())
    {
        return None;
    }
    let view = 1 << 10;
    if everyone & view != 0
        && !overwrites
            .iter()
            .any(|o| o.deny.parse::<u64>().unwrap_or(0) & view != 0)
    {
        return Some("everyone".into());
    }
    let mut entries: Vec<_> = overwrites
        .iter()
        .filter_map(|o| {
            if o.allow.parse::<u64>().unwrap_or(0) & view != 0 {
                Some(format!("allow:{}", o.id))
            } else if o.deny.parse::<u64>().unwrap_or(0) & view != 0 {
                Some(format!("deny:{}", o.id))
            } else {
                None
            }
        })
        .collect();
    entries.sort();
    Some(murmur3(entries.join(",").as_bytes()).to_string())
}
fn murmur3(bytes: &[u8]) -> u32 {
    let mix = |n: u32| {
        n.wrapping_mul(0xcc9e2d51)
            .rotate_left(15)
            .wrapping_mul(0x1b873593)
    };
    let mut hash = 0u32;
    let (chunks, remainder) = bytes.as_chunks::<4>();
    for part in chunks {
        hash ^= mix(u32::from_le_bytes(*part));
        hash = hash
            .rotate_left(13)
            .wrapping_mul(5)
            .wrapping_add(0xe6546b64);
    }
    let tail = remainder
        .iter()
        .enumerate()
        .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (i * 8)));
    if !remainder.is_empty() {
        hash ^= mix(tail);
    }
    hash ^= bytes.len() as u32;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85ebca6b);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xc2b2ae35);
    hash ^ (hash >> 16)
}
#[derive(Deserialize)]
pub struct MemberDto {
    pub user: UserDto,
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub presence: Option<PresenceDto>,
}
#[derive(Deserialize)]
pub struct PresenceDto {
    pub status: String,
}
#[derive(Deserialize)]
#[serde(untagged)]
pub enum MemberItem {
    Member { member: MemberDto },
    Group { group: MemberGroup },
}
#[derive(Deserialize)]
pub struct MemberGroup {
    pub id: String,
    pub count: u64,
}
impl MemberItem {
    pub fn into_model(self) -> Option<model::Member> {
        match self {
            Self::Group { .. } => None,
            Self::Member { member: m } => Some(model::Member {
                user: m.user.into_model(),
                nick: m.nick.map(|n| n.chars().take(128).collect()),
                status: m.presence.and_then(|p| match p.status.as_str() {
                    "online" | "idle" | "dnd" | "offline" => Some(p.status),
                    _ => None,
                }),
            }),
        }
    }
}
#[derive(Deserialize)]
#[serde(tag = "op")]
pub enum MemberOp {
    #[serde(rename = "SYNC")]
    Sync {
        range: [usize; 2],
        items: Vec<MemberItem>,
    },
    #[serde(rename = "INVALIDATE")]
    Invalidate { range: [usize; 2] },
    #[serde(rename = "UPDATE")]
    Update { index: usize, item: MemberItem },
    #[serde(rename = "INSERT")]
    Insert { index: usize, item: MemberItem },
    #[serde(rename = "DELETE")]
    Delete { index: usize },
}
#[derive(Deserialize)]
pub struct MemberUpdate {
    pub guild_id: Id,
    pub id: String,
    pub member_count: u64,
    pub ops: Vec<MemberOp>,
}

#[cfg(test)]
mod member_tests {
    use super::*;
    #[test]
    fn avatars_recipients_and_permission_scoped_list_ids() {
        let mut nested: Ready = decode(br#"{"user":{"id":"1","username":"Synthetic"},"session_id":"synthetic","resume_gateway_url":"wss://gateway.discord.gg","guilds":[{"id":"2","properties":{"name":"Nested","icon":"0123456789abcdef0123456789abcdef"}},{"id":"3","name":"Flat fallback","icon":"0123456789abcdef0123456789abcdef","properties":{"icon":null}}]}"#).unwrap();
        let (nested_guilds, _) = nested.navigation();
        assert_eq!(nested_guilds[0].name, "Nested");
        assert!(nested_guilds[0].icon_key().is_some());
        assert_eq!(nested_guilds[1].name, "Flat fallback");
        assert!(nested_guilds[1].icon.is_none());
        let patch = decode::<GuildPatchDto>(br#"{"id":"2","icon":null}"#)
            .unwrap()
            .into_model();
        assert_eq!(patch.name, Patch::Absent);
        assert_eq!(patch.icon, Patch::Null);
        let patch = decode::<GuildPatchDto>(br#"{"id":"2","name":"Renamed"}"#)
            .unwrap()
            .into_model();
        assert_eq!(patch.name, Patch::Value("Renamed".into()));
        assert_eq!(patch.icon, Patch::Absent);
        let mut ready: Ready = decode(br#"{"user":{"id":"1","username":"Synthetic"},"session_id":"synthetic","resume_gateway_url":"wss://gateway.discord.gg","guilds":[{"id":"2","name":"Server","icon":"a_0123456789abcdef0123456789abcdef"},{"id":"3","name":"Missing icon","icon":"../../invalid"}]}"#).unwrap();
        let (guilds, _) = ready.navigation();
        assert_eq!(
            guilds[0].icon_key().as_deref(),
            Some("guild-2-a_0123456789abcdef0123456789abcdef")
        );
        assert!(guilds[1].icon_key().is_none());
        let user: UserDto = decode(
            br#"{"id":"4194304","username":"Name","avatar":"../../invalid","discriminator":"0"}"#,
        )
        .unwrap();
        let user = user.into_model();
        assert!(user.avatar.is_none());
        assert_eq!(user.avatar_key(), "default-1");
        assert_eq!(
            user.avatar_url(),
            "https://cdn.discordapp.com/embed/avatars/1.png"
        );
        assert_eq!(murmur3(b""), 0);
        assert_eq!(murmur3(b"foo"), 0xf6a5c420);
        assert_eq!(murmur3(b"hello"), 0x248bfa47);
        assert_eq!(member_list_id(1024, &[]), Some("everyone".into()));
        assert_eq!(member_list_id(0, &[]), Some("0".into()));
        let deny = Overwrite {
            id: Id(5),
            allow: "0".into(),
            deny: "1024".into(),
        };
        assert_ne!(member_list_id(1024, &[deny]), Some("everyone".into()));
        assert!(
            member_list_id(
                1024,
                &[Overwrite {
                    id: Id(5),
                    allow: "0".into(),
                    deny: "invalid".into()
                }]
            )
            .is_none()
        );
        let channel:ChannelDto=decode(br#"{"id":"1","type":1,"recipients":[{"id":"2","username":"Person","avatar":"a_0123456789abcdef0123456789abcdef","discriminator":"1337"}]}"#).unwrap();
        let channel = channel.into_model();
        assert_eq!(channel.recipients.len(), 1);
        assert_eq!(
            channel.recipients[0].avatar_key(),
            "2-a_0123456789abcdef0123456789abcdef"
        );
    }
}

#[derive(Deserialize)]
pub struct MemberIdentity {
    pub user: UserIdentity,
}
#[derive(Deserialize)]
pub struct UserIdentity {
    pub id: Id,
}

#[derive(Deserialize)]
pub struct RecipientAdded {
    pub channel_id: Id,
    pub user: UserDto,
}
#[derive(Deserialize)]
pub struct RecipientRemoved {
    pub channel_id: Id,
    pub user: UserIdentity,
}

/// Normal-user DM call dispatches; public developer guild voice docs do not cover CALL_*.
#[derive(Deserialize)]
pub struct CallDto {
    pub channel_id: Id,
    #[serde(default)]
    pub ringing: Option<Vec<Id>>,
    #[serde(default)]
    pub voice_states: Option<Vec<VoiceStateDto>>,
    #[serde(default)]
    pub unavailable: bool,
}
#[derive(Deserialize)]
pub struct VoiceStateDto {
    #[serde(default)]
    pub guild_id: Option<Id>,
    pub channel_id: Option<Id>,
    pub user_id: Id,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub self_mute: bool,
    #[serde(default)]
    pub self_deaf: bool,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub deaf: bool,
    #[serde(default)]
    pub suppress: bool,
    #[serde(default)]
    pub member: Option<VoiceMemberDto>,
}
#[derive(Deserialize)]
pub struct VoiceServerDto {
    #[serde(default)]
    pub guild_id: Option<Id>,
    #[serde(default)]
    pub channel_id: Option<Id>,
    pub token: String,
    pub endpoint: Option<String>,
}

/// READY_SUPPLEMENTAL member identities can reference the READY users array.
#[derive(Deserialize)]
pub struct VoiceMemberDto {
    #[serde(default)]
    pub user: Option<UserDto>,
    #[serde(default)]
    pub user_id: Option<Id>,
    #[serde(default)]
    pub nick: Option<String>,
}
#[derive(Deserialize)]
pub struct ReadySupplemental {
    #[serde(default)]
    pub guilds: Vec<GuildDto>,
    #[serde(default)]
    pub merged_members: Vec<Vec<VoiceMemberDto>>,
}
#[derive(Deserialize)]
pub struct PassiveVoiceUpdate {
    #[serde(default)]
    pub guild_id: Option<Id>,
    #[serde(default)]
    pub updated_voice_states: Vec<VoiceStateDto>,
    #[serde(default)]
    pub removed_voice_states: Vec<Id>,
    #[serde(default)]
    pub updated_members: Vec<VoiceMemberDto>,
}
