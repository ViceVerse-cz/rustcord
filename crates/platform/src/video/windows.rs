//! Audited Media Foundation boundary: COM objects and locked buffers never leave this thread.
use super::{AudioFormat, MAX_DURATION, MAX_ENCODED_BYTES, Metadata, Sample};
use ::windows::{
	Win32::{
		Media::MediaFoundation::*,
		System::Com::{
			COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize, StructuredStorage::PROPVARIANT,
		},
		UI::Shell::SHCreateMemStream,
	},
	core::GUID,
};
use std::{marker::PhantomData, rc::Rc, time::Duration};

const INVALID: &str = "Unsupported or damaged video; download to play externally";
const LIMIT: &str = "Video preview limit: 100 MiB, 1080p, 10 minutes";
const MAX_VIDEO_BYTES: usize = 1920 * 1080 * 4;
const MAX_AUDIO_BYTES: usize = 192_000 * 2 * 4;

// Fields drop in declaration order: release the reader before MF and COM shutdown.
pub struct Decoder {
	reader: IMFSourceReader,
	metadata: Metadata,
	video: u32,
	audio: Option<u32>,
	stride: i32,
	video_ended: bool,
	audio_ended: bool,
	frames_decoded: u32,
	samples_decoded: u32,
	_session: Session,
}

struct Session {
	// COM initialization and shutdown must happen on the same worker thread.
	_thread: PhantomData<Rc<()>>,
}
impl Session {
	fn new() -> Result<Self, &'static str> {
		// SAFETY: This thread exclusively owns the apartment and all decoder objects.
		unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }
			.ok()
			.map_err(|_| INVALID)?;
		if unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.is_err() {
			unsafe { CoUninitialize() };
			return Err("Windows media components unavailable");
		}
		Ok(Self {
			_thread: PhantomData,
		})
	}
}
impl Drop for Session {
	fn drop(&mut self) {
		// SAFETY: Reader and its samples are released before this same-thread guard.
		unsafe {
			let _ = MFShutdown();
			CoUninitialize();
		}
	}
}

