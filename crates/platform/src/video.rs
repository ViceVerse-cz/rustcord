//! Memory-only attachment decoding. Create and use the decoder on a media worker.
use std::time::Duration;

pub const MAX_ENCODED_BYTES: usize = 100 * 1024 * 1024;
pub const MAX_DURATION: Duration = Duration::from_secs(600);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AudioFormat {
	pub channels: u16,
	pub sample_rate: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metadata {
	pub width: u32,
	pub height: u32,
	pub duration: Duration,
	pub audio: Option<AudioFormat>,
}

pub enum Sample {
	Video {
		rgba: Vec<u8>,
		timestamp: Duration,
	},
	Audio {
		samples: Vec<f32>,
		timestamp: Duration,
	},
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::Decoder;

#[cfg(not(target_os = "windows"))]
pub struct Decoder;
#[cfg(not(target_os = "windows"))]
impl Decoder {
	pub fn new(_bytes: Vec<u8>) -> Result<Self, &'static str> {
		Err("Video playback is available on Windows; download to play externally")
	}
	pub fn metadata(&self) -> Metadata {
		unreachable!("unsupported platforms cannot construct a decoder")
	}
	pub fn audio_ended(&self) -> bool {
		true
	}
	#[allow(clippy::should_implement_trait)]
	pub fn next(&mut self) -> Result<Option<Sample>, &'static str> {
		Ok(None)
	}
	pub fn seek(&mut self, _position: Duration) -> Result<(), &'static str> {
		Err("Video playback is unavailable on this platform")
	}
}
