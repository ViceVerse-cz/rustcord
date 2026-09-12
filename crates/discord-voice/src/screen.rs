//! Explicitly selected, memory-only screen capture and H.264 encoding.
pub use client_core::screen::{Settings, Source, SourceId};
mod capture;

use openh264::{
	OpenH264API,
	encoder::{
		BitRate, Encoder, EncoderConfig, FrameRate, FrameType, IntraFramePeriod, RateControlMode,
		UsageType,
	},
	formats::{BgraSliceU8, YUVBuffer},
};
use std::{
	sync::{
		Arc, Mutex,
		atomic::{AtomicBool, Ordering},
		mpsc,
	},
	time::{Duration, Instant},
};

pub const MAX_RAW_BYTES: usize = 3840 * 2160 * 4;
pub const MAX_ENCODED_BYTES: usize = 2 * 1024 * 1024;

pub struct RawFrame {
	pub width: u32,
	pub height: u32,
	pub stride: usize,
	pub data: Vec<u8>,
}

pub struct EncodedFrame {
	pub data: Vec<u8>,
	pub timestamp: u32,
	pub keyframe: bool,
}

pub struct Video {
	pub settings: Settings,
	pub frames: tokio::sync::mpsc::Receiver<EncodedFrame>,
	pub ready: Arc<AtomicBool>,
	pub keyframe: Arc<AtomicBool>,
}

pub fn supported() -> bool {
	cfg!(any(target_os = "macos", target_os = "windows"))
}

pub fn sources() -> Result<Vec<Source>, &'static str> {
	capture::sources()
}

pub struct Worker {
	stop: Arc<AtomicBool>,
	ready: Arc<AtomicBool>,
	preview: Arc<Mutex<Option<image::RgbaImage>>>,
	done: Option<mpsc::Receiver<Result<(), &'static str>>>,
}

impl Worker {
	pub fn start(
		settings: Settings,
		wake: impl Fn() + Send + 'static,
	) -> Result<(Self, Video), &'static str> {
		if !settings.valid() || !supported() {
			return Err("Screen sharing is unavailable for these settings or this platform");
		}
		let stop = Arc::new(AtomicBool::new(false));
		let ready = Arc::new(AtomicBool::new(false));
		let keyframe = Arc::new(AtomicBool::new(true));
		let preview = Arc::new(Mutex::new(None));
		let worker_preview = preview.clone();
		let (send, frames) = tokio::sync::mpsc::channel(1);
		let (complete, done) = mpsc::sync_channel(1);
		let (worker_stop, worker_ready, worker_keyframe) =
			(stop.clone(), ready.clone(), keyframe.clone());
		std::thread::Builder::new()
			.name("screen-encoder".into())
			.spawn(move || {
				let result = encode_loop(
					settings,
					worker_stop,
					worker_ready,
					worker_keyframe,
					send,
					worker_preview,
					&wake,
				);
				let _ = complete.try_send(result);
				wake();
			})
			.map_err(|_| "Could not start screen capture worker")?;
		Ok((
			Self {
				stop,
				ready: ready.clone(),
				preview,
				done: Some(done),
			},
			Video {
				settings,
				frames,
				ready,
				keyframe,
			},
		))
	}

	pub fn result(&self) -> Option<Result<(), &'static str>> {
		self.done.as_ref()?.try_recv().ok()
	}

	/// One local RGBA image, bounded to 640 by 360 pixels and replaced at most ten times a second.
	pub fn take_preview(&self) -> Option<image::RgbaImage> {
		self.preview.try_lock().ok()?.take()
	}

	pub fn shutdown(mut self) -> mpsc::Receiver<Result<(), &'static str>> {
		self.done.take().expect("screen worker completion")
	}
}

impl Drop for Worker {
	fn drop(&mut self) {
		self.ready.store(false, Ordering::Release);
		self.stop.store(true, Ordering::Release);
	}
}