impl Decoder {
	pub fn new(bytes: Vec<u8>) -> Result<Self, &'static str> {
		if bytes.is_empty() || bytes.len() > MAX_ENCODED_BYTES {
			return Err(LIMIT);
		}
		let session = Session::new()?;
		// SAFETY: SHCreateMemStream copies the bounded input. MF retains reference-counted
		// streams; no Rust pointers or remote URLs are supplied to its source resolver.
		let reader = unsafe {
			let stream = SHCreateMemStream(Some(&bytes)).ok_or(INVALID)?;
			drop(bytes);
			let source = MFCreateMFByteStreamOnStream(&stream).map_err(|_| INVALID)?;
			let mut attributes = None;
			MFCreateAttributes(&mut attributes, 1).map_err(|_| INVALID)?;
			let attributes = attributes.ok_or(INVALID)?;
			attributes
				.SetUINT32(&MF_SOURCE_READER_ENABLE_ADVANCED_VIDEO_PROCESSING, 1)
				.map_err(|_| INVALID)?;
			MFCreateSourceReaderFromByteStream(&source, &attributes).map_err(|_| INVALID)?
		};
		// SAFETY: All COM interfaces remain owned and used on this worker.
		unsafe {
			reader
				.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)
				.map_err(|_| INVALID)?;
			let mut video = None;
			let mut audio = None;
			for index in 0..32 {
				let native = match reader.GetNativeMediaType(index, 0) {
					Ok(native) => native,
					Err(error) if error.code() == MF_E_INVALIDSTREAMNUMBER => break,
					Err(_) => return Err(INVALID),
				};
				match native.GetGUID(&MF_MT_MAJOR_TYPE).map_err(|_| INVALID)? {
					kind if kind == MFMediaType_Video && video.is_none() => {
						if native.GetUINT32(&MF_MT_VIDEO_ROTATION).unwrap_or(0) != 0 {
							return Err(
								"Rotated video is not supported yet; download to play externally",
							);
						}
						let size = native.GetUINT64(&MF_MT_FRAME_SIZE).map_err(|_| INVALID)?;
						check_dimensions((size >> 32) as u32, size as u32)?;
						video = Some(index);
					}
					kind if kind == MFMediaType_Audio && audio.is_none() => audio = Some(index),
					_ => {}
				}
			}
			let video = video.ok_or(INVALID)?;
			let duration = reader
				.GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION)
				.map_err(|_| INVALID)?;
			let duration = timestamp(u64::try_from(&duration).map_err(|_| INVALID)?)?;
			if duration.is_zero() {
				return Err(INVALID);
			}
			configure(&reader, video, MFMediaType_Video, MFVideoFormat_RGB32)?;
			let video_type = reader.GetCurrentMediaType(video).map_err(|_| INVALID)?;
			let size = video_type
				.GetUINT64(&MF_MT_FRAME_SIZE)
				.map_err(|_| INVALID)?;
			let (width, height) = ((size >> 32) as u32, size as u32);
			check_dimensions(width, height)?;
			let stride = video_type
				.GetUINT32(&MF_MT_DEFAULT_STRIDE)
				.map(|v| v as i32)
				.or_else(|_| MFGetStrideForBitmapInfoHeader(MFVideoFormat_RGB32.data1, width))
				.map_err(|_| INVALID)?;
			let audio_format = if let Some(index) = audio {
				configure(&reader, index, MFMediaType_Audio, MFAudioFormat_Float)?;
				let format = reader.GetCurrentMediaType(index).map_err(|_| INVALID)?;
				let channels = format
					.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)
					.map_err(|_| INVALID)?;
				let sample_rate = format
					.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)
					.map_err(|_| INVALID)?;
				let bits = format
					.GetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE)
					.map_err(|_| INVALID)?;
				if !(1..=2).contains(&channels)
					|| !(8000..=192_000).contains(&sample_rate)
					|| bits != 32
				{
					return Err("Unsupported video audio format; download to play externally");
				}
				Some(AudioFormat {
					channels: channels as u16,
					sample_rate,
				})
			} else {
				None
			};
			Ok(Self {
				reader,
				metadata: Metadata {
					width,
					height,
					duration,
					audio: audio_format,
				},
				video,
				audio,
				stride,
				video_ended: false,
				audio_ended: audio.is_none(),
				frames_decoded: 0,
				samples_decoded: 0,
				_session: session,
			})
		}
	}

	pub fn metadata(&self) -> Metadata {
		self.metadata
	}

	pub fn audio_ended(&self) -> bool {
		self.audio_ended
	}

	#[allow(clippy::should_implement_trait)]
	pub fn next(&mut self) -> Result<Option<Sample>, &'static str> {
		// Null samples and ticks are legal, but a malformed source cannot spin forever.
		for _ in 0..1024 {
			if self.video_ended && self.audio_ended {
				return Ok(None);
			}
			let (mut index, mut flags, mut time, mut sample) = (0, 0, 0, None);
			let requested = if self.video_ended {
				self.audio.ok_or(INVALID)?
			} else if self.audio_ended {
				self.video
			} else {
				MF_SOURCE_READER_ANY_STREAM.0 as u32
			};
			// SAFETY: Out parameters are valid for the synchronous call and interfaces stay local.
			unsafe {
				self.reader.ReadSample(
					requested,
					0,
					Some(&mut index),
					Some(&mut flags),
					Some(&mut time),
					Some(&mut sample),
				)
			}
			.map_err(|_| INVALID)?;
			let changed = MF_SOURCE_READERF_ERROR.0
				| MF_SOURCE_READERF_NEWSTREAM.0
				| MF_SOURCE_READERF_NATIVEMEDIATYPECHANGED.0;
			if flags & changed as u32 != 0 {
				return Err(INVALID);
			}
			if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
				self.validate_output(index)?;
			}
			if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
				if index == self.video {
					self.video_ended = true;
				}
				if Some(index) == self.audio {
					self.audio_ended = true;
				}
			}
			let Some(sample) = sample else {
				continue;
			};
			// These lifetime budgets also bound repeated seeking and dishonest timestamps.
			self.samples_decoded = self.samples_decoded.saturating_add(1);
			if index == self.video {
				self.frames_decoded = self.frames_decoded.saturating_add(1);
			}
			if self.samples_decoded > 360_000 || self.frames_decoded > 72_000 {
				return Err("Video playback limit reached; reopen the clip");
			}
			let time = timestamp(u64::try_from(time).map_err(|_| INVALID)?)?;
			// SAFETY: The sample owns the resulting buffer for its entire locked lifetime.
			let buffer = unsafe { sample.ConvertToContiguousBuffer() }.map_err(|_| INVALID)?;
			let limit = if index == self.video {
				MAX_VIDEO_BYTES + 64 * 1024
			} else {
				MAX_AUDIO_BYTES
			};
			let locked = Locked::new(buffer, limit)?;
			if index == self.video {
				return Ok(Some(Sample::Video {
					rgba: rgba(
						locked.bytes(),
						self.metadata.width,
						self.metadata.height,
						self.stride,
					)?,
					timestamp: time,
				}));
			}
			if Some(index) == self.audio {
				let format = self.metadata.audio.ok_or(INVALID)?;
				let bytes = locked.bytes();
				if !bytes.len().is_multiple_of(usize::from(format.channels) * 4) {
					return Err(INVALID);
				}
				let samples = bytes
					.as_chunks::<4>()
					.0
					.iter()
					.map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
					.collect::<Vec<_>>();
				if samples.iter().any(|value| !value.is_finite()) {
					return Err(INVALID);
				}
				return Ok(Some(Sample::Audio {
					samples,
					timestamp: time,
				}));
			}
			return Err(INVALID);
		}
		Err(INVALID)
	}

	fn validate_output(&mut self, index: u32) -> Result<(), &'static str> {
		// SAFETY: A decoder may finalize stride on its first frame. Revalidate the full
		// decoded format before accepting that frame; dimensions and audio layout stay fixed.
		unsafe {
			let format = self
				.reader
				.GetCurrentMediaType(index)
				.map_err(|_| INVALID)?;
			if index == self.video {
				let size = format.GetUINT64(&MF_MT_FRAME_SIZE).map_err(|_| INVALID)?;
				if (size >> 32) as u32 != self.metadata.width
					|| size as u32 != self.metadata.height
					|| format.GetGUID(&MF_MT_SUBTYPE).map_err(|_| INVALID)? != MFVideoFormat_RGB32
				{
					return Err(INVALID);
				}
				self.stride = format
					.GetUINT32(&MF_MT_DEFAULT_STRIDE)
					.map(|v| v as i32)
					.or_else(|_| {
						MFGetStrideForBitmapInfoHeader(
							MFVideoFormat_RGB32.data1,
							self.metadata.width,
						)
					})
					.map_err(|_| INVALID)?;
			} else if Some(index) == self.audio {
				let audio = self.metadata.audio.ok_or(INVALID)?;
				if format.GetGUID(&MF_MT_SUBTYPE).map_err(|_| INVALID)? != MFAudioFormat_Float
					|| format
						.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)
						.map_err(|_| INVALID)?
						!= u32::from(audio.channels)
					|| format
						.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)
						.map_err(|_| INVALID)?
						!= audio.sample_rate
					|| format
						.GetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE)
						.map_err(|_| INVALID)?
						!= 32
				{
					return Err(INVALID);
				}
			} else {
				return Err(INVALID);
			}
		}
		Ok(())
	}

	pub fn seek(&mut self, position: Duration) -> Result<(), &'static str> {
		if position > self.metadata.duration {
			return Err("Video seek is outside the clip");
		}
		let value = PROPVARIANT::from((position.as_nanos() / 100) as i64);
		// SAFETY: PROPVARIANT remains live for this synchronous call. Caller clears queued media.
		unsafe {
			self.reader
				.SetStreamSelection(self.video, true)
				.map_err(|_| INVALID)?;
			if let Some(audio) = self.audio {
				self.reader
					.SetStreamSelection(audio, true)
					.map_err(|_| INVALID)?;
			}
			self.reader
				.SetCurrentPosition(&GUID::zeroed(), &value)
				.map_err(|_| "Video seeking unavailable")?;
		}
		self.video_ended = false;
		self.audio_ended = self.audio.is_none();
		Ok(())
	}
}

