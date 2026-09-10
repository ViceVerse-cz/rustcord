//! Only identity, scope and bounded status text survive presence decoding.
use crate::DecodeError;
use model::{Id, Patch};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, SeqAccess, Visitor},
};
use std::fmt;

pub struct PresenceUpdate {
    pub guild: Option<Id>,
    pub user: Id,
    pub status: Patch<String>,
    pub custom_status: Patch<String>,
}

/// Activity payloads are consumed one at a time; only normalized custom text survives.
#[derive(Default)]
pub struct Activities(pub(crate) Option<String>);

// Derived structs also accept positional arrays; wire activities and emoji must be objects.
struct Object<T>(T);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct MapOnly<T>(std::marker::PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for MapOnly<T> {
            type Value = Object<T>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an activity or emoji object")
            }
            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                T::deserialize(serde::de::value::MapAccessDeserializer::new(map)).map(Object)
            }
        }
        d.deserialize_map(MapOnly(std::marker::PhantomData))
    }
}

struct Text<const N: usize>(String);
impl<'de, const N: usize> Deserialize<'de> for Text<N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Bounded<const N: usize>;
        impl<const N: usize> Visitor<'_> for Bounded<N> {
            type Value = Text<N>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded activity text")
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                if value.len() > N {
                    return Err(E::custom("Activity text exceeds capacity"));
                }
                Ok(Text(value.to_owned()))
            }
        }
        d.deserialize_str(Bounded::<N>)
    }
}

#[derive(Deserialize)]
struct Activity {
    #[serde(rename = "type")]
    kind: u8,
    #[serde(default)]
    state: Option<Text<4096>>,
    #[serde(default)]
    emoji: Option<Object<Emoji>>,
}
#[derive(Deserialize)]
struct Emoji {
    #[serde(default)]
    name: Option<Text<128>>,
    #[serde(default)]
    id: Option<Id>,
}
impl Activity {
    fn custom_status(&self) -> Option<String> {
        let emoji = self
            .emoji
            .as_ref()
            .map(|emoji| &emoji.0)
            .filter(|e| e.id.is_none())
            .and_then(|e| e.name.as_ref())
            .map(|name| name.0.as_str())
            .filter(|name| name.chars().count() <= 8);
        let state = self.state.as_ref().map(|s| s.0.trim()).unwrap_or_default();
        let text: String = emoji
            .into_iter()
            .flat_map(str::chars)
            .chain((emoji.is_some() && !state.is_empty()).then_some(' '))
            .chain(state.chars())
            .filter(|c| !c.is_control())
            .take(128)
            .collect();
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }
}
impl<'de> Deserialize<'de> for Activities {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Bounded;
        impl<'de> Visitor<'de> for Bounded {
            type Value = Activities;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("at most 16 activity objects")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Activities, A::Error> {
                let mut custom = None;
                let mut found = false;
                let mut count = 0;
                while let Some(Object(activity)) = seq.next_element::<Object<Activity>>()? {
                    if count == 16 {
                        return Err(serde::de::Error::custom("Activity count exceeds capacity"));
                    }
                    count += 1;
                    if activity.kind == 4 && !found {
                        custom = activity.custom_status();
                        found = true;
                    }
                }
                Ok(Activities(custom))
            }
        }
        d.deserialize_seq(Bounded)
    }
}

#[derive(Deserialize)]
struct Identity {
    id: Id,
}

#[derive(Deserialize)]
struct PresenceDto {
    #[serde(default)]
    guild_id: Option<Id>,
    user: Identity,
    #[serde(default)]
    status: Patch<String>,
    #[serde(default)]
    activities: Patch<Activities>,
}

