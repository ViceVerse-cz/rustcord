use crate::{Command, Envelope, Event, State, permissions::Event as PermissionEvent};
use model::{
    Channel, ChannelPatch, Freshness, Guild, Id, Message, MessagePatch, Patch, User,
    permissions as p,
};

const BITS: u128 = p::VIEW_CHANNEL
    | p::READ_MESSAGE_HISTORY
    | p::SEND_MESSAGES
    | p::SEND_MESSAGES_IN_THREADS
    | p::ATTACH_FILES
    | p::ADD_REACTIONS
    | p::CONNECT
    | p::SPEAK
    | p::USE_VAD;

fn user() -> User {
    User {
        id: Id(2),
        name: "Synthetic member".into(),
        avatar: None,
        discriminator: 0,
    }
}
fn channel(id: u64, kind: u8, parent: Option<Id>) -> Channel {
    Channel {
        id: Id(id),
        guild: Some(Id(10)),
        parent_id: parent,
        kind,
        name: "Synthetic conversation".into(),
        position: 0,
        recipients: vec![],
        last_message: None,
        member_list_id: None,
    }
}
fn message(id: u64, channel: Id) -> Message {
    Message {
        id: Id(id),
        channel,
        kind: 0,
        author: user(),
        content: "Synthetic server content".into(),
        reactions: Some(vec![]),
        edited: false,
        edited_at: None,
        revision: 0,
        nonce: None,
        reply_to: None,
        unsupported: false,
        extra_content: Default::default(),
        embeds: vec![],
        attachments: vec![],
        mentions: vec![],
        embeds_suppressed: false,
    }
}
fn snapshot() -> p::Snapshot {
    p::Snapshot {
        guilds: vec![p::Guild {
            id: Id(10),
            owner: Some(Id(999)),
            roles: Some(vec![
                p::Role {
                    id: Id(10),
                    bits: BITS,
                },
                p::Role {
                    id: Id(11),
                    bits: 0,
                },
            ]),
            member: Some(p::Member {
                roles: vec![],
                timeout_until: None,
            }),
        }],
        channels: vec![
            p::Channel {
                id: Id(20),
                guild: Id(10),
                overwrites: Some(vec![p::Overwrite {
                    id: Id(11),
                    kind: 0,
                    allow: 0,
                    deny: p::SEND_MESSAGES,
                }]),
            },
            p::Channel {
                id: Id(21),
                guild: Id(10),
                overwrites: Some(vec![p::Overwrite {
                    id: Id(2),
                    kind: 1,
                    allow: 0,
                    deny: p::VIEW_CHANNEL,
                }]),
            },
            p::Channel {
                id: Id(22),
                guild: Id(10),
                overwrites: Some(vec![]),
            },
        ],
    }
}
fn apply(state: &mut State, event: Event) {
    state.apply(Envelope {
        generation: state.generation,
        event,
    });
}
fn permission(state: &mut State, event: PermissionEvent) {
    apply(state, Event::Permissions(event));
}
fn deny(state: &mut State, bits: u128) {
    permission(
        state,
        PermissionEvent::Channel {
            channel: Id(20),
            guild: Some(Id(10)),
            overwrites: Patch::Value(vec![p::Overwrite {
                id: Id(2),
                kind: 1,
                allow: 0,
                deny: bits,
            }]),
        },
    );
}
fn history(state: &mut State, channel: Id, request: u64, id: u64) {
    apply(
        state,
        Event::History {
            channel,
            request,
            older: false,
            messages: vec![message(id, channel)],
        },
    );
}
fn state() -> State {
    let mut state = State::default();
    apply(
        &mut state,
        Event::Ready {
            user: user(),
            guilds: vec![Guild {
                id: Id(10),
                name: "Synthetic guild".into(),
                icon: None,
                emojis: None,
            }],
            channels: vec![
                channel(20, 0, None),
                channel(21, 0, None),
                channel(22, 2, None),
                channel(30, 11, Some(Id(20))),
            ],
            permissions: snapshot(),
        },
    );
    let Some(Command::History { request, .. }) = state.select(Id(20)) else {
        panic!("Known non-admin member can open history")
    };
    history(&mut state, Id(20), request, 100);
    assert!(state.can_send(Id(20)) && state.can_read_history(Id(20)));
    state
}

