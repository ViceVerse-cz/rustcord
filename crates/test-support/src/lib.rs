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
