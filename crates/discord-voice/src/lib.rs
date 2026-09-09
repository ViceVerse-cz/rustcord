//! One-to-one Discord DM media. No bot manager, relay, recording, or key persistence.
pub mod audio;
mod crypto;
mod jitter;
mod transport;
pub use transport::run;

pub type Frame = [f32; 960];
#[derive(Clone, Copy, Default)]
pub struct Controls {
    pub muted: bool,
    pub deafened: bool,
}
pub enum Status {
    TransportReady,
    Securing,
    Ready { privacy_code: String },
    RemoteAudio,
}

#[cfg(test)]
mod test_mls;

// Exercise the exact vendored SHAKE adapter, without enabling unused HPKE backends.
#[cfg(test)]
#[path = "../../../vendor/hpke-rs/src/serein_sha3.rs"]
mod hpke_sha3;
