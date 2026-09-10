//! Thread dispatch uses existing channel/history behavior; it never joins a thread.
use client_core::{Event, auth::Failure};
use discord_protocol::{
    ChannelDto, decode,
    threads::{ThreadMembers, ThreadUpdate},
};
use model::{Id, Patch};

pub(super) fn decode_event(
    kind: &str,
    bytes: &[u8],
    owner: Option<Id>,
) -> Result<Option<Event>, Failure> {
    let protocol = |_| Failure::Protocol;
    Ok(match kind {
        "THREAD_CREATE" | "THREAD_DELETE" => {
            let channel: ChannelDto = decode(bytes).map_err(protocol)?;
            let guild = channel.guild_id.ok_or(Failure::Protocol)?;
            let channel =
                discord_protocol::threads::into_thread(channel, guild).map_err(protocol)?;
            Some(if kind == "THREAD_DELETE" {
                Event::ThreadRemoved {
                    guild,
                    id: channel.id,
                }
            } else {
                Event::ChannelCreated(channel)
            })
        }
        "THREAD_UPDATE" => {
            let update: ThreadUpdate = decode(bytes).map_err(protocol)?;
            if matches!(update.patch.kind, Patch::Null | Patch::Value(0..=9 | 13..=u8::MAX))
                || matches!(update.patch.parent_id, Patch::Null)
                || matches!(update.patch.parent_id, Patch::Value(id) if id == update.patch.id)
            {
                return Err(Failure::Protocol);
            }
            Some(if update.thread_metadata.is_some_and(|m| m.archived) {
                Event::ThreadRemoved {
                    guild: update.guild_id,
                    id: update.patch.id,
                }
            } else {
                Event::ThreadChanged {
                    guild: update.guild_id,
                    patch: update.patch.into_model(),
                }
            })
        }
        "THREAD_LIST_SYNC" => {
            let sync = decode::<discord_protocol::threads::ThreadListSync>(bytes)
                .map_err(protocol)?
                .into_model()
                .map_err(protocol)?;
            Some(Event::ThreadsSync {
                guild: sync.guild,
                parents: sync.parents,
                threads: sync.threads,
            })
        }
        "THREAD_MEMBERS_UPDATE" => {
            let members: ThreadMembers = decode(bytes).map_err(protocol)?;
            owner
                .filter(|id| members.removed_member_ids.contains(id))
                .map(|_| Event::ThreadRemoved {
                    guild: members.guild_id,
                    id: members.id,
                })
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thread_dispatch_rejects_malformed_scope_and_only_removes_the_owner() {
        for body in [
            r#"{"id":"3","guild_id":"1","type":0}"#,
            r#"{"id":"3","guild_id":"1","type":null}"#,
            r#"{"id":"3","guild_id":"1","parent_id":null}"#,
            r#"{"id":"3","guild_id":"1","parent_id":"3"}"#,
            r#"{"id":"3","name":"missing guild"}"#,
        ] {
            assert!(decode_event("THREAD_UPDATE", body.as_bytes(), Some(Id(9))).is_err());
        }
        let removal = br#"{"id":"3","guild_id":"1","removed_member_ids":["9"]}"#;
        assert!(matches!(
            decode_event("THREAD_MEMBERS_UPDATE", removal, None),
            Ok(None)
        ));
        assert!(matches!(
            decode_event("THREAD_MEMBERS_UPDATE", removal, Some(Id(8))),
            Ok(None)
        ));
        assert!(matches!(
            decode_event("THREAD_MEMBERS_UPDATE", removal, Some(Id(9))),
            Ok(Some(Event::ThreadRemoved {
                guild: Id(1),
                id: Id(3)
            }))
        ));
        let oversized =
            serde_json::json!({"id":"3","guild_id":"1","removed_member_ids":vec!["9";4001]});
        assert!(
            decode_event(
                "THREAD_MEMBERS_UPDATE",
                &serde_json::to_vec(&oversized).unwrap(),
                Some(Id(9))
            )
            .is_err()
        );
    }
}
