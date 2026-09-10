//! Unofficial normal-user read-state payloads. Non-channel read-state kinds are ignored.
use model::{Id, Patch};
use serde::{
    Deserialize, Deserializer,
    de::{SeqAccess, Visitor},
};
const MAX_ENTRIES: usize = 4000;

fn cursor<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Id>, D::Error> {
    match Option::<String>::deserialize(d)?.as_deref() {
        None | Some("0") => Ok(None),
        Some(value) => value.parse().map(Some).map_err(serde::de::Error::custom),
    }
}
fn entries<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    struct List<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for List<T> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("bounded read-state entries")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element()? {
                if values.len() >= MAX_ENTRIES {
                    return Err(serde::de::Error::custom("Read-state capacity exceeded"));
                }
                values.push(value);
            }
            Ok(values)
        }
    }
    d.deserialize_seq(List(std::marker::PhantomData))
}
#[derive(Deserialize)]
pub struct Entry {
    pub id: Id,
    #[serde(rename = "type", default)]
    pub kind: u8,
    #[serde(default, deserialize_with = "cursor")]
    pub last_message_id: Option<Id>,
}
pub struct Snapshot {
    pub entries: Vec<Entry>,
    pub version: Option<u64>,
    pub partial: bool,
}
impl<'de> Deserialize<'de> for Snapshot {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct SnapshotVisitor;
        impl<'de> Visitor<'de> for SnapshotVisitor {
            type Value = Snapshot;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a bounded legacy list or versioned read-state object")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Snapshot, A::Error> {
                Ok(Snapshot {
                    entries: entries(serde::de::value::SeqAccessDeserializer::new(seq))?,
                    version: None,
                    partial: false,
                })
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<Snapshot, M::Error> {
                #[derive(Deserialize)]
                struct Versioned {
                    #[serde(deserialize_with = "entries")]
                    entries: Vec<Entry>,
                    #[serde(default)]
                    version: Option<u64>,
                    #[serde(default)]
                    partial: bool,
                }
                let value =
                    Versioned::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(Snapshot {
                    entries: value.entries,
                    version: value.version,
                    partial: value.partial,
                })
            }
        }
        d.deserialize_any(SnapshotVisitor)
    }
}
#[derive(Deserialize)]
pub struct Ack {
    pub channel_id: Id,
    #[serde(deserialize_with = "cursor")]
    pub message_id: Option<Id>,
    #[serde(default)]
    pub manual: bool,
    #[serde(default)]
    pub version: Option<u64>,
}
#[derive(Deserialize)]
pub struct LatestChannel {
    pub id: Id,
    #[serde(default)]
    pub last_message_id: Patch<Id>,
}
#[derive(Deserialize)]
pub struct PassiveUpdate {
    #[serde(default, deserialize_with = "entries")]
    pub updated_channels: Vec<LatestChannel>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ready_accepts_legacy_read_state_without_versioned_capability() {
        let ready:crate::Ready=crate::decode(br#"{"user":{"id":"1","username":"Synthetic"},"session_id":"synthetic-session","resume_gateway_url":"wss://gateway.discord.gg/","read_state":[{"id":"2","last_message_id":"3"}]}"#).unwrap();
        let state = ready.read_state.unwrap();
        assert_eq!(state.entries.len(), 1);
        assert_eq!(state.entries[0].id, Id(2));
        assert_eq!(state.version, None);
        assert!(!state.partial);
    }
    #[test]
    fn bounded_snapshots_and_manual_zero_cursors() {
        let snapshot: Snapshot = crate::decode(br#"{"entries":[{"id":"1","last_message_id":"0"},{"id":"2","last_message_id":"3","type":2}],"version":4,"partial":true}"#).unwrap();
        assert_eq!(snapshot.entries[0].last_message_id, None);
        assert_eq!(snapshot.entries[1].last_message_id, Some(Id(3)));
        assert_eq!(snapshot.entries[1].kind, 2);
        assert_eq!(snapshot.version, Some(4));
        assert!(snapshot.partial);
        assert!(crate::decode::<Snapshot>(br#"{"version":4}"#).is_err());
        let ack: Ack =
            crate::decode(br#"{"channel_id":"1","message_id":"0","manual":true,"version":5}"#)
                .unwrap();
        assert!(ack.manual);
        assert_eq!(ack.message_id, None);
        assert!(crate::decode::<Ack>(br#"{"channel_id":"1","message_id":"invalid"}"#).is_err());
        let oversized =
            serde_json::json!({"entries":vec![serde_json::json!({"id":"1"});MAX_ENTRIES+1]});
        assert!(crate::decode::<Snapshot>(&serde_json::to_vec(&oversized).unwrap()).is_err());
        assert!(
            crate::decode::<Snapshot>(&serde_json::to_vec(&oversized["entries"]).unwrap()).is_err()
        );
        let update:PassiveUpdate=crate::decode(br#"{"updated_channels":[{"id":"1"},{"id":"2","last_message_id":null},{"id":"3","last_message_id":"4"}]}"#).unwrap();
        assert!(matches!(
            update.updated_channels[0].last_message_id,
            Patch::Absent
        ));
        assert!(matches!(
            update.updated_channels[1].last_message_id,
            Patch::Null
        ));
        assert!(matches!(
            update.updated_channels[2].last_message_id,
            Patch::Value(Id(4))
        ));
    }
}
