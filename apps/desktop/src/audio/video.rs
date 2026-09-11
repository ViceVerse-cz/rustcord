//! Native decoding stays on the existing media worker; only bounded ready frames reach egui.
use super::*;
use platform::video::{Decoder, Sample};
use std::{collections::VecDeque, time::Instant};

const MAX_FRAMES: usize = 8;
const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;
const INTERLEAVING: &str = "Video exceeds the playback buffer; download to play externally";

pub(super) fn play(
	request: &Request,
	gate: &Arc<Gate>,
	wake: &Arc<Notify>,
	runtime: &tokio::runtime::Handle,
	publish: &impl Fn(Status),
	frame: &impl Fn(eframe::egui::ColorImage),
) -> Result<(), &'static str> {
	let bytes = if let Some(url) = &request.url {
		runtime.block_on(fetch(
			url.clone(),
			request.expected,
			gate,
			request.generation,
			wake,
		))?
	} else {
		#[cfg(not(feature = "demo"))]
		return Err("Synthetic video is unavailable in this build");
		#[cfg(feature = "demo")]
		include_bytes!("../../tests/fixtures/video-bars.mp4").to_vec()
	};
	if !gate.current(request.generation) {
		return Ok(());
	}
	let mut decoder = Decoder::new(bytes)?;
	let mut target = Duration::ZERO;
	loop {
		if segment(
			&mut decoder,
			request,
			gate,
			wake,
			runtime,
			target,
			publish,
			frame,
		)?
		.is_some()
		{
			// The old output stream is dropped before clearing the callback's seek gate.
			let seek = gate.seek_millis.swap(NO_SEEK, Ordering::AcqRel);
			target = Duration::from_millis(seek).min(decoder.metadata().duration);
			decoder.seek(target)?;
		} else {
			return Ok(());
		}
	}
}

#[allow(clippy::too_many_arguments)] // One bounded decoder segment ends on cancellation, seek or EOF.
fn segment(
	decoder: &mut Decoder,
	request: &Request,
	gate: &Arc<Gate>,
	wake: &Arc<Notify>,
	runtime: &tokio::runtime::Handle,
	target: Duration,
	publish: &impl Fn(Status),
	frame: &impl Fn(eframe::egui::ColorImage),
) -> Result<Option<Duration>, &'static str> {
	let metadata = decoder.metadata();
	let eof = Arc::new(AtomicBool::new(false));
	let finished = Arc::new(AtomicBool::new(false));
	gate.failed.store(false, Ordering::Release);
	gate.buffering.store(true, Ordering::Release);
	gate.sample_rate.store(0, Ordering::Release);
	let mut output = metadata
		.audio
		.map(|audio| {
			streaming::video_output(
				audio.sample_rate,
				target,
				eof.clone(),
				finished.clone(),
				gate.clone(),
				request.generation,
			)
		})
		.transpose()?;
	let mut video = VecDeque::<(Duration, Vec<u8>)>::new();
	let frame_bytes = metadata.width as usize * metadata.height as usize * 4;
	let frame_limit = MAX_FRAMES.min(MAX_FRAME_BYTES / frame_bytes.max(1)).max(1);
	let mut pending_audio: Option<(Vec<f32>, usize, u64)> = None;
	let mut audio_position = metadata
		.audio
		.map_or(0, |audio| sample_index(target, audio.sample_rate));
	let mut position = target;
	let mut last_tick = Instant::now();
	let mut last_publish = Instant::now() - Duration::from_secs(1);
	let mut last_progress = Instant::now();
	let mut preview = true;
	let mut preroll = None;
	let mut decoder_ended = false;
	let wait = || {
		runtime.block_on(async {
		tokio::select! { _ = wake.notified() => {}, _ = tokio::time::sleep(Duration::from_millis(5)) => {} }
	})
	};
	loop {
		if !gate.current(request.generation) {
			return Ok(None);
		}
		let seek = gate.seek_millis.load(Ordering::Acquire);
		if seek != NO_SEEK {
			return Ok(Some(Duration::from_millis(seek)));
		}
		if gate.failed.load(Ordering::Acquire) {
			return Err("Audio output disconnected");
		}
		let now = Instant::now();
		let paused = gate.paused.load(Ordering::Acquire);
		let elapsed = now.duration_since(last_tick);
		last_tick = now;
		let previous = position;
		if !paused {
			if let Some(audio) = metadata.audio.filter(|_| !finished.load(Ordering::Acquire)) {
				position = Duration::from_secs_f64(
					gate.position_frames.load(Ordering::Acquire) as f64
						/ f64::from(audio.sample_rate),
				)
				.max(target);
			} else {
				gate.sample_rate.store(0, Ordering::Release);
				position = (position + elapsed).min(metadata.duration);
				gate.buffering.store(false, Ordering::Release);
			}
		}
		if position != previous || paused {
			last_progress = now;
		}
		let mut ready = None;
		while video
			.front()
			.is_some_and(|(timestamp, _)| *timestamp <= position)
		{
			ready = video.pop_front();
		}
		if preview
			&& ready.is_none()
			&& let Some((_, rgba)) = video.pop_front()
		{
			ready = Some((position, rgba));
		}
		if let Some((_, rgba)) = ready {
			frame(eframe::egui::ColorImage::from_rgba_unmultiplied(
				[metadata.width as usize, metadata.height as usize],
				&rgba,
			));
			preview = false;
		}
		if last_publish.elapsed() >= Duration::from_millis(100) {
			publish(Status {
				state: if paused {
					State::Paused
				} else if gate.buffering.load(Ordering::Acquire) {
					State::Loading
				} else {
					State::Playing
				},
				position,
				duration: metadata.duration,
			});
			last_publish = now;
		}
		if paused && !preview {
			runtime.block_on(wake.notified());
			last_tick = Instant::now();
			continue;
		}
		if let Some((samples, offset, timestamp)) = &mut pending_audio {
			let audio = metadata.audio.ok_or("Unexpected video audio track")?;
			let channels = usize::from(audio.channels);
			let (producer, _) = output.as_mut().ok_or("Video audio output missing")?;
			while producer.slots() > 0 && *offset < samples.len() {
				let at = timestamp.saturating_add((*offset / channels) as u64);
				if at < audio_position {
					*offset += channels;
					continue;
				}
				let next = if at > audio_position {
					[0.0; 2]
				} else {
					let clean = |sample: f32| {
						if sample.is_finite() {
							sample.clamp(-1.0, 1.0)
						} else {
							0.0
						}
					};
					let next = [
						clean(samples[*offset]),
						clean(samples[*offset + channels - 1]),
					];
					*offset += channels;
					next
				};
				producer.push(next).map_err(|_| INTERLEAVING)?;
				audio_position += 1;
			}
			if *offset == samples.len() {
				pending_audio = None;
			}
		}
		if decoder_ended
			&& video.is_empty()
			&& (output.is_none() || finished.load(Ordering::Acquire))
			&& position >= metadata.duration
		{
			publish(Status {
				state: State::Ended,
				position: metadata.duration,
				duration: metadata.duration,
			});
			return Ok(None);
		}
		if video.len() >= frame_limit || pending_audio.is_some() || decoder_ended {
			// ponytail: 8 frames / 32 MiB handles ordinary interleaving; use indexed per-track reads if this bound is too small.
			if !paused && last_progress.elapsed() > Duration::from_secs(5) {
				return Err(INTERLEAVING);
			}
			wait();
			continue;
		}
		match decoder.next()? {
			Some(Sample::Video { rgba, timestamp }) => {
				if rgba.len() != frame_bytes {
					return Err("Invalid decoded video frame");
				}
				if timestamp >= target {
					if let Some(previous) = preroll.take() {
						video.push_back((target, previous));
					}
					video.push_back((timestamp, rgba));
				} else {
					preroll = Some(rgba);
				}
			}
			Some(Sample::Audio { samples, timestamp }) => {
				let audio = metadata.audio.ok_or("Unexpected video audio track")?;
				if samples.len() % usize::from(audio.channels) != 0 {
					return Err("Invalid decoded audio frame");
				}
				pending_audio = Some((samples, 0, sample_index(timestamp, audio.sample_rate)));
			}
			None => {
				if let Some(previous) = preroll.take() {
					video.push_back((target, previous));
				}
				decoder_ended = true;
				eof.store(true, Ordering::Release);
			}
		}
		if decoder.audio_ended() {
			eof.store(true, Ordering::Release);
		}
	}
}

