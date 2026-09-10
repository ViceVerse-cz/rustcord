//! Bounded session-only mirror for this account; Discord remains authoritative.
use crate::{Command, State, auth::AuthState};
use model::{Freshness, Id, Patch, ReactionEmoji, permissions as p};
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

pub const MAX_BYTES: usize = 2 * 1024 * 1024;
#[derive(Default)]
pub struct Permissions {
    cache: RefCell<BTreeMap<(Id, Id, Id), Decision>>,
    pub guilds: BTreeMap<Id, p::Guild>,
    pub channels: BTreeMap<Id, p::Channel>,
}
#[derive(Clone, Copy)]
struct Decision {
    at: i64,
    until: i64,
    bits: Option<u128>,
}
impl Clone for Permissions {
    fn clone(&self) -> Self {
        Self {
            guilds: self.guilds.clone(),
            channels: self.channels.clone(),
            cache: RefCell::default(),
        }
    }
}
pub enum Event {
    Snapshot(p::Snapshot),
    Guild(p::Guild),
    Role {
        guild: Id,
        role: p::Role,
    },
    RoleRemoved {
        guild: Id,
        id: Id,
    },
    Member {
        guild: Id,
        roles: Patch<Vec<Id>>,
        timeout_until: Patch<i64>,
    },
    Members(Vec<(Id, Patch<Vec<Id>>, Patch<i64>)>),
    Owner {
        guild: Id,
        owner: Patch<Id>,
    },
    Channel {
        channel: Id,
        guild: Option<Id>,
        overwrites: Patch<Vec<p::Overwrite>>,
    },
    UnavailableGuild(Id),
}
impl Event {
    pub fn bytes(&self) -> usize {
        size_of::<Self>()
            + match self {
                Self::Snapshot(snapshot) => snapshot.bytes(),
                Self::Guild(guild) => guild.bytes(),
                Self::Role { role, .. } => role.bytes() - size_of::<p::Role>(),
                Self::Members(members) => {
                    members.capacity() * size_of::<(Id, Patch<Vec<Id>>, Patch<i64>)>()
                        + members
                            .iter()
                            .map(|(_, roles, _)| match roles {
                                Patch::Value(roles) => roles.capacity() * size_of::<Id>(),
                                _ => 0,
                            })
                            .sum::<usize>()
                }
                Self::Member {
                    roles: Patch::Value(roles),
                    ..
                } => roles.capacity() * size_of::<Id>(),
                Self::Channel {
                    overwrites: Patch::Value(overwrites),
                    ..
                } => overwrites.capacity() * size_of::<p::Overwrite>(),
                _ => 0,
            }
    }
}
impl Permissions {
    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
    }
    fn effective(
        &self,
        target: Id,
        guild: &p::Guild,
        user: Id,
        overwrites: Option<&[p::Overwrite]>,
        now: i64,
    ) -> Option<u128> {
        let key = (guild.id, target, user);
        if let Some(decision) = self.cache.borrow().get(&key)
            && now >= decision.at
            && now < decision.until
        {
            return decision.bits;
        }
        let bits = p::effective(guild, user, overwrites, now);
        let until = guild
            .member
            .as_ref()
            .and_then(|member| member.timeout_until)
            .filter(|until| *until > now)
            .unwrap_or(i64::MAX);
        let mut cache = self.cache.borrow_mut();
        if cache.len() >= crate::MAX_NAV && !cache.contains_key(&key) {
            cache.clear();
        }
        cache.insert(
            key,
            Decision {
                at: now,
                until,
                bits,
            },
        );
        bits
    }
    pub fn bytes(&self) -> usize {
        self.guilds.values().map(p::Guild::bytes).sum::<usize>()
            + self.channels.values().map(p::Channel::bytes).sum::<usize>()
            + (self.guilds.len() + self.channels.len()) * 64
    }
    fn valid(&self) -> bool {
        self.guilds.len() + self.channels.len() <= crate::MAX_NAV
            && self.bytes() + crate::MAX_NAV * 128 <= MAX_BYTES
            && self
                .guilds
                .values()
                .map(|g| g.roles.as_ref().map_or(0, Vec::len))
                .sum::<usize>()
                <= 16_384
            && self
                .channels
                .values()
                .map(|c| c.overwrites.as_ref().map_or(0, Vec::len))
                .sum::<usize>()
                <= 32_768
            && self.guilds.values().all(|g| {
                g.id.0 != 0
                    && g.roles.as_ref().is_none_or(|roles| {
                        roles.len() <= 512
                            && roles.iter().all(|role| {
                                role.name.chars().count() <= 100 && role.color <= 0xff_ffff
                            })
                    })
                    && g.member
                        .as_ref()
                        .is_none_or(|member| member.roles.len() <= 512)
            })
            && self.channels.values().all(|c| {
                c.id.0 != 0
                    && c.guild.0 != 0
                    && c.overwrites
                        .as_ref()
                        .is_none_or(|overwrites| overwrites.len() <= 1000)
            })
    }
    pub fn replace(&mut self, snapshot: p::Snapshot) -> Result<(), &'static str> {
        let mut next = Self::default();
        next.update(Event::Snapshot(snapshot))?;
        *self = next;
        Ok(())
    }
    pub fn update(&mut self, event: Event) -> Result<(), &'static str> {
        // ponytail: at most 2 MiB is cloned for atomic validation; use scoped rollback if event cost becomes measurable.
        let mut next = self.clone();
        match event {
            Event::Snapshot(snapshot) => {
                let ids: BTreeSet<_> = snapshot.guilds.iter().map(|g| g.id).collect();
                let channels: BTreeSet<_> = snapshot.channels.iter().map(|c| c.id).collect();
                if ids.len() != snapshot.guilds.len()
                    || channels.len() != snapshot.channels.len()
                    || snapshot.channels.iter().any(|c| {
                        !ids.contains(&c.guild)
                            || next
                                .channels
                                .get(&c.id)
                                .is_some_and(|old| old.guild != c.guild)
                    })
                {
                    return Err("Invalid permission snapshot scope");
                }
                next.channels
                    .retain(|_, channel| !ids.contains(&channel.guild));
                for guild in snapshot.guilds {
                    next.guilds.insert(guild.id, guild);
                }
                for channel in snapshot.channels {
                    next.channels.insert(channel.id, channel);
                }
            }
            Event::Guild(guild) => {
                next.guilds.insert(guild.id, guild);
            }
            Event::Role { guild, role } => {
                if let Some(roles) = next.guilds.get_mut(&guild).and_then(|g| g.roles.as_mut()) {
                    if let Some(old) = roles.iter_mut().find(|old| old.id == role.id) {
                        *old = role;
                    } else {
                        roles.push(role);
                    }
                }
            }
            Event::RoleRemoved { guild, id } => {
                if let Some(guild) = next.guilds.get_mut(&guild) {
                    if let Some(roles) = &mut guild.roles {
                        roles.retain(|role| role.id != id);
                    }
                    if let Some(member) = &mut guild.member {
                        member.roles.retain(|role| *role != id);
                    }
                }
                for channel in next
                    .channels
                    .values_mut()
                    .filter(|channel| channel.guild == guild)
                {
                    if let Some(overwrites) = &mut channel.overwrites {
                        overwrites.retain(|overwrite| overwrite.kind != 0 || overwrite.id != id);
                    }
                }
            }
            Event::Owner { guild, owner } => {
                if let Some(guild) = next.guilds.get_mut(&guild) {
                    match owner {
                        Patch::Value(owner) => guild.owner = Some(owner),
                        Patch::Null => guild.owner = None,
                        Patch::Absent => {}
                    }
                }
            }
            Event::Members(members) => {
                if members.len() > crate::MAX_NAV {
                    return Err("Permission member batch exceeds capacity");
                }
                let mut seen = BTreeSet::new();
                for (guild, roles, timeout) in members {
                    if !seen.insert(guild) {
                        return Err("Duplicate permission member scope");
                    }
                    next.update_member(guild, roles, timeout);
                }
            }
            Event::Member {
                guild,
                roles,
                timeout_until,
            } => {
                next.update_member(guild, roles, timeout_until);
            }
            Event::Channel {
                channel,
                guild,
                overwrites,
            } => {
                let guild =
                    guild.or_else(|| next.channels.get(&channel).map(|channel| channel.guild));
                if let Some(guild) = guild.filter(|guild| next.guilds.contains_key(guild)) {
                    let old = next.channels.entry(channel).or_insert(p::Channel {
                        id: channel,
                        guild,
                        overwrites: None,
                    });
                    if old.guild != guild {
                        return Err("Permission channel changed guild");
                    }
                    match overwrites {
                        Patch::Value(overwrites) => old.overwrites = Some(overwrites),
                        Patch::Null => old.overwrites = None,
                        Patch::Absent => {}
                    }
                }
            }
            Event::UnavailableGuild(guild) => {
                next.guilds.remove(&guild);
                next.channels.retain(|_, channel| channel.guild != guild);
            }
        }
        if !next.valid() {
            return Err("Permission metadata exceeds safe capacity");
        }
        *self = next;
        Ok(())
    }
    fn update_member(&mut self, guild: Id, roles: Patch<Vec<Id>>, timeout_until: Patch<i64>) {
        if let Some(guild) = self.guilds.get_mut(&guild) {
            if matches!(roles, Patch::Null) {
                guild.member = None;
            } else {
                if let Patch::Value(roles) = roles {
                    if let Some(member) = &mut guild.member {
                        member.roles = roles;
                    } else if !matches!(timeout_until, Patch::Absent) {
                        guild.member = Some(p::Member {
                            roles,
                            timeout_until: None,
                        });
                    }
                }
                if let Some(member) = &mut guild.member {
                    match timeout_until {
                        Patch::Value(until) => member.timeout_until = Some(until),
                        Patch::Null => member.timeout_until = None,
                        Patch::Absent => {}
                    }
                }
            }
        }
    }
}

