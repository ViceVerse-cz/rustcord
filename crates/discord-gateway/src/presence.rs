use crate::{Event, Instant};
use client_core::{MAX_MEMBER_PRESENCE_BYTES, presence::Update};
use discord_protocol::presence::PresenceUpdate;
use model::Id;
use std::{collections::BTreeMap, time::Duration};

#[derive(Default)]
pub(crate) struct Pending {
    updates: BTreeMap<Id, Update>,
    pub bootstrap_users: std::collections::BTreeSet<Id>,
    pub deadline: Option<Instant>,
}
impl Pending {
    pub fn push(&mut self, update: PresenceUpdate, now: Instant) {
        // Guild-scoped presence is not authoritative for a user's global/DM presence.
        if update.guild.is_some() {
            return;
        }
        let update = Update {
            user: update.user,
            status: update.status,
            custom_status: update.custom_status,
            activities: update.activities,
        };
        let mut merged = self
            .updates
            .get(&update.user)
            .cloned()
            .unwrap_or_else(|| update.clone());
        merged.merge(update);
        if self.updates.len() >= 100 && !self.updates.contains_key(&merged.user) {
            return;
        }
        let projected = self
            .updates
            .iter()
            .filter(|(id, _)| **id != merged.user)
            .map(|(_, value)| value.heap_bytes())
            .sum::<usize>()
            + merged.heap_bytes()
            + 100 * size_of::<Update>()
            + size_of::<Event>();
        if projected > MAX_MEMBER_PRESENCE_BYTES {
            return;
        }
        self.updates.insert(merged.user, merged);
        self.deadline
            .get_or_insert(now + Duration::from_millis(100));
    }
    pub fn take(&mut self) -> Option<Event> {
        self.deadline = None;
        let updates = std::mem::take(&mut self.updates);
        (!updates.is_empty()).then(|| Event::DirectPresence(updates.into_values().collect()))
    }
    pub fn supplemental(
        &mut self,
        bytes: &[u8],
        now: Instant,
        emit: &impl Fn(Event) -> Result<(), client_core::auth::Failure>,
    ) -> Result<(), client_core::auth::Failure> {
        // Identify sends no DEDUPE_USER_OBJECTS capability, so READY uses presences.
        // Also accept merged_presences.friends from the unofficial supplemental format.
        // Decode only bounded wire data; no assets/secrets survive the presence decoder.
        let Ok(mut data) = discord_protocol::decode::<serde_json::Value>(bytes) else {
            return Ok(());
        };
        let path = if data.pointer("/merged_presences/friends").is_some() {
            "/merged_presences/friends"
        } else {
            "/presences"
        };
        let Some(friends) = data
            .pointer_mut(path)
            .and_then(serde_json::Value::as_array_mut)
        else {
            return Ok(());
        };
        for friend in friends {
            let Some(object) = friend.as_object_mut() else {
                continue;
            };
            if !object.contains_key("user")
                && let Some(user_id) = object.remove("user_id")
            {
                object.insert("user".into(), serde_json::json!({"id": user_id}));
            }
            let Some(id) = object.get("user").and_then(|u| u.get("id")) else {
                continue;
            };
            let Ok(id) = serde_json::from_value::<Id>(id.clone()) else {
                continue;
            };
            if !self.bootstrap_users.contains(&id) {
                continue;
            }
            if let Ok(bytes) = serde_json::to_vec(friend)
                && let Ok(update) = discord_protocol::presence::decode(&bytes)
            {
                if (self.updates.len() >= 100
                    || self.updates.values().map(Update::heap_bytes).sum::<usize>() > 96 * 1024)
                    && let Some(event) = self.take()
                {
                    emit(event)?;
                }
                self.push(update, now);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use model::Patch;

    #[test]
    fn legacy_ready_restores_an_already_running_game() {
        let mut pending = Pending::default();
        pending.bootstrap_users.insert(Id(3));
        pending.supplemental(br#"{"presences":[{"user":{"id":"3"},"status":"online","activities":[{"type":0,"name":"Genshin Impact"}]},{"user":{"id":"99"},"status":"online","activities":[{"type":0,"name":"Unknown recipient"}]}]}"#, Instant::now(), &|_| Ok(())).unwrap();
        let Event::DirectPresence(updates) = pending
            .take()
            .expect("READY must restore existing activities")
        else {
            panic!()
        };
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].user, Id(3));
        let Patch::Value(activities) = &updates[0].activities else {
            panic!()
        };
        assert_eq!(activities[0].summary(), "Playing Genshin Impact");
    }

    #[test]
    fn direct_patches_coalesce_without_losing_absent_fields_and_stay_bounded() {
        let now = Instant::now();
        let mut pending = Pending::default();
        pending.bootstrap_users.insert(Id(3));
        pending.supplemental(br#"{"merged_presences":{"friends":[{"user_id":"3","status":"online","activities":[{"type":0,"name":"Synthetic game"}]}]}}"#, now, &|_| Ok(())).unwrap();
        pending.push(
            discord_protocol::presence::decode(br#"{"user":{"id":"3"},"status":"idle"}"#).unwrap(),
            now + Duration::from_millis(50),
        );
        assert_eq!(pending.deadline, Some(now + Duration::from_millis(100)));
        let event = pending.take().unwrap();
        assert!(event.bytes() <= MAX_MEMBER_PRESENCE_BYTES);
        let Event::DirectPresence(updates) = event else {
            panic!()
        };
        assert_eq!(updates.len(), 1);
        assert_eq!(updates[0].status, Patch::Value("idle".into()));
        assert!(
            matches!(&updates[0].activities, Patch::Value(a) if a.len()==1 && a[0].name=="Synthetic game")
        );
        assert!(pending.take().is_none());
        for id in 1..1000 {
            let bytes = format!(
                r#"{{"user":{{"id":"{id}"}},"activities":[{{"type":0,"name":"Synthetic"}}]}}"#
            );
            pending.push(
                discord_protocol::presence::decode(bytes.as_bytes()).unwrap(),
                now,
            );
        }
        assert_eq!(pending.updates.len(), 100);
        assert!(pending.take().unwrap().bytes() <= MAX_MEMBER_PRESENCE_BYTES);
        for wire in [
            br#"{"user":{"id":"3"},"status":"online","activities":[{"type":0,"name":"Old activity"}]}"#.as_slice(),
            br#"{"user":{"id":"3"},"status":"offline"}"#,
            br#"{"user":{"id":"3"},"status":"online"}"#,
        ] { pending.push(discord_protocol::presence::decode(wire).unwrap(), now); }
        let Event::DirectPresence(updates) = pending.take().unwrap() else {
            panic!()
        };
        assert_eq!(updates[0].activities, Patch::Null);
        assert_eq!(updates[0].custom_status, Patch::Null);
    }
    #[test]
    fn supplemental_filters_unknown_users_and_streams_more_than_one_batch() {
        let now = Instant::now();
        let mut pending = Pending::default();
        pending.bootstrap_users.extend((100..=255).map(Id));
        let friends: Vec<_> = (1..=300).map(|id| serde_json::json!({"user_id":id.to_string(),"status":"online","activities":[{"type":0,"name":"Synthetic"}]})).collect();
        let bytes =
            serde_json::to_vec(&serde_json::json!({"merged_presences":{"friends":friends}}))
                .unwrap();
        let received = std::cell::RefCell::new(Vec::new());
        pending
            .supplemental(&bytes, now, &|event| {
                assert!(event.bytes() <= MAX_MEMBER_PRESENCE_BYTES);
                let Event::DirectPresence(updates) = event else {
                    panic!()
                };
                received
                    .borrow_mut()
                    .extend(updates.into_iter().map(|u| u.user));
                Ok(())
            })
            .unwrap();
        let Event::DirectPresence(updates) = pending.take().unwrap() else {
            panic!()
        };
        received
            .borrow_mut()
            .extend(updates.into_iter().map(|u| u.user));
        assert_eq!(*received.borrow(), (100..=255).map(Id).collect::<Vec<_>>());
    }
}
