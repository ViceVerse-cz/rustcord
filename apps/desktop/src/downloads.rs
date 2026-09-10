//! Explicit attachment downloads. One job; no credentials, URL logs, or automatic saves.
use model::Attachment;
use std::{
	fs::{self, File, OpenOptions},
	io::Write,
	path::{Path, PathBuf},
	sync::{
		Arc,
		atomic::{AtomicBool, Ordering},
	},
	time::{Duration, Instant},
};
use tokio::sync::{Notify, watch};

const MAX_BYTES: u64 = 100 * 1024 * 1024;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Status {
	#[default]
	Idle,
	Choosing,
	Downloading {
		received: u64,
		total: u64,
	},
	Saved,
	Cancelled,
	Failed(&'static str),
}
struct Job {
	cancelled: Arc<AtomicBool>,
	wake_cancel: Arc<Notify>,
	status: watch::Receiver<Status>,
	done: Arc<AtomicBool>,
}
#[derive(Default)]
pub struct Downloads {
	job: Option<Job>,
	status: Status,
}
impl Downloads {
	pub fn start(
		&mut self,
		attachment: Attachment,
		runtime: &tokio::runtime::Handle,
		context: &eframe::egui::Context,
		parent: Arc<winit::window::Window>,
	) -> Result<(), &'static str> {
		self.poll();
		if self.job.is_some() {
			return Err("A download is already active");
		}
		let Some(url) = original_url(&attachment) else {
			self.status = Status::Failed("Attachment download unavailable");
			return Err("Attachment download unavailable");
		};
		// Construct on the native UI thread; Cocoa reads its application/window here.
		let dialog = platform::save::attachment_destination(parent, &attachment.filename);
		let cancelled = Arc::new(AtomicBool::new(false));
		let wake_cancel = Arc::new(Notify::new());
		let done = Arc::new(AtomicBool::new(false));
		let (send, receive) = watch::channel(Status::Choosing);
		self.status = Status::Choosing;
		self.job = Some(Job {
			cancelled: cancelled.clone(),
			wake_cancel: wake_cancel.clone(),
			status: receive,
			done: done.clone(),
		});
		let runtime = runtime.clone();
		let context = context.clone();
		// One dedicated owner keeps all file work and cleanup away from the render/runtime threads.
		let spawn = std::thread::Builder::new()
			.name("serein-download".into())
			.spawn(move || {
				let publish = |status| {
					send.send_replace(status);
					context.request_repaint();
				};
				let result = runtime.block_on(async {
					// rfd dispatches native work to Cocoa/Windows/the desktop portal. Keep the
					// single slot until the dialog closes even if cancelled: no accumulating dialogs.
					let path = dialog.await;
					if cancelled.load(Ordering::Acquire) {
						return Err("Cancelled");
					}
					let Some(path) = path else {
						return Err("Cancelled");
					};
					let client = reqwest::Client::builder()
						.no_proxy()
						.redirect(reqwest::redirect::Policy::none())
						.connect_timeout(Duration::from_secs(15))
						.read_timeout(Duration::from_secs(30))
						.timeout(Duration::from_secs(300))
						.build()
						.map_err(|_| "Download unavailable")?;
					download(
						&client,
						url,
						&path,
						attachment.size,
						true,
						&cancelled,
						&wake_cancel,
						&publish,
					)
					.await
				});
				publish(match result {
					Ok(()) => Status::Saved,
					Err("Cancelled") => Status::Cancelled,
					Err(error) => Status::Failed(error),
				});
				done.store(true, Ordering::Release);
				context.request_repaint();
			});
		if spawn.is_err() {
			self.job = None;
			self.status = Status::Failed("Download worker unavailable");
			return Err("Download worker unavailable");
		}
		Ok(())
	}
	pub fn poll(&mut self) -> &Status {
		if let Some(job) = &self.job {
			let done = job.done.load(Ordering::Acquire);
			self.status = job.status.borrow().clone();
			if done {
				self.job = None;
			}
		}
		&self.status
	}
	pub fn is_active(&self) -> bool {
		self.job.is_some()
	}
	pub fn cancel(&mut self) {
		if let Some(job) = &self.job {
			job.cancelled.store(true, Ordering::Release);
			job.wake_cancel.notify_one();
		}
	}
}
impl Drop for Downloads {
	fn drop(&mut self) {
		self.cancel();
	}
}

