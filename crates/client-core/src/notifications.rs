//! Bounded session activity; badge counts never use snowflake subtraction.
use crate::{MAX_NAV, State};
use model::{Id, Message};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_OBSERVED: usize = 4096;
const MAX_OBSERVED_BYTES: usize = 128 * 1024;
const MAX_NOTIFICATIONS: usize = 32;
const MAX_NOTIFICATION_BYTES: usize = 16 * 1024;

pub struct Notification {
    pub channel: Id,
    pub message: Id,
    mention: bool,
}
impl Notification {
    fn bytes(&self) -> usize {
        size_of::<Self>()
    }
}
#[derive(Default)]
pub(crate) struct Activity {
    counts: BTreeMap<Id, u32>,
    high_water: BTreeMap<Id, Id>,
    observed_counts: BTreeMap<Id, (u32, u32)>,
    observed: VecDeque<(Id, Id, bool)>,
    notifications: VecDeque<Notification>,
}
impl Activity {
    pub(crate) fn clear_notifications(&mut self) {
        self.notifications.clear();
    }
    pub(crate) fn forget(&mut self, channel: Id) {
        self.high_water.remove(&channel);
        self.revoke(channel);
    }
    fn revoke(&mut self, channel: Id) {
        self.counts.remove(&channel);
        self.observed_counts.remove(&channel);
        self.observed.retain(|(c, ..)| *c != channel);
        self.notifications.retain(|n| n.channel != channel);
    }
    pub(crate) fn delete(&mut self, channel: Id, message: Id) {
        self.observed
            .retain(|(c, id, _)| (*c, *id) != (channel, message));
        self.notifications
            .retain(|n| (n.channel, n.message) != (channel, message));
        if self.observed_counts.contains_key(&channel) {
            self.recount(channel);
        }
    }
    pub(crate) fn observe_latest(&mut self, channel: Id, message: Id) {
        let latest = self.high_water.entry(channel).or_insert(message);
        *latest = (*latest).max(message);
    }
    pub(crate) fn set_count(&mut self, channel: Id, count: u32) {
        self.counts.insert(channel, count);
    }
    pub(crate) fn ack(&mut self, channel: Id, message: Option<Id>, count: Option<u32>) {
        self.observed
            .retain(|(c, id, _)| *c != channel || message.is_none_or(|read| *id > read));
        self.notifications
            .retain(|n| n.channel != channel || message.is_none_or(|read| n.message > read));
        // The service count includes messages after this ACK. Avoid counting those twice.
        if let Some(count) = count {
            self.counts.insert(channel, count);
            self.observed.retain(|(c, ..)| *c != channel);
        } else {
            // Without a refreshed service count, only retained observed messages are countable.
            self.counts.remove(&channel);
        }
        self.recount(channel);
    }
    fn recount(&mut self, channel: Id) {
        let mut counts = (0, 0);
        for (c, _, mention) in &self.observed {
            if *c == channel {
                counts.0 += 1;
                counts.1 += u32::from(*mention);
            }
        }
        if counts.0 == 0 {
            self.observed_counts.remove(&channel);
        } else {
            self.observed_counts.insert(channel, counts);
        }
    }
    fn count(&self, channel: Id, mentions_only: bool) -> u32 {
        let counts = self
            .observed_counts
            .get(&channel)
            .copied()
            .unwrap_or_default();
        self.counts
            .get(&channel)
            .copied()
            .unwrap_or(0)
            .saturating_add(if mentions_only { counts.1 } else { counts.0 })
    }
}

