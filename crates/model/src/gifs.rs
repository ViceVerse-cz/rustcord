//! Tenor GIF results relayed by the service: bounded strings, fixed hosts, static previews.

pub const GIF_PAGE_SIZE: usize = 50;
pub const GIF_CATEGORIES: usize = 32;
pub const MAX_GIF_BYTES: usize = 96 * 1024;
pub const MAX_GIF_FAVORITES: usize = 100;
const MAX_URL: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gif {
	pub id: String,
	pub title: String,
	/// Tenor page address; this is the text sent when the GIF is chosen.
	pub url: String,
	/// Static PNG preview on Tenor's media host; nothing animates automatically.
	pub preview: String,
	pub width: u32,
	pub height: u32,
}
impl Gif {
	pub fn bytes(&self) -> usize {
		size_of::<Self>()
			+ self.id.capacity()
			+ self.title.capacity()
			+ self.url.capacity()
			+ self.preview.capacity()
	}
	pub fn valid(&self) -> bool {
		(1..=64).contains(&self.id.len())
			&& self
				.id
				.bytes()
				.all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
			&& self.title.len() <= 256
			&& !self.title.chars().any(char::is_control)
			&& valid_gif_url(&self.url)
			&& valid_gif_preview(&self.preview)
			&& (1..=4096).contains(&self.width)
			&& (1..=4096).contains(&self.height)
	}
}

fn plain_https_path(url: &str, hosts: &[&str]) -> bool {
	url.len() <= MAX_URL
		&& url
			.bytes()
			.all(|b| b.is_ascii_graphic() && b != b'\\' && b != b'?' && b != b'#')
		&& !url.contains("..")
		&& hosts.iter().any(|host| {
			url.strip_prefix("https://")
				.and_then(|rest| rest.strip_prefix(host))
				.and_then(|rest| rest.strip_prefix('/'))
				.is_some_and(|path| !path.is_empty() && !path.starts_with('/'))
		})
}

/// Only Tenor page or media addresses may be sent as a chosen GIF.
pub fn valid_gif_url(url: &str) -> bool {
	plain_https_path(url, &["tenor.com", "media.tenor.com"])
}

/// Previews are static PNG files on Tenor's media host.
pub fn valid_gif_preview(url: &str) -> bool {
	plain_https_path(url, &["media.tenor.com", "c.tenor.com"]) && url.ends_with(".png")
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GifPage {
	pub gifs: Vec<Gif>,
	/// Trending category names; each opens a search for that name.
	pub categories: Vec<String>,
}
impl GifPage {
	pub fn bytes(&self) -> usize {
		self.gifs.capacity() * size_of::<Gif>()
			+ self
				.gifs
				.iter()
				.map(|gif| gif.bytes() - size_of::<Gif>())
				.sum::<usize>()
			+ self.categories.capacity() * size_of::<String>()
			+ self.categories.iter().map(String::capacity).sum::<usize>()
	}
	pub fn valid(&self) -> bool {
		self.gifs.len() <= GIF_PAGE_SIZE
			&& self.categories.len() <= GIF_CATEGORIES
			&& self.bytes() <= MAX_GIF_BYTES
			&& self.gifs.iter().all(Gif::valid)
			&& self
				.gifs
				.iter()
				.enumerate()
				.all(|(i, gif)| self.gifs[..i].iter().all(|other| other.id != gif.id))
			&& self.categories.iter().all(|name| {
				(1..=64).contains(&name.len())
					&& !name.trim().is_empty()
					&& !name.chars().any(char::is_control)
			})
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn gif(id: &str) -> Gif {
		Gif {
			id: id.into(),
			title: "Synthetic wave".into(),
			url: "https://tenor.com/view/synthetic-wave-gif-1".into(),
			preview: "https://media.tenor.com/synthetic/AAAAe/wave.png".into(),
			width: 498,
			height: 280,
		}
	}

	#[test]
	fn gif_hosts_and_pages_are_bounded() {
		assert!(gif("a1").valid());
		assert!(valid_gif_url("https://media.tenor.com/x/tenor.gif"));
		for url in [
			"http://tenor.com/view/x",
			"https://tenor.com.evil.example/view/x",
			"https://tenor.com//x",
			"https://tenor.com/view/x?y=1",
			"https://tenor.com/view/x#frag",
			"https://tenor.com/../x",
			"https://tenor.com/",
			"https://example.com/tenor.com/x",
			"https://tenor.com/view/x y",
		] {
			assert!(!valid_gif_url(url), "{url}");
		}
		assert!(!valid_gif_preview("https://media.tenor.com/x/tenor.gif"));
		assert!(!valid_gif_preview("https://tenor.com/view/x.png"));
		let mut wrong = gif("a1");
		wrong.width = 0;
		assert!(!wrong.valid());
		let mut wrong = gif("a1");
		wrong.title = "bad\u{7}".into();
		assert!(!wrong.valid());
		assert!(!gif("").valid());
		assert!(!gif("a/1").valid());

		let page = GifPage {
			gifs: (0..GIF_PAGE_SIZE).map(|i| gif(&format!("g{i}"))).collect(),
			categories: vec!["happy".into(), "dance".into()],
		};
		assert!(page.valid());
		assert!(page.bytes() <= MAX_GIF_BYTES);
		let mut duplicate = page.clone();
		duplicate.gifs[1].id = "g0".into();
		assert!(!duplicate.valid());
		let mut oversized = page.clone();
		oversized.gifs.push(gif("extra"));
		assert!(!oversized.valid());
		let mut blank = page.clone();
		blank.categories.push("   ".into());
		assert!(!blank.valid());
	}
}
