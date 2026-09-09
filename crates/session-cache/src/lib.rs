//! Explicitly bounded RAM timeline. Patches live for a page; tombstones live for this window.
use model::{Id, Message, MessagePatch, Patch};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_MESSAGES: usize = 500;
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_MUTATIONS: usize = 1024;
#[derive(Default)]
pub struct Timeline {
    messages: BTreeMap<Id, Message>,
    bytes: usize,
    changed: BTreeSet<Id>,
    patches: BTreeMap<Id, MessagePatch>,
    deleted: BTreeSet<Id>,
    loading: bool,
    retain_older: bool,
}
impl Timeline {
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Message> {
        self.messages.values()
    }
    pub fn get(&self, id: Id) -> Option<&Message> {
        self.messages.get(&id)
    }
    pub fn len(&self) -> usize {
        self.messages.len()
    }
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
    pub fn bytes(&self) -> usize {
        self.bytes
    }
    pub fn begin_page(&mut self) {
        self.loading = true;
        self.changed.clear();
        self.patches.clear();
    }
    pub fn cancel_page(&mut self) {
        self.loading = false;
        self.changed.clear();
        self.patches.clear();
    }
    fn remember(&mut self, id: Id) -> Result<(), &'static str> {
        if self.loading {
            if self.changed.len() >= MAX_MUTATIONS && !self.changed.contains(&id) {
                return Err("Reconciliation capacity exceeded; reload required");
            }
            self.changed.insert(id);
        }
        Ok(())
    }
    pub fn insert(
        &mut self,
        message: Message,
        live: bool,
        older: bool,
    ) -> Result<(), &'static str> {
        if self.deleted.contains(&message.id) {
            return Ok(());
        }
        if message.bytes() > MAX_BYTES || message.content.len() > 64 * 1024 {
            return Err("Message exceeds safe capacity");
        }
        if live {
            self.remember(message.id)?;
        }
        let mut message = message;
        if let Some(patch) = self.patches.get(&message.id) {
            apply_patch(&mut message, patch);
        } else if !live && self.changed.contains(&message.id) {
            return Ok(());
        }
        if let Some(previous) = self.messages.get(&message.id) {
            if previous
                .edited_at
                .is_some_and(|old| message.edited_at.is_none_or(|new| new < old))
            {
                return Ok(());
            }
            message.revision = previous.revision
                + u64::from(
                    previous.content != message.content || previous.edited != message.edited,
                );
        }
        self.bytes += message.bytes();
        if let Some(old) = self.messages.insert(message.id, message) {
            self.bytes -= old.bytes();
        }
        while self.messages.len() > MAX_MESSAGES || self.bytes > MAX_BYTES {
            let item = if older || self.retain_older {
                self.messages.pop_last()
            } else {
                self.messages.pop_first()
            };
            if let Some((_, old)) = item {
                self.bytes -= old.bytes();
            }
        }
        Ok(())
    }
    pub fn seed_cache(&mut self, items: Vec<Message>) -> Result<(), &'static str> {
        for item in items {
            self.insert(item, false, false)?;
        }
        Ok(())
    }
    pub fn finish_page(&mut self, items: Vec<Message>, older: bool) -> Result<(), &'static str> {
        // A recent-page reload is authoritative for the whole retained view. Preserve only
        // mutations observed during this request, never missing cached/old RAM records.
        self.retain_older = older;
        if !older {
            self.messages.retain(|id, _| self.changed.contains(id));
            self.bytes = self.messages.values().map(Message::bytes).sum();
        }
        for item in items {
            self.insert(item, false, older)?;
        }
        self.loading = false;
        self.changed.clear();
        self.patches.clear();
        Ok(())
    }
    pub fn patch(&mut self, patch: MessagePatch) -> Result<(), &'static str> {
        if self.deleted.contains(&patch.id) {
            return Ok(());
        }
        if matches!(&patch.content, Patch::Value(s) if s.len() > 64 * 1024) {
            return Err("Message patch exceeds capacity");
        }
        self.remember(patch.id)?;
        if let Some(mut message) = self.messages.remove(&patch.id) {
            self.bytes -= message.bytes();
            apply_patch(&mut message, &patch);
            self.insert(message, true, false)?;
        } else if self.loading {
            if self.patches.get(&patch.id).is_some_and(|previous| {
                matches!((&previous.edited, &patch.edited), (Patch::Value(old), Patch::Value(new)) if new < old)
            }) {
                return Ok(());
            }
            let bytes: usize = self
                .patches
                .values()
                .map(|p| match &p.content {
                    Patch::Value(s) => s.capacity(),
                    _ => 0,
                })
                .sum();
            let incoming = match &patch.content {
                Patch::Value(s) => s.capacity(),
                _ => 0,
            };
            let replaced = self.patches.get(&patch.id).map_or(0, |previous| {
                match (&patch.content, &previous.content) {
                    (Patch::Absent, _) => 0,
                    (_, Patch::Value(s)) => s.capacity(),
                    _ => 0,
                }
            });
            if bytes - replaced + incoming > 1024 * 1024 {
                return Err("Pending patch byte budget exceeded; reload required");
            }
            if let Some(previous) = self.patches.get_mut(&patch.id) {
                if !matches!(patch.content, Patch::Absent) {
                    previous.content = patch.content;
                }
                if !matches!(patch.edited, Patch::Absent) {
                    previous.edited = patch.edited;
                }
            } else {
                self.patches.insert(patch.id, patch);
            }
        }
        Ok(())
    }
    pub fn delete(&mut self, id: Id) -> Result<(), &'static str> {
        if self.deleted.len() >= MAX_MUTATIONS && !self.deleted.contains(&id) {
            return Err("Deletion reconciliation capacity exceeded; reload required");
        }
        self.deleted.insert(id);
        self.remember(id)?;
        self.patches.remove(&id);
        if let Some(old) = self.messages.remove(&id) {
            self.bytes -= old.bytes();
        }
        Ok(())
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}
fn apply_patch(message: &mut Message, patch: &MessagePatch) {
    if matches!(patch.edited,Patch::Value(new) if message.edited_at.is_some_and(|old|new<old)) {
        return;
    }
    match &patch.content {
        Patch::Value(s) => message.content.clone_from(s),
        Patch::Null => message.content.clear(),
        Patch::Absent => {}
    }
    match &patch.edited {
        Patch::Value(at) => {
            message.edited = true;
            message.edited_at = Some(*at);
        }
        Patch::Null => {
            message.edited = false;
            message.edited_at = None;
        }
        Patch::Absent => {}
    }
    message.revision += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message(id: u64) -> Message {
        Message {
            id: Id(id),
            channel: Id(1),
            author: model::User {
                avatar: None,
                discriminator: 0,
                id: Id(2),
                name: "Synthetic".into(),
            },
            content: "before".into(),
            edited: false,
            edited_at: None,
            revision: 0,
            nonce: None,
            reply_to: None,
            unsupported: false,
        }
    }
    #[test]
    fn mutations_win_over_late_history_and_memory_is_bounded() {
        let mut t = Timeline::default();
        t.begin_page();
        t.delete(Id(1)).unwrap();
        t.patch(MessagePatch {
            id: Id(2),
            channel: Id(1),
            content: Patch::Value("after".into()),
            edited: Patch::Value(1),
        })
        .unwrap();
        t.finish_page(vec![message(1), message(2)], false).unwrap();
        assert!(t.get(Id(1)).is_none());
        t.patch(MessagePatch {
            id: Id(1),
            channel: Id(1),
            content: Patch::Value("late edit".into()),
            edited: Patch::Absent,
        })
        .unwrap();
        t.insert(message(1), false, false).unwrap();
        assert!(t.get(Id(1)).is_none());
        assert_eq!(t.get(Id(2)).unwrap().content, "after");
        let mut old = message(2);
        old.edited = true;
        old.edited_at = Some(0);
        t.insert(old, true, false).unwrap();
        assert_eq!(t.get(Id(2)).unwrap().content, "after");
        for id in 3..10_000 {
            t.insert(message(id), true, false).unwrap();
        }
        assert_eq!(t.len(), MAX_MESSAGES);
        assert!(t.bytes() <= MAX_BYTES);
        t.clear();
        assert_eq!(t.bytes(), 0);
        t.begin_page();
        t.seed_cache(vec![message(1), message(2)]).unwrap();
        t.finish_page(vec![message(2)], false).unwrap();
        assert!(t.get(Id(1)).is_none());
    }
    #[test]
    fn earlier_window_stays_put_and_out_of_order_pending_edits_do_not_regress() {
        let mut timeline = Timeline::default();
        for id in 1001..=1500 {
            timeline.insert(message(id), false, false).unwrap();
        }
        timeline.begin_page();
        timeline
            .finish_page((951..=1000).map(message).collect(), true)
            .unwrap();
        assert_eq!(timeline.iter().next().unwrap().id, Id(951));
        assert_eq!(timeline.iter().next_back().unwrap().id, Id(1450));
        timeline.insert(message(1501), true, false).unwrap();
        assert_eq!(timeline.iter().next().unwrap().id, Id(951));
        assert!(timeline.get(Id(1501)).is_none()); // live arrivals do not evict the reading anchor
        assert_eq!(timeline.len(), MAX_MESSAGES);

        timeline.clear();
        timeline.begin_page();
        for (at, content) in [(20, "new edit"), (10, "old edit")] {
            timeline
                .patch(MessagePatch {
                    id: Id(1),
                    channel: Id(1),
                    content: Patch::Value(content.into()),
                    edited: Patch::Value(at),
                })
                .unwrap();
        }
        timeline.insert(message(1), true, false).unwrap();
        assert_eq!(timeline.get(Id(1)).unwrap().content, "new edit");
        timeline.finish_page(vec![message(1)], false).unwrap();
        assert_eq!(timeline.get(Id(1)).unwrap().edited_at, Some(20));

        timeline.begin_page();
        timeline.delete(Id(1)).unwrap();
        timeline.cancel_page();
        timeline.begin_page();
        timeline
            .finish_page(vec![message(1), message(2)], false)
            .unwrap();
        assert!(timeline.get(Id(1)).is_none());
        assert_eq!(timeline.get(Id(2)).unwrap().content, "before");
    }
}
