use crate::{Channel, Id};

pub const PAGE_SIZE: usize = 25;
pub const MAX_BYTES: usize = 64 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Public,
    Private,
    JoinedPrivate,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cursor {
    Time(i128),
    Id(Id),
}
pub struct Page {
    pub threads: Vec<Channel>,
    pub next: Option<Cursor>,
}
impl Page {
    pub fn bytes(&self) -> usize {
        self.threads.capacity().saturating_sub(self.threads.len()) * size_of::<Channel>()
            + self.threads.iter().map(Channel::bytes).sum::<usize>()
    }
    pub fn valid(&self, parent: Id, guild: Id, kind: Kind, before: Option<Cursor>) -> bool {
        let valid_cursor = |cursor| {
            matches!(
                (kind, cursor),
                (Kind::Public | Kind::Private, Cursor::Time(_))
                    | (Kind::JoinedPrivate, Cursor::Id(Id(1..)))
            )
        };
        let advances = match (self.next, before) {
            (Some(Cursor::Time(next)), Some(Cursor::Time(before))) => next < before,
            (Some(Cursor::Id(next)), Some(Cursor::Id(before))) => next < before,
            (Some(_), Some(_)) => false,
            _ => true,
        };
        parent.0 > 0
            && guild.0 > 0
            && self.threads.len() <= PAGE_SIZE
            && self.bytes() <= MAX_BYTES
            && before.is_none_or(valid_cursor)
            && self.next.is_none_or(valid_cursor)
            && advances
            && (self.next.is_none() || !self.threads.is_empty())
            && self.threads.iter().enumerate().all(|(i, thread)| {
                thread.id.0 > 0
                    && thread.id != parent
                    && thread.guild == Some(guild)
                    && thread.parent_id == Some(parent)
                    && thread.name.len() <= 512
                    && matches!(
                        (kind, thread.kind),
                        (Kind::Public, 10 | 11) | (Kind::Private | Kind::JoinedPrivate, 12)
                    )
                    && self.threads[..i].iter().all(|other| other.id != thread.id)
                    && before.is_none_or(|cursor| match cursor {
                        Cursor::Id(id) => thread.id < id,
                        Cursor::Time(_) => true,
                    })
            })
            && (kind != Kind::JoinedPrivate
                || (self.threads.windows(2).all(|pair| pair[0].id > pair[1].id)
                    && self.next.is_none_or(|cursor| {
                        self.threads
                            .last()
                            .is_some_and(|thread| cursor == Cursor::Id(thread.id))
                    })))
    }
}
