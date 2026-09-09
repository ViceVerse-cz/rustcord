//! Handcrafted synthetic data. No network imports; never evidence of live compatibility.
use client_core::{Envelope, Event, State};
use model::*;
pub fn message(id: u64, channel: Id) -> Message {
    let content = match id % 6 {
        0 => "A short synthetic message.".into(),
        1 => "A longer synthetic message which wraps at smaller window sizes. ".repeat(8),
        2 => "Unicode: 日本語 · čeština · العربية · e\u{301} · 👩🏽‍💻".into(),
        3 => "```rust\nfn main() {\n    println!(\"synthetic fixture\");\n}\n```".into(),
        4 => "> A quoted thought\nA second line, and a third.\nThis stays only in session memory."
            .into(),
        _ => "**Synthetic history** — this is an offline fixture, never a Discord reply.".into(),
    };
    Message {
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
    }
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
                id: Id(10),
                name: "Synthetic workspace".into(),
            }],
            channels: vec![
                Channel {
                    id: Id(20),
                    guild: Some(Id(10)),
                    name: "getting-started".into(),
                    kind: 0,
                    recipients: vec![],
                    member_list_id: Some("everyone".into()),
                },
                Channel {
                    id: Id(21),
                    guild: Some(Id(10)),
                    name: "long-form".into(),
                    kind: 0,
                    recipients: vec![],
                    member_list_id: Some("everyone".into()),
                },
                Channel {
                    id: Id(22),
                    guild: None,
                    name: "Robin (synthetic)".into(),
                    kind: 1,
                    recipients: vec![message(1, Id(22)).author],
                    member_list_id: None,
                },
            ],
        },
    });
    state.select(Id(20));
    load_page(&mut state, None);
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
