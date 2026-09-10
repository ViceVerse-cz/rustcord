//! A single explicitly requested profile; no directory or persistent profile cache.
use crate::{
    Command, State,
    auth::{AuthState, Failure},
};
use model::{Id, UserProfile};

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
        self.profile_request = self.profile_request.wrapping_add(1);
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
        result: Result<UserProfile, Failure>,
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
                view.data = Some(data);
                view.error = None;
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
}
