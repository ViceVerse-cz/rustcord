//! Single UI-thread state owner. Adapters deliver generation-tagged typed events.
pub mod auth;
pub mod profile;
pub mod reactions;
pub mod read_state;
pub mod search;
pub mod voice;
use model::*;
use session_cache::Timeline;
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_DRAFT_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_CONTENT: usize = 2000;
pub const MAX_NAV: usize = 4000;
pub const MAX_EVENT_BYTES: usize = 4 * 1024 * 1024;
pub const EVENT_SLOTS: usize = 8; // <= 32 MiB wire-derived data, not including one decoder
pub const COMMAND_SLOTS: usize = 16; // each admitted command <= 16 KiB

pub enum Command {
    Pins {
        channel: Id,
        request: u64,
    },
    Search {
        channel: Id,
        guild: Option<Id>,
        query: String,
        before: Option<Id>,
        request: u64,
    },
    CancelSearch,
    MarkRead {
        channel: Id,
        message: Id,
        request: u64,
    },
    Reactions(reactions::Command),
    Profile {
        user: Id,
        guild: Option<Id>,
        request: u64,
    },
    CancelProfile,
    Voice(voice::Command),
    Members {
        guild: Option<Id>,
        channel: Option<Id>,
        request: u64,
        list_id: Option<String>,
    },
    History {
        channel: Id,
        before: Option<Id>,
        request: u64,
    },
    Send {
        channel: Id,
        content: String,
        nonce: String,
        reply: Option<Id>,
    },
    Edit {
        channel: Id,
        message: Id,
        content: String,
    },
    Delete {
        channel: Id,
        message: Id,
    },
}
pub enum Event {
    Search {
        channel: Id,
        request: u64,
        result: Result<search::Outcome, auth::Failure>,
    },
    ReadState(read_state::Event),
    Reactions(reactions::Event),
    Profile {
        user: Id,
        guild: Option<Id>,
        request: u64,
        result: Result<UserProfile, auth::Failure>,
    },
    Voice(voice::Event),
    ChannelCreated(Channel),
    ChannelChanged(ChannelPatch),
    GuildChanged(GuildPatch),
    Members(MemberList),
    RecipientAdded {
        channel: Id,
        user: User,
    },
    RecipientRemoved {
        channel: Id,
        user: Id,
    },
    Ready {
        user: User,
        guilds: Vec<Guild>,
        channels: Vec<Channel>,
    },
    History {
        channel: Id,
        request: u64,
        older: bool,
        messages: Vec<Message>,
    },
    HistoryFailed {
        channel: Id,
        request: u64,
        failure: auth::Failure,
    },
    Message(Message),
    Patch(MessagePatch),
    Delete {
        channel: Id,
        id: Id,
    },
    DeleteBulk {
        channel: Id,
        ids: Vec<Id>,
    },
    SendResult {
        nonce: String,
        result: Result<Message, auth::Failure>,
    },
    Failure(auth::Failure),
    Disconnected,
    Resumed,
    Resync,
    Unavailable(Id),
    PermissionsChanged,
}
pub struct Envelope {
    pub generation: u64,
    pub event: Event,
}
pub struct Pending {
    pub channel: Id,
    pub content: String,
    pub nonce: String,
    pub delivery: Delivery,
    pub confirmed: Option<Id>,
}
pub struct State {
    pub search: Option<search::SearchView>,
    pub search_request: u64,
    pub search_target: Option<Id>,
    pub read_state: read_state::ReadState,
    pub reactions: reactions::Reactions,
    pub profile: Option<profile::ProfileView>,
    pub profile_request: u64,
    pub voice: voice::State,
    pub generation: u64,
    pub auth: auth::AuthState,
    pub user: Option<User>,
    pub members: Option<MemberList>,
    pub member_request: u64,
    pub guilds: Vec<Guild>,
    pub channels: Vec<Channel>,
    pub selected: Option<Id>,
    pub timeline: Timeline,
    pub freshness: Freshness,
    pub status: &'static str,
    pub drafts: BTreeMap<Id, String>,
    pub pending: Vec<Pending>,
    pub reply: Option<Id>,
    pub send_sequence: u64,
    pub request: u64,
    pub history_before: Option<Id>,
    pub history_pending: bool,
    pub older_exhausted: bool,
    pub gateway_connected: bool,
    pub revision: u64,
    pub demo: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            search: None,
            search_request: 0,
            search_target: None,
            read_state: read_state::ReadState::default(),
            reactions: reactions::Reactions::default(),
            profile: None,
            profile_request: 0,
            voice: voice::State::default(),
            generation: 1,
            auth: auth::AuthState::Unauthenticated,
            user: None,
            members: None,
            member_request: 0,
            guilds: vec![],
            channels: vec![],
            selected: None,
            timeline: Timeline::default(),
            freshness: Freshness::Stale,
            status: "Disconnected",
            drafts: BTreeMap::new(),
            pending: vec![],
            reply: None,
            send_sequence: 0,
            request: 0,
            history_before: None,
            history_pending: false,
            older_exhausted: false,
            gateway_connected: false,
            revision: 0,
            demo: false,
        }
    }
}
impl State {
    pub fn logout(&mut self) {
        let generation = self.generation.wrapping_add(1);
        *self = Self {
            generation,
            ..Self::default()
        };
    }
    pub fn has_unsent(&self) -> bool {
        self.drafts.values().any(|s| !s.is_empty())
            || self
                .pending
                .iter()
                .any(|p| p.delivery != Delivery::Confirmed)
    }
    pub fn draft_bytes(&self) -> usize {
        self.drafts.values().map(String::capacity).sum::<usize>()
            + self
                .pending
                .iter()
                .map(|p| p.content.capacity() + p.nonce.capacity() + size_of::<Pending>())
                .sum::<usize>()
    }
    pub fn select(&mut self, channel: Id) -> Option<Command> {
        if !self
            .channels
            .iter()
            .any(|c| c.id == channel && c.supports_text())
        {
            self.status = "This channel kind is unsupported";
            return None;
        }
        self.members = None;
        self.selected = Some(channel);
        self.clear_search();
        self.search_target = None;
        self.reactions.reset();
        self.timeline.clear();
        self.older_exhausted = false;
        self.reply = None;
        self.revision += 1;
        Some(self.history(None))
    }
    pub fn request_members(&mut self) -> Option<Command> {
        let channel = self.channels.iter().find(|c| Some(c.id) == self.selected)?;
        self.member_request = self.member_request.wrapping_add(1);
        let rows = if channel.guild.is_none() && self.freshness != Freshness::Unavailable {
            let mut users = channel.recipients.clone();
            if let Some(user) = &self.user
                && !users.iter().any(|u| u.id == user.id)
            {
                users.push(user.clone());
            }
            users
                .into_iter()
                .map(|user| {
                    Some(Member {
                        user,
                        nick: None,
                        status: None,
                    })
                })
                .collect()
        } else {
            vec![]
        };
        let freshness = if self.freshness == Freshness::Unavailable {
            Freshness::Unavailable
        } else if channel.guild.is_none() {
            Freshness::Fresh
        } else if channel.member_list_id.is_none() {
            Freshness::Unavailable
        } else {
            Freshness::Loading
        };
        self.members = Some(MemberList {
            guild: channel.guild,
            channel: channel.id,
            request: self.member_request,
            total: rows.len() as u64,
            rows,
            freshness,
        });
        Some(Command::Members {
            guild: channel.guild.filter(|_| {
                channel.member_list_id.is_some() && self.freshness != Freshness::Unavailable
            }),
            channel: Some(channel.id),
            request: self.member_request,
            list_id: channel.member_list_id.clone(),
        })
    }
    pub fn close_members(&mut self) -> Command {
        self.member_request = self.member_request.wrapping_add(1);
        self.members = None;
        Command::Members {
            guild: None,
            channel: None,
            request: self.member_request,
            list_id: None,
        }
    }
    pub fn history(&mut self, before: Option<Id>) -> Command {
        if self.search.as_ref().is_some_and(|s| s.loading) {
            self.clear_search();
        }
        if before.is_none() {
            self.search_target = None;
        }
        self.reactions.cancel_read();
        self.request += 1;
        self.history_before = before;
        self.history_pending = true;
        self.freshness = Freshness::Loading;
        self.timeline.begin_page();
        Command::History {
            channel: self.selected.expect("selected channel"),
            before,
            request: self.request,
        }
    }
    pub fn can_load_older(&self) -> bool {
        self.gateway_connected
            && self.freshness == Freshness::Fresh
            && !self.history_pending
            && !self.older_exhausted
            && !self.timeline.is_empty()
    }
    pub fn older_history(&mut self) -> Option<Command> {
        if !self.can_load_older() {
            return None;
        }
        let before = self.timeline.iter().next()?.id;
        Some(self.history(Some(before)))
    }
    pub fn prepare_send(&mut self) -> Option<Command> {
        let channel = self.selected?;
        if self.auth != auth::AuthState::Authenticated || self.freshness != Freshness::Fresh {
            self.status = "Wait for a current connected channel";
            return None;
        }
        let content = self.drafts.get(&channel)?;
        if content.trim().is_empty()
            || content.chars().count() > MAX_CONTENT
            || self.pending.len() >= 64
            || self.draft_bytes() + content.len() > MAX_DRAFT_BYTES
        {
            self.status = "Send exceeds the session input budget";
            return None;
        }
        let content = content.clone();
        self.send_sequence += 1;
        let epoch = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let nonce = format!("{epoch}{:06}", self.send_sequence % 1_000_000);
        self.pending.push(Pending {
            channel,
            content: content.clone(),
            nonce: nonce.clone(),
            delivery: Delivery::Sending,
            confirmed: None,
        });
        self.drafts.remove(&channel);
        Some(Command::Send {
            channel,
            content,
            nonce,
            reply: self.reply.take(),
        })
    }
    pub fn command_rejected(&mut self, command: Command) {
        if let Command::Search {
            channel, request, ..
        }
        | Command::Pins { channel, request } = command
        {
            self.apply_search(channel, request, Err(auth::Failure::Capacity));
            return;
        }
        if matches!(command, Command::CancelSearch) {
            return;
        }
        if let Command::MarkRead {
            channel,
            message,
            request,
        } = command
        {
            let _ = self.apply_read_state(read_state::Event::Result {
                channel,
                message,
                request,
                result: Err(auth::Failure::RateLimited),
            });
            self.read_state.status = Some("Work queue full; read marker was not sent");
            return;
        }
        if let Command::Reactions(command) = command {
            let event = match command {
                reactions::Command::Read {
                    channel,
                    message,
                    request,
                } => reactions::Event::Read {
                    channel,
                    message,
                    request,
                    result: Err(auth::Failure::RateLimited),
                },
                reactions::Command::Set {
                    channel,
                    message,
                    request,
                    ..
                } => reactions::Event::Written {
                    channel,
                    message,
                    request,
                    result: Err(auth::Failure::RateLimited),
                },
            };
            let _ = self.apply_reactions(event);
            self.status = "Work queue full; reaction action was not sent";
            return;
        }
        if let Command::Profile {
            user,
            guild,
            request,
        } = command
        {
            self.apply_profile(user, guild, request, Err(auth::Failure::Capacity));
            return;
        }
        if matches!(command, Command::CancelProfile) {
            return;
        }

        if let Command::Voice(control) = command {
            match control {
                voice::Command::Join {
                    channel, request, ..
                }
                | voice::Command::Ring { channel, request } => {
                    self.apply_voice(voice::Event::Failed {
                        channel,
                        request,
                        message: "Call action was not sent; the work queue is full",
                    })
                }
                _ => self.status = "Call control was not sent; local mute/hangup still applies",
            }
            return;
        }
        if matches!(&command, Command::Members { .. }) {
            self.invalidate_members();
            return;
        }
        if matches!(&command, Command::History { request, .. } if *request == self.request) {
            self.cancel_history();
        }
        if let Command::Send { nonce, .. } = command
            && let Some(p) = self.pending.iter_mut().find(|p| p.nonce == nonce)
        {
            p.delivery = Delivery::Rejected;
        }
        self.status = "Work queue full; action was not sent";
        self.freshness = Freshness::Stale;
    }
    pub fn apply(&mut self, envelope: Envelope) {
        if envelope.generation != self.generation {
            return;
        }
        self.revision += 1;
        if matches!(
            &envelope.event,
            Event::Disconnected | Event::Resync | Event::PermissionsChanged | Event::Ready { .. }
        ) || matches!(&envelope.event,Event::Unavailable(id) if Some(*id)==self.selected)
            || matches!(&envelope.event,Event::HistoryFailed{channel,failure:auth::Failure::Forbidden,..} if Some(*channel)==self.selected)
            || matches!(&envelope.event,Event::RecipientRemoved{channel,user} if Some(*channel)==self.selected && self.user.as_ref().is_some_and(|u|u.id==*user))
        {
            self.clear_search();
            self.search_target = None;
        }
        // Search is a snapshot. A mutation can race an in-flight index response; invalidate
        // its snippets instead of restoring deleted/edited text from an older index.
        if matches!(&envelope.event,Event::Patch(p) if Some(p.channel)==self.selected)
            || matches!(&envelope.event,Event::Delete{channel,..}|Event::DeleteBulk{channel,..} if Some(*channel)==self.selected)
        {
            self.clear_search();
        }
        let result = match envelope.event {
            Event::Search {
                channel,
                request,
                result,
            } => {
                self.apply_search(channel, request, result);
                Ok(())
            }
            Event::ReadState(event) => self.apply_read_state(event),
            Event::Reactions(event) => self.apply_reactions(event),
            Event::Profile {
                user,
                guild,
                request,
                result,
            } => {
                self.apply_profile(user, guild, request, result);
                Ok(())
            }

            Event::GuildChanged(patch) => {
                if let Some(guild) = self.guilds.iter_mut().find(|guild| guild.id == patch.id) {
                    match patch.name {
                        Patch::Value(name) => guild.name = name.chars().take(128).collect(),
                        Patch::Null => guild.name.clear(),
                        Patch::Absent => {}
                    }
                    match patch.icon {
                        Patch::Value(icon) => guild.icon = valid_avatar_hash(&icon).then_some(icon),
                        Patch::Null => guild.icon = None,
                        Patch::Absent => {}
                    }
                }
                Ok(())
            }
            Event::ChannelCreated(channel) => {
                let old = self.channels.iter().position(|c| c.id == channel.id);
                if channel.recipients.len() > 64
                    || (old.is_none() && self.channels.len() + self.guilds.len() >= MAX_NAV)
                    || self
                        .channels
                        .iter()
                        .filter(|c| c.id != channel.id)
                        .map(Channel::bytes)
                        .sum::<usize>()
                        + channel.bytes()
                        > MAX_EVENT_BYTES
                {
                    self.fail(auth::Failure::Capacity);
                    return;
                }
                if let Some(index) = old {
                    self.channels[index] = channel;
                } else {
                    self.channels.push(channel);
                }
                Ok(())
            }
            Event::ChannelChanged(patch) => {
                if let Some(channel) = self.channels.iter_mut().find(|c| c.id == patch.id) {
                    match patch.last_message {
                        Patch::Value(id) => channel.last_message = Some(id),
                        Patch::Null => channel.last_message = None,
                        Patch::Absent => {}
                    }
                    match patch.name {
                        Patch::Value(name) => channel.name = name.chars().take(128).collect(),
                        Patch::Null => channel.name.clear(),
                        Patch::Absent => {}
                    }
                    match patch.parent_id {
                        Patch::Value(id) => channel.parent_id = Some(id),
                        Patch::Null => channel.parent_id = None,
                        Patch::Absent => {}
                    }
                    if let Patch::Value(position) = patch.position {
                        channel.position = position;
                    }
                    if let Patch::Value(kind) = patch.kind {
                        channel.kind = kind;
                    }
                    if self.selected == Some(channel.id) && !channel.supports_text() {
                        self.selected = None;
                        self.invalidate_members();
                        self.timeline.clear();
                        self.cancel_history();
                        self.freshness = Freshness::Unavailable;
                    }
                }
                Ok(())
            }
            Event::Voice(event) => {
                self.apply_voice(event);
                Ok(())
            }
            Event::RecipientAdded { channel, user } => {
                if let Some(c) = self
                    .channels
                    .iter_mut()
                    .find(|c| c.id == channel && c.guild.is_none())
                {
                    if let Some(old) = c.recipients.iter_mut().find(|u| u.id == user.id) {
                        *old = user;
                    } else if c.recipients.len() < 64 {
                        c.recipients.push(user);
                    } else {
                        self.fail(auth::Failure::Capacity);
                        return;
                    }
                    if self.selected == Some(channel) && self.members.is_some() {
                        let _ = self.request_members();
                    }
                }
                Ok(())
            }
            Event::RecipientRemoved { channel, user } => {
                if self.user.as_ref().is_some_and(|u| u.id == user) {
                    self.read_state.forget(channel);
                    self.channels.retain(|c| c.id != channel);
                    if self.selected == Some(channel) {
                        self.invalidate_members();
                        self.timeline.clear();
                        self.freshness = Freshness::Unavailable;
                        self.cancel_history();
                        self.status = "Conversation unavailable; account removed";
                    }
                    return;
                }
                if let Some(c) = self
                    .channels
                    .iter_mut()
                    .find(|c| c.id == channel && c.guild.is_none())
                {
                    c.recipients.retain(|u| u.id != user);
                    if self.selected == Some(channel) && self.members.is_some() {
                        let _ = self.request_members();
                    }
                }
                Ok(())
            }
            Event::Members(list) => {
                if !self.gateway_connected
                    || self.freshness == Freshness::Unavailable
                    || self.selected != Some(list.channel)
                    || self
                        .members
                        .as_ref()
                        .is_none_or(|m| m.request != list.request || m.guild != list.guild)
                {
                    return;
                }
                if list.rows.len() > 100
                    || list.rows.iter().flatten().map(Member::bytes).sum::<usize>() > 128 * 1024
                {
                    self.members.as_mut().unwrap().freshness = Freshness::Unavailable;
                } else {
                    self.members = Some(list);
                }
                Ok(())
            }
            Event::Ready {
                user,
                guilds,
                channels,
            } => {
                if self
                    .user
                    .as_ref()
                    .is_some_and(|previous| previous.id != user.id)
                {
                    self.auth = auth::AuthState::Failed;
                    self.status = "Different account rejected; log out before switching accounts";
                    return;
                }
                if channels.len() + guilds.len() > MAX_NAV
                    || channels.iter().any(|c| c.recipients.len() > 64)
                {
                    self.auth = auth::AuthState::Failed;
                    self.status = "Account navigation exceeds safe capacity";
                    return;
                }
                self.members = None;
                self.clear_profile();
                self.read_state.reset();
                self.user = Some(user);
                self.guilds = guilds;
                self.channels = channels;
                self.auth = auth::AuthState::Authenticated;
                self.gateway_connected = true;
                self.status = "Connected · unofficial session";
                Ok(())
            }
            Event::History {
                channel,
                request,
                older,
                mut messages,
            } => {
                if self.selected != Some(channel)
                    || request != self.request
                    || !self.history_pending
                {
                    return;
                }
                let mut ids = BTreeSet::new();
                if older != self.history_before.is_some()
                    || messages.len() > 50
                    || messages.iter().any(|message| {
                        message.channel != channel
                            || self
                                .history_before
                                .is_some_and(|before| message.id >= before)
                            || !ids.insert(message.id)
                    })
                {
                    self.cancel_history();
                    self.fail(auth::Failure::Protocol);
                    return;
                }
                self.history_pending = false;
                if !older && let Some(latest) = messages.iter().map(|m| m.id).max() {
                    self.observe_last_message(channel, latest);
                }
                for message in &mut messages {
                    if self.reactions.invalidated(message.id) {
                        message.reactions = None;
                    }
                }
                self.older_exhausted = messages.len() < 50;
                let r = self.timeline.finish_page(messages, older);
                if r.is_ok() && self.gateway_connected {
                    self.freshness = Freshness::Fresh;
                }
                r
            }
            Event::HistoryFailed {
                channel,
                request,
                failure,
            } => {
                if self.selected != Some(channel)
                    || request != self.request
                    || !self.history_pending
                {
                    return;
                }
                self.cancel_history();
                if failure == auth::Failure::Forbidden {
                    self.invalidate_members();
                    self.timeline.clear();
                    self.freshness = Freshness::Unavailable;
                    self.status = "Channel unavailable or permission denied";
                } else {
                    self.fail(failure);
                }
                Ok(())
            }
            Event::Message(mut m) => {
                self.observe_last_message(m.channel, m.id);
                if self.selected == Some(m.channel)
                    && self.reactions.invalidated(m.id)
                    && m.reactions.is_some()
                {
                    self.refresh_reactions(m.id);
                }
                if self.reactions.invalidated(m.id) {
                    m.reactions = None;
                }
                self.confirm(&m);
                if self.selected == Some(m.channel) && self.freshness != Freshness::Unavailable {
                    self.timeline.insert(m, true, false)
                } else {
                    Ok(())
                }
            }
            Event::Patch(mut p) => {
                if self.selected == Some(p.channel)
                    && self.reactions.invalidated(p.id)
                    && !matches!(p.reactions, Patch::Absent)
                {
                    self.refresh_reactions(p.id);
                }
                if self.reactions.invalidated(p.id) {
                    p.reactions = Patch::Absent;
                }
                if self.selected == Some(p.channel) && self.freshness != Freshness::Unavailable {
                    self.timeline.patch(p)
                } else {
                    Ok(())
                }
            }
            Event::Delete { channel, id } => {
                if let Some(channel) = self
                    .channels
                    .iter_mut()
                    .find(|c| c.id == channel && c.last_message == Some(id))
                {
                    channel.last_message = None;
                }
                if self.selected == Some(channel) {
                    self.timeline.delete(id)
                } else {
                    Ok(())
                }
            }
            Event::DeleteBulk { channel, ids } => {
                if let Some(channel) = self
                    .channels
                    .iter_mut()
                    .find(|c| c.id == channel && c.last_message.is_some_and(|id| ids.contains(&id)))
                {
                    channel.last_message = None;
                }
                if ids.len() > 100 {
                    Err("Bulk deletion exceeds safe capacity")
                } else if self.selected == Some(channel) {
                    ids.into_iter().try_for_each(|id| self.timeline.delete(id))
                } else {
                    Ok(())
                }
            }
            Event::SendResult { nonce, result } => {
                match result {
                    Ok(m) => {
                        if let Some(p) = self.pending.iter_mut().find(|p| p.nonce == nonce) {
                            p.delivery = Delivery::Confirmed;
                            p.confirmed = Some(m.id);
                        }
                        if self.selected == Some(m.channel)
                            && self.freshness != Freshness::Unavailable
                            && self.timeline.get(m.id).is_none()
                            && self.timeline.insert(m, false, false).is_err()
                        {
                            self.freshness = Freshness::Stale;
                            self.status = "Message exceeds safe capacity";
                        }
                    }
                    Err(f) => {
                        if let Some(p) = self
                            .pending
                            .iter_mut()
                            .find(|p| p.nonce == nonce && p.delivery != Delivery::Confirmed)
                        {
                            p.delivery = if f == auth::Failure::Ambiguous {
                                Delivery::Ambiguous
                            } else {
                                Delivery::Rejected
                            };
                        }
                        self.fail(f);
                    }
                }
                self.pending.retain(|p| p.delivery != Delivery::Confirmed);
                Ok(())
            }
            Event::Failure(f) => {
                self.fail(f);
                Ok(())
            }
            Event::Disconnected => {
                self.read_state.cancel();
                self.clear_profile();
                self.disconnect_voice();
                self.invalidate_members();
                self.gateway_connected = false;
                self.cancel_history();
                self.freshness = Freshness::Stale;
                self.status = "Disconnected · history may be stale";
                Ok(())
            }
            Event::Resumed => {
                self.members = None;
                self.gateway_connected = true;
                self.cancel_history();
                self.freshness = Freshness::Stale;
                self.status = "Gateway resumed · reload active history to verify freshness";
                Ok(())
            }
            Event::Resync | Event::PermissionsChanged => {
                self.read_state.cancel();
                self.clear_profile();
                self.disconnect_voice();
                self.invalidate_members();
                self.timeline.clear();
                self.freshness = Freshness::Stale;
                self.cancel_history();
                self.status = "Session or permissions changed · reload active history";
                Ok(())
            }
            Event::Unavailable(channel) => {
                self.read_state.forget(channel);
                self.clear_profile();
                self.channels.retain(|c| c.id != channel);
                if self
                    .voice
                    .active
                    .as_ref()
                    .is_some_and(|c| c.channel == channel)
                {
                    self.disconnect_voice();
                }
                if self.selected == Some(channel) {
                    self.invalidate_members();
                    self.timeline.clear();
                    self.freshness = Freshness::Unavailable;
                    self.cancel_history();
                }
                self.status = "Channel unavailable or permission denied";
                Ok(())
            }
        };
        if let Err(status) = result {
            self.status = status;
            self.freshness = Freshness::Stale;
            self.timeline.clear();
            self.cancel_history();
        }
        if !self.can_search() && self.search.is_some() {
            self.clear_search();
        }
    }
    fn invalidate_members(&mut self) {
        self.member_request = self.member_request.wrapping_add(1);
        if let Some(list) = &mut self.members {
            list.request = self.member_request;
            list.rows.clear();
            list.freshness = Freshness::Unavailable;
        }
    }
    fn confirm(&mut self, message: &Message) {
        if self.user.as_ref().map(|u| u.id) != Some(message.author.id) {
            return;
        }
        if let Some(nonce) = &message.nonce {
            self.pending
                .retain(|p| !(p.nonce == *nonce && p.channel == message.channel));
        }
    }
    fn fail(&mut self, failure: auth::Failure) {
        self.status = failure.label();
        match failure {
            auth::Failure::Expired => self.auth = auth::AuthState::Expired,
            auth::Failure::Challenged => self.auth = auth::AuthState::Challenged,
            _ => {}
        }
        if failure.ends_session() {
            self.clear_search();
            self.search_target = None;
            self.read_state.cancel();
            self.clear_profile();
            self.disconnect_voice();
            self.gateway_connected = false;
            self.invalidate_members();
            self.cancel_history();
        }
        self.freshness = Freshness::Stale;
    }
    fn cancel_history(&mut self) {
        self.reactions.reset();
        self.request += 1;
        self.history_pending = false;
        self.timeline.cancel_page();
    }
}