#[test]
fn revoked_view_cannot_return_through_stale_gateway_content_or_old_history() {
    for resync in [false, true] {
        let mut state = state();
        state.drafts.insert(Id(20), "Keep my draft".into());
        let Command::History { request, .. } = state.history(None) else {
            panic!()
        };
        deny(&mut state, p::VIEW_CHANNEL);
        assert!(state.timeline.is_empty());
        assert!(!state.history_pending);
        apply(
            &mut state,
            if resync {
                Event::Resync
            } else {
                Event::Disconnected
            },
        );
        history(&mut state, Id(20), request, 200);
        apply(&mut state, Event::Message(message(201, Id(20))));
        apply(
            &mut state,
            Event::SendResult {
                nonce: "synthetic late confirmation".into(),
                result: Ok(message(202, Id(20))),
            },
        );
        apply(
            &mut state,
            Event::Patch(MessagePatch {
                extra_content: Default::default(),
                id: Id(203),
                channel: Id(20),
                content: Patch::Value("Late inaccessible edit".into()),
                reactions: Patch::Absent,
                mentions: Patch::Absent,
                edited: Patch::Absent,
                embeds: Patch::Absent,
                embeds_suppressed: Patch::Absent,
                attachments: Patch::Absent,
            }),
        );
        assert!(!state.can_view(Id(20)));
        assert!(
            state.timeline.is_empty(),
            "Stale is not permission to display content"
        );
        assert_eq!(state.drafts[&Id(20)], "Keep my draft");

        permission(&mut state, PermissionEvent::Snapshot(snapshot()));
        apply(&mut state, Event::Resumed);
        let Command::History { request, .. } = state.history(None) else {
            panic!()
        };
        history(&mut state, Id(20), request, 203);
        assert_eq!(
            state.timeline.get(Id(203)).unwrap().content,
            "Synthetic server content"
        );
        assert_eq!(state.freshness, Freshness::Fresh);
    }
}

#[test]
fn send_only_access_accepts_new_live_messages_without_restoring_old_history() {
    let mut state = state();
    let Command::History { request, .. } = state.history(None) else {
        panic!()
    };
    deny(&mut state, p::READ_MESSAGE_HISTORY);
    assert!(state.can_view(Id(20)) && state.can_send(Id(20)));
    assert!(!state.can_read_history(Id(20)));
    assert!(state.timeline.is_empty());
    assert!(matches!(state.history(None), Command::CancelSearch));
    history(&mut state, Id(20), request, 200);
    assert!(state.timeline.is_empty());
    apply(&mut state, Event::Message(message(201, Id(20))));
    permission(
        &mut state,
        PermissionEvent::Role {
            guild: Id(10),
            role: p::Role {
                id: Id(11),
                bits: p::ATTACH_FILES,
            },
        },
    );
    assert!(
        state.timeline.get(Id(201)).is_some(),
        "Unchanged read denial must not erase the live stream"
    );
    state.drafts.insert(Id(20), "New outgoing message".into());
    assert!(matches!(
        state.prepare_send(),
        Some(Command::Send {
            channel: Id(20),
            ..
        })
    ));
    assert!(!state.history_pending);
}

#[test]
fn deleting_an_unassigned_role_prunes_its_overwrites_and_invalidates_cached_decisions() {
    let mut state = state();
    // These queries populate the decision cache before each mutation.
    assert!(state.can_view(Id(20)) && state.can_send(Id(20)));
    permission(
        &mut state,
        PermissionEvent::RoleRemoved {
            guild: Id(10),
            id: Id(11),
        },
    );
    assert!(state.can_view(Id(20)) && state.can_send(Id(20)));
    assert!(state.timeline.get(Id(100)).is_some());
    assert!(
        state.permissions.channels[&Id(20)]
            .overwrites
            .as_ref()
            .unwrap()
            .is_empty()
    );
    permission(
        &mut state,
        PermissionEvent::Role {
            guild: Id(10),
            role: p::Role {
                id: Id(10),
                bits: BITS & !p::SEND_MESSAGES,
            },
        },
    );
    assert!(
        !state.can_send(Id(20)),
        "Role changes must not reuse an earlier cached allow"
    );
    assert!(state.can_read_history(Id(20)) && state.timeline.get(Id(100)).is_some());
}

