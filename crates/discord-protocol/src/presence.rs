//! Only identity, scope and a bounded normalized status survive presence decoding.
use crate::DecodeError;
use model::{Id, Patch};
use serde::Deserialize;

pub struct PresenceUpdate {
    pub guild: Option<Id>,
    pub user: Id,
    pub status: Patch<String>,
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
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_identity_and_status_patches_do_not_retain_unrelated_presence_data() {
        for status in ["online", "idle", "dnd", "offline"] {
            let bytes = format!(
                r#"{{"guild_id":"1","user":{{"id":"2"}},"status":"{status}","activities":[{{"name":"Synthetic"}}],"client_status":{{"desktop":"dnd"}}}}"#
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
