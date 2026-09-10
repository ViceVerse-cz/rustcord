//! Session-local viewing markers; these never acknowledge messages to Discord.
use crate::{Freshness, Id, MAX_NAV, Message, State};

/// At most MAX_NAV fixed-size records, with no message content or dynamic payload.
pub struct ReadState {
    read_through: Id,
    latest: Id,
    latest_deleted: bool,
    first_unread: Option<Id>,
}

impl Default for ReadState {
    fn default() -> Self {
        Self {
            read_through: Id(0),
            latest: Id(0),
            latest_deleted: false,
            first_unread: None,
        }
    }
}

impl State {
    pub fn has_unread(&self, channel: Id) -> bool {
        self.first_unread(channel).is_some()
    }

    pub fn first_unread(&self, channel: Id) -> Option<Id> {
        self.read_state.get(&channel)?.first_unread
    }

    /// Call only when the focused view is following the latest messages.
    pub fn mark_read(&mut self, channel: Id) {
        if self.selected != Some(channel) || self.freshness != Freshness::Fresh {
            return;
        }
        let Some(latest) = self.timeline.iter().last().map(|m| m.id) else {
            return;
        };
        if !self.read_state.contains_key(&channel) && self.read_state.len() >= MAX_NAV {
            return;
        }
        let state = self.read_state.entry(channel).or_default();
        // An older retained window cannot acknowledge newer messages it does not show.
        if latest < state.latest {
            return;
        }
        state.read_through = state.read_through.max(latest);
        state.latest = state.latest.max(latest);
        if state.first_unread.take().is_some() {
            self.revision += 1;
        }
    }

    pub(super) fn delete_unread(&mut self, channel: Id, id: Id) {
        if let Some(state) = self.read_state.get_mut(&channel)
            && state.latest == id
        {
            state.latest_deleted = true;
            if state.first_unread == Some(id) {
                state.first_unread = None;
                state.read_through = state.read_through.max(id);
            }
        }
    }

    pub(super) fn refresh_unread_latest(&mut self, channel: Id) {
        if let Some(state) = self.read_state.get_mut(&channel)
            && state.latest_deleted
        {
            state.latest = self
                .timeline
                .iter()
                .last()
                .map_or(state.read_through, |m| m.id);
            state.latest_deleted = false;
            if self.timeline.is_empty() {
                state.first_unread = None;
            }
        }
    }

    pub(super) fn observe_unread(&mut self, message: &Message) {
        if self
            .user
            .as_ref()
            .is_none_or(|user| user.id == message.author.id)
            || !self
                .channels
                .iter()
                .any(|c| c.id == message.channel && c.supports_text())
            || (self.selected == Some(message.channel) && self.freshness == Freshness::Unavailable)
            || (!self.read_state.contains_key(&message.channel) && self.read_state.len() >= MAX_NAV)
        {
            return;
        }
        let state = self.read_state.entry(message.channel).or_default();
        if message.id > state.latest {
            state.latest_deleted = false;
        }
        state.latest = state.latest.max(message.id);
        if message.id > state.read_through {
            state.first_unread = Some(
                state
                    .first_unread
                    .map_or(message.id, |id| id.min(message.id)),
            );
        }
    }
}
