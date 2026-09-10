//! Unofficial normal-user settings/session payloads; unknown preferences disable OS alerts.
use model::Id;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Override {
    pub channel_id: Id,
    #[serde(default)]
    pub muted: Option<bool>,
    #[serde(default)]
    pub message_notifications: Option<u8>,
}
#[derive(Deserialize)]
pub struct Overrides(#[serde(deserialize_with = "crate::read_state::entries")] pub Vec<Override>);
#[derive(Deserialize)]
pub struct Setting {
    pub guild_id: Option<Id>,
    #[serde(default)]
    pub muted: Option<bool>,
    #[serde(default)]
    pub message_notifications: Option<u8>,
    #[serde(default)]
    pub channel_overrides: Option<Overrides>,
}
#[derive(Deserialize)]
#[serde(untagged)]
pub enum Snapshot {
    Versioned {
        #[serde(deserialize_with = "crate::read_state::entries")]
        entries: Vec<Setting>,
        #[serde(default)]
        partial: bool,
    },
    Legacy(#[serde(deserialize_with = "crate::read_state::entries")] Vec<Setting>),
}
impl Snapshot {
    pub fn entries(self) -> (Vec<Setting>, bool) {
        match self {
            Self::Versioned { entries, partial } => (entries, !partial),
            Self::Legacy(entries) => (entries, true),
        }
    }
}
#[derive(Deserialize)]
pub struct Session {
    pub status: String,
}
#[derive(Deserialize)]
pub struct Sessions(#[serde(deserialize_with = "crate::read_state::entries")] pub Vec<Session>);
impl Sessions {
    pub fn dnd(&self) -> Option<bool> {
        if self.0.iter().any(|s| s.status == "dnd") {
            return Some(true);
        }
        (!self.0.is_empty()
            && self.0.iter().all(|s| {
                matches!(
                    s.status.as_str(),
                    "online" | "idle" | "offline" | "invisible"
                )
            }))
        .then_some(false)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_preferences_and_unknown_presence_fail_closed() {
        let snapshot: Snapshot = crate::decode(br#"{"entries":[{"guild_id":null,"muted":false,"message_notifications":0,"channel_overrides":[{"channel_id":"2","muted":true,"message_notifications":2}]}],"partial":false}"#).unwrap();
        let (settings, complete) = snapshot.entries();
        assert!(complete);
        assert_eq!(
            settings[0].channel_overrides.as_ref().unwrap().0[0].channel_id,
            Id(2)
        );
        assert_eq!(
            settings[0].channel_overrides.as_ref().unwrap().0[0].muted,
            Some(true)
        );
        assert_eq!(
            crate::decode::<Sessions>(br#"[{"status":"online"},{"status":"dnd"}]"#)
                .unwrap()
                .dnd(),
            Some(true)
        );
        assert_eq!(
            crate::decode::<Sessions>(br#"[{"status":"new-status"}]"#)
                .unwrap()
                .dnd(),
            None
        );
        assert_eq!(crate::decode::<Sessions>(br#"[]"#).unwrap().dnd(), None);
        let oversized =
            serde_json::json!({"entries":vec![serde_json::json!({"guild_id":null});4001]});
        assert!(crate::decode::<Snapshot>(&serde_json::to_vec(&oversized).unwrap()).is_err());
    }
}
