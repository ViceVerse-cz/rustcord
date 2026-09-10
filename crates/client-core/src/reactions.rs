use crate::{
    State,
    auth::{AuthState, Failure},
};
use model::{Freshness, Id, Reaction, ReactionEmoji};
use std::collections::BTreeSet;

pub enum Command {
    Read {
        channel: Id,
        message: Id,
        request: u64,
    },
    Set {
        channel: Id,
        message: Id,
        emoji: ReactionEmoji,
        add: bool,
        request: u64,
    },
}
pub enum Event {
    Changed {
        channel: Id,
        message: Id,
    },
    Read {
        channel: Id,
        message: Id,
        request: u64,
        result: Result<Vec<Reaction>, Failure>,
    },
    Written {
        channel: Id,
        message: Id,
        request: u64,
        result: Result<(), Failure>,
    },
}
#[derive(Default)]
pub struct Reactions {
    dirty: BTreeSet<Id>,
    read: Option<(Id, u64)>,
    pub writing: Option<(Id, u64)>,
    sequence: u64,
}
impl Reactions {
    pub fn cancel_read(&mut self) {
        if let Some((message, _)) = self.read.take() {
            self.dirty.insert(message);
        }
    }
    pub fn reset(&mut self) {
        self.dirty.clear();
        self.read = None;
        self.writing = None;
        self.sequence = self.sequence.wrapping_add(1);
    }
    pub fn invalidated(&self, id: Id) -> bool {
        self.dirty.contains(&id) || self.read.is_some_and(|(message, _)| message == id)
    }
}
impl State {
    pub fn refresh_reactions(&mut self, message: Id) {
        if self.timeline.get(message).is_none() && !self.history_pending {
            return;
        }
        if self.reactions.dirty.len() >= session_cache::MAX_MUTATIONS
            && !self.reactions.dirty.contains(&message)
        {
            self.fail(Failure::Capacity);
            return;
        }
        self.reactions.dirty.insert(message);
        let _ = self.timeline.set_reactions(message, None);
        self.revision += 1;
    }
    pub fn next_reaction_read(&mut self) -> Option<crate::Command> {
        if self.auth != AuthState::Authenticated
            || !self.gateway_connected
            || self.freshness != Freshness::Fresh
            || self.history_pending
            || self.reactions.read.is_some()
        {
            return None;
        }
        let channel = self.selected?;
        self.reactions
            .dirty
            .retain(|id| self.timeline.get(*id).is_some());
        let message = self.reactions.dirty.pop_first()?;
        self.reactions.sequence = self.reactions.sequence.wrapping_add(1);
        let request = self.reactions.sequence;
        self.reactions.read = Some((message, request));
        Some(crate::Command::Reactions(Command::Read {
            channel,
            message,
            request,
        }))
    }
    pub fn prepare_reaction(
        &mut self,
        message: Id,
        emoji: ReactionEmoji,
    ) -> Option<crate::Command> {
        if self.auth != AuthState::Authenticated
            || !self.gateway_connected
            || self.freshness != Freshness::Fresh
            || self.reactions.writing.is_some()
            || !emoji.valid()
            || emoji.name.is_none()
        {
            return None;
        }
        let channel = self.selected?;
        let reactions = self.timeline.get(message)?.reactions.as_ref()?;
        let existing = reactions.iter().find(|r| r.emoji.same(&emoji));
        if existing.is_none() && reactions.len() >= model::MAX_REACTIONS {
            self.status = "Reaction limit reached";
            return None;
        }
        let add = existing.is_none_or(|r| !r.me);
        self.reactions.sequence = self.reactions.sequence.wrapping_add(1);
        let request = self.reactions.sequence;
        self.reactions.writing = Some((message, request));
        Some(crate::Command::Reactions(Command::Set {
            channel,
            message,
            emoji,
            add,
            request,
        }))
    }
    pub fn apply_reactions(&mut self, event: Event) -> Result<(), &'static str> {
        match event {
            Event::Changed { channel, message } => {
                if self.selected == Some(channel)
                    && self.freshness != Freshness::Unavailable
                    && self.gateway_connected
                {
                    self.refresh_reactions(message);
                }
            }
            Event::Read {
                channel,
                message,
                request,
                result,
            } => {
                if self.selected != Some(channel) || self.reactions.read != Some((message, request))
                {
                    return Ok(());
                }
                self.reactions.read = None;
                if !self.gateway_connected || self.freshness == Freshness::Unavailable {
                    return Ok(());
                }
                match result {
                    Ok(reactions) if !self.reactions.dirty.contains(&message) => {
                        self.timeline.set_reactions(message, Some(reactions))?
                    }
                    Ok(_) => {} // A newer invalidation schedules one fresh read, never an old snapshot.
                    Err(failure) => {
                        self.reactions.dirty.remove(&message); // No retry storm on a rejected read.
                        self.status = "Reactions unavailable; use Reload reactions to retry";
                        if failure == Failure::Forbidden {
                            self.apply(crate::Envelope {
                                generation: self.generation,
                                event: crate::Event::Unavailable(channel),
                            });
                        }
                        if failure.ends_session() {
                            self.fail(failure);
                        }
                    }
                }
            }
            Event::Written {
                channel,
                message,
                request,
                result,
            } => {
                if self.selected != Some(channel)
                    || self.reactions.writing != Some((message, request))
                {
                    return Ok(());
                }
                self.reactions.writing = None;
                match result {
                    Ok(()) => {
                        self.refresh_reactions(message);
                        self.status = "Reaction saved";
                    }
                    Err(failure) => {
                        // A timed-out write may have succeeded. Read back; never repeat the write.
                        if failure == Failure::Ambiguous {
                            self.refresh_reactions(message);
                        }
                        self.status = failure.label();
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