impl Event {
    /// Admission estimate for heap-owned fields; rejects oversized items before entering the UI queue.
    pub fn bytes(&self) -> usize {
        size_of::<Self>()
            + match self {
                Self::Search {
                    result: Ok(search::Outcome::Page(page) | search::Outcome::Pins(page)),
                    ..
                } => page.bytes(),
                Self::ReadState(read_state::Event::Snapshot { entries, .. }) => entries
                    .as_ref()
                    .map_or(0, |e| e.capacity() * size_of::<(Id, Option<Id>)>()),
                Self::ReadState(read_state::Event::Latest(entries)) => {
                    entries.capacity() * size_of::<(Id, Patch<Id>)>()
                }
                Self::Reactions(reactions::Event::Read { result, .. }) => {
                    result.as_ref().map_or(0, |r| model::reaction_bytes(r))
                }
                Self::Profile { result, .. } => result.as_ref().map_or(0, UserProfile::bytes),
                Self::Voice(event) => event.bytes(),
                Self::GuildChanged(patch) => [&patch.name, &patch.icon]
                    .into_iter()
                    .map(|value| match value {
                        Patch::Value(value) => value.capacity(),
                        _ => 0,
                    })
                    .sum(),
                Self::ChannelCreated(channel) => channel.bytes(),
                Self::ChannelChanged(patch) => match &patch.name {
                    Patch::Value(name) => name.capacity(),
                    _ => 0,
                },
                Self::Ready {
                    user,
                    guilds,
                    channels,
                } => {
                    user.heap_bytes()
                        + guilds
                            .iter()
                            .map(|g| {
                                size_of::<Guild>()
                                    + g.name.capacity()
                                    + g.icon.as_ref().map_or(0, String::capacity)
                            })
                            .sum::<usize>()
                        + channels.iter().map(Channel::bytes).sum::<usize>()
                }
                Self::Members(list) => {
                    list.rows.capacity() * size_of::<Option<Member>>()
                        + list.rows.iter().flatten().map(Member::bytes).sum::<usize>()
                }
                Self::RecipientAdded { user, .. } => user.heap_bytes(),
                Self::History { messages, .. } => messages.iter().map(Message::bytes).sum(),
                Self::Message(m) => m.bytes(),
                Self::Patch(p) => {
                    let content = match &p.content {
                        Patch::Value(s) => s.capacity(),
                        _ => 0,
                    };
                    content
                        + match &p.reactions {
                            Patch::Value(r) => model::reaction_bytes(r),
                            _ => 0,
                        }
                        + match &p.mentions {
                            Patch::Value(users) => model::mention_bytes(users),
                            _ => 0,
                        }
                        + match &p.attachments {
                            Patch::Value(attachments) => model::attachment_bytes(attachments),
                            _ => 0,
                        }
                        + match &p.embeds {
                            Patch::Value(embeds) => model::embed_bytes(embeds),
                            _ => 0,
                        }
                }
                Self::DeleteBulk { ids, .. } => ids.capacity() * size_of::<Id>(),
                Self::SendResult { nonce, result } => {
                    nonce.capacity() + result.as_ref().map_or(0, Message::bytes)
                }
                _ => 0,
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaction_readback_coalesces_races_and_never_replays_uncertain_writes() {
        use reactions::{Command as R, Event as E};
        let channel = Id(1);
        let id = Id(10);
        let emoji = ReactionEmoji {
            id: None,
            name: Some("👍".into()),
        };
        let values = vec![Reaction {
            emoji: emoji.clone(),
            count: 3,
            me: true,
            me_burst: false,
        }];
        let mut state = State {
            selected: Some(channel),
            gateway_connected: true,
            auth: auth::AuthState::Authenticated,
            freshness: Freshness::Fresh,
            ..State::default()
        };
        state.timeline.insert(message(10), false, false).unwrap();
        let Command::Reactions(R::Set { request, add, .. }) =
            state.prepare_reaction(id, emoji.clone()).unwrap()
        else {
            panic!()
        };
        assert!(add);
        assert!(state.prepare_reaction(id, emoji.clone()).is_none());
        apply(
            &mut state,
            Event::Reactions(E::Written {
                channel,
                message: id,
                request,
                result: Err(auth::Failure::Ambiguous),
            }),
        );
        assert!(state.reactions.writing.is_none());
        let Command::Reactions(R::Read { request, .. }) = state.next_reaction_read().unwrap()
        else {
            panic!()
        };
        for _ in 0..100 {
            apply(
                &mut state,
                Event::Reactions(E::Changed {
                    channel,
                    message: id,
                }),
            );
        }
        assert!(state.next_reaction_read().is_none());
        apply(
            &mut state,
            Event::Reactions(E::Read {
                channel,
                message: id,
                request,
                result: Ok(values.clone()),
            }),
        );
        assert!(state.timeline.get(id).unwrap().reactions.is_none());
        let Command::Reactions(R::Read { request, .. }) = state.next_reaction_read().unwrap()
        else {
            panic!()
        };
        apply(
            &mut state,
            Event::Reactions(E::Read {
                channel,
                message: id,
                request,
                result: Ok(values.clone()),
            }),
        );
        assert_eq!(
            state.timeline.get(id).unwrap().reactions,
            Some(values.clone())
        );
        assert!(state.next_reaction_read().is_none());
        let command = state.prepare_reaction(id, emoji.clone()).unwrap();
        assert!(matches!(
            &command,
            Command::Reactions(R::Set { add: false, .. })
        ));
        state.command_rejected(command);
        assert!(state.reactions.writing.is_none());
        assert_eq!(state.freshness, Freshness::Fresh);
        // Successful reaction removal/readback changes only reactions, not chat access.
        let original_content = state.timeline.get(id).unwrap().content.clone();
        let Command::Reactions(R::Set {
            request,
            add: false,
            ..
        }) = state.prepare_reaction(id, emoji.clone()).unwrap()
        else {
            panic!()
        };
        apply(
            &mut state,
            Event::Reactions(E::Written {
                channel,
                message: id,
                request,
                result: Ok(()),
            }),
        );
        assert_eq!(state.timeline.get(id).unwrap().content, original_content);
        let Command::Reactions(R::Read { request, .. }) = state.next_reaction_read().unwrap()
        else {
            panic!()
        };
        apply(
            &mut state,
            Event::Reactions(E::Read {
                channel,
                message: id,
                request,
                result: Ok(vec![]),
            }),
        );
        assert_eq!(state.timeline.get(id).unwrap().content, original_content);
        assert_eq!(state.timeline.get(id).unwrap().reactions, Some(vec![]));
        assert_eq!(state.freshness, Freshness::Fresh);
        assert!(state.next_reaction_read().is_none());
        // Reaction events during history loading cannot be overwritten by that page.
        let _ = state.history(None);
        apply(
            &mut state,
            Event::Reactions(E::Changed {
                channel,
                message: Id(11),
            }),
        );
        let history_request = state.request;
        apply(
            &mut state,
            Event::History {
                channel,
                request: history_request,
                older: false,
                messages: vec![message(11)],
            },
        );
        assert!(state.timeline.get(Id(11)).unwrap().reactions.is_none());
        let Command::Reactions(R::Read {
            message: id,
            request,
            ..
        }) = state.next_reaction_read().unwrap()
        else {
            panic!()
        };
        // Deletion wins over an in-flight reaction response.
        apply(&mut state, Event::Delete { channel, id });
        apply(
            &mut state,
            Event::Reactions(E::Read {
                channel,
                message: id,
                request,
                result: Ok(values.clone()),
            }),
        );
        assert!(state.timeline.get(id).is_none());
        // Rate limits leave an explicit retry, never an automatic loop.
        state.timeline.insert(message(12), false, false).unwrap();
        state.refresh_reactions(Id(12));
        let Command::Reactions(R::Read {
            message: id,
            request,
            ..
        }) = state.next_reaction_read().unwrap()
        else {
            panic!()
        };
        apply(
            &mut state,
            Event::Reactions(E::Read {
                channel,
                message: id,
                request,
                result: Err(auth::Failure::RateLimited),
            }),
        );
        assert!(state.next_reaction_read().is_none());
        state.refresh_reactions(id);
        let Command::Reactions(R::Read { request, .. }) = state.next_reaction_read().unwrap()
        else {
            panic!()
        };
        state.reactions.reset(); // navigation/disconnect cancels this request
        apply(
            &mut state,
            Event::Reactions(E::Read {
                channel,
                message: id,
                request,
                result: Ok(values),
            }),
        );
        assert!(state.timeline.get(id).unwrap().reactions.is_none());
        state.refresh_reactions(id);
        let Command::Reactions(R::Read { request, .. }) = state.next_reaction_read().unwrap()
        else {
            panic!()
        };
        apply(
            &mut state,
            Event::Reactions(E::Read {
                channel,
                message: id,
                request,
                result: Err(auth::Failure::Forbidden),
            }),
        );
        assert!(state.timeline.is_empty());
        assert_eq!(state.freshness, Freshness::Unavailable);
        state.logout();
        assert!(state.next_reaction_read().is_none());
    }

    #[test]
    fn guild_identity_patches_preserve_omitted_fields_and_change_icon_keys() {
        let mut state = State {
            guilds: vec![Guild {
                id: Id(2),
                name: "Synthetic server".into(),
                icon: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()),
            }],
            ..State::default()
        };
        let key = state.guilds[0].icon_key();
        apply(
            &mut state,
            Event::GuildChanged(GuildPatch {
                id: Id(2),
                name: Patch::Value("Renamed".into()),
                icon: Patch::Absent,
            }),
        );
        assert_eq!(state.guilds[0].name, "Renamed");
        assert_eq!(state.guilds[0].icon_key(), key);
        apply(
            &mut state,
            Event::GuildChanged(GuildPatch {
                id: Id(2),
                name: Patch::Absent,
                icon: Patch::Value("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into()),
            }),
        );
        assert_ne!(state.guilds[0].icon_key(), key);
        assert_eq!(state.guilds[0].name, "Renamed");
        apply(
            &mut state,
            Event::GuildChanged(GuildPatch {
                id: Id(2),
                name: Patch::Absent,
                icon: Patch::Null,
            }),
        );
        assert!(state.guilds[0].icon_key().is_none());
        apply(
            &mut state,
            Event::GuildChanged(GuildPatch {
                id: Id(2),
                name: Patch::Value("x".repeat(1024)),
                icon: Patch::Value("../../invalid".into()),
            }),
        );
        assert_eq!(state.guilds[0].name.len(), 128);
        assert!(state.guilds[0].icon.is_none());
        apply(
            &mut state,
            Event::GuildChanged(GuildPatch {
                id: Id(3),
                name: Patch::Value("Unknown".into()),
                icon: Patch::Absent,
            }),
        );
        assert_eq!(state.guilds.len(), 1);
        state.apply(Envelope {
            generation: state.generation + 1,
            event: Event::GuildChanged(GuildPatch {
                id: Id(2),
                name: Patch::Value("Late event".into()),
                icon: Patch::Absent,
            }),
        });
        assert_eq!(state.guilds[0].name.len(), 128);
    }

    #[test]
    fn channel_mutations_preserve_partial_metadata_and_remove_deleted_categories() {
        let mut state = State::default();
        let channel = Channel {
            last_message: None,
            id: Id(2),
            guild: Some(Id(1)),
            parent_id: Some(Id(3)),
            position: 7,
            name: "Synthetic channel".into(),
            kind: 0,
            recipients: vec![],
            member_list_id: None,
        };
        apply(&mut state, Event::ChannelCreated(channel.clone()));
        apply(&mut state, Event::ChannelCreated(channel));
        assert_eq!(state.channels.len(), 1);
        apply(
            &mut state,
            Event::ChannelChanged(ChannelPatch {
                last_message: model::Patch::Absent,
                id: Id(2),
                name: Patch::Absent,
                parent_id: Patch::Null,
                position: Patch::Value(0),
                kind: Patch::Absent,
            }),
        );
        assert_eq!(state.channels[0].parent_id, None);
        assert_eq!(state.channels[0].position, 0);
        assert_eq!(state.channels[0].name, "Synthetic channel");
        assert_eq!(state.channels[0].kind, 0);
        assert!(state.select(Id(2)).is_some());
        apply(
            &mut state,
            Event::ChannelChanged(ChannelPatch {
                last_message: model::Patch::Absent,
                id: Id(2),
                name: Patch::Absent,
                parent_id: Patch::Absent,
                position: Patch::Absent,
                kind: Patch::Value(4),
            }),
        );
        assert!(state.selected.is_none());
        assert!(!state.history_pending);
        assert!(state.select(Id(2)).is_none());
        apply(&mut state, Event::Unavailable(Id(2)));
        assert!(state.channels.is_empty());
        for id in 1..=MAX_NAV + 1 {
            apply(
                &mut state,
                Event::ChannelCreated(Channel {
                    last_message: None,
                    id: Id(id as u64),
                    guild: None,
                    parent_id: None,
                    position: 0,
                    name: String::new(),
                    kind: 4,
                    recipients: vec![],
                    member_list_id: None,
                }),
            );
        }
        assert_eq!(state.channels.len(), MAX_NAV);
        assert_eq!(state.status, auth::Failure::Capacity.label());
    }

    fn apply(state: &mut State, event: Event) {
        state.apply(Envelope {
            generation: state.generation,
            event,
        });
    }
    fn message(id: u64) -> Message {
        Message {
            reactions: Some(vec![]),
            id: Id(id),
            channel: Id(1),
            author: User {
                id: Id(2),
                name: "Synthetic".into(),
                avatar: None,
                discriminator: 0,
            },
            content: "Synthetic history".into(),
            edited: false,
            edited_at: None,
            revision: 0,
            nonce: None,
            reply_to: None,
            unsupported: false,
            embeds: vec![],
            attachments: vec![],
            mentions: Vec::new(),
            embeds_suppressed: false,
        }
    }
    #[test]
    fn members_reject_late_requests_and_permission_invalidations() {
        let user = message(1).author;
        let mut state = State {
            user: Some(user.clone()),
            gateway_connected: true,
            auth: auth::AuthState::Authenticated,
            channels: vec![Channel {
                last_message: None,
                id: Id(1),
                guild: None,
                parent_id: None,
                position: 0,
                name: "DM".into(),
                kind: 1,
                recipients: vec![user.clone()],
                member_list_id: None,
            }],
            ..State::default()
        };
        state.select(Id(1));
        state.request_members();
        assert_eq!(state.members.as_ref().unwrap().rows.len(), 1);
        let previous = state.members.clone().unwrap();
        state.close_members();
        apply(&mut state, Event::Members(previous.clone()));
        assert!(state.members.is_none());
        state.request_members();
        apply(&mut state, Event::PermissionsChanged);
        apply(&mut state, Event::Members(previous));
        assert!(state.members.as_ref().unwrap().rows.is_empty());
        assert_eq!(
            state.members.as_ref().unwrap().freshness,
            Freshness::Unavailable
        );
        apply(&mut state, Event::Resumed);
        assert!(state.members.is_none());
        state.request_members();
        apply(
            &mut state,
            Event::RecipientAdded {
                channel: Id(1),
                user: User {
                    id: Id(3),
                    name: "Other".into(),
                    avatar: None,
                    discriminator: 0,
                },
            },
        );
        assert_eq!(state.members.as_ref().unwrap().rows.len(), 2);
        apply(
            &mut state,
            Event::RecipientRemoved {
                channel: Id(1),
                user: Id(3),
            },
        );
        assert_eq!(state.members.as_ref().unwrap().rows.len(), 1);
    }
    #[test]
    fn history_is_scoped_bounded_and_cannot_restore_freshness_after_disconnect() {
        let mut state = State::default();
        apply(
            &mut state,
            Event::Ready {
                user: User {
                    id: Id(2),
                    name: "Synthetic".into(),
                    avatar: None,
                    discriminator: 0,
                },
                guilds: vec![],
                channels: vec![Channel {
                    last_message: None,
                    id: Id(1),
                    guild: None,
                    parent_id: None,
                    position: 0,
                    name: "Synthetic".into(),
                    kind: 1,
                    recipients: vec![],
                    member_list_id: None,
                }],
            },
        );
        state.select(Id(1));
        let old_request = state.request;
        apply(&mut state, Event::Disconnected);
        apply(
            &mut state,
            Event::History {
                channel: Id(1),
                request: old_request,
                older: false,
                messages: vec![message(99)],
            },
        );
        assert_eq!(state.freshness, Freshness::Stale);
        assert!(state.timeline.is_empty());
        assert!(!state.can_load_older());

        apply(&mut state, Event::Resumed);
        state.history(None);
        let request = state.request;
        apply(
            &mut state,
            Event::History {
                channel: Id(1),
                request,
                older: false,
                messages: (51..101).map(message).collect(),
            },
        );
        assert_eq!(state.freshness, Freshness::Fresh);
        assert!(state.can_load_older());
        assert!(matches!(
            state.older_history(),
            Some(Command::History {
                before: Some(Id(51)),
                ..
            })
        ));
        assert!(state.older_history().is_none()); // no duplicate concurrent page
        let request = state.request;
        apply(
            &mut state,
            Event::HistoryFailed {
                channel: Id(1),
                request: old_request,
                failure: auth::Failure::Forbidden,
            },
        );
        assert_eq!(state.freshness, Freshness::Loading);
        apply(
            &mut state,
            Event::History {
                channel: Id(1),
                request,
                older: true,
                messages: vec![message(50)],
            },
        );
        assert_eq!(state.timeline.len(), 51);
        assert!(!state.can_load_older()); // short final page
        apply(
            &mut state,
            Event::DeleteBulk {
                channel: Id(1),
                ids: vec![Id(50), Id(51)],
            },
        );
        assert_eq!(state.timeline.len(), 49);
        assert!(state.timeline.get(Id(50)).is_none());

        state.history(None);
        let request = state.request;
        apply(
            &mut state,
            Event::History {
                channel: Id(1),
                request,
                older: false,
                messages: vec![message(100)],
            },
        );
        assert_eq!(state.timeline.len(), 1); // old records absent from fresh page disappear
        state.history(Some(Id(100)));
        let request = state.request;
        apply(
            &mut state,
            Event::History {
                channel: Id(1),
                request,
                older: true,
                messages: vec![message(100)],
            },
        );
        assert_eq!(state.freshness, Freshness::Stale); // before boundary cannot repeat itself
        state.history(None);
        let request = state.request;
        apply(
            &mut state,
            Event::HistoryFailed {
                channel: Id(1),
                request,
                failure: auth::Failure::Forbidden,
            },
        );
        assert_eq!(state.freshness, Freshness::Unavailable);
        assert!(state.timeline.is_empty());
        apply(&mut state, Event::Message(message(200)));
        assert!(state.timeline.is_empty()); // queued live content cannot undo revocation
    }
}
