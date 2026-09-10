//! A single explicitly requested profile; no directory or persistent profile cache.
use crate::{
    Command, State,
    auth::{AuthState, Failure},
};
use model::{Id, UserProfile};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

/// Recently viewed profiles stay in RAM so reopening a card does not repeat the request.
pub const CACHE_ENTRIES: usize = 32;
pub const CACHE_BYTES: usize = 1024 * 1024;
pub const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
#[derive(Default)]
pub struct ProfileCache {
    entries: VecDeque<CachedProfile>,
}
struct CachedProfile {
    user: Id,
    guild: Option<Id>,
    fetched: Instant,
    data: UserProfile,
}
impl ProfileCache {
    fn get(&mut self, user: Id, guild: Option<Id>, now: Instant) -> Option<&UserProfile> {
        self.entries
            .retain(|entry| now.saturating_duration_since(entry.fetched) < CACHE_TTL);
        self.entries
            .iter()
            .find(|entry| entry.user == user && entry.guild == guild)
            .map(|entry| &entry.data)
    }
    fn insert(&mut self, user: Id, guild: Option<Id>, data: UserProfile, now: Instant) {
        self.entries
            .retain(|entry| !(entry.user == user && entry.guild == guild));
        self.entries.push_back(CachedProfile {
            user,
            guild,
            fetched: now,
            data,
        });
        while self.entries.len() > CACHE_ENTRIES || self.bytes() > CACHE_BYTES {
            self.entries.pop_front();
        }
    }
    pub fn bytes(&self) -> usize {
        self.entries
            .iter()
            .map(|entry| size_of::<CachedProfile>() + entry.data.bytes())
            .sum()
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.entries.shrink_to_fit();
    }
}

