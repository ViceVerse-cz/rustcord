use crate::{Freshness, Id, State};

impl State {
    pub(crate) fn apply_member_presence(
        &mut self,
        guild: Id,
        channel: Id,
        request: u64,
        updates: &[model::MemberPresence],
    ) {
        if !self.gateway_connected
            || self.selected != Some(channel)
            || !self.can_view(channel)
            || !self.channels.iter().any(|known| {
                known.id == channel && known.guild == Some(guild) && known.supports_text()
            })
            || updates.len() > 100
            || updates.iter().enumerate().any(|(index, update)| {
                !update.valid()
                    || updates[..index]
                        .iter()
                        .any(|other| other.user == update.user)
            })
        {
            return;
        }
        let Some(list) = self.members.as_mut().filter(|list| {
            list.guild == Some(guild)
                && list.channel == channel
                && list.request == request
                && list.freshness == Freshness::Fresh
        }) else {
            return;
        };
        // ponytail: at most 100 loaded rows and updates; index only if the pane cap grows.
        let projected_bytes = list
            .rows
            .iter()
            .flatten()
            .map(|row| {
                let Some(update) = updates.iter().find(|update| update.user == row.user.id) else {
                    return row.bytes();
                };
                if row.status == update.status && row.custom_status == update.custom_status {
                    return row.bytes();
                }
                row.bytes()
                    - row.status.as_ref().map_or(0, String::capacity)
                    - row.custom_status.as_ref().map_or(0, String::capacity)
                    + update.status.as_ref().map_or(0, String::len)
                    + update.custom_status.as_ref().map_or(0, String::len)
            })
            .sum::<usize>();
        if projected_bytes > 128 * 1024 {
            return;
        }
        for row in list.rows.iter_mut().flatten() {
            if let Some(update) = updates.iter().find(|update| update.user == row.user.id)
                && (row.status != update.status || row.custom_status != update.custom_status)
            {
                row.status = update.status.clone();
                row.custom_status = update.custom_status.clone();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Envelope, Event, auth::AuthState};
    use model::{Channel, Guild, Member, MemberList, MemberPresence, User};

    fn state() -> State {
        let user = User {
            id: Id(2),
            name: "Synthetic".into(),
            avatar: None,
            discriminator: 0,
        };
        let mut state = State {
            user: Some(user.clone()),
            auth: AuthState::Authenticated,
            gateway_connected: true,
            freshness: Freshness::Fresh,
            selected: Some(Id(1)),
            guilds: vec![Guild {
                id: Id(10),
                name: "Synthetic".into(),
                icon: None,
                emojis: None,
            }],
            channels: vec![Channel {
                id: Id(1),
                guild: Some(Id(10)),
                parent_id: None,
                position: 0,
                name: "Synthetic".into(),
                kind: 0,
                recipients: vec![],
                member_list_id: Some("everyone".into()),
                last_message: None,
            }],
            members: Some(MemberList {
                guild: Some(Id(10)),
                channel: Id(1),
                request: 7,
                rows: vec![
                    Some(Member {
                        roles: vec![],
                        user,
                        nick: None,
                        status: Some("online".into()),
                        custom_status: None,
                    }),
                    None,
                ],
                total: 200,
                freshness: Freshness::Fresh,
            }),
            ..State::default()
        };
        crate::tests::grant_permissions(&mut state);
        state
    }

    fn event(updates: Vec<MemberPresence>) -> Event {
        Event::MemberPresence {
            guild: Id(10),
            channel: Id(1),
            request: 7,
            updates,
        }
    }

    fn apply(state: &mut State, event: Event) {
        state.apply(Envelope {
            generation: state.generation,
            event,
        });
    }

    fn update(user: u64, status: Option<&str>, custom: Option<&str>) -> MemberPresence {
        MemberPresence {
            user: Id(user),
            status: status.map(str::to_owned),
            custom_status: custom.map(str::to_owned),
        }
    }

    fn row(state: &State) -> &Member {
        state.members.as_ref().unwrap().rows[0].as_ref().unwrap()
    }

    #[test]
    fn complete_presence_values_preserve_replace_and_clear_without_timeline_churn() {
        let mut state = state();
        state.members.as_mut().unwrap().rows[0]
            .as_mut()
            .unwrap()
            .custom_status = Some("Old custom status".into());
        let revision = state.revision;
        let resident = (
            state.resident_history_rows(),
            state.resident_history_bytes(),
        );
        // Gateway resolves an absent activity field before delivering this complete record.
        apply(
            &mut state,
            event(vec![update(2, Some("idle"), Some("Old custom status"))]),
        );
        assert_eq!(row(&state).status.as_deref(), Some("idle"));
        assert_eq!(
            row(&state).custom_status.as_deref(),
            Some("Old custom status")
        );
        apply(
            &mut state,
            event(vec![
                update(2, Some("idle"), Some("\u{1f642} Synthetic status")),
                update(3, Some("online"), Some("Unloaded row")),
            ]),
        );
        assert_eq!(
            row(&state).custom_status.as_deref(),
            Some("\u{1f642} Synthetic status")
        );
        let allocation = row(&state).custom_status.as_ref().unwrap().as_ptr();
        apply(
            &mut state,
            event(vec![update(
                2,
                Some("idle"),
                Some("\u{1f642} Synthetic status"),
            )]),
        );
        assert_eq!(
            row(&state).custom_status.as_ref().unwrap().as_ptr(),
            allocation
        );
        let list = state.members.as_ref().unwrap();
        assert_eq!(list.total, 200);
        assert_eq!(list.rows.len(), 2);
        assert!(list.rows[1].is_none());
        apply(
            &mut state,
            event(vec![update(2, None, Some("\u{1f642} Synthetic status"))]),
        );
        assert!(row(&state).status.is_none());
        assert_eq!(
            row(&state).custom_status.as_deref(),
            Some("\u{1f642} Synthetic status")
        );
        apply(&mut state, event(vec![update(2, Some("offline"), None)]));
        assert_eq!(row(&state).status.as_deref(), Some("offline"));
        assert!(row(&state).custom_status.is_none());
        assert_eq!(state.revision, revision);
        assert_eq!(
            (
                state.resident_history_rows(),
                state.resident_history_bytes()
            ),
            resident
        );
    }

    #[test]
    fn invalid_batches_are_rejected_atomically_including_allocation_bounds() {
        let mut state = state();
        for updates in [
            vec![update(2, None, None), update(2, Some("idle"), None)],
            vec![update(2, None, None), update(0, None, None)],
            vec![update(2, Some("unknown"), None)],
            (1..=101).map(|id| update(id, None, None)).collect(),
            {
                let mut status = String::with_capacity(crate::MAX_MEMBER_PRESENCE_BYTES);
                status.push_str("idle");
                vec![MemberPresence {
                    user: Id(2),
                    status: Some(status),
                    custom_status: None,
                }]
            },
            {
                let mut custom = String::with_capacity(crate::MAX_MEMBER_PRESENCE_BYTES);
                custom.push_str("Bounded text, excessive allocation");
                vec![MemberPresence {
                    user: Id(2),
                    status: None,
                    custom_status: Some(custom),
                }]
            },
            {
                let mut updates = Vec::with_capacity(crate::MAX_MEMBER_PRESENCE_BYTES);
                updates.push(update(2, None, None));
                updates
            },
        ] {
            apply(&mut state, event(updates));
            assert_eq!(row(&state).status.as_deref(), Some("online"));
            assert!(row(&state).custom_status.is_none());
        }
        for custom in [
            "".into(),
            " ".into(),
            " padded".into(),
            "padded ".into(),
            "control\ntext".into(),
            "control\u{7f}".into(),
            "x".repeat(129),
            "\u{1f642}".repeat(129),
        ] {
            // One invalid unloaded row invalidates the batch before the valid loaded row changes.
            apply(
                &mut state,
                event(vec![update(2, None, None), update(3, None, Some(&custom))]),
            );
            assert_eq!(row(&state).status.as_deref(), Some("online"));
        }
        let maximum = "\u{1f642}".repeat(128);
        let full_batch = event(
            (1..=100)
                .map(|id| update(id, Some("dnd"), Some(&maximum)))
                .collect(),
        );
        assert!(full_batch.bytes() > 8 * 1024);
        assert!(full_batch.bytes() <= crate::MAX_MEMBER_PRESENCE_BYTES);
        apply(&mut state, full_batch);
        assert_eq!(row(&state).status.as_deref(), Some("dnd"));
        assert_eq!(row(&state).custom_status.as_deref(), Some(maximum.as_str()));
    }

    #[test]
    fn projected_row_budget_counts_custom_text_and_retained_equal_allocations() {
        let mut state = state();
        let first = state.members.as_mut().unwrap().rows[0].as_mut().unwrap();
        first.custom_status = Some(String::with_capacity(1024));
        first.custom_status.as_mut().unwrap().push_str("Same text");
        let mut second = first.clone();
        second.user.id = Id(3);
        second.custom_status = None;
        let spare = 128 * 1024 - first.bytes() - second.bytes();
        first.nick = Some("x".repeat(spare));
        state.members.as_mut().unwrap().rows[1] = Some(second);
        assert_eq!(
            state
                .members
                .as_ref()
                .unwrap()
                .rows
                .iter()
                .flatten()
                .map(Member::bytes)
                .sum::<usize>(),
            128 * 1024
        );
        // Equal updates retain their existing capacity; they cannot manufacture room for another row.
        apply(
            &mut state,
            event(vec![
                update(2, Some("online"), Some("Same text")),
                update(3, Some("online"), Some("x")),
            ]),
        );
        assert!(
            state.members.as_ref().unwrap().rows[1]
                .as_ref()
                .unwrap()
                .custom_status
                .is_none()
        );
        // Replacing the first value releases its old allocation and admits the complete batch.
        apply(
            &mut state,
            event(vec![
                update(2, Some("idle"), Some("Replacement")),
                update(3, Some("online"), Some("x")),
            ]),
        );
        assert_eq!(row(&state).custom_status.as_deref(), Some("Replacement"));
        assert_eq!(
            state.members.as_ref().unwrap().rows[1]
                .as_ref()
                .unwrap()
                .custom_status
                .as_deref(),
            Some("x")
        );
        assert!(
            state
                .members
                .as_ref()
                .unwrap()
                .rows
                .iter()
                .flatten()
                .map(Member::bytes)
                .sum::<usize>()
                <= 128 * 1024
        );
    }

    #[test]
    fn stale_scope_disconnect_and_access_loss_cannot_change_presence() {
        for change in 0..10 {
            let mut state = state();
            let mut envelope = Envelope {
                generation: state.generation,
                event: event(vec![update(2, None, Some("Late custom status"))]),
            };
            match change {
                0 => envelope.generation += 1,
                1 => state.members.as_mut().unwrap().guild = Some(Id(11)),
                2 => state.members.as_mut().unwrap().channel = Id(3),
                3 => state.members.as_mut().unwrap().request += 1,
                4 => state.members.as_mut().unwrap().freshness = Freshness::Loading,
                5 => state.gateway_connected = false,
                6 => state.selected = Some(Id(3)),
                7 => state.permissions = Default::default(),
                8 => state.channels[0].guild = None,
                _ => {
                    state.close_members();
                }
            }
            state.apply(envelope);
            if state.members.is_some() {
                assert_eq!(row(&state).status.as_deref(), Some("online"));
                assert!(row(&state).custom_status.is_none());
            }
        }
    }
}