#[derive(Clone, Default)]
pub struct Setting {
    pub guild: Option<Id>,
    pub muted: Option<bool>,
    pub level: Option<u8>,
    pub channels: Vec<(Id, Option<bool>, Option<u8>)>,
}
pub enum Event {
    Settings {
        entries: Vec<Setting>,
        replace: bool,
    },
    Presence(Option<bool>), // Some(true) means at least one session is DND.
    Invalidate,
}
impl Event {
    pub fn bytes(&self) -> usize {
        match self {
            Self::Settings { entries, .. } => {
                entries.capacity() * size_of::<Setting>()
                    + entries
                        .iter()
                        .map(|e| {
                            e.channels.capacity() * size_of::<(Id, Option<bool>, Option<u8>)>()
                        })
                        .sum::<usize>()
            }
            _ => 0,
        }
    }
}
#[derive(Default)]
pub struct Preferences {
    settings: BTreeMap<Option<Id>, Setting>,
    dnd: Option<bool>,
}
impl State {
    /// Service badge count plus bounded activity observed since that count. A lower bound
    /// when history or settings are incomplete; this is not an exact total unread count.
    pub fn unread_count(&self, channel: Id) -> u32 {
        if self.can_view(channel) {
            self.read_state.activity.count(channel, false)
        } else {
            0
        }
    }
    pub fn mention_count(&self, channel: Id) -> u32 {
        if self.can_view(channel) {
            self.read_state.activity.count(channel, true)
        } else {
            0
        }
    }
    pub(crate) fn reconcile_notifications(&mut self) {
        let activity = &self.read_state.activity;
        let revoked: BTreeSet<_> = activity
            .counts
            .keys()
            .chain(activity.observed_counts.keys())
            .copied()
            .chain(
                activity
                    .notifications
                    .iter()
                    .map(|notification| notification.channel),
            )
            .filter(|channel| !self.can_view(*channel))
            .collect();
        for channel in revoked {
            self.read_state.activity.revoke(channel);
        }
    }
    pub fn take_notification(&mut self) -> Option<Notification> {
        while let Some(notification) = self.read_state.activity.notifications.pop_front() {
            if self.notification_allowed_for(notification.channel, notification.mention) {
                return Some(notification);
            }
        }
        None
    }
    pub fn notification_preferences_known(&self) -> bool {
        self.notification_preferences.dnd.is_some()
            && !self.notification_preferences.settings.is_empty()
    }
    pub fn notification_allowed(&self, channel: Id) -> bool {
        self.notification_allowed_for(channel, true)
    }
    fn notification_allowed_for(&self, channel: Id, mention: bool) -> bool {
        if !self.gateway_connected
            || !self.can_view(channel)
            || self.notification_preferences.dnd != Some(false)
        {
            return false;
        }
        let Some(channel) = self
            .channels
            .iter()
            .find(|c| c.id == channel && c.supports_text())
        else {
            return false;
        };
        let Some(setting) = self.notification_preferences.settings.get(&channel.guild) else {
            return false;
        };
        if setting.muted != Some(false) {
            return false;
        }
        let mut level = setting.level;
        for id in [channel.parent_id, Some(channel.id)].into_iter().flatten() {
            if let Some((_, muted, override_level)) =
                setting.channels.iter().find(|(c, ..)| *c == id)
            {
                if *muted != Some(false) {
                    return false;
                }
                if override_level.is_some_and(|l| l != 3) {
                    level = *override_level;
                }
            }
        }
        level == Some(0) || (mention && level == Some(1))
    }
    pub fn apply_notification_preferences(&mut self, event: Event) -> Result<(), &'static str> {
        self.read_state.activity.clear_notifications();
        if event.bytes() > 512 * 1024 {
            self.notification_preferences = Preferences::default();
            return Err("Notification settings exceed safe byte capacity");
        }
        match event {
            Event::Presence(dnd) => self.notification_preferences.dnd = dnd,
            Event::Invalidate => self.notification_preferences = Preferences::default(),
            Event::Settings { entries, replace } => {
                let keys: BTreeSet<_> = entries.iter().map(|s| s.guild).collect();
                let old: Vec<_> = self
                    .notification_preferences
                    .settings
                    .iter()
                    .filter(|(key, _)| !replace && !keys.contains(key))
                    .map(|(_, setting)| setting)
                    .collect();
                let overrides = entries.iter().map(|s| s.channels.len()).sum::<usize>();
                let old_overrides: usize = old.iter().map(|s| s.channels.len()).sum();
                if old.len().saturating_add(entries.len()) > MAX_NAV
                    || overrides.saturating_add(old_overrides) > MAX_NAV
                    || keys.len() != entries.len()
                    || entries.iter().any(|s| {
                        s.channels
                            .iter()
                            .map(|(id, ..)| *id)
                            .collect::<BTreeSet<_>>()
                            .len()
                            != s.channels.len()
                    })
                {
                    self.notification_preferences = Preferences::default();
                    return Err(
                        "Notification settings exceed safe capacity or contain duplicate IDs",
                    );
                }
                if replace {
                    self.notification_preferences.settings.clear();
                }
                for setting in entries {
                    self.notification_preferences
                        .settings
                        .insert(setting.guild, setting);
                }
            }
        }
        Ok(())
    }
    pub(crate) fn observe_notification(&mut self, message: &Message) {
        let Some(channel) = self
            .channels
            .iter()
            .find(|c| c.id == message.channel && c.supports_text())
        else {
            return;
        };
        let activity = &mut self.read_state.activity;
        let previous = activity.high_water.get(&message.channel).copied();
        if previous.is_some_and(|id| message.id <= id) {
            return;
        }
        activity.high_water.insert(message.channel, message.id);
        if !self.can_view(message.channel) {
            return;
        }
        let Some(owner) = self.user.as_ref() else {
            return;
        };
        if message.author.id == owner.id {
            // The unofficial service also implicitly acknowledges our own gateway messages.
            let _ = self.apply_read_state(crate::read_state::Event::Ack {
                channel: message.channel,
                message: Some(message.id),
                manual: false,
                mention_count: Some(0),
                version: None,
            });
            return;
        }
        if self
            .read_marker(message.channel)
            .flatten()
            .is_some_and(|read| message.id <= read)
        {
            return;
        }
        let mention = channel.guild.is_none() || message.mentions.iter().any(|u| u.id == owner.id);
        let allowed = self.notification_allowed_for(message.channel, mention);
        let activity = &mut self.read_state.activity;
        // ponytail: retain 4096 observed messages; counts become a lower bound after eviction.
        while activity.observed.len() >= MAX_OBSERVED
            || (activity.observed.len() + 1) * size_of::<(Id, Id, bool)>() > MAX_OBSERVED_BYTES
        {
            if let Some((channel, _, mention)) = activity.observed.pop_front()
                && let Some(counts) = activity.observed_counts.get_mut(&channel)
            {
                counts.0 -= 1;
                counts.1 -= u32::from(mention);
                if counts.0 == 0 {
                    activity.observed_counts.remove(&channel);
                }
            }
        }
        activity
            .observed
            .push_back((message.channel, message.id, mention));
        let counts = activity.observed_counts.entry(message.channel).or_default();
        counts.0 += 1;
        counts.1 += u32::from(mention);
        if !allowed {
            return;
        }
        let notification = Notification {
            channel: message.channel,
            message: message.id,
            mention,
        };
        while activity.notifications.len() >= MAX_NOTIFICATIONS
            || activity
                .notifications
                .iter()
                .map(Notification::bytes)
                .sum::<usize>()
                + notification.bytes()
                > MAX_NOTIFICATION_BYTES
        {
            activity.notifications.pop_front();
        }
        activity.notifications.push_back(notification);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_revocation_clears_alerts_and_badges_without_replaying_hidden_activity() {
        use model::{Channel, Guild, Patch, User, permissions as p};
        let owner = User {
            id: Id(2),
            name: "Synthetic".into(),
            avatar: None,
            discriminator: 0,
        };
        let mut state = State {
            user: Some(owner.clone()),
            gateway_connected: true,
            auth: crate::auth::AuthState::Authenticated,
            guilds: vec![Guild {
                id: Id(1),
                name: "Synthetic".into(),
                icon: None,
                emojis: None,
            }],
            channels: [20, 21]
                .into_iter()
                .map(|id| Channel {
                    id: Id(id),
                    guild: Some(Id(1)),
                    kind: 0,
                    name: "Synthetic".into(),
                    parent_id: None,
                    last_message: Some(Id(95)),
                    position: 0,
                    recipients: vec![],
                    member_list_id: None,
                })
                .collect(),
            ..State::default()
        };
        state
            .permissions
            .replace(p::Snapshot {
                guilds: vec![p::Guild {
                    id: Id(1),
                    owner: Some(Id(999)),
                    roles: Some(vec![p::Role {
                        name: String::new(),
                        color: 0,
                        position: 0,
                        hoist: false,
                        id: Id(1),
                        bits: p::VIEW_CHANNEL,
                    }]),
                    member: Some(p::Member {
                        roles: vec![],
                        timeout_until: None,
                    }),
                }],
                channels: [20, 21]
                    .into_iter()
                    .map(|id| p::Channel {
                        id: Id(id),
                        guild: Id(1),
                        overwrites: Some(vec![]),
                    })
                    .collect(),
            })
            .unwrap();
        state
            .apply_read_state(crate::read_state::Event::Snapshot {
                entries: Some(vec![(Id(20), Some(Id(90)), 0), (Id(21), Some(Id(90)), 0)]),
                version: Some(1),
                partial: false,
            })
            .unwrap();
        state
            .apply_notification_preferences(Event::Settings {
                entries: vec![Setting {
                    guild: Some(Id(1)),
                    muted: Some(false),
                    level: Some(0),
                    channels: vec![],
                }],
                replace: true,
            })
            .unwrap();
        state
            .apply_notification_preferences(Event::Presence(Some(false)))
            .unwrap();
        let message = |id, channel| Message {
            kind: 0,
            id: Id(id),
            channel: Id(channel),
            author: User {
                id: Id(3),
                ..owner.clone()
            },
            content: "Synthetic".into(),
            mentions: vec![owner.clone()],
            reactions: Some(vec![]),
            edited: false,
            edited_at: None,
            revision: 0,
            nonce: None,
            reply_to: None,
            reply_deleted: false,
            unsupported: false,
            extra_content: Default::default(),
            embeds: vec![],
            embeds_suppressed: false,
            attachments: vec![],
        };
        let access = |view| crate::permissions::Event::Channel {
            channel: Id(20),
            guild: Some(Id(1)),
            overwrites: Patch::Value(vec![p::Overwrite {
                id: Id(1),
                kind: 0,
                allow: 0,
                deny: if view { 0 } else { p::VIEW_CHANNEL },
            }]),
        };
        state.observe_notification(&message(100, 20));
        state.observe_notification(&message(101, 21));
        assert!(!state.can_read_history(Id(20)));
        assert_eq!(
            state.unread_count(Id(20)),
            1,
            "Live activity requires VIEW, not history access"
        );
        assert_eq!(state.mention_count(Id(20)), 1);
        assert_eq!(state.unread(Id(20)), Some(true));
        assert_eq!(state.channel_unread(&state.channels[0]), Some(true));
        state.apply(crate::Envelope {
            generation: state.generation,
            event: crate::Event::Permissions(access(false)),
        });
        assert_eq!(state.unread_count(Id(20)), 0);
        assert_eq!(state.mention_count(Id(20)), 0);
        assert_eq!(state.unread(Id(20)), None);
        assert_eq!(state.channel_unread(&state.channels[0]), None);
        assert_eq!(state.unread_count(Id(21)), 1);
        assert_eq!(
            state.read_state.activity.notifications.len(),
            1,
            "Revocation removes queued alerts before permission can return"
        );
        state.observe_notification(&message(102, 20));
        state.apply(crate::Envelope {
            generation: state.generation,
            event: crate::Event::Permissions(access(true)),
        });
        assert_eq!(state.take_notification().unwrap().channel, Id(21));
        state.observe_notification(&message(100, 20));
        state.observe_notification(&message(102, 20));
        assert_eq!(state.unread_count(Id(20)), 0);
        assert!(state.take_notification().is_none());
        state.observe_notification(&message(103, 20));
        assert_eq!(state.mention_count(Id(20)), 1);
        state.permissions.update(access(false)).unwrap();
        assert!(
            state.take_notification().is_none(),
            "Delivery independently checks current VIEW access"
        );
        state.permissions.update(access(true)).unwrap();
        state
            .apply_read_state(crate::read_state::Event::Ack {
                channel: Id(20),
                message: Some(Id(103)),
                manual: false,
                mention_count: Some(0),
                version: None,
            })
            .unwrap();
        state.observe_notification(&message(103, 20));
        assert_eq!(state.mention_count(Id(20)), 0);
        assert!(state.take_notification().is_none());
    }

    #[test]
    fn unknown_deletes_and_empty_recounts_never_admit_channel_state() {
        let mut activity = Activity::default();
        for id in 1..=10_000 {
            activity.delete(Id(id), Id(id));
            activity.recount(Id(id));
        }
        assert!(activity.observed_counts.is_empty());
        assert!(activity.high_water.is_empty());
        assert!(activity.counts.is_empty());
        activity.observed.push_back((Id(1), Id(2), true));
        activity.recount(Id(1));
        assert_eq!(activity.count(Id(1), true), 1);
        activity.delete(Id(1), Id(2));
        assert!(activity.observed_counts.is_empty());
        assert!(activity.observed.is_empty());
    }
}