impl State {
    /// Highest separately displayed role, then highest role carrying a name color.
    pub fn member_roles(
        &self,
        guild: Id,
        member: &model::Member,
    ) -> (Option<&p::Role>, Option<&p::Role>) {
        let Some(roles) = self
            .permissions
            .guilds
            .get(&guild)
            .and_then(|guild| guild.roles.as_deref())
        else {
            return (None, None);
        };
        if member.roles.len() > p::MAX_MEMBER_ROLES {
            return (None, None);
        }
        let assigned: BTreeSet<_> = member.roles.iter().copied().collect();
        let assigned = roles
            .iter()
            .filter(|role| role.id != guild && assigned.contains(&role.id));
        (
            assigned
                .clone()
                .filter(|role| role.hoist)
                .max_by(|a, b| a.cmp_hierarchy(b)),
            assigned
                .filter(|role| role.color != 0)
                .max_by(|a, b| a.cmp_hierarchy(b)),
        )
    }

    pub(crate) fn member_list_id(&self, channel: &model::Channel) -> Option<String> {
        // Threads use a different member protocol; never borrow their parent's list.
        if matches!(channel.kind, 10..=12) {
            return None;
        }
        let guild_id = channel.guild?;
        let guild = self.permissions.guilds.get(&guild_id)?;
        let metadata = self.permissions.channels.get(&channel.id)?;
        if metadata.guild != guild_id {
            return None;
        }
        let everyone = guild
            .roles
            .as_ref()?
            .iter()
            .find(|role| role.id == guild_id)?;
        p::member_list_id(everyone.bits, metadata.overwrites.as_deref()?)
    }

