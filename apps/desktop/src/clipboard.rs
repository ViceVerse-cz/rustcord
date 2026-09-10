//! One user-initiated paste job; clipboard contents never enter logs or disk caches.
use discord_api::upload::Source;
use eframe::egui;
use image::ImageEncoder;
use model::Id;
use std::sync::mpsc;

pub enum Content {
	File(Source),
	Text(String),
}

pub struct Paste {
	pub generation: u64,
	pub channel: Id,
	pub target: egui::Id,
	result: mpsc::Receiver<Result<Content, &'static str>>,
}

impl Paste {
	pub fn start(
		generation: u64,
		channel: Id,
		request: ui::AttachmentPaste,
		runtime: &tokio::runtime::Handle,
		context: &egui::Context,
	) -> Self {
		let target = request.target;
		let (send, result) = mpsc::sync_channel(1);
		let context = context.clone();
		runtime.spawn(async move {
			let result = match tokio::task::spawn_blocking(move || read(request)).await {
				Ok(Ok(Read::Path(path))) => Source::inspect(path).await.map(Content::File),
				Ok(Ok(Read::Content(content))) => Ok(content),
				Ok(Err(error)) => Err(error),
				Err(_) => Err("Clipboard reading interrupted; paste again"),
			};
			let _ = send.send(result);
			context.request_repaint();
		});
		Self {
			generation,
			channel,
			target,
			result,
		}
	}

	pub fn poll(&self) -> Option<Result<Content, &'static str>> {
		match self.result.try_recv() {
			Ok(result) => Some(result),
			Err(mpsc::TryRecvError::Empty) => None,
			Err(mpsc::TryRecvError::Disconnected) => Some(Err("Clipboard reading interrupted")),
		}
	}
}

enum Read {
	Path(std::path::PathBuf),
	Content(Content),
}

fn read(request: ui::AttachmentPaste) -> Result<Read, &'static str> {
	let mut clipboard = arboard::Clipboard::new().map_err(|_| "Clipboard unavailable")?;
	match clipboard.get().file_list() {
		Ok(mut paths) if !paths.is_empty() => {
			if paths.len() != 1 {
				return Err("Paste exactly one local file");
			}
			let path = paths.remove(0);
			if !path.is_absolute() || path.as_os_str().as_encoded_bytes().len() > 4096 {
				return Err("Paste a local file with a supported path");
			}
			return Ok(Read::Path(path));
		}
		Err(arboard::Error::ContentNotAvailable) | Ok(_) => {}
		Err(_) => return Err("Could not read copied files; paste again"),
	}
	if let Some(image) = request.image {
		let pixels = image
			.width()
			.checked_mul(image.height())
			.ok_or("Image is too large")?;
		if pixels == 0 || pixels > 4 * 1024 * 1024 || pixels != image.pixels.len() {
			return Err("Paste an image with at most 4 million pixels");
		}
		let bytes: Vec<u8> = image
			.pixels
			.iter()
			.flat_map(|pixel| pixel.to_srgba_unmultiplied())
			.collect();
		return png(&bytes, image.width(), image.height());
	}
	if let Some(text) = request.text.or_else(|| clipboard.get_text().ok()) {
		if text.len() > client_core::MAX_DRAFT_BYTES {
			return Err("Pasted text exceeds the draft limit");
		}
		return Ok(Read::Content(Content::Text(text.replace("\r\n", "\n"))));
	}
	let image = clipboard
		.get_image()
		.map_err(|_| "Copy a file, image, or text before pasting")?;
	png(&image.bytes, image.width, image.height)
}

fn png(bytes: &[u8], width: usize, height: usize) -> Result<Read, &'static str> {
	let pixels = width.checked_mul(height).ok_or("Image is too large")?;
	if pixels == 0 || pixels > 4 * 1024 * 1024 || bytes.len() != pixels * 4 {
		return Err("Paste an image with at most 4 million pixels");
	}
	let mut encoded = Vec::new();
	image::codecs::png::PngEncoder::new(&mut encoded)
		.write_image(
			bytes,
			width as u32,
			height as u32,
			image::ExtendedColorType::Rgba8,
		)
		.map_err(|_| "Could not prepare pasted image")?;
	Source::pasted_png(encoded).map(|source| Read::Content(Content::File(source)))
}

#[cfg(test)]
mod tests {
	#[test]
	fn pasted_images_are_bounded_png_upload_sources() {
		let super::Read::Content(super::Content::File(source)) =
			super::png(&[255, 0, 0, 255], 1, 1).unwrap()
		else {
			panic!("Expected image source")
		};
		assert_eq!(source.filename(), "pasted-image.png");
		assert!(source.size() > 0 && source.size() <= discord_api::upload::MAX_BYTES);
		assert!(super::png(&[], usize::MAX, 2).is_err());
		assert!(super::png(&[], 4096, 4096).is_err());
		assert!(super::png(&[], 1, 1).is_err());
		assert!(super::png(&[], 0, 0).is_err());
	}
}
