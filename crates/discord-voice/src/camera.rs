//! User-started, ephemeral camera capture. No device is opened before `start`.

use std::sync::{
	Arc, Mutex,
	atomic::{AtomicBool, Ordering},
};

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;
pub const MAX_ENCODED_BYTES: usize = 128 * 1024;

pub struct Frame {
	pub rgb: Vec<u8>,
	pub h264: Vec<u8>,
}

#[derive(Default)]
struct Shared {
	stopped: AtomicBool,
	active: AtomicBool,
	finished: AtomicBool,
	error: Mutex<Option<&'static str>>,
}

pub struct Camera {
	shared: Arc<Shared>,
}

impl Camera {
	/// Call only after an explicit camera-on gesture in a connected call.
	pub fn start(
		on_frame: Arc<dyn Fn(Frame) + Send + Sync>,
		wake: Arc<dyn Fn() + Send + Sync>,
	) -> Result<Self, &'static str> {
		#[cfg(target_os = "macos")]
		{
			macos::start(on_frame, wake)
		}
		#[cfg(not(target_os = "macos"))]
		{
			let _ = (on_frame, wake);
			Err("Camera capture is currently available on macOS only")
		}
	}

	pub fn stop(&self) {
		self.shared.stopped.store(true, Ordering::Release);
		self.shared.active.store(false, Ordering::Release);
	}

	pub fn stopped(&self) -> bool {
		self.shared.finished.load(Ordering::Acquire)
	}

	pub fn error(&self) -> Option<&'static str> {
		*self.shared.error.lock().ok()?
	}

	pub fn active(&self) -> bool {
		self.shared.active.load(Ordering::Acquire)
	}
}

impl Drop for Camera {
	fn drop(&mut self) {
		self.stop();
	}
}

#[cfg(target_os = "macos")]
mod macos {
	#![allow(unsafe_code)]

	use super::*;
	use block2::RcBlock;
	use dispatch2::{DispatchQueue, DispatchRetained};
	use objc2::{
		AnyThread, DefinedClass, define_class, msg_send,
		rc::{Retained, autoreleasepool},
		runtime::{AnyObject, Bool, NSObject, NSObjectProtocol, ProtocolObject},
	};
	use objc2_av_foundation::*;
	use objc2_core_media::CMSampleBuffer;
	use objc2_core_video::*;
	use objc2_foundation::{NSDictionary, NSNumber, NSString};
	use openh264::{
		OpenH264API,
		encoder::{BitRate, Encoder, EncoderConfig, FrameRate, Profile},
		formats::{RgbSliceU8, YUVBuffer},
	};
	use std::{
		sync::mpsc::{self, Receiver, SyncSender},
		thread,
		time::{Duration, Instant},
	};

	// One process-wide slot also covers asynchronous teardown: repeated clicks cannot
	// accumulate workers or open a second device while the previous session closes.
	static RUNNING: AtomicBool = AtomicBool::new(false);
	const DENIED: &str = "Camera access denied. Allow Serein (or your terminal) in System Settings > Privacy & Security > Camera, then try again.";
	const FRAME_INTERVAL: Duration = Duration::from_millis(67);

	struct DelegateState {
		send: SyncSender<Result<Vec<u8>, &'static str>>,
		shared: Arc<Shared>,
		last: Mutex<Instant>,
	}

	define_class!(
		// SAFETY: NSObject superclass, initialized Send + Sync ivars, and the
		// exact AVFoundation delegate signature. AVFoundation uses a serial queue.
		#[unsafe(super = NSObject)]
		#[ivars = DelegateState]
		struct SereinCameraDelegate;

		unsafe impl NSObjectProtocol for SereinCameraDelegate {}
		unsafe impl AVCaptureVideoDataOutputSampleBufferDelegate for SereinCameraDelegate {
			#[unsafe(method(captureOutput:didOutputSampleBuffer:fromConnection:))]
			fn capture(
				&self,
				_output: &AVCaptureOutput,
				sample: &CMSampleBuffer,
				_connection: &AVCaptureConnection,
			) {
				let state = self.ivars();
				if state.shared.stopped.load(Ordering::Acquire) {
					return;
				}
				let Ok(mut last) = state.last.try_lock() else {
					return;
				};
				if last.elapsed() < FRAME_INTERVAL {
					return;
				}
				*last = Instant::now();
				let _ = state.send.try_send(copy_bgra(sample));
			}
		}
	);

