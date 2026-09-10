//! One on-demand profile. Profile metadata is never persisted by the client.
use crate::{Id, User, valid_avatar_hash};

pub const MAX_PROFILE_BYTES: usize = 64 * 1024;
#[derive(Clone)]
pub struct UserProfile {
    pub user: User,
    pub username: String,
    pub global_name: Option<String>,
    pub banner: Option<String>,
    pub accent_color: Option<u32>,
    pub bio: String,
    pub pronouns: String,
    pub badges: Vec<ProfileBadge>,
    pub connections: Vec<ProfileConnection>,
    pub mutual_guilds: Vec<ProfileGuild>,
    pub guild: Option<GuildProfile>,
    pub limited: bool,
}
#[derive(Clone)]
pub struct ProfileBadge {
    pub id: String,
    pub description: String,
}
#[derive(Clone)]
pub struct ProfileConnection {
    pub kind: String,
    pub name: String,
    pub verified: bool,
}
#[derive(Clone)]
pub struct ProfileGuild {
    pub id: Id,
    pub nick: Option<String>,
}
#[derive(Clone)]
pub struct GuildProfile {
    pub guild: Id,
    pub nick: Option<String>,
    pub avatar: Option<String>,
    pub banner: Option<String>,
    pub bio: String,
    pub pronouns: String,
    pub joined_at: Option<String>,
}
fn bytes(value: &Option<String>) -> usize {
    value.as_ref().map_or(0, String::capacity)
}
impl UserProfile {
    pub fn bytes(&self) -> usize {
        size_of::<Self>()
            + self.user.heap_bytes()
            + self.username.capacity()
            + bytes(&self.global_name)
            + bytes(&self.banner)
            + self.bio.capacity()
            + self.pronouns.capacity()
            + self.badges.capacity() * size_of::<ProfileBadge>()
            + self
                .badges
                .iter()
                .map(|b| b.id.capacity() + b.description.capacity())
                .sum::<usize>()
            + self.connections.capacity() * size_of::<ProfileConnection>()
            + self
                .connections
                .iter()
                .map(|c| c.kind.capacity() + c.name.capacity())
                .sum::<usize>()
            + self.mutual_guilds.capacity() * size_of::<ProfileGuild>()
            + self
                .mutual_guilds
                .iter()
                .map(|g| bytes(&g.nick))
                .sum::<usize>()
            + self.guild.as_ref().map_or(0, |g| {
                size_of::<GuildProfile>()
                    + bytes(&g.nick)
                    + bytes(&g.avatar)
                    + bytes(&g.banner)
                    + g.bio.capacity()
                    + g.pronouns.capacity()
                    + bytes(&g.joined_at)
            })
    }
    pub fn valid(&self) -> bool {
        self.bytes() <= MAX_PROFILE_BYTES
            && self.user.id.0 != 0
            && self.user.name.len() <= 512
            && self.username.len() <= 512
            && self.global_name.as_ref().is_none_or(|s| s.len() <= 512)
            && self.bio.len() <= 4096
            && self.pronouns.len() <= 256
            && self.accent_color.is_none_or(|c| c <= 0xff_ffff)
            && [&self.banner, &self.user.avatar]
                .into_iter()
                .all(|h| h.as_deref().is_none_or(valid_avatar_hash))
            && self.badges.len() <= 16
            && self.connections.len() <= 16
            && self.mutual_guilds.len() <= 50
            && self
                .badges
                .iter()
                .all(|b| b.id.len() <= 64 && b.description.len() <= 1024)
            && self
                .connections
                .iter()
                .all(|c| c.kind.len() <= 64 && c.name.len() <= 512)
            && self
                .mutual_guilds
                .iter()
                .all(|g| g.id.0 != 0 && g.nick.as_ref().is_none_or(|n| n.len() <= 512))
            && self.guild.as_ref().is_none_or(|g| {
                g.guild.0 != 0
                    && g.nick.as_ref().is_none_or(|n| n.len() <= 512)
                    && g.bio.len() <= 4096
                    && g.pronouns.len() <= 256
                    && g.joined_at.as_ref().is_none_or(|s| s.len() <= 64)
                    && [&g.avatar, &g.banner]
                        .into_iter()
                        .all(|h| h.as_deref().is_none_or(valid_avatar_hash))
            })
    }
    pub fn banner_key(&self) -> Option<String> {
        if let Some(guild) = &self.guild
            && let Some(hash) = guild.banner.as_deref().filter(|h| valid_avatar_hash(h))
        {
            return Some(format!(
                "member-banner-{}-{}-{hash}",
                guild.guild, self.user.id
            ));
        }
        self.banner
            .as_deref()
            .filter(|h| valid_avatar_hash(h))
            .map(|h| format!("banner-{}-{h}", self.user.id))
    }
    pub fn avatar_key(&self) -> String {
        if let Some(guild) = &self.guild
            && let Some(hash) = guild.avatar.as_deref().filter(|h| valid_avatar_hash(h))
        {
            return format!("member-avatar-{}-{}-{hash}", guild.guild, self.user.id);
        }
        self.user.avatar_key()
    }
}