pub struct ProfileView {
    pub user: Id,
    pub guild: Option<Id>,
    pub request: u64,
    pub loading: bool,
    pub error: Option<&'static str>,
    pub data: Option<UserProfile>,
}
impl State {
    pub fn request_profile(&mut self, user: Id, guild: Option<Id>) -> Option<Command> {
        self.request_profile_at(user, guild, Instant::now())
    }
    pub fn request_profile_at(
        &mut self,
        user: Id,
        guild: Option<Id>,
        now: Instant,
    ) -> Option<Command> {
        self.profile_request = self.profile_request.wrapping_add(1);
        if let Some(data) = self.profile_cache.get(user, guild, now) {
            self.profile = Some(ProfileView {
                user,
                guild,
                request: self.profile_request,
                loading: false,
                error: None,
                data: Some(data.clone()),
            });
            return None;
        }
        let allowed = !self.demo
            && self.auth == AuthState::Authenticated
            && self.gateway_connected
            && user.0 != 0
            && guild.is_none_or(|id| self.guilds.iter().any(|g| g.id == id));
        self.profile = Some(ProfileView {
            user,
            guild,
            request: self.profile_request,
            loading: allowed,
            error: (!allowed)
                .then_some("Profile unavailable while disconnected or outside this session"),
            data: None,
        });
        allowed.then_some(Command::Profile {
            user,
            guild,
            request: self.profile_request,
        })
    }
    pub fn clear_profile(&mut self) -> Command {
        self.profile_request = self.profile_request.wrapping_add(1);
        self.profile = None;
        Command::CancelProfile
    }
    pub(crate) fn apply_profile(
        &mut self,
        user: Id,
        guild: Option<Id>,
        request: u64,
        result: Result<Box<UserProfile>, Failure>,
    ) {
        if let Err(failure) = &result
            && failure.ends_session()
            && *failure != Failure::Capacity
        {
            self.fail(*failure);
            return;
        }
        let Some(view) = self
            .profile
            .as_mut()
            .filter(|v| v.user == user && v.guild == guild && v.request == request)
        else {
            return;
        };
        view.loading = false;
        match result {
            Ok(data)
                if data.user.id == user
                    && data.guild.as_ref().is_none_or(|g| Some(g.guild) == guild)
                    && data.valid() =>
            {
                view.error = None;
                view.data = Some(*data);
                let data = view.data.clone().expect("stored profile");
                self.profile_cache.insert(user, guild, data, Instant::now());
            }
            Ok(_) => {
                view.error = Some("Profile response was invalid or too large");
                view.data = None;
            }
            Err(failure) => {
                view.error = Some(if failure == Failure::Capacity {
                    "Profile request or response exceeded safe capacity"
                } else {
                    failure.label()
                });
                view.data = None;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Envelope, Event};
    #[test]
    fn profiles_require_explicit_request_and_reject_late_views() {
        let mut state = State::default();
        assert!(state.request_profile(Id(1), None).is_none());
        state.auth = AuthState::Authenticated;
        state.gateway_connected = true;
        let Some(Command::Profile { request, .. }) = state.request_profile(Id(1), None) else {
            panic!("missing request")
        };
        assert!(state.profile.as_ref().unwrap().loading);
        let Some(Command::Profile { request: new, .. }) = state.request_profile(Id(2), None) else {
            panic!("missing request")
        };
        state.apply(Envelope {
            generation: state.generation,
            event: Event::Profile {
                user: Id(1),
                guild: None,
                request,
                result: Err(Failure::Forbidden),
            },
        });
        assert!(state.profile.as_ref().unwrap().loading);
        state.apply(Envelope {
            generation: state.generation + 1,
            event: Event::Profile {
                user: Id(2),
                guild: None,
                request: new,
                result: Err(Failure::Forbidden),
            },
        });
        assert!(state.profile.as_ref().unwrap().loading);
        state.command_rejected(Command::Profile {
            user: Id(2),
            guild: None,
            request: new,
        });
        assert!(!state.profile.as_ref().unwrap().loading);
        assert!(state.profile.as_ref().unwrap().error.is_some());
        assert!(matches!(state.clear_profile(), Command::CancelProfile));
        state.apply(Envelope {
            generation: state.generation,
            event: Event::Profile {
                user: Id(2),
                guild: None,
                request: new,
                result: Err(Failure::Network),
            },
        });
        assert!(state.profile.is_none());
        state.demo = true;
        assert!(state.request_profile(Id(1), None).is_none());
    }

    #[test]
    fn viewed_profiles_are_reused_until_they_expire_or_the_session_changes() {
        fn profile(user: Id) -> Box<UserProfile> {
            Box::new(UserProfile {
                user: model::User {
                    id: user,
                    name: "Synthetic".into(),
                    avatar: None,
                    discriminator: 0,
                },
                username: "synthetic".into(),
                global_name: None,
                banner: None,
                accent_color: None,
                bio: String::new(),
                pronouns: String::new(),
                badges: vec![],
                connections: vec![],
                mutual_guilds: vec![],
                guild: None,
                theme_colors: None,
                clan: None,
                limited: false,
            })
        }
        let mut state = State::default();
        state.auth = AuthState::Authenticated;
        state.gateway_connected = true;
        state.guilds.push(model::Guild {
            emojis: None,
            id: Id(9),
            name: "Synthetic".into(),
            icon: None,
        });
        let Some(Command::Profile { request, .. }) = state.request_profile(Id(1), None) else {
            panic!("missing request")
        };
        state.apply(Envelope {
            generation: state.generation,
            event: Event::Profile {
                user: Id(1),
                guild: None,
                request,
                result: Ok(profile(Id(1))),
            },
        });
        assert_eq!(state.profile_cache.len(), 1);
        state.clear_profile();
        let now = Instant::now();
        // Reopening within the TTL is served from RAM without a command.
        assert!(state.request_profile_at(Id(1), None, now).is_none());
        let view = state.profile.as_ref().unwrap();
        assert!(!view.loading && view.error.is_none() && view.data.is_some());
        // A different server scope is a different profile.
        assert!(state.request_profile_at(Id(1), Some(Id(9)), now).is_some());
        // Expired entries are requested again.
        assert!(
            state
                .request_profile_at(Id(1), None, now + CACHE_TTL + Duration::from_secs(1))
                .is_some()
        );
        assert!(state.profile_cache.is_empty());
        // Bounded by entries and bytes; session invalidation clears everything.
        for id in 1..=(CACHE_ENTRIES as u64 + 8) {
            state
                .profile_cache
                .insert(Id(id), None, *profile(Id(id)), now);
        }
        assert_eq!(state.profile_cache.len(), CACHE_ENTRIES);
        assert!(state.profile_cache.bytes() <= CACHE_BYTES);
        state.apply(Envelope {
            generation: state.generation,
            event: Event::PermissionsChanged,
        });
        assert!(state.profile_cache.is_empty());
    }
}