	fn copy_bgra(sample: &CMSampleBuffer) -> Result<Vec<u8>, &'static str> {
		// SAFETY: The sample is valid for this delegate invocation. Retain its image,
		// verify packed BGRA dimensions/stride before reading, and unlock every path.
		unsafe {
			let pixels = sample.image_buffer().ok_or("Camera returned no image")?;
			let stride = CVPixelBufferGetBytesPerRow(&pixels);
			let bytes = stride
				.checked_mul(HEIGHT)
				.ok_or("Camera frame exceeds bounds")?;
			if CVPixelBufferGetWidth(&pixels) != WIDTH
				|| CVPixelBufferGetHeight(&pixels) != HEIGHT
				|| CVPixelBufferGetPixelFormatType(&pixels) != kCVPixelFormatType_32BGRA
				|| !(WIDTH * 4..=WIDTH * 4 + 4096).contains(&stride)
				|| bytes > CVPixelBufferGetDataSize(&pixels)
			{
				return Err("Camera did not provide a bounded 640×480 BGRA frame");
			}
			if CVPixelBufferLockBaseAddress(&pixels, CVPixelBufferLockFlags::ReadOnly) != 0 {
				return Err("Camera image could not be read");
			}
			let base = CVPixelBufferGetBaseAddress(&pixels);
			let result = if base.is_null() {
				Err("Camera returned an empty image")
			} else {
				let source = std::slice::from_raw_parts(base.cast::<u8>(), bytes);
				let mut bgra = vec![0; WIDTH * HEIGHT * 4];
				for (row, dest) in source
					.chunks_exact(stride)
					.zip(bgra.as_chunks_mut::<{ WIDTH * 4 }>().0)
				{
					dest.copy_from_slice(&row[..WIDTH * 4]);
				}
				Ok(bgra)
			};
			CVPixelBufferUnlockBaseAddress(&pixels, CVPixelBufferLockFlags::ReadOnly);
			result
		}
	}

	pub(super) fn start(
		on_frame: Arc<dyn Fn(Frame) + Send + Sync>,
		wake: Arc<dyn Fn() + Send + Sync>,
	) -> Result<Camera, &'static str> {
		if RUNNING
			.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
			.is_err()
		{
			return Err("Previous camera session is still closing; try again shortly");
		}
		let shared = Arc::new(Shared::default());
		let worker = shared.clone();
		if thread::Builder::new()
			.name("serein-camera".into())
			.spawn(move || {
				let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
					autoreleasepool(|_| run(&worker, &on_frame, &wake))
				}))
				.unwrap_or(Err("Camera worker failed"));
				worker.active.store(false, Ordering::Release);
				if !worker.stopped.load(Ordering::Acquire)
					&& let Err(error) = result
					&& let Ok(mut slot) = worker.error.lock()
				{
					*slot = Some(error);
				}
				worker.finished.store(true, Ordering::Release);
				RUNNING.store(false, Ordering::Release);
				wake();
			})
			.is_err()
		{
			RUNNING.store(false, Ordering::Release);
			return Err("Camera worker could not start");
		}
		Ok(Camera { shared })
	}

	fn authorize(shared: &Shared) -> Result<(), &'static str> {
		// SAFETY: Framework-owned constant, class methods callable from this worker;
		// AVFoundation copies the block, which owns only a bounded result sender.
		let receive = unsafe {
			let media = AVMediaTypeVideo.ok_or("macOS camera authorization is unavailable")?;
			match AVCaptureDevice::authorizationStatusForMediaType(media) {
				AVAuthorizationStatus::Authorized => return Ok(()),
				AVAuthorizationStatus::NotDetermined => {
					let (send, receive) = mpsc::sync_channel(1);
					let block = RcBlock::new(move |granted: Bool| {
						let _ = send.try_send(granted.as_bool());
					});
					AVCaptureDevice::requestAccessForMediaType_completionHandler(media, &block);
					receive
				}
				_ => return Err(DENIED),
			}
		};
		let deadline = Instant::now() + Duration::from_secs(20);
		while !shared.stopped.load(Ordering::Acquire) && Instant::now() < deadline {
			match receive.recv_timeout(Duration::from_millis(100)) {
				Ok(true) => return Ok(()),
				Ok(false) => return Err(DENIED),
				Err(mpsc::RecvTimeoutError::Disconnected) => {
					return Err("Camera permission request failed");
				}
				Err(mpsc::RecvTimeoutError::Timeout) => {}
			}
		}
		Err("Camera permission canceled or timed out; respond to the macOS prompt and try again")
	}

	struct CaptureSession {
		session: Retained<AVCaptureSession>,
		output: Retained<AVCaptureVideoDataOutput>,
		_delegate: Retained<SereinCameraDelegate>,
		queue: DispatchRetained<DispatchQueue>,
	}
	impl Drop for CaptureSession {
		fn drop(&mut self) {
			// SAFETY: Owned session is configured, and teardown runs on its worker.
			unsafe {
				self.output.setSampleBufferDelegate_queue(None, None);
				self.session.stopRunning();
			}
			self.queue.exec_sync(|| {});
		}
	}

	fn run(
		shared: &Arc<Shared>,
		on_frame: &Arc<dyn Fn(Frame) + Send + Sync>,
		wake: &Arc<dyn Fn() + Send + Sync>,
	) -> Result<(), &'static str> {
		authorize(shared)?;
		if shared.stopped.load(Ordering::Acquire) {
			return Ok(());
		}
		let mut encoder = Encoder::with_api_config(
			OpenH264API::from_source(),
			EncoderConfig::new()
				.bitrate(BitRate::from_bps(600_000))
				.max_frame_rate(FrameRate::from_hz(15.0))
				.profile(Profile::Baseline)
				.num_threads(1)
				.debug(false),
		)
		.map_err(|_| "Camera H264 encoder could not start")?;
		let (send, receive) = mpsc::sync_channel(1);
		let queue = DispatchQueue::new("serein.camera.frames", None);
		// SAFETY: Only this worker configures/owns the session. Delegate lives until
		// capture is stopped and the serial callback queue has drained.
		let capture = unsafe {
			let device = AVCaptureDevice::defaultDeviceWithMediaType(
				AVMediaTypeVideo.ok_or("Camera media type unavailable")?,
			)
			.ok_or("No camera is available")?;
			let input = AVCaptureDeviceInput::deviceInputWithDevice_error(&device)
				.map_err(|_| "Camera is busy or unavailable")?;
			let session = AVCaptureSession::new();
			let output = AVCaptureVideoDataOutput::new();
			if !session.canAddInput(&input) || !session.canAddOutput(&output) {
				return Err("Camera cannot join capture session");
			}
			session.addInput(&input);
			session.addOutput(&output);
			if !session.canSetSessionPreset(AVCaptureSessionPreset640x480) {
				return Err("Camera does not support 640×480 capture");
			}
			session.setSessionPreset(AVCaptureSessionPreset640x480);
			let format = NSNumber::new_u32(kCVPixelFormatType_32BGRA);
			let key = NSString::from_str(&kCVPixelBufferPixelFormatTypeKey.to_string());
			let settings = NSDictionary::from_slices(&[&*key], &[&*format as &AnyObject]);
			output.setVideoSettings(Some(&settings));
			output.setAlwaysDiscardsLateVideoFrames(true);
			let allocated = SereinCameraDelegate::alloc().set_ivars(DelegateState {
				send,
				shared: shared.clone(),
				last: Mutex::new(Instant::now() - FRAME_INTERVAL),
			});
			let delegate: Retained<SereinCameraDelegate> = msg_send![super(allocated), init];
			output.setSampleBufferDelegate_queue(
				Some(ProtocolObject::from_ref(&*delegate)),
				Some(&queue),
			);
			let capture = CaptureSession {
				session,
				output,
				_delegate: delegate,
				queue,
			};
			if !shared.stopped.load(Ordering::Acquire) {
				capture.session.startRunning();
			}
			capture
		};
		let result = encode_loop(shared, on_frame, wake, &receive, &mut encoder);
		drop(capture);
		result
	}

	fn encode_loop(
		shared: &Shared,
		on_frame: &Arc<dyn Fn(Frame) + Send + Sync>,
		wake: &Arc<dyn Fn() + Send + Sync>,
		receive: &Receiver<Result<Vec<u8>, &'static str>>,
		encoder: &mut Encoder,
	) -> Result<(), &'static str> {
		let mut last_frame = Instant::now();
		let mut yuv = YUVBuffer::new(WIDTH, HEIGHT);
		while !shared.stopped.load(Ordering::Acquire) {
			let bgra = match receive.recv_timeout(Duration::from_millis(100)) {
				Ok(frame) => frame?,
				Err(mpsc::RecvTimeoutError::Timeout)
					if last_frame.elapsed() < Duration::from_secs(5) =>
				{
					continue;
				}
				_ => {
					return Err("Camera stopped delivering frames; check the device and try again");
				}
			};
			last_frame = Instant::now();
			let mut rgb = vec![0; WIDTH * HEIGHT * 3];
			for (bgra, rgb) in bgra.as_chunks::<4>().0.iter().zip(rgb.as_chunks_mut::<3>().0) {
				rgb.copy_from_slice(&[bgra[2], bgra[1], bgra[0]]);
			}
			yuv.read_rgb8(RgbSliceU8::new(&rgb, (WIDTH, HEIGHT)));
			// ponytail: independently decodable frames tolerate latest-slot drops;
			// add feedback-aware inter frames when bandwidth adaptation is implemented.
			encoder.force_intra_frame();
			let bits = encoder
				.encode(&yuv)
				.map_err(|_| "Camera frame could not be encoded")?;
			if bits.raw_info().iFrameSizeInBytes < 0
				|| bits.raw_info().iFrameSizeInBytes as usize > MAX_ENCODED_BYTES
			{
				return Err("Camera encoded frame exceeded its 128 KiB limit");
			}
			let h264 = bits.to_vec();
			if h264.is_empty() {
				continue;
			}
			if shared.stopped.load(Ordering::Acquire) {
				break;
			}
			on_frame(Frame { rgb, h264 });
			shared.active.store(true, Ordering::Release);
			wake();
		}
		Ok(())
	}
}
