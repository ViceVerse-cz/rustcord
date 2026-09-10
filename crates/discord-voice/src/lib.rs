//! Discord DM and guild voice media. No bot manager, relay, recording, or key persistence.
mod activity;
pub mod audio;
mod capture;
mod crypto;
mod jitter;
mod mixer;
mod transport;
pub use transport::run;

pub type Frame = [f32; 960];
#[derive(Clone, Copy, Default)]
pub struct Controls {
	pub muted: bool,
	pub deafened: bool,
}
pub enum Status {
	Connecting,
	Discovering,
	TransportReady,
	Securing,
	WaitingForPeer,
	Ready {
		privacy_code: String,
	},
	RemoteAudio,
	/// Latest active user IDs, zero-padded to the 64-participant limit.
	Speaking(Box<[u64; 64]>),
}

#[cfg(test)]
mod test_mls;

// Exercise the exact vendored SHAKE adapter, without enabling unused HPKE backends.
#[cfg(test)]
#[path = "../../../vendor/hpke-rs/src/serein_sha3.rs"]
mod hpke_sha3;
