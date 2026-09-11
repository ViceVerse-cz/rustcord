//! Bounded group icon encoding shared by the native picker and HTTP boundary.
use base64::{Engine as _, engine::general_purpose::STANDARD};

pub const MAX_GROUP_ICON_PNG_BYTES: usize = 256 * 1024;
pub const MAX_ICON_DATA_URI: usize = 22 + 4 * MAX_GROUP_ICON_PNG_BYTES.div_ceil(3);

fn valid_png(png: &[u8]) -> bool {
	png.len() >= 33
		&& png.len() <= MAX_GROUP_ICON_PNG_BYTES
		&& png.starts_with(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR")
		&& [16, 20].into_iter().all(|offset| {
			(1..=256).contains(&u32::from_be_bytes(
				png[offset..offset + 4].try_into().unwrap(),
			))
		})
}

pub fn icon_data_uri(png: &[u8]) -> Option<String> {
	valid_png(png).then(|| {
		let mut uri = format!("data:image/png;base64,{}", STANDARD.encode(png));
		uri.shrink_to_fit();
		uri
	})
}

pub fn valid_icon_data_uri(uri: &str) -> bool {
	uri.len() <= MAX_ICON_DATA_URI
		&& uri
			.strip_prefix("data:image/png;base64,")
			.is_some_and(|data| STANDARD.decode(data).is_ok_and(|png| valid_png(&png)))
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn group_icon_encoding_is_bounded_and_requires_png_dimensions() {
		let mut png =
			b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x80\0\0\0\x80\0\0\0\0\0\0\0\0\0".to_vec();
		let uri = icon_data_uri(&png).unwrap();
		assert!(valid_icon_data_uri(&uri));
		assert!(!valid_icon_data_uri("data:image/png;base64,bad!"));
		png[18] = 2;
		assert!(icon_data_uri(&png).is_none());
		assert!(icon_data_uri(&vec![0; MAX_GROUP_ICON_PNG_BYTES + 1]).is_none());
	}
}
