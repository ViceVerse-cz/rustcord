use std::{fmt, future::Future};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthState {
    Unauthenticated,
    Authenticating,
    Challenged,
    Authenticated,
    Expired,
    Failed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Failure {
    #[error("Session expired; authenticated traffic stopped")]
    Expired,
    #[error("Verification challenge unsupported; complete it in an official flow")]
    Challenged,
    #[error("Permission denied")]
    Forbidden,
    #[error("Rate limited; wait for the service cooldown before retrying")]
    RateLimited,
    #[error("Connection failed; no automatic write retry")]
    Network,
    #[error("Outcome unknown; inspect the conversation before a deliberate retry")]
    Ambiguous,
    #[error("Service response rejected or incompatible")]
    Protocol,
    // Only compile-time labels are allowed here, never remote text, IDs or decode errors.
    #[error("{0}")]
    ProtocolAt(&'static str),
    #[error("Safe session capacity exceeded; connection stopped")]
    Capacity,
    #[error("Only an owner-supplied normal-user session is supported")]
    InvalidCredential,
}
impl Failure {
    pub fn protocol_at(self, stage: &'static str) -> Self {
        if self == Self::Protocol {
            Self::ProtocolAt(stage)
        } else {
            self
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Expired => "Session expired · reconnect explicitly; drafts remain in RAM",
            Self::Challenged => "Security challenge · unsupported here; connection stopped",
            Self::Forbidden => "Permission denied",
            Self::RateLimited => "Rate limited · wait before retrying",
            Self::Network => "Connection failed",
            Self::Ambiguous => "Outcome unknown · check the official client before retrying",
            Self::Protocol => "Unsupported service response",
            Self::ProtocolAt(stage) => stage,
            Self::Capacity => "Safe capacity exceeded · connection stopped",
            Self::InvalidCredential => "Invalid session input or bot account rejected",
        }
    }
    pub fn ends_session(self) -> bool {
        matches!(
            self,
            Self::Expired | Self::Challenged | Self::Capacity | Self::InvalidCredential
        )
    }
}
/// Intentionally neither Clone nor Serialize. Exposure is limited to transport adapters.
pub struct SessionSecret(Zeroizing<String>);
impl SessionSecret {
    pub fn from_owner_input(value: String) -> Result<Self, Failure> {
        let value = Zeroizing::new(value);
        if value.len() < 16
            || value.len() > 2048
            || value.starts_with("Bot ")
            || value.starts_with("Bearer ")
            || !value.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(Failure::InvalidCredential);
        }
        Ok(Self(value))
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for SessionSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionSecret([REDACTED])")
    }
}
pub trait AuthProvider {
    fn authenticate(&mut self) -> impl Future<Output = Result<model::User, Failure>> + Send;
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secret_is_redacted_and_header_injection_rejected() {
        let contextual = Failure::Protocol.protocol_at("Synthetic stage");
        assert_eq!(contextual.label(), "Synthetic stage");
        for failure in [
            Failure::Expired,
            Failure::Challenged,
            Failure::Forbidden,
            Failure::RateLimited,
            Failure::Network,
            Failure::Capacity,
            contextual,
        ] {
            assert_eq!(failure.protocol_at("Other stage"), failure);
        }
        let secret = SessionSecret::from_owner_input("SYNTHETIC_SECRET_MARKER".into()).unwrap();
        assert!(!format!("{secret:?}").contains("SYNTHETIC"));
        for value in [
            "Bot SYNTHETIC_SECRET",
            "Bearer SYNTHETIC_SECRET",
            "SYNTHETIC\r\nAuthorization: injected",
        ] {
            assert!(SessionSecret::from_owner_input(value.into()).is_err());
        }
    }
}
