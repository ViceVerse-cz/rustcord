//! One explicit attachment selection or upload; paths never enter UI state or diagnostics.
use client_core::Command;
use discord_api::upload::{Source, Status};
use eframe::egui;
use model::Id;
use std::sync::{
	Arc,
	atomic::{AtomicBool, Ordering},
	mpsc,
};
use tokio::sync::watch;

pub struct UploadRequest {
	pub command: Command,
	pub source: Source,
	pub progress: watch::Sender<Status>,
	pub cancel: watch::Sender<bool>,
}

type Selected = (Source, Option<egui::ColorImage>);
struct Choosing {
	result: mpsc::Receiver<Result<Option<Selected>, &'static str>>,
	cancelled: Arc<AtomicBool>,
}
/// Longest edge of the composer thumbnail; the full decode stays bounded by `image::Limits`.
const PREVIEW_EDGE: u32 = 320;
const PREVIEW_ALLOC: u64 = 64 * 1024 * 1024;
fn previewable(filename: &str) -> bool {
	filename.rsplit_once('.').is_some_and(|(_, extension)| {
		matches!(
			extension.to_ascii_lowercase().as_str(),
			"png" | "jpg" | "jpeg" | "gif" | "webp"
		)
	})
}
/// Downscaled pixels for the composer card, decoded on a blocking worker, never in a frame.
async fn preview(source: &Source) -> Option<egui::ColorImage> {
	if !previewable(source.filename()) {
		return None;
	}
	let bytes = source.preview_bytes(discord_api::upload::MAX_BYTES).await?;
	tokio::task::spawn_blocking(move || decode_preview(&bytes))
		.await
		.ok()
		.flatten()
}
fn decode_preview(bytes: &[u8]) -> Option<egui::ColorImage> {
	let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
		.with_guessed_format()
		.ok()?;
	let mut limits = image::Limits::default();
	limits.max_image_width = Some(8192);
	limits.max_image_height = Some(8192);
	limits.max_alloc = Some(PREVIEW_ALLOC);
	reader.limits(limits);
	let image = reader
		.decode()
		.ok()?
		.thumbnail(PREVIEW_EDGE, PREVIEW_EDGE)
		.into_rgba8();
	Some(egui::ColorImage::from_rgba_unmultiplied(
		[image.width() as usize, image.height() as usize],
		image.as_raw(),
	))
}
struct Uploading {
	progress: watch::Receiver<Status>,
	cancel: watch::Sender<bool>,
	cancelling: bool,
}
#[derive(Default)]
pub struct Uploads {
	scope: Option<(u64, Id)>,
	selected: Option<Source>,
	preview: Option<Arc<egui::ColorImage>>,
	previewing: Option<mpsc::Receiver<Option<egui::ColorImage>>>,
	choosing: Option<Choosing>,
	uploading: Option<Uploading>,
	last: Option<Status>,
}
impl Uploads {
	pub fn select_pasted(
		&mut self,
		generation: u64,
		channel: Id,
		source: Source,
		runtime: &tokio::runtime::Handle,
		context: &egui::Context,
	) -> Result<(), &'static str> {
		if self.busy() || self.selected.is_some() {
			return Err("Remove the current attachment or wait for its operation to finish");
		}
		self.scope = Some((generation, channel));
		self.last = None;
		self.preview = None;
		let (send, receive) = mpsc::sync_channel(1);
		let context = context.clone();
		let copy = source.clone();
		runtime.spawn(async move {
			let _ = send.send(preview(&copy).await);
			context.request_repaint();
		});
		self.previewing = Some(receive);
		self.selected = Some(source);
		Ok(())
	}
	pub fn start_choose(
		&mut self,
		generation: u64,
		channel: Id,
		runtime: &tokio::runtime::Handle,
		context: &egui::Context,
		parent: Arc<winit::window::Window>,
	) -> Result<(), &'static str> {
		if self.busy() || self.selected.is_some() {
			return Err("Remove the current attachment or wait for its operation to finish");
		}
		// Construct on the native UI thread; await and inspect outside rendering.
		let dialog = platform::save::attachment_source(parent);
		self.start_selection(generation, channel, runtime, context, dialog);
		Ok(())
	}
	pub fn start_drop(
		&mut self,
		generation: u64,
		channel: Id,
		runtime: &tokio::runtime::Handle,
		context: &egui::Context,
		files: Vec<egui::DroppedFileHandle>,
	) -> Result<(), &'static str> {
		if self.busy() || self.selected.is_some() {
			return Err("Remove the current attachment or wait for its operation to finish");
		}
		if files.len() != 1 {
			return Err("Drop exactly one local file");
		}
		// Native egui handles expose a local path. Never call their whole-file bytes API.
		let path = files[0].path();
		if !path.is_absolute() || path.as_os_str().as_encoded_bytes().len() > 4096 {
			return Err("Drop a local file with a supported path");
		}
		let path = path.to_owned();
		self.start_selection(
			generation,
			channel,
			runtime,
			context,
			async move { Some(path) },
		);
		Ok(())
	}
	fn start_selection(
		&mut self,
		generation: u64,
		channel: Id,
		runtime: &tokio::runtime::Handle,
		context: &egui::Context,
		selection: impl std::future::Future<Output = Option<std::path::PathBuf>> + Send + 'static,
	) {
		let cancelled = Arc::new(AtomicBool::new(false));
		let flag = cancelled.clone();
		let (send, result) = mpsc::sync_channel(1);
		let context = context.clone();
		runtime.spawn(async move {
			let result = async {
				let path = selection.await;
				if flag.load(Ordering::Acquire) {
					return Ok(None);
				}
				match path {
					Some(path) => match Source::inspect(path).await {
						Ok(source) => {
							let thumbnail = preview(&source).await;
							Ok(Some((source, thumbnail)))
						}
						Err(error) => Err(error),
					},
					None => Ok(None),
				}
			}
			.await;
			let _ = send.send(result);
			context.request_repaint();
		});
		self.scope = Some((generation, channel));
		self.last = None;
		self.choosing = Some(Choosing { result, cancelled });
	}
	pub fn revalidate_scope(&mut self, generation: u64, channel: Option<Id>, allowed: bool) {
		if self
			.scope
			.is_some_and(|scope| !allowed || Some(scope) != channel.map(|id| (generation, id)))
		{
			self.remove();
		}
	}
	pub fn poll(
		&mut self,
		generation: u64,
		channel: Option<Id>,
		allowed: bool,
		context: &egui::Context,
	) {
		self.revalidate_scope(generation, channel, allowed);
		if let Some(choosing) = &self.choosing {
			let result = match choosing.result.try_recv() {
				Ok(result) => Some(result),
				Err(mpsc::TryRecvError::Disconnected) => {
					Some(Err("Attachment selection interrupted"))
				}
				Err(mpsc::TryRecvError::Empty) => None,
			};
			if let Some(result) = result {
				let cancelled = choosing.cancelled.load(Ordering::Acquire);
				self.choosing = None;
				if cancelled {
					self.last = Some(Status::Cancelled);
				} else {
					match result {
						Ok(Some((source, thumbnail))) => {
							self.selected = Some(source);
							self.preview = thumbnail.map(Arc::new);
							self.last = None;
						}
						Ok(None) => self.last = Some(Status::Cancelled),
						Err(error) => self.last = Some(Status::Failed(error)),
					}
				}
			}
		}
		if let Some(previewing) = &self.previewing {
			match previewing.try_recv() {
				Ok(thumbnail) => {
					self.preview = thumbnail.filter(|_| self.selected.is_some()).map(Arc::new);
					self.previewing = None;
				}
				Err(mpsc::TryRecvError::Disconnected) => self.previewing = None,
				Err(mpsc::TryRecvError::Empty) => {}
			}
		}
		if let Some(uploading) = &mut self.uploading {
			let status = uploading.progress.borrow_and_update().clone();
			let closed = uploading.progress.has_changed().is_err();
			self.last = Some(if closed {
				match status {
					Status::Sending => Status::Failed(
						"Message outcome unknown; check the conversation before retrying",
					),
					Status::Preparing | Status::Uploading { .. } if uploading.cancelling => {
						Status::Cancelled
					}
					Status::Preparing | Status::Uploading { .. } => {
						Status::Failed("Attachment upload interrupted")
					}
					terminal => terminal,
				}
			} else {
				status
			});
			// Cancellation retains this slot until the actual network worker releases its sender.
			if closed {
				self.uploading = None;
			}
		}
		if self.busy() {
			context.request_repaint_after(std::time::Duration::from_millis(100));
		}
	}
	pub fn selection(&self) -> Option<(&str, u64)> {
		self.selected
			.as_ref()
			.map(|source| (source.filename(), source.size()))
	}
	pub fn preview(&self) -> Option<Arc<egui::ColorImage>> {
		self.selected.as_ref().and(self.preview.clone())
	}
	pub fn busy(&self) -> bool {
		self.choosing.is_some() || self.uploading.is_some()
	}
	pub fn has_unsent(&self) -> bool {
		self.selected.is_some() || self.busy()
	}
	pub fn transfer_progress(&self) -> (Option<(u64, u64)>, bool) {
		match self.last {
			Some(Status::Uploading { sent, total }) => (Some((sent, total)), false),
			Some(Status::Sending | Status::Finished) => (None, true),
			_ => (None, false),
		}
	}
	pub fn status(&self) -> Option<String> {
		if let Some(choosing) = &self.choosing {
			return Some(
				if choosing.cancelled.load(Ordering::Acquire) {
					"Attachment selection cancelled; close any open file chooser"
				} else {
					"Selecting attachment..."
				}
				.into(),
			);
		}
		if self.uploading.as_ref().is_some_and(|job| job.cancelling) {
			return Some("Cancelling upload; a message already sending may still arrive".into());
		}
		self.last.as_ref().map(|status| match status {
			Status::Preparing => "Preparing attachment...".into(),
			Status::Uploading { sent, total } => {
				format!("Uploading attachment: {sent} / {total} bytes")
			}
			Status::Sending => "Sending attachment message...".into(),
			Status::Finished => "Attachment message sent".into(),
			Status::Cancelled => "Attachment upload cancelled".into(),
			Status::Failed(error) => (*error).into(),
		})
	}
	pub fn remove(&mut self) {
		self.selected = None;
		self.preview = None;
		self.previewing = None;
		self.cancel();
	}
	pub fn cancel(&mut self) {
		if let Some(choosing) = &self.choosing {
			choosing.cancelled.store(true, Ordering::Release);
		}
		if let Some(uploading) = &mut self.uploading {
			uploading.cancelling = true;
			uploading.cancel.send_replace(true);
		}
	}
	pub fn take_source(&mut self, generation: u64, channel: Id) -> Option<Source> {
		if self.scope != Some((generation, channel)) || self.busy() {
			return None;
		}
		self.preview = None;
		self.previewing = None;
		self.selected.take()
	}
	pub fn begin_upload(
		&mut self,
		progress: watch::Receiver<Status>,
		cancel: watch::Sender<bool>,
	) -> Result<(), &'static str> {
		if self.busy() || self.selected.is_some() || self.scope.is_none() {
			cancel.send_replace(true);
			return Err("Attachment operation already active or no selection scope");
		}
		self.last = Some(progress.borrow().clone());
		self.uploading = Some(Uploading {
			progress,
			cancel,
			cancelling: false,
		});
		Ok(())
	}
}
impl Drop for Uploads {
	fn drop(&mut self) {
		self.cancel();
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn progress_distinguishes_streamed_bytes_from_message_confirmation() {
		let mut uploads = Uploads::default();
		assert_eq!(uploads.transfer_progress(), (None, false));
		uploads.last = Some(Status::Uploading {
			sent: 42,
			total: 100,
		});
		assert_eq!(uploads.transfer_progress(), (Some((42, 100)), false));
		uploads.last = Some(Status::Sending);
		assert_eq!(uploads.transfer_progress(), (None, true));
		uploads.last = Some(Status::Failed("rejected"));
		assert_eq!(uploads.transfer_progress(), (None, false));
	}
	#[tokio::test]
	async fn file_drop_is_single_scoped_selection_and_never_reads_handle_bytes() {
		struct SyntheticDrop(std::path::PathBuf);
		impl std::fmt::Debug for SyntheticDrop {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				f.write_str("SyntheticDrop([REDACTED])")
			}
		}
		impl egui::DroppedFile for SyntheticDrop {
			fn path(&self) -> &std::path::Path {
				&self.0
			}
			fn bytes(&self) -> Result<Vec<u8>, String> {
				panic!("Drop admission must never read whole-file bytes")
			}
		}
		async fn settle(uploads: &mut Uploads, context: &egui::Context, channel: Id) {
			tokio::time::timeout(std::time::Duration::from_secs(5), async {
				while uploads.busy() {
					uploads.poll(1, Some(channel), true, context);
					tokio::task::yield_now().await;
				}
			})
			.await
			.unwrap();
		}
		let context = egui::Context::default();
		let runtime = tokio::runtime::Handle::current();
		let mut uploads = Uploads::default();
		let path = std::env::temp_dir().join(format!(
			"serein-drop-{}-{}.txt",
			std::process::id(),
			std::time::SystemTime::now()
				.duration_since(std::time::UNIX_EPOCH)
				.unwrap()
				.as_nanos()
		));
		let handle =
			|path: std::path::PathBuf| -> egui::DroppedFileHandle { Arc::new(SyntheticDrop(path)) };
		for files in [
			vec![],
			vec![handle(path.clone()), handle(path.clone())],
			vec![handle(std::path::PathBuf::new())],
			vec![handle("memory-only.txt".into())],
			vec![handle(std::env::temp_dir().join("x".repeat(4097)))],
		] {
			assert!(
				uploads
					.start_drop(1, Id(2), &runtime, &context, files)
					.is_err()
			);
			assert!(!uploads.busy());
		}
		let mut file = tokio::fs::OpenOptions::new()
			.write(true)
			.create_new(true)
			.open(&path)
			.await
			.unwrap();
		tokio::io::AsyncWriteExt::write_all(&mut file, b"synthetic drop")
			.await
			.unwrap();
		tokio::io::AsyncWriteExt::flush(&mut file).await.unwrap();
		drop(file);
		assert!(
			uploads
				.start_drop(1, Id(2), &runtime, &context, vec![handle(path.clone())])
				.is_ok()
		);
		assert!(
			uploads
				.start_drop(1, Id(2), &runtime, &context, vec![handle(path.clone())])
				.is_err()
		);
		settle(&mut uploads, &context, Id(2)).await;
		assert_eq!(
			uploads.selection(),
			Some((path.file_name().unwrap().to_str().unwrap(), 14))
		);
		assert!(
			uploads
				.start_drop(1, Id(2), &runtime, &context, vec![handle(path.clone())])
				.is_err()
		);
		assert!(uploads.selection().is_some());
		assert!(uploads.take_source(1, Id(3)).is_none());
		uploads.remove();
		assert!(
			uploads
				.start_drop(1, Id(2), &runtime, &context, vec![handle(path.clone())])
				.is_ok()
		);
		// A result inspected before or after navigation must never enter the new conversation.
		uploads.poll(1, Some(Id(3)), true, &context);
		settle(&mut uploads, &context, Id(3)).await;
		assert!(uploads.selection().is_none());
		tokio::fs::remove_file(path).await.unwrap();
	}
	#[test]
	fn cancelled_chooser_and_upload_keep_the_single_slot_until_the_worker_finishes() {
		let context = egui::Context::default();
		let (send, result) = mpsc::sync_channel(1);
		let cancelled = Arc::new(AtomicBool::new(false));
		let mut uploads = Uploads {
			scope: Some((1, Id(2))),
			choosing: Some(Choosing {
				result,
				cancelled: cancelled.clone(),
			}),
			selected: None,
			preview: None,
			previewing: None,
			uploading: None,
			last: None,
		};
		uploads.poll(2, Some(Id(2)), true, &context);
		assert!(cancelled.load(Ordering::Acquire));
		assert!(uploads.busy());
		assert!(send.send(Ok(None)).is_ok());
		uploads.poll(2, Some(Id(2)), true, &context);
		assert!(!uploads.busy());
		assert!(uploads.selection().is_none());

		uploads.scope = Some((2, Id(2)));
		let (progress, receive) = watch::channel(Status::Sending);
		let (cancel, cancellation) = watch::channel(false);
		assert!(uploads.begin_upload(receive, cancel).is_ok());
		uploads.poll(2, Some(Id(3)), true, &context);
		assert!(*cancellation.borrow());
		assert!(uploads.busy());
		drop(progress);
		uploads.poll(2, Some(Id(3)), true, &context);
		assert!(!uploads.busy());
		assert!(uploads.status().unwrap().contains("outcome unknown"));

		let (progress, receive) = watch::channel(Status::Preparing);
		let (cancel, _) = watch::channel(false);
		assert!(uploads.begin_upload(receive, cancel).is_ok());
		progress.send_replace(Status::Finished);
		uploads.poll(2, Some(Id(2)), true, &context);
		assert!(uploads.busy());
		drop(progress);
		uploads.poll(2, Some(Id(2)), true, &context);
		assert!(!uploads.busy());
		assert_eq!(uploads.status().as_deref(), Some("Attachment message sent"));
	}
}