    pub fn permission(&self, channel: Id, bits: u128) -> Option<bool> {
        let channel = self.channels.iter().find(|c| c.id == channel)?;
        let Some(guild) = channel.guild else {
            return matches!(channel.kind, 1 | 3).then_some(true);
        };
        if !self.guilds.iter().any(|known| known.id == guild) {
            return None;
        }
        let guild = self.permissions.guilds.get(&guild)?;
        let target = if matches!(channel.kind, 10..=12) {
            let parent = channel.parent_id?;
            self.channels
                .iter()
                .find(|c| {
                    c.id == parent && c.guild == Some(guild.id) && matches!(c.kind, 0 | 5 | 15 | 16)
                })?
                .id
        } else {
            channel.id
        };
        let overwrites = self
            .permissions
            .channels
            .get(&target)
            .filter(|c| c.guild == guild.id)
            .and_then(|c| c.overwrites.as_deref());
        self.permissions
            .effective(
                target,
                guild,
                self.user.as_ref()?.id,
                overwrites,
                Self::permission_time(),
            )
            .map(|permissions| permissions & bits == bits)
    }
    fn permission_time() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs().min(i64::MAX as u64) as i64)
    }
    pub fn can_view(&self, channel: Id) -> bool {
        self.permission(channel, p::VIEW_CHANNEL) == Some(true)
    }
    pub fn can_read_history(&self, channel: Id) -> bool {
        self.permission(channel, p::VIEW_CHANNEL | p::READ_MESSAGE_HISTORY) == Some(true)
    }
    pub fn can_send(&self, channel: Id) -> bool {
        self.auth == AuthState::Authenticated
            && self.gateway_connected
            && self.selected == Some(channel)
            && self.freshness == Freshness::Fresh
            && self
                .channels
                .iter()
                .find(|c| c.id == channel && c.supports_text())
                .is_some_and(|c| {
                    let send = if matches!(c.kind, 10..=12) {
                        p::SEND_MESSAGES_IN_THREADS
                    } else {
                        p::SEND_MESSAGES
                    };
                    self.permission(channel, p::VIEW_CHANNEL | send) == Some(true)
                })
    }
    pub fn can_attach(&self, channel: Id) -> bool {
        self.can_send(channel) && self.permission(channel, p::ATTACH_FILES) == Some(true)
    }
    pub fn can_speak(&self, channel: Id) -> bool {
        self.permission(channel, p::VIEW_CHANNEL | p::CONNECT | p::SPEAK) == Some(true)
    }
    pub fn can_react(&self, message: Id, emoji: Option<&ReactionEmoji>, add: bool) -> bool {
        let Some(channel) = self.selected else {
            return false;
        };
        if self.auth != AuthState::Authenticated
            || !self.gateway_connected
            || self.freshness != Freshness::Fresh
            || !self.can_read_history(channel)
        {
            return false;
        }
        let Some(message) = self.timeline.get(message) else {
            return false;
        };
        if message.channel != channel || message.reactions.is_none() {
            return false;
        }
        let existing = emoji.is_some_and(|emoji| {
            message.reactions.as_ref().is_some_and(|reactions| {
                reactions
                    .iter()
                    .any(|reaction| reaction.emoji.same(emoji) && reaction.count > 0)
            })
        });
        (!add || existing || self.permission(channel, p::ADD_REACTIONS) == Some(true))
            && (!add || !self.timed_out(channel))
    }
    fn timed_out(&self, channel: Id) -> bool {
        self.channels
            .iter()
            .find(|c| c.id == channel)
            .and_then(|c| c.guild)
            .and_then(|guild| self.permissions.guilds.get(&guild))
            .is_some_and(|guild| {
                guild
                    .member
                    .as_ref()
                    .and_then(|m| m.timeout_until)
                    .is_some_and(|until| until > Self::permission_time())
                    && self.permission(channel, p::ADMINISTRATOR) != Some(true)
            })
    }
    pub fn can_edit(&self, channel: Id, message: Id) -> bool {
        self.auth == AuthState::Authenticated
            && self.gateway_connected
            && self.can_view(channel)
            && self.timeline.get(message).is_some_and(|message| {
                message.channel == channel
                    && self
                        .user
                        .as_ref()
                        .is_some_and(|user| user.id == message.author.id)
            })
    }
    pub fn can_delete(&self, channel: Id, message: Id) -> bool {
        self.can_edit(channel, message)
    }
    pub fn prepare_edit(&mut self, channel: Id, message: Id, content: String) -> Option<Command> {
        if !self.can_edit(channel, message)
            || content.trim().is_empty()
            || content.chars().count() > crate::MAX_CONTENT
        {
            self.status = "This message cannot be edited with the current access";
            return None;
        }
        Some(Command::Edit {
            channel,
            message,
            content,
        })
    }
    pub fn prepare_delete(&mut self, channel: Id, message: Id) -> Option<Command> {
        if !self.can_delete(channel, message) {
            self.status = "This message cannot be deleted with the current access";
            return None;
        }
        Some(Command::Delete { channel, message })
    }
    pub(crate) fn permission_access(&self) -> Option<(Option<bool>, Option<bool>)> {
        self.selected.map(|channel| {
            (
                self.permission(channel, p::VIEW_CHANNEL),
                self.permission(channel, p::VIEW_CHANNEL | p::READ_MESSAGE_HISTORY),
            )
        })
    }
    pub(crate) fn reconcile_permissions(&mut self, previous: Option<(Option<bool>, Option<bool>)>) {
        if let Some(channel) = self.selected
            && previous != self.permission_access()
            && !self.can_read_history(channel)
        {
            self.cancel_history();
            self.clear_search();
            self.clear_archives();
            self.timeline.clear();
            self.reply = None;
            self.reactions.reset();
            self.invalidate_members();
            self.freshness = if self.can_view(channel) && self.gateway_connected {
                Freshness::Fresh
            } else {
                Freshness::Unavailable
            };
            self.status = if self.can_view(channel) {
                "Message history is unavailable with the current permissions"
            } else {
                "Channel permissions are unavailable or access was revoked"
            };
            self.revision += 1;
        }
        // Roster access is independent of whether this account has joined a call.
        let mut roster = std::mem::take(&mut self.voice.roster);
        roster.retain(|entry| self.can_view(entry.channel));
        self.voice.roster = roster;
        if let Some(channel) = self.voice.active.as_ref().map(|call| call.channel)
            && !self.has_voice_access(channel)
        {
            self.end_voice_channel(channel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decisions_expire_at_timeout_and_recompute_after_clock_rollback() {
        let store = Permissions::default();
        let guild = p::Guild {
            id: Id(1),
            owner: Some(Id(9)),
            roles: Some(vec![p::Role {
                name: String::new(),
                color: 0,
                position: 0,
                hoist: false,
                id: Id(1),
                bits: p::VIEW_CHANNEL | p::READ_MESSAGE_HISTORY | p::SEND_MESSAGES,
            }]),
            member: Some(p::Member {
                roles: vec![],
                timeout_until: Some(100),
            }),
        };
        let decide = |now| {
            store
                .effective(Id(2), &guild, Id(3), Some(&[]), now)
                .unwrap()
                & p::SEND_MESSAGES
                != 0
        };
        assert!(!decide(90));
        assert!(!decide(99));
        assert!(decide(100));
        assert!(decide(101));
        assert!(!decide(95));
        assert!(decide(100));
    }
}