unsafe fn configure(
	reader: &IMFSourceReader,
	index: u32,
	major: GUID,
	subtype: GUID,
) -> Result<(), &'static str> {
	// SAFETY: Parameters are owned, initialized COM objects and fixed known format GUIDs.
	unsafe {
		let format = MFCreateMediaType().map_err(|_| INVALID)?;
		format
			.SetGUID(&MF_MT_MAJOR_TYPE, &major)
			.map_err(|_| INVALID)?;
		format
			.SetGUID(&MF_MT_SUBTYPE, &subtype)
			.map_err(|_| INVALID)?;
		reader
			.SetCurrentMediaType(index, None, &format)
			.map_err(|_| INVALID)?;
		reader.SetStreamSelection(index, true).map_err(|_| INVALID)
	}
}

struct Locked {
	buffer: IMFMediaBuffer,
	pointer: *mut u8,
	length: usize,
}
impl Locked {
	fn new(buffer: IMFMediaBuffer, limit: usize) -> Result<Self, &'static str> {
		let length = unsafe { buffer.GetCurrentLength() }.map_err(|_| INVALID)? as usize;
		if length == 0 || length > limit {
			return Err(LIMIT);
		}
		let (mut pointer, mut maximum, mut actual) = (std::ptr::null_mut(), 0, 0);
		// SAFETY: Valid output pointers; guard always unlocks, including validation failures.
		unsafe { buffer.Lock(&mut pointer, Some(&mut maximum), Some(&mut actual)) }
			.map_err(|_| INVALID)?;
		let locked = Self {
			buffer,
			pointer,
			length: actual as usize,
		};
		if pointer.is_null() || actual > maximum || actual as usize > limit || actual == 0 {
			return Err(INVALID);
		}
		Ok(locked)
	}
	fn bytes(&self) -> &[u8] {
		// SAFETY: MF owns this valid, checked-length region until this guard unlocks it.
		unsafe { std::slice::from_raw_parts(self.pointer, self.length) }
	}
}
impl Drop for Locked {
	fn drop(&mut self) {
		unsafe {
			let _ = self.buffer.Unlock();
		}
	}
}

