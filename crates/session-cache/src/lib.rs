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
    pub fn begin_page(&mut self, older: bool) {
        // Set eviction direction before live events can race the history response.
        self.retain_older = older;
        self.loading = true;
        self.changed.clear();
        self.patches.clear();
    }
    pub fn cancel_page(&mut self) {
        // Failure stops reconciliation, but preserves the window the user is reading.
        // Starting a recent-page request restores newest-first retention.
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
        if message.bytes() > MAX_BYTES
            || message.content.len() > 64 * 1024
            || !model::valid_mentions(&message.mentions)
            || !model::valid_embeds(&message.embeds)
            || !model::valid_attachments(&message.attachments)
            || message
                .reactions
                .as_ref()
                .is_some_and(|r| !model::valid_reactions(r))
        {
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
                    previous.content != message.content
                        || previous.mentions != message.mentions
                        || previous.reactions != message.reactions
                        || previous.edited != message.edited
                        || previous.unsupported != message.unsupported
                        || previous.extra_content != message.extra_content
                        || previous.embeds != message.embeds
                        || previous.attachments != message.attachments
                        || previous.embeds_suppressed != message.embeds_suppressed,
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
        if matches!(&patch.content, Patch::Value(s) if s.len() > 64 * 1024)
            || matches!(&patch.reactions, Patch::Value(r) if !model::valid_reactions(r))
            || matches!(&patch.mentions, Patch::Value(users) if !model::valid_mentions(users))
            || matches!(&patch.embeds, Patch::Value(embeds) if !model::valid_embeds(embeds))
            || matches!(&patch.attachments, Patch::Value(attachments) if !model::valid_attachments(attachments))
        {
            return Err("Message patch exceeds capacity");
        }
        self.remember(patch.id)?;
        if let Some(mut message) = self.messages.remove(&patch.id) {
            self.bytes -= message.bytes();
            // Once hydrated, changed protects this record from the in-flight history page.
            self.patches.remove(&patch.id);
            apply_patch(&mut message, &patch);
            self.insert(message, true, false)?;
        } else if self.loading {
            if self.patches.get(&patch.id).is_some_and(|previous| {
                matches!((&previous.edited, &patch.edited), (Patch::Value(old), Patch::Value(new)) if new < old)
            }) {
                return Ok(());
            }
            let bytes: usize = self.patches.values().map(patch_bytes).sum();
            let mut merged = self
                .patches
                .get(&patch.id)
                .cloned()
                .unwrap_or_else(|| patch.clone());
            if !matches!(patch.content, Patch::Absent) {
                merged.content = patch.content;
            }
            if !matches!(patch.reactions, Patch::Absent) {
                merged.reactions = patch.reactions;
            }
            if !matches!(patch.mentions, Patch::Absent) {
                merged.mentions = patch.mentions;
            }
            if !matches!(patch.edited, Patch::Absent) {
                merged.edited = patch.edited;
            }
            if !matches!(patch.embeds, Patch::Absent) {
                merged.embeds = patch.embeds;
            }
            if !matches!(patch.attachments, Patch::Absent) {
                merged.attachments = patch.attachments;
            }
            if !matches!(patch.embeds_suppressed, Patch::Absent) {
                merged.embeds_suppressed = patch.embeds_suppressed;
            }
            merged.extra_content.merge(&patch.extra_content);
            let replaced = self.patches.get(&patch.id).map_or(0, patch_bytes);
            if bytes - replaced + patch_bytes(&merged) > 1024 * 1024 {
                return Err("Pending patch byte budget exceeded; reload required");
            }
            self.patches.insert(merged.id, merged);
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
    pub fn set_reactions(
        &mut self,
        id: Id,
        reactions: Option<Vec<model::Reaction>>,
    ) -> Result<(), &'static str> {
        if reactions
            .as_ref()
            .is_some_and(|r| !model::valid_reactions(r))
        {
            return Err("Reaction data exceeds safe capacity");
        }
        let Some(message) = self.messages.get_mut(&id) else {
            return Ok(());
        };
        let old = message.bytes();
        let new = reactions.as_ref().map_or(0, |r| {
            model::reaction_bytes(r)
                + r.capacity().saturating_sub(r.len()) * size_of::<model::Reaction>()
        });
        let previous = message.reactions.as_ref().map_or(0, |r| {
            model::reaction_bytes(r)
                + r.capacity().saturating_sub(r.len()) * size_of::<model::Reaction>()
        });
        if self.bytes - previous + new > MAX_BYTES {
            return Err("Reaction data exceeds timeline capacity");
        }
        message.reactions = reactions;
        message.revision += 1;
        self.bytes = self.bytes - old + message.bytes();
        Ok(())
    }
}
fn patch_bytes(patch: &MessagePatch) -> usize {
    let content = match &patch.content {
        Patch::Value(value) => value.capacity(),
        _ => 0,
    };
    size_of::<MessagePatch>()
        + content
        + match &patch.reactions {
            Patch::Value(r) => model::reaction_bytes(r),
            _ => 0,
        }
        + match &patch.mentions {
            Patch::Value(users) => model::mention_bytes(users),
            _ => 0,
        }
        + match &patch.embeds {
            Patch::Value(value) => model::embed_bytes(value),
            _ => 0,
        }
        + match &patch.attachments {
            Patch::Value(value) => model::attachment_bytes(value),
            _ => 0,
        }
}
fn apply_patch(message: &mut Message, patch: &MessagePatch) {
    if matches!(patch.edited,Patch::Value(new) if message.edited_at.is_some_and(|old|new<old)) {
        return;
    }
    match &patch.mentions {
        Patch::Value(users) => message.mentions.clone_from(users),
        Patch::Null => message.mentions.clear(),
        Patch::Absent => {}
    }
    match &patch.reactions {
        Patch::Absent => {}
        Patch::Null => message.reactions = Some(vec![]),
        Patch::Value(r) => message.reactions = Some(r.clone()),
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
    match &patch.embeds {
        Patch::Value(embeds) => message.embeds.clone_from(embeds),
        Patch::Null => message.embeds.clear(),
        Patch::Absent => {}
    }
    match &patch.attachments {
        Patch::Value(attachments) => message.attachments.clone_from(attachments),
        Patch::Null => message.attachments.clear(),
        Patch::Absent => {}
    }
    match patch.embeds_suppressed {
        Patch::Value(suppressed) => message.embeds_suppressed = suppressed,
        Patch::Null => message.embeds_suppressed = false,
        Patch::Absent => {}
    }
    patch.extra_content.apply(&mut message.extra_content);
    message.revision += 1;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn content_markers_reconcile_independent_updates_before_and_after_history() {
        let update = |extra_content| MessagePatch {
            id: Id(1),
            channel: Id(1),
            extra_content,
            reactions: Patch::Absent,
            content: Patch::Absent,
            mentions: Patch::Absent,
            edited: Patch::Absent,
            embeds: Patch::Absent,
            embeds_suppressed: Patch::Absent,
            attachments: Patch::Absent,
        };
        let mut original = message(1);
        original.extra_content.sticker_items = true;
        original.extra_content.poll = true;
        let mut timeline = Timeline::default();
        timeline.begin_page(false);
        timeline
            .patch(update(model::ExtraContentPatch {
                components: Patch::Value(true),
                ..Default::default()
            }))
            .unwrap();
        timeline
            .patch(update(model::ExtraContentPatch {
                poll: Patch::Null,
                components_v2: Patch::Value(true),
                ..Default::default()
            }))
            .unwrap();
        timeline
            .patch(update(model::ExtraContentPatch {
                components: Patch::Value(false),
                ..Default::default()
            }))
            .unwrap();
        timeline.finish_page(vec![original.clone()], false).unwrap();
        let current = timeline.get(Id(1)).unwrap();
        assert!(!current.extra_content.poll && !current.extra_content.components);
        assert!(current.extra_content.sticker_items && current.extra_content.components_v2);

        timeline.begin_page(false);
        timeline
            .patch(update(model::ExtraContentPatch {
                sticker_items: Patch::Null,
                stickers: Patch::Value(true),
                ..Default::default()
            }))
            .unwrap();
        timeline
            .patch(update(model::ExtraContentPatch {
                components_v2: Patch::Null,
                ..Default::default()
            }))
            .unwrap();
        timeline.finish_page(vec![original], false).unwrap();
        let current = timeline.get(Id(1)).unwrap();
        assert_eq!(
            current.extra_content,
            model::ExtraContent {
                stickers: true,
                ..Default::default()
            }
        );
        let mut cleared = update(model::ExtraContentPatch {
            stickers: Patch::Null,
            ..Default::default()
        });
        cleared.edited = Patch::Value(10);
        timeline.patch(cleared).unwrap();
        let mut stale = update(model::ExtraContentPatch {
            poll: Patch::Value(true),
            ..Default::default()
        });
        stale.edited = Patch::Value(9);
        timeline.patch(stale).unwrap();
        assert!(!timeline.get(Id(1)).unwrap().extra_content.any());

        let mut replacement = timeline.get(Id(1)).unwrap().clone();
        let before = replacement.revision;
        replacement.extra_content.components = true;
        timeline.insert(replacement, true, false).unwrap();
        assert_eq!(timeline.get(Id(1)).unwrap().revision, before + 1);
        let mut replacement = timeline.get(Id(1)).unwrap().clone();
        replacement.unsupported = true;
        timeline.insert(replacement.clone(), true, false).unwrap();
        assert_eq!(timeline.get(Id(1)).unwrap().revision, before + 2);
        timeline.begin_page(false);
        timeline.delete(Id(1)).unwrap();
        timeline
            .patch(update(model::ExtraContentPatch {
                poll: Patch::Value(true),
                ..Default::default()
            }))
            .unwrap();
        timeline.finish_page(vec![replacement], false).unwrap();
        assert!(timeline.is_empty());
        assert_eq!(timeline.bytes(), 0);
    }

    #[test]
    fn mention_patches_survive_stale_pages_and_release_replaced_users() {
        let mut timeline = Timeline::default();
        let mut original = message(1);
        original.mentions = vec![original.author.clone()];
        timeline.insert(original.clone(), false, false).unwrap();
        let before = timeline.bytes;
        let patch = MessagePatch {
            extra_content: Default::default(),
            reactions: model::Patch::Absent,
            id: Id(1),
            channel: original.channel,
            content: Patch::Absent,
            mentions: Patch::Value(vec![]),
            edited: Patch::Absent,
            embeds: Patch::Absent,
            embeds_suppressed: Patch::Absent,
            attachments: Patch::Absent,
        };
        timeline.patch(patch.clone()).unwrap();
        assert!(timeline.get(Id(1)).unwrap().mentions.is_empty());
        assert!(timeline.bytes < before);
        timeline.clear();
        timeline.begin_page(false);
        timeline.patch(patch).unwrap();
        timeline.finish_page(vec![original], false).unwrap();
        assert!(timeline.get(Id(1)).unwrap().mentions.is_empty());
    }
    fn message(id: u64) -> Message {
        Message {
            reactions: Some(vec![]),
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
            extra_content: Default::default(),
            attachments: Vec::new(),
            embeds: Vec::new(),
            mentions: Vec::new(),
            embeds_suppressed: false,
        }
    }
    #[test]
    fn pending_and_cancelled_older_pages_preserve_the_reading_window() {
        let mut timeline = Timeline::default();
        for id in 1..=MAX_MESSAGES as u64 {
            timeline.insert(message(id), false, false).unwrap();
        }
        timeline.begin_page(true);
        timeline.insert(message(501), true, false).unwrap();
        assert!(timeline.get(Id(1)).is_some());
        assert!(timeline.get(Id(501)).is_none());
        timeline.cancel_page(); // Failed/queue-rejected page does not change reading position.
        timeline.insert(message(502), true, false).unwrap();
        assert!(timeline.get(Id(1)).is_some());
        assert!(timeline.get(Id(502)).is_none());
        timeline.begin_page(true);
        timeline.finish_page(vec![message(0)], true).unwrap();
        timeline.begin_page(true); // A further older-page failure preserves that older window.
        timeline.cancel_page();
        timeline.insert(message(503), true, false).unwrap();
        assert!(timeline.get(Id(0)).is_some());
        assert!(timeline.get(Id(503)).is_none());
        timeline.begin_page(false); // Jump/reload latest resets retention immediately.
        timeline.insert(message(504), true, false).unwrap();
        assert!(timeline.get(Id(0)).is_none());
        assert!(timeline.get(Id(504)).is_some());
        timeline.cancel_page();
        timeline.insert(message(505), true, false).unwrap();
        assert!(timeline.get(Id(505)).is_some());
        assert_eq!(timeline.len(), MAX_MESSAGES);
        assert!(timeline.bytes() <= MAX_BYTES);
    }

    #[test]
    fn attachment_only_mutations_clear_without_late_history_resurrection() {
        let attachment = |id| model::Attachment {
            id: Id(id),
            filename: "synthetic.png".into(),
            description: None,
            content_type: Some("image/png".into()),
            size: 1024,
            media: model::EmbedMedia {
                url: Some(format!(
                    "https://cdn.discordapp.com/attachments/1/{id}/synthetic.png"
                )),
                width: 640,
                height: 480,
                ..Default::default()
            },
            spoiler: false,
        };
        let update = |attachments| MessagePatch {
            extra_content: Default::default(),
            reactions: model::Patch::Absent,
            id: Id(1),
            channel: Id(1),
            content: Patch::Absent,
            edited: Patch::Absent,
            embeds: Patch::Absent,
            mentions: Patch::Absent,
            embeds_suppressed: Patch::Absent,
            attachments,
        };
        let mut timeline = Timeline::default();
        timeline.begin_page(false);
        timeline
            .patch(update(Patch::Value(vec![attachment(10)])))
            .unwrap();
        timeline.patch(update(Patch::Absent)).unwrap();
        timeline.insert(message(1), true, false).unwrap();
        assert_eq!(timeline.get(Id(1)).unwrap().attachments[0].id, Id(10));
        let revision = timeline.get(Id(1)).unwrap().revision;
        timeline
            .patch(update(Patch::Value(vec![attachment(11)])))
            .unwrap();
        timeline.finish_page(vec![message(1)], false).unwrap();
        assert_eq!(timeline.get(Id(1)).unwrap().attachments[0].id, Id(11));
        assert!(timeline.get(Id(1)).unwrap().revision > revision);
        for clear in [Patch::Null, Patch::Value(Vec::new())] {
            timeline.begin_page(false);
            let mut old = message(1);
            old.attachments = vec![attachment(10)];
            timeline.patch(update(clear)).unwrap();
            timeline.finish_page(vec![old], false).unwrap();
            assert!(timeline.get(Id(1)).unwrap().attachments.is_empty());
            assert_eq!(
                timeline.bytes(),
                timeline.iter().map(Message::bytes).sum::<usize>()
            );
        }
        timeline.begin_page(false);
        timeline.delete(Id(1)).unwrap();
        timeline
            .patch(update(Patch::Value(vec![attachment(12)])))
            .unwrap();
        timeline.finish_page(vec![message(1)], false).unwrap();
        assert!(timeline.is_empty());
        timeline.clear();
        timeline.begin_page(false);
        let mut large = attachment(10);
        large.media.url = Some(format!("https://cdn.discordapp.com/{}", "x".repeat(1900)));
        large.media.proxy_url = large.media.url.clone();
        let mut rejected = false;
        for id in 1..100 {
            let mut patch = update(Patch::Value(vec![large.clone(); model::MAX_ATTACHMENTS]));
            patch.id = Id(id);
            if timeline.patch(patch).is_err() {
                rejected = true;
                break;
            }
        }
        assert!(
            rejected,
            "Pending attachments share the one MiB mutation budget"
        );
        assert!(
            !timeline.patches.is_empty(),
            "The budget test uses valid attachments"
        );
    }
    #[test]
    fn embed_only_mutations_merge_clear_and_survive_late_history() {
        let embed = |title: &str| model::Embed {
            title: Some(title.into()),
            ..Default::default()
        };
        let update = |embeds| MessagePatch {
            extra_content: Default::default(),
            reactions: model::Patch::Absent,
            id: Id(1),
            channel: Id(1),
            content: Patch::Absent,
            edited: Patch::Absent,
            embeds,
            mentions: Patch::Absent,
            embeds_suppressed: Patch::Absent,
            attachments: Patch::Absent,
        };
        let mut timeline = Timeline::default();
        timeline.begin_page(false);
        timeline
            .patch(update(Patch::Value(vec![embed("first")])))
            .unwrap();
        let mut suppression = update(Patch::Absent);
        suppression.embeds_suppressed = Patch::Value(true);
        timeline.patch(suppression).unwrap();
        timeline.insert(message(1), true, false).unwrap();
        assert_eq!(
            timeline.get(Id(1)).unwrap().embeds[0].title.as_deref(),
            Some("first")
        );
        assert!(timeline.get(Id(1)).unwrap().embeds_suppressed);
        let revision = timeline.get(Id(1)).unwrap().revision;
        timeline
            .patch(update(Patch::Value(vec![embed("newer")])))
            .unwrap();
        timeline.finish_page(vec![message(1)], false).unwrap();
        assert_eq!(
            timeline.get(Id(1)).unwrap().embeds[0].title.as_deref(),
            Some("newer")
        );
        assert!(timeline.get(Id(1)).unwrap().revision > revision);
        for clear in [Patch::Null, Patch::Value(Vec::new())] {
            timeline.begin_page(false);
            let mut old = message(1);
            old.embeds = vec![embed("stale")];
            timeline.patch(update(clear)).unwrap();
            timeline.finish_page(vec![old], false).unwrap();
            assert!(timeline.get(Id(1)).unwrap().embeds.is_empty());
            assert_eq!(
                timeline.bytes(),
                timeline.iter().map(Message::bytes).sum::<usize>()
            );
        }
        timeline.clear();
        timeline.begin_page(false);
        let large = model::Embed {
            description: Some("x".repeat(16_384)),
            ..Default::default()
        };
        let mut rejected = false;
        for id in 1..100 {
            let mut patch = update(Patch::Value(vec![large.clone()]));
            patch.id = Id(id);
            if timeline.patch(patch).is_err() {
                rejected = true;
                break;
            }
        }
        assert!(
            rejected,
            "Pending embeds must share the one MiB patch budget"
        );
    }
    #[test]
    fn mutations_win_over_late_history_and_memory_is_bounded() {
        let mut t = Timeline::default();
        t.begin_page(false);
        t.delete(Id(1)).unwrap();
        t.patch(MessagePatch {
            extra_content: Default::default(),
            reactions: model::Patch::Absent,
            id: Id(2),
            channel: Id(1),
            content: Patch::Value("after".into()),
            edited: Patch::Value(1),
            embeds: Patch::Absent,
            mentions: Patch::Absent,
            embeds_suppressed: Patch::Absent,
            attachments: Patch::Absent,
        })
        .unwrap();
        t.finish_page(vec![message(1), message(2)], false).unwrap();
        assert!(t.get(Id(1)).is_none());
        t.patch(MessagePatch {
            extra_content: Default::default(),
            reactions: model::Patch::Absent,
            id: Id(1),
            channel: Id(1),
            content: Patch::Value("late edit".into()),
            edited: Patch::Absent,
            embeds: Patch::Absent,
            mentions: Patch::Absent,
            embeds_suppressed: Patch::Absent,
            attachments: Patch::Absent,
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
        t.begin_page(false);
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
        timeline.begin_page(true);
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
        timeline.begin_page(false);
        for (at, content) in [(20, "new edit"), (10, "old edit")] {
            timeline
                .patch(MessagePatch {
                    extra_content: Default::default(),
                    reactions: model::Patch::Absent,
                    id: Id(1),
                    channel: Id(1),
                    content: Patch::Value(content.into()),
                    edited: Patch::Value(at),
                    embeds: Patch::Absent,
                    mentions: Patch::Absent,
                    embeds_suppressed: Patch::Absent,
                    attachments: Patch::Absent,
                })
                .unwrap();
        }
        timeline.insert(message(1), true, false).unwrap();
        assert_eq!(timeline.get(Id(1)).unwrap().content, "new edit");
        timeline.finish_page(vec![message(1)], false).unwrap();
        assert_eq!(timeline.get(Id(1)).unwrap().edited_at, Some(20));

        timeline.begin_page(false);
        timeline.delete(Id(1)).unwrap();
        timeline.cancel_page();
        timeline.begin_page(false);
        timeline
            .finish_page(vec![message(1), message(2)], false)
            .unwrap();
        assert!(timeline.get(Id(1)).is_none());
        assert_eq!(timeline.get(Id(2)).unwrap().content, "before");
    }
}