fn sample_index(position: Duration, rate: u32) -> u64 {
	(position.as_secs_f64() * f64::from(rate)).round() as u64
}

#[cfg(all(debug_assertions, feature = "demo", target_os = "windows"))]
pub(super) fn debug_clip_end(bytes: &[u8], runtime: &tokio::runtime::Handle) {
	let mut decoder = Decoder::new(bytes.to_vec()).unwrap();
	let duration = decoder.metadata().duration;
	let request = Request {
		generation: 0,
		url: None,
		expected: bytes.len(),
		video: true,
		voice_message: false,
		duration,
	};
	let gate = Arc::new(Gate::default());
	let wake = Arc::new(Notify::new());
	let pause = if decoder.metadata().audio.is_none() {
		let (gate, wake) = (gate.clone(), wake.clone());
		Some(std::thread::spawn(move || {
			std::thread::sleep(Duration::from_millis(350));
			gate.paused.store(true, Ordering::Release);
			wake.notify_one();
			std::thread::sleep(Duration::from_millis(500));
			gate.paused.store(false, Ordering::Release);
			wake.notify_one();
		}))
	} else {
		None
	};
	let ended = std::cell::Cell::new(false);
	let count = std::cell::Cell::new(0);
	let start = Instant::now();
	segment(
		&mut decoder,
		&request,
		&gate,
		&wake,
		runtime,
		Duration::ZERO,
		&|status| {
			assert!(
				start.elapsed() < Duration::from_secs(10),
				"video tail stalled"
			);
			ended.set(status.state == State::Ended);
		},
		&|_| count.set(count.get() + 1),
	)
	.unwrap();
	let paused_duration = if let Some(pause) = pause {
		pause.join().unwrap();
		Duration::from_millis(500)
	} else {
		Duration::ZERO
	};
	assert!(ended.get() && count.get() > 1);
	assert!(
		start.elapsed() >= (duration + paused_duration).saturating_sub(Duration::from_millis(100)),
		"video tail ended early"
	);
}