fn original_url(attachment: &Attachment) -> Option<url::Url> {
	if !model::valid_attachments(std::slice::from_ref(attachment))
		|| attachment.size == 0
		|| attachment.size > MAX_BYTES
	{
		return None;
	}
	let url = url::Url::parse(attachment.media.url.as_deref()?).ok()?;
	let path: Vec<_> = url.path_segments()?.collect();
	(url.scheme() == "https"
		&& url.host_str() == Some("cdn.discordapp.com")
		&& url.port_or_known_default() == Some(443)
		&& url.username().is_empty()
		&& url.password().is_none()
		&& url.fragment().is_none()
		&& path.len() == 4
		&& path[0] == "attachments"
		&& path[1].parse::<model::Id>().is_ok()
		&& path[2] == attachment.id.to_string()
		&& !path[3].is_empty()
		&& !path[3].contains('\\')
		&& !["%2f", "%5c"]
			.iter()
			.any(|escape| path[3].to_ascii_lowercase().contains(escape))
		&& url
			.query_pairs()
			.all(|(name, _)| matches!(name.as_ref(), "ex" | "is" | "hm")))
	.then_some(url)
}

struct Partial<'a> {
	cleanup_failed: &'a AtomicBool,
	path: PathBuf,
	file: Option<File>,
}
impl<'a> Partial<'a> {
	fn create(destination: &Path, cleanup_failed: &'a AtomicBool) -> Result<Self, &'static str> {
		let parent = destination.parent().ok_or("Invalid save destination")?;
		let mut random = [0u8; 16];
		getrandom::fill(&mut random).map_err(|_| "Temporary file unavailable")?;
		let random: String = random.iter().map(|b| format!("{b:02x}")).collect();
		let path = parent.join(format!(".serein-{random}.partial"));
		let mut options = OpenOptions::new();
		options.write(true).create_new(true);
		#[cfg(unix)]
		{
			use std::os::unix::fs::OpenOptionsExt;
			options.mode(0o600);
		}
		let file = options
			.open(&path)
			.map_err(|_| "Cannot create download file")?;
		Ok(Self {
			path,
			file: Some(file),
			cleanup_failed,
		})
	}
	fn commit(
		mut self,
		destination: &Path,
		existed: bool,
		cancelled: &AtomicBool,
	) -> Result<(), &'static str> {
		self.file
			.take()
			.expect("partial file")
			.sync_all()
			.map_err(|_| "Could not finish download")?;
		if cancelled.load(Ordering::Acquire) {
			return Err("Cancelled");
		}
		if existed {
			// Native Save dialog explicitly confirms replacement. Rename never truncates
			// the old file on an interrupted network transfer.
			fs::rename(&self.path, destination).map_err(|_| "Could not replace selected file")?;
		} else {
			// Atomic no-clobber publication: a file created meanwhile must survive.
			fs::hard_link(&self.path, destination)
				.map_err(|_| "Destination already exists or cannot be saved atomically")?;
		}
		Ok(())
	}
}
impl Drop for Partial<'_> {
	fn drop(&mut self) {
		self.file.take();
		if let Err(error) = fs::remove_file(&self.path)
			&& error.kind() != std::io::ErrorKind::NotFound
		{
			self.cleanup_failed.store(true, Ordering::Release);
		}
	}
}
#[allow(clippy::too_many_arguments)]
async fn download(
	client: &reqwest::Client,
	url: url::Url,
	destination: &Path,
	expected: u64,
	allow_replace: bool,
	cancelled: &AtomicBool,
	wake_cancel: &Notify,
	publish: &impl Fn(Status),
) -> Result<(), &'static str> {
	if expected == 0 || expected > MAX_BYTES {
		return Err("Attachment must be nonempty and at most 100 MiB");
	}
	let metadata = fs::symlink_metadata(destination);
	let existed = match metadata {
		Ok(meta) if meta.is_file() && allow_replace => true,
		Ok(_) => return Err("Destination exists or is not a regular file"),
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
		Err(_) => return Err("Cannot access save destination"),
	};
	if cancelled.load(Ordering::Acquire) {
		return Err("Cancelled");
	}
	let response = tokio::select! {
		biased;
		_ = wake_cancel.notified() => return Err("Cancelled"),
		response = client.get(url).header(reqwest::header::ACCEPT_ENCODING, "identity").send() => response.map_err(|_| "Attachment download failed")?,
	};
	if response.status() != reqwest::StatusCode::OK {
		return Err("Attachment unavailable; reload the conversation and try again");
	}
	if response
		.headers()
		.get(reqwest::header::CONTENT_ENCODING)
		.is_some_and(|encoding| encoding != "identity")
	{
		return Err("Unexpected attachment encoding; reload the conversation");
	}
	if response
		.content_length()
		.is_some_and(|length| length != expected)
	{
		return Err("Attachment size changed; reload the conversation");
	}
	let mut response = response;
	let cleanup_failed = AtomicBool::new(false);
	let mut partial = Partial::create(destination, &cleanup_failed)?;
	let result = async move {
		let mut received = 0u64;
		let mut repaint = Instant::now();
		publish(Status::Downloading {
			received,
			total: expected,
		});
		loop {
			if cancelled.load(Ordering::Acquire) {
				return Err("Cancelled");
			}
			let chunk = tokio::select! {
				biased;
				_ = wake_cancel.notified() => return Err("Cancelled"),
				chunk = response.chunk() => chunk.map_err(|_| "Attachment transfer interrupted")?,
			};
			let Some(chunk) = chunk else {
				break;
			};
			received = received
				.checked_add(chunk.len() as u64)
				.ok_or("Attachment exceeds download limit")?;
			if received > expected || received > MAX_BYTES {
				return Err("Attachment exceeds download limit");
			}
			// reqwest supplies transport chunks; split disk writes without retaining a file buffer.
			for block in chunk.chunks(32 * 1024) {
				if cancelled.load(Ordering::Acquire) {
					return Err("Cancelled");
				}
				partial
					.file
					.as_mut()
					.expect("partial file")
					.write_all(block)
					.map_err(|_| "Could not write attachment; check available disk space")?;
			}
			if repaint.elapsed() >= Duration::from_millis(100) {
				publish(Status::Downloading {
					received,
					total: expected,
				});
				repaint = Instant::now();
			}
		}
		if received != expected {
			return Err("Attachment transfer incomplete");
		}
		if cancelled.load(Ordering::Acquire) {
			return Err("Cancelled");
		}
		partial.commit(destination, existed, cancelled)
	}
	.await;
	if cleanup_failed.load(Ordering::Acquire) {
		Err(if result.is_ok() {
			"Attachment saved, but its temporary file could not be removed"
		} else {
			"Download stopped, but its temporary file could not be removed"
		})
	} else {
		result
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use tokio::io::{AsyncReadExt, AsyncWriteExt};
	fn attachment() -> Attachment {
		Attachment { id: model::Id(2), filename: "synthetic image.png".into(), description: None,
            content_type: Some("image/png".into()), size: 1024, spoiler: false,
            media: model::EmbedMedia { url: Some("https://cdn.discordapp.com/attachments/1/2/synthetic%20image.png?ex=123&is=123&hm=abc".into()), ..Default::default() } }
	}
	async fn endpoint(
		body: Vec<u8>,
		chunked: bool,
		truncated: bool,
	) -> (url::Url, tokio::task::JoinHandle<()>) {
		let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
		let url = url::Url::parse(&format!(
			"http://{}/attachment",
			listener.local_addr().unwrap()
		))
		.unwrap();
		let task = tokio::spawn(async move {
			let (mut socket, _) = listener.accept().await.unwrap();
			let mut request = Vec::new();
			let mut byte = [0u8; 1];
			while !request.ends_with(b"\r\n\r\n") && request.len() < 8192 {
				socket.read_exact(&mut byte).await.unwrap();
				request.push(byte[0]);
			}
			let headers = String::from_utf8(request).unwrap().to_ascii_lowercase();
			assert!(
				!headers.contains("authorization:") && !headers.contains("proxy-authorization:")
			);
			assert!(headers.contains("accept-encoding: identity"));
			let header = if chunked {
				"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".into()
			} else {
				format!(
					"HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
					body.len() + usize::from(truncated)
				)
			};
			if socket.write_all(header.as_bytes()).await.is_err() {
				return;
			}
			for block in body.chunks(1024) {
				if chunked
					&& socket
						.write_all(format!("{:x}\r\n", block.len()).as_bytes())
						.await
						.is_err()
				{
					return;
				}
				if socket.write_all(block).await.is_err() {
					return;
				}
				if chunked && socket.write_all(b"\r\n").await.is_err() {
					return;
				}
			}
			if chunked {
				let _ = socket.write_all(b"0\r\n\r\n").await;
			}
		});
		(url, task)
	}
	#[tokio::test]
	async fn explicit_download_stream_limits_cancel_and_atomic_replacement() {
		let mut image = attachment();
		assert!(original_url(&image).is_some());
		// Admission depends on a bounded original CDN target, not an image MIME type.
		image.filename = "synthetic file.bin".into();
		image.media.url = Some(
			"https://cdn.discordapp.com/attachments/1/2/synthetic%20file.bin?ex=123&is=123&hm=abc"
				.into(),
		);
		for kind in [
			Some("application/pdf"),
			Some("application/octet-stream"),
			None,
		] {
			image.content_type = kind.map(str::to_owned);
			assert!(!image.is_image());
			assert!(original_url(&image).is_some());
		}
		for size in [0, MAX_BYTES + 1] {
			image.size = size;
			assert!(original_url(&image).is_none());
		}
		image.size = MAX_BYTES;
		assert!(original_url(&image).is_some());
		image.size = 1024;
		for url in [
			"http://cdn.discordapp.com/attachments/1/2/a.png",
			"https://cdn.discordapp.com.evil.test/attachments/1/2/a.png",
			"https://user@cdn.discordapp.com/attachments/1/2/a.png",
			"https://cdn.discordapp.com:444/attachments/1/2/a.png",
			"https://cdn.discordapp.com/attachments/1/99/a.png",
			"https://cdn.discordapp.com/attachments/1/2/a.png?width=1",
			"https://cdn.discordapp.com/attachments/1/2/%2fapi",
			"https://cdn.discordapp.com/attachments/1/2/a.png#fragment",
			"https://127.0.0.1/attachments/1/2/a.png",
			"https://media.discordapp.net/attachments/1/2/a.png",
		] {
			image.media.url = Some(url.into());
			assert!(original_url(&image).is_none());
		}
		let root =
			std::env::temp_dir().join(format!("serein-download-tests-{}", std::process::id()));
		fs::create_dir_all(&root).unwrap();
		let destination = root.join("chosen.bin");
		let client = reqwest::Client::builder()
			.no_proxy()
			.redirect(reqwest::redirect::Policy::none())
			.timeout(Duration::from_secs(5))
			.build()
			.unwrap();
		let cancelled = AtomicBool::new(false);
		let wake = Notify::new();
		// Preserve arbitrary binary bytes (including NUL and invalid UTF-8) without decoding.
		let content: Vec<u8> = (0..=255).cycle().take(64 * 1024).collect();
		let (url, task) = endpoint(content.clone(), false, false).await;
		download(
			&client,
			url,
			&destination,
			content.len() as u64,
			false,
			&cancelled,
			&wake,
			&|_| {},
		)
		.await
		.unwrap();
		task.await.unwrap();
		assert_eq!(fs::read(&destination).unwrap(), content);
		// A failed or oversized stream cannot truncate the existing destination.
		for (chunked, truncated, expected) in
			[(true, false, 1024), (false, true, content.len() as u64 + 1)]
		{
			let (url, task) = endpoint(content.clone(), chunked, truncated).await;
			assert!(
				download(
					&client,
					url,
					&destination,
					expected,
					true,
					&cancelled,
					&wake,
					&|_| {}
				)
				.await
				.is_err()
			);
			task.await.unwrap();
			assert_eq!(fs::read(&destination).unwrap(), content);
			assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
		}
		// Explicit overwrite publishes only the complete new file.
		let replacement = vec![91; 8192];
		let (url, task) = endpoint(replacement.clone(), false, false).await;
		download(
			&client,
			url,
			&destination,
			replacement.len() as u64,
			true,
			&cancelled,
			&wake,
			&|_| {},
		)
		.await
		.unwrap();
		task.await.unwrap();
		assert_eq!(fs::read(&destination).unwrap(), replacement);
		// Cancellation after partial creation leaves the previous file and no partial sibling.
		let (url, task) = endpoint(content.clone(), false, false).await;
		assert_eq!(
			download(
				&client,
				url,
				&destination,
				content.len() as u64,
				true,
				&cancelled,
				&wake,
				&|_| {
					cancelled.store(true, Ordering::Release);
					wake.notify_one();
				}
			)
			.await,
			Err("Cancelled")
		);
		task.await.unwrap();
		assert_eq!(fs::read(&destination).unwrap(), replacement);
		assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
		cancelled.store(false, Ordering::Release);
		let wake = Notify::new();
		// An unrelated file appearing during a new save is never overwritten.
		fs::remove_file(&destination).unwrap();
		let (url, task) = endpoint(content.clone(), false, false).await;
		assert!(
			download(
				&client,
				url,
				&destination,
				content.len() as u64,
				false,
				&cancelled,
				&wake,
				&|_| {
					fs::write(&destination, b"unrelated").unwrap();
				}
			)
			.await
			.is_err()
		);
		task.await.unwrap();
		assert_eq!(fs::read(&destination).unwrap(), b"unrelated");
		assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
		// A stalled HTTP body is interrupted by cancellation, not the long request timeout.
		let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
		let url = url::Url::parse(&format!(
			"http://{}/stalled",
			listener.local_addr().unwrap()
		))
		.unwrap();
		let server = tokio::spawn(async move {
			let (mut socket, _) = listener.accept().await.unwrap();
			let mut request = [0; 8192];
			let _ = socket.read(&mut request).await.unwrap();
			socket
				.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1024\r\n\r\n")
				.await
				.unwrap();
			std::future::pending::<()>().await;
		});
		let cancelled = Arc::new(AtomicBool::new(false));
		let wake = Arc::new(Notify::new());
		let flag = cancelled.clone();
		let notify = wake.clone();
		let cancel = tokio::spawn(async move {
			tokio::time::sleep(Duration::from_millis(50)).await;
			flag.store(true, Ordering::Release);
			notify.notify_one();
		});
		assert_eq!(
			tokio::time::timeout(
				Duration::from_secs(2),
				download(
					&client,
					url,
					&destination,
					1024,
					true,
					&cancelled,
					&wake,
					&|_| {}
				)
			)
			.await
			.unwrap(),
			Err("Cancelled")
		);
		cancel.await.unwrap();
		server.abort();
		assert_eq!(fs::read(&destination).unwrap(), b"unrelated");
		assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
		// Filesystem cleanup failures are signalled rather than silently reported as success.
		let cleanup_failed = AtomicBool::new(false);
		let mut partial = Partial::create(&destination, &cleanup_failed).unwrap();
		let partial_path = partial.path.clone();
		partial.file.take();
		fs::remove_file(&partial_path).unwrap();
		fs::create_dir(&partial_path).unwrap();
		drop(partial);
		assert!(cleanup_failed.load(Ordering::Acquire));
		fs::remove_dir(&partial_path).unwrap();
		fs::remove_dir_all(root).unwrap();
	}
}
