use crate::{Freshness, Id, State};

impl State {
    pub(crate) fn apply_member_presence(
        &mut self,
        guild: Id,
        channel: Id,
        request: u64,
        updates: &[(Id, Option<String>)],
    ) {
        if !self.gateway_connected
            || self.selected != Some(channel)
            || !self.can_view(channel)
            || !self.channels.iter().any(|known| {
                known.id == channel && known.guild == Some(guild) && known.supports_text()
            })
            || updates.len() > 100
            || updates.iter().enumerate().any(|(index, (user, status))| {
                user.0 == 0
                    || updates[..index].iter().any(|(other, _)| other == user)
                    || status.as_deref().is_some_and(|status| {
                        !matches!(status, "online" | "idle" | "dnd" | "offline")
                    })
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
                let Some((_, status)) = updates.iter().find(|(user, _)| *user == row.user.id)
                else {
                    return row.bytes();
                };
                row.bytes()
                    - row.status.as_ref().map_or(0, String::capacity)
                    - row.custom_status.as_ref().map_or(0, String::capacity)
                    + status.as_ref().map_or(0, String::len)
            })
            .sum::<usize>();
        if projected_bytes > 128 * 1024 {
            return;
        }
        for row in list.rows.iter_mut().flatten() {
            if let Some((_, status)) = updates.iter().find(|(user, _)| *user == row.user.id) {
                row.status = status.clone();
                // Compact presence updates do not retain activities. A newer update
                // invalidates the custom status from the last member-list snapshot.
                row.custom_status = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Envelope, Event, auth::AuthState};
    use model::{Channel, Guild, Member, MemberList, User};

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

    fn event(updates: Vec<(Id, Option<String>)>) -> Event {
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

    fn status(state: &State) -> Option<&str> {
        state.members.as_ref().unwrap().rows[0]
            .as_ref()
            .unwrap()
            .status
            .as_deref()
    }

    #[test]
    fn presence_updates_only_loaded_rows_without_invalidating_timeline() {
        let mut state = state();
        state.members.as_mut().unwrap().rows[0]
            .as_mut()
            .unwrap()
            .custom_status = Some("Old custom status".into());
        let revision = state.revision;
        // Activity-only gateway changes preserve online status but clear old activity text.
        apply(&mut state, event(vec![(Id(2), Some("online".into()))]));
        assert_eq!(status(&state), Some("online"));
        assert!(
            state.members.as_ref().unwrap().rows[0]
                .as_ref()
                .unwrap()
                .custom_status
                .is_none()
        );
        apply(
            &mut state,
            event(vec![
                (Id(2), Some("idle".into())),
                (Id(3), Some("dnd".into())),
            ]),
        );
        assert_eq!(status(&state), Some("idle"));
        let list = state.members.as_ref().unwrap();
        assert_eq!(list.total, 200);
        assert_eq!(list.rows.len(), 2);
        assert!(list.rows[1].is_none());
        apply(&mut state, event(vec![(Id(2), None)]));
        assert_eq!(status(&state), None);
        assert_eq!(state.revision, revision);
    }

    #[test]
    fn invalid_batches_are_rejected_atomically_including_allocation_bounds() {
        let mut state = state();
        for updates in [
            vec![(Id(2), None), (Id(2), Some("idle".into()))],
            vec![(Id(2), None), (Id(0), None)],
            vec![(Id(2), Some("unknown".into()))],
            (1..=101).map(|id| (Id(id), None)).collect(),
            {
                let mut status = String::with_capacity(8192);
                status.push_str("idle");
                vec![(Id(2), Some(status))]
            },
            {
                let mut updates = Vec::with_capacity(8192);
                updates.push((Id(2), None));
                updates
            },
        ] {
            apply(&mut state, event(updates));
            assert_eq!(status(&state), Some("online"));
        }
        let row = state.members.as_mut().unwrap().rows[0].as_mut().unwrap();
        row.status = None;
        let spare = 128 * 1024 - row.bytes();
        row.nick = Some("x".repeat(spare));
        assert_eq!(row.bytes(), 128 * 1024);
        apply(&mut state, event(vec![(Id(2), Some("online".into()))]));
        assert_eq!(status(&state), None);
    }

    #[test]
    fn stale_scope_disconnect_and_access_loss_cannot_change_presence() {
        for change in 0..10 {
            let mut state = state();
            let mut envelope = Envelope {
                generation: state.generation,
                event: event(vec![(Id(2), None)]),
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
                assert_eq!(status(&state), Some("online"));
            }
        }
    }
}