fn encode_loop(
	settings: Settings,
	stop: Arc<AtomicBool>,
	ready: Arc<AtomicBool>,
	keyframe: Arc<AtomicBool>,
	send: tokio::sync::mpsc::Sender<EncodedFrame>,
	preview: Arc<Mutex<Option<image::RgbaImage>>>,
	wake: &impl Fn(),
) -> Result<(), &'static str> {
	if stop.load(Ordering::Acquire) || send.is_closed() {
		return Ok(());
	}
	let origin = Instant::now();
	let (raw_send, raw) = mpsc::sync_channel(1);
	let capture_stop = Arc::new(AtomicBool::new(false));
	let _native = capture::Capture::start(settings, raw_send, capture_stop.clone())?;
	let mut encoding = None;
	let mut first_frame_deadline = Some(Instant::now() + Duration::from_secs(15));
	let mut next_frame = Instant::now();
	let mut next_preview = Instant::now();

	while !stop.load(Ordering::Acquire) && !send.is_closed() {
		if capture_stop.load(Ordering::Acquire) {
			return Err("The selected screen or window stopped sharing");
		}
		let frame = match raw.recv_timeout(Duration::from_millis(100)) {
			Ok(frame) => frame,
			Err(_) if stop.load(Ordering::Acquire) || send.is_closed() => break,
			Err(mpsc::RecvTimeoutError::Timeout)
				if first_frame_deadline.is_none_or(|deadline| Instant::now() < deadline) =>
			{
				continue;
			}
			Err(_) => {
				return Err(
					"No screen frames received; check screen recording permission and the selected source",
				);
			}
		};
		first_frame_deadline = None;
		if stop.load(Ordering::Acquire) || send.is_closed() {
			break;
		}
		let now = Instant::now();
		if now >= next_preview {
			let image = preview_frame(&frame)?;
			if let Ok(mut slot) = preview.try_lock() {
				*slot = Some(image);
			}
			next_preview = now + Duration::from_millis(100);
			wake();
		}
		// Local capture remains available while alone; only secure media is encoded or queued.
		if !ready.load(Ordering::Acquire) {
			encoding = None;
			keyframe.store(true, Ordering::Release);
			continue;
		}
		if now < next_frame {
			continue;
		}
		next_frame = now + Duration::from_secs_f64(1.0 / f64::from(settings.fps));
		if encoding.is_none() {
			encoding = Some((
				encoder(settings)?,
				YUVBuffer::new(settings.width as usize, settings.height as usize),
			));
		}
		let (encoder, yuv) = encoding.as_mut().expect("secure screen encoder");
		let pixels = fit_frame(frame, settings.width, settings.height)?;
		let force_keyframe = keyframe.swap(false, Ordering::AcqRel);
		let (data, is_keyframe) = encode_pixels(
			encoder,
			yuv,
			&pixels,
			(settings.width as usize, settings.height as usize),
			force_keyframe,
		)?;
		if force_keyframe && (data.is_empty() || !is_keyframe) {
			keyframe.store(true, Ordering::Release);
		}
		if data.is_empty() {
			continue;
		}
		if !ready.load(Ordering::Acquire) || stop.load(Ordering::Acquire) {
			continue;
		}
		let frame = EncodedFrame {
			data,
			timestamp: (origin.elapsed().as_micros() * 90 / 1000) as u32,
			keyframe: is_keyframe,
		};
		if send.try_send(frame).is_err() {
			keyframe.store(true, Ordering::Release);
		}
	}
	capture_stop.store(true, Ordering::Release);
	Ok(())
}

fn encoder(settings: Settings) -> Result<Encoder, &'static str> {
	let config = EncoderConfig::new()
		.bitrate(BitRate::from_bps(settings.bit_rate()))
		.max_frame_rate(FrameRate::from_hz(settings.fps as f32))
		.usage_type(UsageType::ScreenContentRealTime)
		.rate_control_mode(RateControlMode::Bitrate)
		.num_threads(2)
		.intra_frame_period(IntraFramePeriod::from_num_frames(settings.fps * 2));
	Encoder::with_api_config(OpenH264API::from_source(), config)
		.map_err(|_| "Screen video encoder is unavailable")
}

fn encode_pixels(
	encoder: &mut Encoder,
	yuv: &mut YUVBuffer,
	pixels: &[u8],
	dimensions: (usize, usize),
	force_keyframe: bool,
) -> Result<(Vec<u8>, bool), &'static str> {
	yuv.read_bgra8(BgraSliceU8::new(pixels, dimensions));
	if force_keyframe {
		encoder.force_intra_frame();
	}
	let encoded = encoder
		.encode(yuv)
		.map_err(|_| "Screen video encoding failed")?;
	let is_keyframe = matches!(encoded.frame_type(), FrameType::IDR);
	let mut encoded_len = 0usize;
	for layer_index in 0..encoded.num_layers() {
		let layer = encoded
			.layer(layer_index)
			.ok_or("Screen video encoder returned an invalid layer")?;
		for nal_index in 0..layer.nal_count() {
			encoded_len = encoded_len
				.checked_add(
					layer
						.nal_unit(nal_index)
						.ok_or("Screen video encoder returned an invalid NAL")?
						.len(),
				)
				.filter(|length| *length <= MAX_ENCODED_BYTES)
				.ok_or("Encoded screen frame exceeds the sharing limit; choose a lower quality")?;
		}
	}
	let mut data = Vec::with_capacity(encoded_len);
	encoded.write_vec(&mut data);
	Ok((data, is_keyframe))
}

fn validate_frame(frame: &RawFrame) -> Result<(usize, usize), &'static str> {
	let row_bytes = (frame.width as usize)
		.checked_mul(4)
		.ok_or("Screen capture returned an unsupported frame size")?;
	let required = frame
		.stride
		.checked_mul(frame.height as usize)
		.ok_or("Screen capture returned an unsupported frame size")?;
	if frame.width == 0
		|| frame.height == 0
		|| frame.width > 3840
		|| frame.height > 2160
		|| frame.data.len() > MAX_RAW_BYTES
		|| frame.stride < row_bytes
		|| required > frame.data.len()
	{
		return Err("Screen capture returned an unsupported frame size");
	}

	Ok((row_bytes, required))
}