pub fn decode(bytes: &[u8]) -> Result<PresenceUpdate, DecodeError> {
    let presence: PresenceDto = crate::decode(bytes)?;
    let status = match presence.status {
        Patch::Value(status)
            if matches!(status.as_str(), "online" | "idle" | "dnd" | "offline") =>
        {
            Patch::Value(status)
        }
        Patch::Absent => Patch::Absent,
        _ => Patch::Null,
    };
    Ok(PresenceUpdate {
        guild: presence.guild_id,
        user: presence.user.id,
        status,
        custom_status: match presence.activities {
            Patch::Absent => Patch::Absent,
            Patch::Null => Patch::Null,
            Patch::Value(Activities(custom)) => custom.map_or(Patch::Null, Patch::Value),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(activities: &str) -> Result<PresenceUpdate, DecodeError> {
        decode(format!(r#"{{"user":{{"id":"2"}},"activities":{activities}}}"#).as_bytes())
    }

    #[test]
    fn custom_status_preserves_absence_and_clears_explicit_missing_activity() {
        for wire in [
            r#"{"user":{"id":"2"}}"#,
            r#"{"user":{"id":"2"},"status":"offline"}"#,
        ] {
            assert_eq!(
                decode(wire.as_bytes()).unwrap().custom_status,
                Patch::Absent
            );
        }
        for activities in [
            "null",
            "[]",
            r#"[{"type":0,"state":"Not a custom status"}]"#,
            r#"[{"type":4}]"#,
            r#"[{"type":4,"state":null,"emoji":null}]"#,
            r#"[{"type":4,"state":" \n\t "}]"#,
            r#"[{"type":4,"emoji":{"name":"custom","id":"123"}}]"#,
        ] {
            let presence = update(activities).unwrap();
            assert_eq!(presence.custom_status, Patch::Null, "{activities}");
            assert_eq!(presence.status, Patch::Absent);
        }
    }

    #[test]
    fn snapshots_and_updates_share_bounded_unicode_custom_status_normalization() {
        for (activities, expected) in [
            (
                r#"[{"type":0,"state":"Ignored"},{"type":4,"state":" Rest\ning \t","emoji":{"name":"\ud83c\udf19","id":null}}]"#,
                Some("🌙 Resting"),
            ),
            (
                r#"[{"type":4,"emoji":{"name":"\ud83c\udf19"}}]"#,
                Some("🌙"),
            ),
            (
                r#"[{"type":4,"state":"Text","emoji":{"name":"custom","id":"123"}}]"#,
                Some("Text"),
            ),
            (
                r#"[{"type":4,"state":"Text","emoji":{"name":"123456789"}}]"#,
                Some("Text"),
            ),
            (
                r#"[{"type":4,"state":"First"},{"type":4,"state":"Second"}]"#,
                Some("First"),
            ),
            (r#"[{"type":4},{"type":4,"state":"Second"}]"#, None),
        ] {
            let snapshot: crate::PresenceDto = crate::decode(
                format!(r#"{{"status":"online","activities":{activities}}}"#).as_bytes(),
            )
            .unwrap();
            assert_eq!(snapshot.custom_status().as_deref(), expected);
            assert_eq!(
                update(activities).unwrap().custom_status,
                expected.map_or(Patch::Null, |s| Patch::Value(s.into()))
            );
        }
        let activities = format!(r#"[{{"type":4,"state":"{}"}}]"#, "🌙".repeat(1024));
        let Patch::Value(text) = update(&activities).unwrap().custom_status else {
            panic!()
        };
        assert_eq!(text.chars().count(), 128);
        assert_eq!(text.len(), 512);
        let snapshot: crate::PresenceDto =
            crate::decode(format!(r#"{{"status":"online","activities":{activities}}}"#).as_bytes())
                .unwrap();
        assert_eq!(snapshot.custom_status().as_deref(), Some(text.as_str()));
        // Rich activity metadata is ignored instead of surviving in application state.
        let rich = format!(
            r#"[{{"type":0,"secrets":{{"join":"{}"}},"assets":{{"large_image":"unused"}}}},{{"type":4,"state":"Visible"}}]"#,
            "synthetic".repeat(4096)
        );
        assert_eq!(
            update(&rich).unwrap().custom_status,
            Patch::Value("Visible".into())
        );
    }

    #[test]
    fn filtering_and_truncation_leave_only_valid_trimmed_custom_statuses() {
        let truncated = format!(r#"[{{"type":4,"state":"{} trailing"}}]"#, "x".repeat(127));
        for (activities, expected) in [
            (r#"[{"type":4,"emoji":{"name":" \t "}}]"#.to_owned(), None),
            (
                r#"[{"type":4,"emoji":{"name":" \t "},"state":"Visible"}]"#.to_owned(),
                Some("Visible".to_owned()),
            ),
            (
                r#"[{"type":4,"state":"\u0000  Visible  \u0000"}]"#.to_owned(),
                Some("Visible".to_owned()),
            ),
            (truncated, Some("x".repeat(127))),
        ] {
            let decoded = update(&activities).unwrap();
            assert_eq!(
                decoded.custom_status,
                expected.clone().map_or(Patch::Null, Patch::Value)
            );
            let snapshot: crate::PresenceDto = crate::decode(
                format!(r#"{{"status":"online","activities":{activities}}}"#).as_bytes(),
            )
            .unwrap();
            assert_eq!(snapshot.custom_status(), expected);
            assert!(
                model::MemberPresence {
                    user: Id(2),
                    status: None,
                    custom_status: snapshot.custom_status(),
                }
                .valid()
            );
        }
    }

    #[test]
    fn malformed_activity_shapes_and_resource_overflows_are_rejected() {
        for activities in [
            "42",
            "true",
            r#""text""#,
            "{}",
            "[null]",
            "[[]]",
            r#"[[4,"Array status",null]]"#,
            "[42]",
            "[{}]",
            r#"[{"type":"4"}]"#,
            r#"[{"type":256}]"#,
            r#"[{"type":4,"state":42}]"#,
            r#"[{"type":4,"emoji":[]}]"#,
            r#"[{"type":4,"emoji":["Array emoji",null]}]"#,
            r#"[{"type":4,"emoji":{"name":42}}]"#,
            r#"[{"type":4,"emoji":{"id":"0"}}]"#,
            r#"[{"type":4,"emoji":{"id":123}}]"#,
            r#"[{"type":4,"state":"First","state":"Second"}]"#,
        ] {
            assert!(update(activities).is_err(), "{activities}");
            assert!(
                crate::decode::<crate::PresenceDto>(
                    format!(r#"{{"status":"online","activities":{activities}}}"#).as_bytes(),
                )
                .is_err(),
                "{activities}"
            );
        }
        assert!(decode(br#"{"user":{"id":"2"},"activities":[],"activities":null}"#).is_err());
        let sixteen = format!("[{}]", [r#"{"type":0}"#; 16].join(","));
        assert_eq!(update(&sixteen).unwrap().custom_status, Patch::Null);
        for activities in [
            format!("[{}]", [r#"{"type":0}"#; 17].join(",")),
            format!(r#"[{{"type":4,"state":"{}"}}]"#, "x".repeat(4097)),
            format!(r#"[{{"type":4,"emoji":{{"name":"{}"}}}}]"#, "x".repeat(129)),
        ] {
            assert!(update(&activities).is_err());
            assert!(
                crate::decode::<crate::PresenceDto>(
                    format!(r#"{{"status":"online","activities":{activities}}}"#).as_bytes(),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn partial_identity_and_status_patches_do_not_retain_unrelated_presence_data() {
        for status in ["online", "idle", "dnd", "offline"] {
            let bytes = format!(
                r#"{{"guild_id":"1","user":{{"id":"2"}},"status":"{status}","activities":[{{"type":0,"name":"Synthetic"}}],"client_status":{{"desktop":"dnd"}}}}"#
            );
            let presence = decode(bytes.as_bytes()).unwrap();
            assert_eq!(presence.guild, Some(Id(1)));
            assert_eq!(presence.user, Id(2));
            assert_eq!(presence.status, Patch::Value(status.into()));
        }
        let presence = decode(br#"{"user":{"id":"2","username":"Ignored"}}"#).unwrap();
        assert_eq!(presence.guild, None);
        assert_eq!(presence.status, Patch::Absent);
        assert_eq!(
            decode(br#"{"guild_id":null,"user":{"id":"2"},"status":null}"#)
                .unwrap()
                .status,
            Patch::Null
        );
        for status in ["invisible", "future-status", "ONLINE", ""] {
            let bytes = format!(r#"{{"user":{{"id":"2"}},"status":"{status}"}}"#);
            assert_eq!(decode(bytes.as_bytes()).unwrap().status, Patch::Null);
        }
        let huge = format!(
            r#"{{"user":{{"id":"2"}},"status":"{}"}}"#,
            "x".repeat(128 * 1024)
        );
        assert_eq!(decode(huge.as_bytes()).unwrap().status, Patch::Null);
        let oversized = format!(
            r#"{{"user":{{"id":"2"}},"status":"{}"}}"#,
            "x".repeat(crate::MAX_WIRE)
        );
        assert!(decode(oversized.as_bytes()).is_err());
    }

    #[test]
    fn malformed_identity_scope_or_status_is_rejected() {
        for bytes in [
            r#"{"user":{}}"#,
            r#"{"user":{"id":"0"}}"#,
            r#"{"user":{"id":2}}"#,
            r#"{"user":{"id":"18446744073709551616"}}"#,
            r#"{"user":{"id":"000000000000000000002"}}"#,
            r#"{"guild_id":"0","user":{"id":"2"}}"#,
            r#"{"guild_id":"bad","user":{"id":"2"}}"#,
            r#"{"guild_id":1,"user":{"id":"2"}}"#,
            r#"{"guild_id":"1","user":null}"#,
            r#"{"user":{"id":"2"},"status":42}"#,
            r#"{"user":{"id":"2"},"status":{"online":null}}"#,
            r#"{"user":{"id":"2"},"status":"online","status":"idle"}"#,
        ] {
            assert!(decode(bytes.as_bytes()).is_err());
        }
        assert_eq!(
            decode(br#"{"user":{"id":"18446744073709551615"}}"#)
                .unwrap()
                .user,
            Id(u64::MAX)
        );
    }
}