fn check_dimensions(width: u32, height: u32) -> Result<(), &'static str> {
	if width == 0 || height == 0 || width.max(height) > 1920 || width.min(height) > 1080 {
		Err(LIMIT)
	} else {
		Ok(())
	}
}
fn timestamp(ticks: u64) -> Result<Duration, &'static str> {
	if ticks > MAX_DURATION.as_secs() * 10_000_000 {
		return Err(LIMIT);
	}
	Ok(Duration::from_nanos(ticks * 100))
}
fn rgba(bytes: &[u8], width: u32, height: u32, stride: i32) -> Result<Vec<u8>, &'static str> {
	check_dimensions(width, height)?;
	let row = width as usize * 4;
	let pitch = stride.unsigned_abs() as usize;
	if pitch < row
		|| pitch
			.checked_mul(height as usize)
			.is_none_or(|len| len > bytes.len())
	{
		return Err(INVALID);
	}
	let mut output = Vec::with_capacity(row * height as usize);
	for y in 0..height as usize {
		let offset = if stride < 0 {
			height as usize - 1 - y
		} else {
			y
		} * pitch;
		for pixel in bytes[offset..offset + row].as_chunks::<4>().0 {
			output.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
		}
	}
	Ok(output)
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn decodes_b_frames_aac_and_seeks_synthetic_mp4() {
		let bytes = include_bytes!("../../../../apps/desktop/tests/fixtures/video-bars.mp4");
		let mut decoder = Decoder::new(bytes.to_vec()).expect("Windows MP4 decoder");
		let metadata = decoder.metadata();
		assert_eq!((metadata.width, metadata.height), (320, 180));
		assert_eq!(
			metadata.audio,
			Some(AudioFormat {
				channels: 1,
				sample_rate: 48_000
			})
		);
		assert!((2.9..=3.1).contains(&metadata.duration.as_secs_f64()));
		let (mut videos, mut samples, mut previous) = (0, 0, Duration::ZERO);
		while let Some(sample) = decoder.next().unwrap_or_else(|error| {
			panic!(
				"{error}: video frames={videos}, audio samples={samples}, timestamp={previous:?}"
			)
		}) {
			match sample {
				Sample::Video { rgba, timestamp } => {
					assert_eq!(rgba.len(), 320 * 180 * 4);
					assert!(
						timestamp >= previous,
						"B-frames must be returned in presentation order"
					);
					previous = timestamp;
					videos += 1;
				}
				Sample::Audio { samples: pcm, .. } => samples += pcm.len(),
			}
			assert!(videos < 100 && samples < 200_000);
		}
		assert_eq!(videos, 72);
		assert!(samples > 140_000);
		decoder.seek(Duration::from_secs(2)).unwrap();
		let mut reached = false;
		while let Some(sample) = decoder.next().unwrap() {
			if let Sample::Video { timestamp, .. } = sample
				&& timestamp >= Duration::from_secs(2)
			{
				reached = true;
				break;
			}
		}
		assert!(reached);
		decoder.seek(Duration::ZERO).unwrap();
		assert!(decoder.next().unwrap().is_some());
	}
	#[test]
	fn silent_and_short_audio_clips_expose_audio_end_before_video_end() {
		for (bytes, silent) in [
			(
				include_bytes!("../../../../apps/desktop/tests/fixtures/video-silent.mp4")
					.as_slice(),
				true,
			),
			(
				include_bytes!("../../../../apps/desktop/tests/fixtures/video-short-audio.mp4")
					.as_slice(),
				false,
			),
		] {
			let mut decoder = Decoder::new(bytes.to_vec()).unwrap();
			assert_eq!(decoder.audio_ended(), silent);
			assert_eq!(decoder.metadata().audio.is_none(), silent);
			let mut video_after_audio = 0;
			let mut videos = 0;
			while let Some(sample) = decoder.next().unwrap() {
				if let Sample::Video { timestamp, .. } = sample {
					videos += 1;
					if timestamp >= Duration::from_secs(2) && decoder.audio_ended() {
						video_after_audio += 1;
					}
				}
				assert!(videos < 100);
			}
			assert_eq!(videos, 72);
			assert!(video_after_audio >= 23);
			decoder.seek(Duration::ZERO).unwrap();
			assert_eq!(decoder.audio_ended(), silent);
		}
	}
	#[test]
	fn bounded_dimensions_timestamps_and_bgra_stride() {
		assert!(Decoder::new(Vec::new()).is_err());
		assert!(Decoder::new(vec![0; 64]).is_err());
		let rotated = include_bytes!("../../../../apps/desktop/tests/fixtures/video-rotated.mp4");
		assert!(matches!(
			Decoder::new(rotated.to_vec()),
			Err("Rotated video is not supported yet; download to play externally")
		));
		assert!(check_dimensions(1920, 1080).is_ok());
		assert!(check_dimensions(1080, 1920).is_ok());
		assert!(check_dimensions(1921, 1080).is_err());
		assert!(check_dimensions(0, 1).is_err());
		assert!(timestamp(u64::MAX).is_err());
		let pixels = [3, 2, 1, 0, 6, 5, 4, 0];
		assert_eq!(
			rgba(&pixels, 1, 2, 4).unwrap(),
			[1, 2, 3, 255, 4, 5, 6, 255]
		);
		assert_eq!(
			rgba(&pixels, 1, 2, -4).unwrap(),
			[4, 5, 6, 255, 1, 2, 3, 255]
		);
		assert!(rgba(&pixels[..4], 1, 2, 4).is_err());
		assert!(rgba(&pixels, 2, 1, 4).is_err());
	}
}