fn preview_frame(frame: &RawFrame) -> Result<image::RgbaImage, &'static str> {
	validate_frame(frame)?;
	let scale = (640.0 / f64::from(frame.width))
		.min(360.0 / f64::from(frame.height))
		.min(1.0);
	let width = (f64::from(frame.width) * scale).round().max(1.0) as u32;
	let height = (f64::from(frame.height) * scale).round().max(1.0) as u32;
	// ponytail: nearest sampling keeps the ten-fps preview cheap; use filtered scaling if needed.
	Ok(image::RgbaImage::from_fn(width, height, |x, y| {
		let source_x = (x * frame.width / width) as usize;
		let source_y = (y * frame.height / height) as usize;
		let offset = source_y * frame.stride + source_x * 4;
		image::Rgba([
			frame.data[offset + 2],
			frame.data[offset + 1],
			frame.data[offset],
			255,
		])
	}))
}

fn fit_frame(frame: RawFrame, width: u32, height: u32) -> Result<Vec<u8>, &'static str> {
	let (row_bytes, required) = validate_frame(&frame)?;

	let mut packed = if frame.stride == row_bytes {
		frame.data
	} else {
		let mut packed = Vec::with_capacity(row_bytes * frame.height as usize);
		for row in frame.data[..required].chunks_exact(frame.stride) {
			packed.extend_from_slice(&row[..row_bytes]);
		}
		packed
	};
	packed.truncate(row_bytes * frame.height as usize);
	if frame.width == width && frame.height == height {
		return Ok(packed);
	}
	let image = image::RgbaImage::from_raw(frame.width, frame.height, packed)
		.ok_or("Invalid screen frame")?;
	let scale = (f64::from(width) / f64::from(frame.width))
		.min(f64::from(height) / f64::from(frame.height));
	let (scaled_width, scaled_height) = (
		(f64::from(frame.width) * scale).round().max(1.0) as u32,
		(f64::from(frame.height) * scale).round().max(1.0) as u32,
	);
	let scaled = image::imageops::resize(
		&image,
		scaled_width,
		scaled_height,
		image::imageops::FilterType::Triangle,
	);
	let mut output = image::RgbaImage::new(width, height);
	image::imageops::replace(
		&mut output,
		&scaled,
		i64::from((width - scaled_width) / 2),
		i64::from((height - scaled_height) / 2),
	);
	Ok(output.into_raw())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn local_preview_is_bounded_and_converts_padded_bgra_without_media_readiness() {
		let preview = preview_frame(&RawFrame {
			width: 1,
			height: 2,
			stride: 8,
			data: vec![1, 2, 3, 0, 9, 9, 9, 9, 4, 5, 6, 0, 9, 9, 9, 9],
		})
		.unwrap();
		assert_eq!(preview.as_raw(), &[3, 2, 1, 255, 6, 5, 4, 255]);
		let worker = Worker {
			stop: Arc::new(AtomicBool::new(false)),
			ready: Arc::new(AtomicBool::new(false)),
			preview: Arc::new(Mutex::new(Some(preview))),
			done: None,
		};
		assert!(worker.take_preview().is_some());
		assert!(worker.take_preview().is_none());
		assert!(!worker.ready.load(Ordering::Acquire));
		for (width, height) in [(3840, 2160), (2, 2160), (3840, 2)] {
			let frame = RawFrame {
				width,
				height,
				stride: width as usize * 4,
				data: vec![0; width as usize * height as usize * 4],
			};
			let preview = preview_frame(&frame).unwrap();
			assert!(preview.width() > 0 && preview.width() <= 640);
			assert!(preview.height() > 0 && preview.height() <= 360);
			assert!(preview.as_raw().len() <= 640 * 360 * 4);
		}
		assert!(
			preview_frame(&RawFrame {
				width: 2,
				height: 2,
				stride: 4,
				data: vec![0; 8],
			})
			.is_err()
		);
	}

	#[test]
	fn synthetic_frame_is_bounded_and_encodes_a_keyframe() {
		let frame = RawFrame {
			width: 2,
			height: 2,
			stride: 12,
			data: vec![255; 24],
		};
		let pixels = fit_frame(frame, 1280, 720).unwrap();
		assert_eq!(pixels.len(), 1280 * 720 * 4);

		let settings = Settings {
			source: SourceId::Display(1),
			width: 1280,
			height: 720,
			fps: 30,
			cursor: true,
		};
		let mut encoder = encoder(settings).unwrap();
		let mut yuv = YUVBuffer::new(1280, 720);
		let (encoded, keyframe) =
			encode_pixels(&mut encoder, &mut yuv, &pixels, (1280, 720), true).unwrap();
		assert!(keyframe);
		assert!(!encoded.is_empty() && encoded.len() <= MAX_ENCODED_BYTES);
		assert!(encoded.windows(5).any(|nal| nal == [0, 0, 0, 1, 0x65]));

		assert!(
			fit_frame(
				RawFrame {
					width: 2,
					height: 2,
					stride: 4,
					data: vec![0; 8],
				},
				1280,
				720,
			)
			.is_err()
		);
	}
}
