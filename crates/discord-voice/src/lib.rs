//! Discord DM and guild voice media. No bot manager, relay, recording, or key persistence.
mod activity;
pub mod audio;
pub mod camera;
mod capture;
mod crypto;
mod diagnostics;
mod jitter;
mod mixer;
pub mod screen;
mod transport;
mod video;
pub use crypto::Identity;
pub use transport::{run, run_stream, run_with_identity};
pub mod camera_video;

pub type Frame = [f32; 960];
#[derive(Clone, Copy, Default)]
pub struct Controls {
	pub muted: bool,
	/// Zero means off; a new value invalidates frames from the previous camera instance.
	pub camera: u64,
	pub deafened: bool,
}
pub enum Status {
	Connecting,
	Discovering,
	TransportReady,
	CameraAvailable(bool),
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