#[test]
fn thread_target_changes_revoke_content_for_patches_creates_and_snapshots() {
    for parent in [Id(21), Id(22)] {
        for kind in 0..3 {
            let mut state = state();
            let Some(Command::History { request, .. }) = state.select(Id(30)) else {
                panic!()
            };
            history(&mut state, Id(30), request, 300);
            state.reply = Some(Id(300));
            state.drafts.insert(Id(30), "Keep thread draft".into());
            let Command::History { request, .. } = state.history(None) else {
                panic!()
            };
            let replacement = channel(30, 11, Some(parent));
            let event = match kind {
                0 => Event::ThreadChanged {
                    guild: Id(10),
                    patch: ChannelPatch {
                        id: Id(30),
                        parent_id: Patch::Value(parent),
                        kind: Patch::Absent,
                        name: Patch::Absent,
                        position: Patch::Absent,
                        last_message: Patch::Absent,
                    },
                },
                1 => Event::ChannelCreated(replacement),
                _ => Event::ThreadsSync {
                    guild: Id(10),
                    parents: None,
                    threads: vec![replacement],
                    removed: vec![],
                },
            };
            apply(&mut state, event);
            assert!(
                !state.can_view(Id(30)),
                "A denied or unsupported parent cannot supply thread access"
            );
            assert!(state.timeline.is_empty() && !state.history_pending && state.reply.is_none());
            assert_eq!(state.drafts[&Id(30)], "Keep thread draft");
            history(&mut state, Id(30), request, 301);
            assert!(state.timeline.is_empty());
        }
    }
}

#[test]
fn malformed_snapshots_are_atomic_and_rejected_permission_events_fail_closed() {
    let mut state = state();
    let original = state.permissions.clone();
    let mut oversized = snapshot();
    oversized.guilds[0].roles = Some(
        (1..=513)
            .map(|id| p::Role {
                id: Id(id),
                bits: BITS,
            })
            .collect(),
    );
    let mut duplicate = snapshot();
    duplicate.channels.push(duplicate.channels[0].clone());
    for bad in [oversized.clone(), duplicate] {
        assert!(
            state
                .permissions
                .update(PermissionEvent::Snapshot(bad))
                .is_err()
        );
        assert_eq!(state.permissions.guilds, original.guilds);
        assert_eq!(state.permissions.channels, original.channels);
        assert!(state.can_view(Id(20)));
    }
    permission(
        &mut state,
        PermissionEvent::Channel {
            channel: Id(20),
            guild: None,
            overwrites: Patch::Absent,
        },
    );
    assert!(
        state.can_read_history(Id(20)),
        "Absent overwrite metadata preserves known state"
    );
    permission(
        &mut state,
        PermissionEvent::Channel {
            channel: Id(20),
            guild: None,
            overwrites: Patch::Null,
        },
    );
    assert_eq!(state.permission(Id(20), p::VIEW_CHANNEL), None);
    assert!(
        state.timeline.is_empty(),
        "Explicit unknown metadata invalidates a cached allow"
    );
    permission(&mut state, PermissionEvent::Snapshot(snapshot()));
    assert!(state.can_view(Id(20)));
    permission(&mut state, PermissionEvent::Snapshot(oversized));
    assert!(!state.can_view(Id(20)) && !state.can_send(Id(20)));
    apply(&mut state, Event::Message(message(400, Id(20))));
    assert!(
        state.timeline.is_empty(),
        "Rejected reducer updates cannot keep granting old access"
    );
}
