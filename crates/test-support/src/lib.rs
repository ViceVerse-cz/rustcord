//! Handcrafted synthetic data. No network imports; never evidence of live compatibility.
use client_core::{Envelope, Event, State};
use model::*;
pub fn message(id: u64, channel: Id) -> Message {
    let mut content = match id % 6 {
        0 => "A short synthetic message.".into(),
        1 => "A longer synthetic message which wraps at smaller window sizes. ".repeat(8),
        2 => "Unicode: 日本語 · čeština · العربية · e\u{301} · 👩🏽‍💻".into(),
        3 => "```rust\nfn main() {\n    println!(\"synthetic fixture\");\n}\n```".into(),
        4 => "> A quoted thought\nA second line, and a third.\nThis stays only in session memory."
            .into(),
        _ => "**Synthetic history** — this is an offline fixture, never a Discord reply.".into(),
    };
    if id == 500 {
        content = format!("Hey <@2> — {content}");
    }
    Message {
        reactions: Some(if id == 500 {
            vec![Reaction {
                emoji: ReactionEmoji {
                    id: None,
                    name: Some("👍".into()),
                },
                count: 3,
                me: false,
                me_burst: false,
            }]
        } else {
            vec![]
        }),
        id: Id(id),
        channel,
        author: User {
            avatar: None,
            discriminator: 0,
            id: Id(if id.is_multiple_of(2) { 1 } else { 2 }),
            name: if id.is_multiple_of(2) {
                "You (synthetic)"
            } else {
                "Robin (synthetic)"
            }
            .into(),
        },
        content,
        edited: false,
        edited_at: None,
        revision: 0,
        nonce: None,
        reply_to: None,
        unsupported: false,
        embeds: demo_embeds(id),
        attachments: if id == 500 {
            vec![Attachment {
                id: Id(700),
                filename: "synthetic-landscape.png".into(),
                description: Some("Original synthetic landscape · offline preview".into()),
                content_type: Some("image/png".into()),
                size: 2048,
                spoiler: false,
                media: EmbedMedia {
                    url: Some(
                        "https://cdn.discordapp.com/attachments/1/700/synthetic-landscape.png"
                            .into(),
                    ),
                    proxy_url: None,
                    width: 640,
                    height: 240,
                },
            }]
        } else {
            vec![]
        },
        mentions: if id == 500 {
            vec![User {
                id: Id(2),
                name: "Robin (synthetic)".into(),
                avatar: None,
                discriminator: 0,
            }]
        } else {
            Vec::new()
        },
        embeds_suppressed: false,
    }
}
fn demo_embeds(id: u64) -> Vec<Embed> {
    if id != 500 {
        return vec![];
    }
    vec![Embed {
        kind: "rich".into(),
        title: Some("A quieter place for your conversations".into()),
        description: Some("**Native embed preview**\nFormatted descriptions, *useful details*, and a static image.\n[Markdown link](https://example.com/synthetic) · https://example.org".into()),
        url: Some("https://example.com/synthetic".into()),
        color: Some(0x68ada4),
        author: Some(EmbedAuthor { name: "Serein · synthetic example".into(), ..Default::default() }),
        fields: vec![EmbedField { name:"Interface".into(),value:"Rust + egui".into(),inline:true },EmbedField { name:"Preview".into(),value:"Offline only".into(),inline:true }],
        image: Some(EmbedMedia { url:Some("https://example.com/synthetic-image.png".into()),width:640,height:240,..Default::default() }),
        footer: Some(EmbedFooter { text:"Synthetic content · no service request".into(),..Default::default() }),
        ..Default::default()
    }]
}
pub fn demo_state() -> State {
    let mut state = State {
        demo: true,
        ..State::default()
    };
    state.apply(Envelope {
        generation: state.generation,
        event: Event::Ready {
            user: User {
                avatar: None,
                discriminator: 0,
                id: Id(1),
                name: "You (synthetic)".into(),
            },
            guilds: vec![Guild {
                icon: Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()),
                id: Id(10),
                name: "Synthetic workspace".into(),
            }],
            channels: vec![
                Channel {
                    last_message: None,
                    id: Id(20),
                    guild: Some(Id(10)),
                    parent_id: Some(Id(23)),
                    position: 0,
                    name: "getting-started".into(),
                    kind: 0,
                    recipients: vec![],
                    member_list_id: Some("everyone".into()),
                },
                Channel {
                    last_message: None,
                    id: Id(21),
                    guild: Some(Id(10)),
                    parent_id: Some(Id(24)),
                    position: 0,
                    name: "long-form".into(),
                    kind: 0,
                    recipients: vec![],
                    member_list_id: Some("everyone".into()),
                },
                Channel {
                    last_message: None,
                    id: Id(22),
                    guild: None,
                    parent_id: None,
                    position: 0,
                    name: "Robin (synthetic)".into(),
                    kind: 1,
                    recipients: vec![message(1, Id(22)).author],
                    member_list_id: None,
                },
                Channel {
                    last_message: None,
                    id: Id(23),
                    guild: Some(Id(10)),
                    parent_id: None,
                    position: 0,
                    name: "WELCOME".into(),
                    kind: 4,
                    recipients: vec![],
                    member_list_id: None,
                },
                Channel {
                    last_message: None,
                    id: Id(24),
                    guild: Some(Id(10)),
                    parent_id: None,
                    position: 1,
                    name: "CONVERSATIONS".into(),
                    kind: 4,
                    recipients: vec![],
                    member_list_id: None,
                },
                Channel {
                    last_message: None,
                    id: Id(25),
                    guild: Some(Id(10)),
                    parent_id: Some(Id(24)),
                    position: 1,
                    name: "hangout".into(),
                    kind: 2,
                    recipients: vec![],
                    member_list_id: None,
                },
            ],
        },
    });
    state.select(Id(20));
    load_page(&mut state, None);
    state.apply(Envelope {
        generation: state.generation,
        event: Event::ReadState(client_core::read_state::Event::Snapshot {
            partial: false,
            entries: Some(vec![(Id(20), Some(Id(495)))]),
            version: Some(1),
        }),
    });
    state.status = "Offline fixture · no network access";
    state
}
pub fn load_page(state: &mut State, before: Option<Id>) {
    let channel = state.selected.unwrap();
    let end = before.map_or(501, |id| id.0);
    let start = end.saturating_sub(50).max(1);
    state.apply(Envelope {
        generation: state.generation,
        event: Event::History {
            channel,
            request: state.request,
            older: before.is_some(),
            messages: (start..end).map(|id| message(id, channel)).collect(),
        },
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pin_snapshots_are_scoped_cancellable_and_open_revalidated_history() {
        use client_core::{Command, search::Outcome};
        let mut state = demo_state();
        let page = || SearchPage {
            hits: [480, 499]
                .into_iter()
                .map(|id| SearchHit {
                    id: Id(id),
                    channel: Id(20),
                    author: "Synthetic".into(),
                    excerpt: "pin".into(),
                })
                .collect(),
            total: 0,
            partial: true,
        };
        let Command::Pins { request: old, .. } = state.request_pins().unwrap() else {
            panic!()
        };
        let Command::Pins { request, .. } = state.request_pins().unwrap() else {
            panic!()
        };
        state.apply_search(Id(20), old, Ok(Outcome::Pins(page())));
        assert!(state.search.as_ref().unwrap().loading);
        state.apply_search(Id(20), request, Ok(Outcome::Page(page())));
        assert!(state.search.as_ref().unwrap().page.is_none());
        let Command::Pins { request, .. } = state.request_pins().unwrap() else {
            panic!()
        };
        state.apply_search(Id(21), request, Ok(Outcome::Pins(page())));
        assert!(state.search.as_ref().unwrap().loading);
        state.apply_search(Id(20), request, Ok(Outcome::Pins(page())));
        assert_eq!(
            state.search.as_ref().unwrap().page.as_ref().unwrap().hits[0].id,
            Id(480)
        );
        assert!(state.open_search_hit(Id(478)).is_none());
        assert!(matches!(
            state.open_search_hit(Id(480)),
            Some(Command::History {
                before: Some(Id(481)),
                ..
            })
        ));
        load_page(&mut state, Some(Id(481)));
        assert!(state.timeline.get(Id(480)).is_some());
        let command = state.request_pins().unwrap();
        state.command_rejected(command);
        assert!(!state.search.as_ref().unwrap().loading);
        assert!(state.search.as_ref().unwrap().error.is_some());
        let Command::Pins { request, .. } = state.request_pins().unwrap() else {
            panic!()
        };
        state.clear_search();
        state.apply_search(Id(20), request, Ok(Outcome::Pins(page())));
        assert!(state.search.is_none());
        state.gateway_connected = false;
        assert!(state.request_pins().is_none());
    }
    #[test]
    fn search_pages_reject_late_results_and_open_only_revalidated_history() {
        use client_core::{Command, auth::Failure, search::Outcome};
        let mut state = demo_state();
        let page = || {
            Outcome::Page(SearchPage {
                hits: vec![SearchHit {
                    id: Id(499),
                    channel: Id(20),
                    author: "Synthetic".into(),
                    excerpt: "index text".into(),
                }],
                total: 50,
                partial: false,
            })
        };
        assert!(state.request_search("x".into(), Some(Id(500))).is_none());
        let Command::Search {
            request: old,
            guild,
            ..
        } = state.request_search("first".into(), None).unwrap()
        else {
            panic!()
        };
        assert_eq!(guild, Some(Id(10)));
        let Command::Search { request, .. } = state.request_search("second".into(), None).unwrap()
        else {
            panic!()
        };
        state.apply_search(Id(20), old, Ok(page()));
        assert!(state.search.as_ref().unwrap().loading);
        state.apply_search(Id(20), request, Ok(page()));
        assert_eq!(
            state.timeline.get(Id(499)).unwrap().content,
            message(499, Id(20)).content
        );
        assert!(state.open_search_hit(Id(498)).is_none());
        assert!(matches!(
            state.open_search_hit(Id(499)),
            Some(Command::History {
                channel: Id(20),
                before: Some(Id(500)),
                ..
            })
        ));
        assert!(state.timeline.is_empty());
        assert_eq!(state.search_target, Some(Id(499)));
        load_page(&mut state, Some(Id(500)));
        assert_eq!(state.timeline.iter().last().unwrap().id, Id(499));
        assert_eq!(
            state.timeline.get(Id(499)).unwrap().content,
            message(499, Id(20)).content
        );
        let Command::Search { request, .. } = state
            .request_search("second".into(), Some(Id(499)))
            .unwrap()
        else {
            panic!()
        };
        state.apply_search(Id(20), request, Ok(page())); // inclusive cursor violates the requested page
        assert!(state.search.as_ref().unwrap().page.is_none());
        let command = state.request_search("third".into(), None).unwrap();
        state.command_rejected(command);
        assert!(!state.search.as_ref().unwrap().loading);
        let Command::Search { request, .. } = state.request_search("fourth".into(), None).unwrap()
        else {
            panic!()
        };
        state.apply(Envelope {
            generation: state.generation,
            event: Event::Delete {
                channel: Id(20),
                id: Id(499),
            },
        });
        state.apply_search(Id(20), request, Ok(page()));
        assert!(state.search.is_none());
        let Command::Search { request, .. } = state.request_search("fifth".into(), None).unwrap()
        else {
            panic!()
        };
        state.select(Id(22));
        state.apply_search(Id(20), request, Err(Failure::Forbidden));
        assert!(state.search.is_none());
        let Command::Search { request, .. } = state.request_search("sixth".into(), None).unwrap()
        else {
            panic!()
        };
        state.apply_search(Id(22), request, Ok(Outcome::Indexing));
        assert!(
            state
                .search
                .as_ref()
                .unwrap()
                .error
                .unwrap()
                .contains("indexing")
        );
        state.apply(Envelope {
            generation: state.generation,
            event: Event::Disconnected,
        });
        assert!(state.search.is_none());
        assert!(state.request_search("offline".into(), None).is_none());
        state.logout();
        state.apply(Envelope {
            generation: state.generation - 1,
            event: Event::Search {
                channel: Id(20),
                request,
                result: Ok(page()),
            },
        });
        assert!(state.search.is_none());
    }
    #[test]
    fn read_markers_are_explicit_scoped_and_preserve_newer_service_updates() {
        use client_core::{Command, auth::Failure, read_state::Event as R};
        let mut state = demo_state();
        assert_eq!(state.unread(Id(20)), Some(true));
        assert!(!state.can_mark_read(Id(495)));
        assert!(!state.can_mark_read(Id(900)));
        let Command::MarkRead {
            channel,
            message,
            request,
        } = state.prepare_mark_read(Id(500)).unwrap()
        else {
            panic!()
        };
        assert!(state.prepare_mark_read(Id(499)).is_none());
        assert_eq!(state.read_marker(channel), Some(Some(Id(495))));
        state
            .apply_read_state(R::Ack {
                channel,
                message: Some(Id(480)),
                manual: true,
                version: Some(3),
            })
            .unwrap();
        state
            .apply_read_state(R::Result {
                channel,
                message,
                request,
                result: Ok(()),
            })
            .unwrap();
        assert_eq!(state.read_marker(channel), Some(Some(Id(480))));
        state
            .apply_read_state(R::Ack {
                channel,
                message: Some(Id(499)),
                manual: false,
                version: Some(2),
            })
            .unwrap();
        assert_eq!(state.read_marker(channel), Some(Some(Id(480))));
        let Command::MarkRead { request, .. } = state.prepare_mark_read(Id(500)).unwrap() else {
            panic!()
        };
        state
            .apply_read_state(R::Result {
                channel,
                message,
                request,
                result: Err(Failure::Ambiguous),
            })
            .unwrap();
        assert_eq!(state.read_marker(channel), Some(Some(Id(480))));
        let command = state.prepare_mark_read(Id(500)).unwrap();
        state.command_rejected(command);
        assert_eq!(state.freshness, Freshness::Fresh);
        let Command::MarkRead { request, .. } = state.prepare_mark_read(Id(500)).unwrap() else {
            panic!()
        };
        state.select(Id(22));
        state
            .apply_read_state(R::Result {
                channel,
                message,
                request,
                result: Ok(()),
            })
            .unwrap();
        assert_eq!(state.unread(channel), Some(false));
        state
            .apply_read_state(R::Latest(vec![(channel, Patch::Value(Id(600)))]))
            .unwrap();
        assert_eq!(state.unread(channel), Some(true));
        state.select(channel);
        load_page(&mut state, None);
        state
            .apply_read_state(R::Ack {
                channel,
                message: None,
                manual: true,
                version: Some(4),
            })
            .unwrap();
        let Command::MarkRead { request, .. } = state.prepare_mark_read(Id(500)).unwrap() else {
            panic!()
        };
        state.apply(Envelope {
            generation: state.generation,
            event: Event::Disconnected,
        });
        state
            .apply_read_state(R::Result {
                channel,
                message,
                request,
                result: Ok(()),
            })
            .unwrap();
        assert_eq!(state.read_marker(channel), None);
        assert!(state.prepare_mark_read(message).is_none());
        state.gateway_connected = true;
        assert_eq!(state.read_marker(channel), Some(None));
        state.logout();
        assert_eq!(state.read_marker(channel), None);
        state = demo_state();
        let Command::MarkRead { request: old, .. } = state.prepare_mark_read(message).unwrap()
        else {
            panic!()
        };
        state
            .apply_read_state(R::Snapshot {
                partial: false,
                entries: None,
                version: None,
            })
            .unwrap();
        assert_eq!(state.unread(channel), None);
        let Command::MarkRead { request: new, .. } = state.prepare_mark_read(message).unwrap()
        else {
            panic!()
        };
        assert_ne!(old, new);
        state
            .apply_read_state(R::Result {
                channel,
                message,
                request: old,
                result: Ok(()),
            })
            .unwrap();
        assert_eq!(state.read_marker(channel), None);
        state
            .apply_read_state(R::Result {
                channel,
                message,
                request: new,
                result: Ok(()),
            })
            .unwrap();
        assert_eq!(state.unread(channel), Some(false));
        state.apply(Envelope {
            generation: state.generation,
            event: Event::RecipientRemoved {
                channel,
                user: Id(1),
            },
        });
        assert_eq!(state.read_marker(channel), None);
        assert!(
            state
                .apply_read_state(R::Snapshot {
                    partial: false,
                    entries: Some(vec![(Id(22), None), (Id(22), None)]),
                    version: None
                })
                .is_err()
        );
        assert_eq!(state.read_marker(Id(22)), None);
        state = demo_state();
        state
            .apply_read_state(R::Snapshot {
                entries: Some(vec![(Id(20), Some(Id(495)))]),
                version: Some(1),
                partial: true,
            })
            .unwrap();
        assert_eq!(state.read_marker(Id(20)), Some(Some(Id(495))));
        assert_eq!(state.read_marker(Id(22)), None);
    }
    #[test]
    fn late_logout_events_and_duplicate_send_confirmations() {
        let mut state = demo_state();
        let old = state.generation;
        state.logout();
        state.apply(Envelope {
            generation: old,
            event: Event::Message(message(42, Id(20))),
        });
        assert!(state.timeline.is_empty());
        assert!(state.user.is_none());
        state = demo_state();
        state.drafts.insert(Id(20), "synthetic outbound".into());
        let client_core::Command::Send { nonce, .. } = state.prepare_send().unwrap() else {
            panic!()
        };
        let mut m = message(900, Id(20));
        m.nonce = Some(nonce.clone());
        state.apply(Envelope {
            generation: state.generation,
            event: Event::Message(m.clone()),
        });
        state.apply(Envelope {
            generation: state.generation,
            event: Event::SendResult {
                nonce,
                result: Ok(m),
            },
        });
        assert_eq!(state.timeline.iter().filter(|m| m.id == Id(900)).count(), 1);
        assert!(state.pending.is_empty());
    }
}
