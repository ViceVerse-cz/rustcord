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
        if !self.can_read_history(channel) {
            return None;
        }
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
        if !self.can_react(message, Some(&emoji), add) {
            return None;
        }
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
                    && self.can_view(channel)
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
                if !self.gateway_connected
                    || self.freshness == Freshness::Unavailable
                    || !self.can_read_history(channel)
                {
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

#[cfg(test)]
mod tests {
    use super::*;
    use model::{Channel, Message, User, permissions as p};

    #[test]
    fn reaction_permissions_distinguish_existing_emoji_and_late_reads() {
        let user = User {
            id: Id(2),
            name: "Synthetic".into(),
            avatar: None,
            discriminator: 0,
        };
        let mut state = State {
            auth: AuthState::Authenticated,
            gateway_connected: true,
            freshness: Freshness::Fresh,
            selected: Some(Id(10)),
            user: Some(user.clone()),
            guilds: vec![model::Guild {
                id: Id(1),
                name: "Synthetic".into(),
                icon: None,
                emojis: None,
            }],
            channels: vec![Channel {
                id: Id(10),
                guild: Some(Id(1)),
                kind: 0,
                name: "Synthetic".into(),
                parent_id: None,
                last_message: None,
                position: 0,
                recipients: vec![],
                member_list_id: None,
            }],
            ..State::default()
        };
        state
            .permissions
            .replace(p::Snapshot {
                guilds: vec![p::Guild {
                    id: Id(1),
                    owner: Some(Id(999)),
                    roles: Some(vec![p::Role {
                        id: Id(1),
                        bits: p::VIEW_CHANNEL | p::READ_MESSAGE_HISTORY,
                    }]),
                    member: Some(p::Member {
                        roles: vec![],
                        timeout_until: None,
                    }),
                }],
                channels: vec![p::Channel {
                    id: Id(10),
                    guild: Id(1),
                    overwrites: Some(vec![]),
                }],
            })
            .unwrap();
        let emoji = ReactionEmoji {
            id: None,
            name: Some("a".into()),
        };
        state
            .timeline
            .insert(
                Message {
                    id: Id(50),
                    channel: Id(10),
                    author: user,
                    content: "Synthetic".into(),
                    reactions: Some(vec![]),
                    mentions: vec![],
                    edited: false,
                    edited_at: None,
                    revision: 0,
                    nonce: None,
                    reply_to: None,
                    unsupported: false,
                    extra_content: Default::default(),
                    embeds: vec![],
                    embeds_suppressed: false,
                    attachments: vec![],
                },
                true,
                false,
            )
            .unwrap();
        assert!(state.prepare_reaction(Id(50), emoji.clone()).is_none());
        state
            .timeline
            .set_reactions(
                Id(50),
                Some(vec![Reaction {
                    emoji: emoji.clone(),
                    count: 1,
                    me: false,
                    me_burst: false,
                }]),
            )
            .unwrap();
        assert!(matches!(
            state.prepare_reaction(Id(50), emoji.clone()),
            Some(crate::Command::Reactions(Command::Set { add: true, .. }))
        ));
        state.reactions.writing = None;
        state
            .timeline
            .set_reactions(
                Id(50),
                Some(vec![Reaction {
                    emoji: emoji.clone(),
                    count: 1,
                    me: true,
                    me_burst: false,
                }]),
            )
            .unwrap();
        assert!(matches!(
            state.prepare_reaction(Id(50), emoji.clone()),
            Some(crate::Command::Reactions(Command::Set { add: false, .. }))
        ));
        state.reactions.writing = None;
        state.refresh_reactions(Id(50));
        let Some(crate::Command::Reactions(Command::Read { request, .. })) =
            state.next_reaction_read()
        else {
            panic!()
        };
        state
            .permissions
            .guilds
            .get_mut(&Id(1))
            .unwrap()
            .roles
            .as_mut()
            .unwrap()[0]
            .bits = p::VIEW_CHANNEL;
        state.permissions.clear_cache();
        state
            .apply_reactions(Event::Read {
                channel: Id(10),
                message: Id(50),
                request,
                result: Ok(vec![Reaction {
                    emoji,
                    count: 1,
                    me: true,
                    me_burst: false,
                }]),
            })
            .unwrap();
        assert!(state.timeline.get(Id(50)).unwrap().reactions.is_none());
        state.refresh_reactions(Id(50));
        assert!(state.next_reaction_read().is_none());
        assert!(
            state.can_mark_read(Id(50)),
            "Observed live messages may be marked read without history permission"
        );
        let Some(crate::Command::MarkRead { request, .. }) = state.prepare_mark_read(Id(50)) else {
            panic!()
        };
        state
            .permissions
            .guilds
            .get_mut(&Id(1))
            .unwrap()
            .roles
            .as_mut()
            .unwrap()[0]
            .bits = 0;
        state.permissions.clear_cache();
        state
            .apply_read_state(crate::read_state::Event::Result {
                channel: Id(10),
                message: Id(50),
                request,
                result: Ok(()),
            })
            .unwrap();
        assert!(state.read_marker(Id(10)).is_none());
        assert!(!state.can_mark_read(Id(50)));
        state
            .permissions
            .guilds
            .get_mut(&Id(1))
            .unwrap()
            .roles
            .as_mut()
            .unwrap()[0]
            .bits = p::VIEW_CHANNEL;
        state.permissions.clear_cache();
        assert!(
            state.read_marker(Id(10)).flatten().is_none(),
            "A late success cannot record a revoked read marker"
        );
    }
}
