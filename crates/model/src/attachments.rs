//! Bounded attachment metadata. Image bytes use the shared client media cache.
use crate::{EmbedMedia, Id};
use serde::{Deserialize, Serialize};

pub const MAX_ATTACHMENTS: usize = 10;
pub const MAX_ATTACHMENT_BYTES: usize = 64 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Attachment {
	pub id: Id,
	pub filename: String,
	pub description: Option<String>,
	pub content_type: Option<String>,
	pub size: u64,
	pub media: EmbedMedia,
	pub spoiler: bool,
}
impl Attachment {
	pub fn is_audio(&self) -> bool {
		match self.content_type.as_deref().map(|kind| {
			kind.split(';')
				.next()
				.unwrap_or(kind)
				.trim()
				.to_ascii_lowercase()
		}) {
			Some(kind) if kind != "application/octet-stream" => matches!(
				kind.as_str(),
				"audio/mpeg" | "audio/mp3" | "audio/wav" | "audio/x-wav" | "audio/wave"
			),
			_ => self
				.filename
				.rsplit_once('.')
				.is_some_and(|(_, extension)| {
					matches!(extension.to_ascii_lowercase().as_str(), "mp3" | "wav")
				}),
		}
	}
	pub fn bytes(&self) -> usize {
		size_of::<Self>()
			+ self.filename.capacity()
			+ self.description.as_ref().map_or(0, String::capacity)
			+ self.content_type.as_ref().map_or(0, String::capacity)
			+ self.media.url.as_ref().map_or(0, String::capacity)
			+ self.media.proxy_url.as_ref().map_or(0, String::capacity)
	}
	pub fn is_image(&self) -> bool {
		if let Some(kind) = &self.content_type {
			matches!(
				kind.to_ascii_lowercase().as_str(),
				"image/png" | "image/jpeg" | "image/webp" | "image/gif" | "image/avif"
			)
		} else {
			self.filename
				.rsplit_once('.')
				.is_some_and(|(_, extension)| {
					matches!(
						extension.to_ascii_lowercase().as_str(),
						"png" | "jpg" | "jpeg" | "webp" | "gif" | "avif"
					)
				})
		}
	}
}
pub fn attachment_bytes(attachments: &[Attachment]) -> usize {
	attachments.iter().map(Attachment::bytes).sum()
}
pub fn valid_attachments(attachments: &[Attachment]) -> bool {
	attachments.len() <= MAX_ATTACHMENTS
		&& attachment_bytes(attachments) <= MAX_ATTACHMENT_BYTES
		&& attachments.iter().all(|a| {
			a.filename.len() <= 1024
				&& a.description.as_ref().is_none_or(|s| s.len() <= 4096)
				&& a.content_type.as_ref().is_none_or(|s| s.len() <= 128)
				&& [&a.media.url, &a.media.proxy_url]
					.into_iter()
					.all(|s| s.as_ref().is_none_or(|s| s.len() <= 2048))
		})
}

#[cfg(test)]
mod audio_tests {
	use super::*;
	#[test]
	fn audio_detection_uses_mime_and_safe_filename_fallback() {
		let mut file = Attachment {
			id: Id(1),
			filename: "TRACK.MP3".into(),
			description: None,
			content_type: None,
			size: 32,
			media: EmbedMedia::default(),
			spoiler: false,
		};
		assert!(file.is_audio());
		file.content_type = Some("application/octet-stream".into());
		assert!(file.is_audio());
		file.content_type = Some("text/plain".into());
		assert!(!file.is_audio());
		file.content_type = Some("Audio/Wav; codec=pcm".into());
		assert!(file.is_audio());
		file.content_type = Some("audio/ogg".into());
		assert!(!file.is_audio());
		file.content_type = None;
		file.filename = "track.mp3.exe".into();
		assert!(!file.is_audio());
	}
}
#[derive(Default)]
pub struct AttachmentList(pub Vec<Attachment>);
impl<'de> Deserialize<'de> for AttachmentList {
	fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		struct Visitor;
		impl<'de> serde::de::Visitor<'de> for Visitor {
			type Value = AttachmentList;
			fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				f.write_str("at most ten attachments")
			}
			fn visit_seq<A: serde::de::SeqAccess<'de>>(
				self,
				mut sequence: A,
			) -> Result<Self::Value, A::Error> {
				let mut items = Vec::new();
				for _ in 0..MAX_ATTACHMENTS {
					match sequence.next_element()? {
						Some(item) => items.push(item),
						None => return Ok(AttachmentList(items)),
					}
				}
				if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
					return Err(serde::de::Error::custom("attachment limit"));
				}
				Ok(AttachmentList(items))
			}
		}
		deserializer.deserialize_seq(Visitor)
	}
}
