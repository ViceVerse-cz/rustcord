//! Worker-owned AEC3. No devices, recording, or processing in audio callbacks.
use nnnoiseless::DenoiseState;
use sonora::{AudioProcessing, Config, StreamConfig, config::EchoCanceller};

pub struct Echo {
	processor: AudioProcessing,
	noise: Option<Box<DenoiseState<'static>>>,
}

impl Echo {
	pub fn new() -> Self {
		Self {
			processor: AudioProcessing::builder()
				.config(Config {
					echo_canceller: Some(EchoCanceller::default()),
					..Default::default()
				})
				.capture_config(StreamConfig::new(48_000, 1))
				.render_config(StreamConfig::new(48_000, 1))
				.build(),
			noise: None,
		}
	}

	/// Switches only the denoiser; echo cancellation keeps its adaptation and reference.
	pub fn set_noise_suppression(&mut self, enabled: bool) {
		if enabled && self.noise.is_none() {
			let mut noise = DenoiseState::new();
			// Discard RNNoise's first fade-in block without dropping a microphone frame.
			noise.process_frame(&mut [0.0; 480], &[0.0; 480]);
			self.noise = Some(noise);
		} else if !enabled {
			self.noise = None;
		}
	}

	pub fn render(&mut self, frame: &[f32; 960]) -> Result<(), &'static str> {
		let mut output = [0.0; 480];
		for chunk in frame.chunks_exact(480) {
			self.processor
				.process_render_f32(&[chunk], &mut [&mut output])
				.map_err(|_| "Echo cancellation could not process speaker audio")?;
		}
		Ok(())
	}

	pub fn capture(&mut self, frame: &mut [f32; 960]) -> Result<(), &'static str> {
		for chunk in frame.chunks_exact_mut(480) {
			let mut output = [0.0; 480];
			// AEC3 estimates the acoustic delay from the actual rendered reference.
			self.processor
				.set_stream_delay_ms(0)
				.map_err(|_| "Echo cancellation delay is invalid")?;
			self.processor
				.process_capture_f32(&[chunk], &mut [&mut output])
				.map_err(|_| "Echo cancellation could not process microphone audio")?;
			if let Some(noise) = &mut self.noise {
				// RNNoise expects 48 kHz mono float samples on the signed i16 scale.
				let input = output.map(|s| {
					if s.is_finite() {
						(s * 32768.0).clamp(-32768.0, 32767.0)
					} else {
						0.0
					}
				});
				noise.process_frame(&mut output, &input);
				for sample in &mut output {
					*sample = if sample.is_finite() {
						(*sample / 32768.0).clamp(-1.0, 1.0)
					} else {
						0.0
					};
				}
			}
			chunk.copy_from_slice(&output);
		}
		Ok(())
	}
}
