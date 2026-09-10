use crate::{
    State,
    auth::{AuthState, Failure},
};
use model::{Freshness, Id, Patch};
use std::collections::BTreeMap;

pub enum Event {
    Snapshot {
        entries: Option<Vec<(Id, Option<Id>)>>,
        version: Option<u64>,
        partial: bool,
    },
    Ack {
        channel: Id,
        message: Option<Id>,
        manual: bool,
        version: Option<u64>,
    },
    Latest(Vec<(Id, Patch<Id>)>),
    Result {
        channel: Id,
        message: Id,
        request: u64,
        result: Result<(), Failure>,
    },
}
#[derive(Default)]
pub struct ReadState {
    entries: BTreeMap<Id, (Option<Id>, u64)>,
    known: bool,
    version: Option<u64>,
    revision: u64,
    pending: Option<(Id, Id, u64, u64)>,
    pub status: Option<&'static str>,
}
impl ReadState {
    pub fn reset(&mut self) {
        *self = Self {
            revision: self.revision.wrapping_add(1),
            ..Self::default()
        };
    }
    pub fn cancel(&mut self) {
        self.pending = None;
        self.status = None;
    }
    pub fn forget(&mut self, channel: Id) {
        self.entries.remove(&channel);
        if self.pending.is_some_and(|(id, ..)| id == channel) {
            self.cancel();
        }
    }
}
impl State {
    pub fn read_marker(&self, channel: Id) -> Option<Option<Id>> {
        if !self.gateway_connected
            || !self
                .channels
                .iter()
                .any(|c| c.id == channel && c.supports_text())
        {
            return None;
        }
        (self.read_state.known || self.read_state.entries.contains_key(&channel)).then(|| {
            self.read_state
                .entries
                .get(&channel)
                .and_then(|(id, _)| *id)
        })
    }
    pub fn unread(&self, channel: Id) -> Option<bool> {
        let read = self.read_marker(channel)?;
        let latest = self
            .channels
            .iter()
            .find(|c| c.id == channel)?
            .last_message?;
        Some(read.is_none_or(|read| latest > read))
    }
    pub fn can_mark_read(&self, message: Id) -> bool {
        self.auth == AuthState::Authenticated
            && self.gateway_connected
            && self.freshness == Freshness::Fresh
            && self.read_state.pending.is_none()
            && self
                .timeline
                .get(message)
                .is_some_and(|m| Some(m.channel) == self.selected)
            && self.selected.is_some_and(|channel| {
                self.read_marker(channel)
                    .flatten()
                    .is_none_or(|read| message > read)
            })
    }
    pub fn prepare_mark_read(&mut self, message: Id) -> Option<crate::Command> {
        if !self.can_mark_read(message) {
            return None;
        }
        let channel = self.selected?;
        self.read_state.revision = self.read_state.revision.wrapping_add(1);
        let request = self.read_state.revision;
        let epoch = self
            .read_state
            .entries
            .get(&channel)
            .map_or(0, |(_, epoch)| *epoch);
        self.read_state.pending = Some((channel, message, request, epoch));
        self.read_state.status = Some("Marking read…");
        Some(crate::Command::MarkRead {
            channel,
            message,
            request,
        })
    }
    pub fn observe_last_message(&mut self, channel: Id, message: Id) {
        if let Some(channel) = self.channels.iter_mut().find(|c| c.id == channel)
            && channel.last_message.is_none_or(|id| message > id)
        {
            channel.last_message = Some(message);
        }
    }
    pub fn apply_read_state(&mut self, event: Event) -> Result<(), &'static str> {
        match event {
            Event::Snapshot {
                entries,
                version,
                partial,
            } => {
                if entries
                    .as_ref()
                    .is_some_and(|items| items.len() > crate::MAX_NAV)
                {
                    return Err("Read-state capacity exceeded");
                }
                self.read_state = ReadState {
                    revision: self.read_state.revision.wrapping_add(1),
                    known: entries.is_some() && !partial,
                    version,
                    ..ReadState::default()
                };
                for (channel, message) in entries.unwrap_or_default() {
                    if self
                        .channels
                        .iter()
                        .any(|c| c.id == channel && c.supports_text())
                        && self
                            .read_state
                            .entries
                            .insert(channel, (message, self.read_state.revision))
                            .is_some()
                    {
                        self.read_state.reset();
                        return Err("Duplicate channel read state");
                    }
                }
            }
            Event::Ack {
                channel,
                message,
                manual,
                version,
            } => {
                if !self
                    .channels
                    .iter()
                    .any(|c| c.id == channel && c.supports_text())
                    || matches!((self.read_state.version,version),(Some(old),Some(new)) if new<old)
                {
                    return Ok(());
                }
                self.read_state.version = version.or(self.read_state.version);
                self.read_state.revision = self.read_state.revision.wrapping_add(1);
                let current = self
                    .read_state
                    .entries
                    .get(&channel)
                    .and_then(|(id, _)| *id);
                let message = if manual {
                    message
                } else {
                    message.max(current)
                };
                self.read_state
                    .entries
                    .insert(channel, (message, self.read_state.revision));
            }
            Event::Latest(channels) => {
                if channels.len() > crate::MAX_NAV {
                    return Err("Channel update capacity exceeded");
                }
                for (id, latest) in channels {
                    if let Some(channel) = self.channels.iter_mut().find(|c| c.id == id) {
                        match latest {
                            Patch::Value(id) => channel.last_message = Some(id),
                            Patch::Null => channel.last_message = None,
                            Patch::Absent => {}
                        }
                    }
                }
            }
            Event::Result {
                channel,
                message,
                request,
                result,
            } => {
                let Some((pending_channel, pending_message, pending_request, epoch)) =
                    self.read_state.pending
                else {
                    return Ok(());
                };
                if (channel, message, request)
                    != (pending_channel, pending_message, pending_request)
                {
                    return Ok(());
                }
                self.read_state.pending = None;
                match result {
                    Ok(()) => {
                        // A newer service ACK (including manual mark-unread) wins over this HTTP completion.
                        if self.channels.iter().any(|c| c.id == channel)
                            && self
                                .read_state
                                .entries
                                .get(&channel)
                                .map_or(0, |(_, epoch)| *epoch)
                                == epoch
                        {
                            self.read_state.revision = self.read_state.revision.wrapping_add(1);
                            let current = self
                                .read_state
                                .entries
                                .get(&channel)
                                .and_then(|(id, _)| *id);
                            self.read_state.entries.insert(
                                channel,
                                (Some(message).max(current), self.read_state.revision),
                            );
                        }
                        self.read_state.status = Some("Read marker saved");
                    }
                    Err(failure) => {
                        self.read_state.status = Some(failure.label());
                        if failure.ends_session() {
                            self.fail(failure);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
