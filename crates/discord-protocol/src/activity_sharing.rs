//! Unofficial settings-proto/1 status.show_current_game interoperability.
//! Schema: discord-userdoccers/discord-protos discord_users/v1/PreloadedUserSettings.proto.
use crate::{
	DecodeError,
	guild_folders::{fields, integer_wrapper, message},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::Deserialize;

pub use crate::guild_folders::MAX_SETTINGS_RESPONSE;
const MAX_STATUS_BYTES: usize = 16 * 1024;

pub struct Settings {
	pub version: u64,
	pub enabled: bool,
	status_wire: Vec<u8>,
}

pub fn decode_response(bytes: &[u8]) -> Result<Settings, DecodeError> {
	#[derive(Deserialize)]
	struct Response {
		settings: String,
		#[serde(default)]
		out_of_date: bool,
	}
	if bytes.len() > MAX_SETTINGS_RESPONSE {
		return Err(DecodeError);
	}
	let response: Response = serde_json::from_slice(bytes).map_err(|_| DecodeError)?;
	if response.out_of_date {
		return Err(DecodeError);
	}
	let wire = STANDARD
		.decode(response.settings)
		.map_err(|_| DecodeError)?;
	let mut version = None;
	let mut status = None;
	for field in fields(&wire)? {
		match field.number {
			1 => {
				if version.is_some() {
					return Err(DecodeError);
				}
				let mut data_version = None;
				for field in fields(field.message()?)? {
					if field.number == 3 {
						if data_version.is_some() {
							return Err(DecodeError);
						}
						let value = field.integer()?;
						if value > u32::MAX.into() {
							return Err(DecodeError);
						}
						data_version = Some(value);
					}
				}
				version = Some(data_version.unwrap_or_default());
			}
			11 => {
				if status.is_some() {
					return Err(DecodeError);
				}
				let value = field.message()?;
				if value.len() > MAX_STATUS_BYTES {
					return Err(DecodeError);
				}
				status = Some(value);
			}
			_ => {}
		}
	}
	let status = status.unwrap_or_default();
	let mut enabled = None;
	for field in fields(status)? {
		if field.number == 3 {
			if enabled.is_some() {
				return Err(DecodeError);
			}
			let mut value = None;
			for field in fields(field.message()?)? {
				if field.number == 1 {
					if value.is_some() {
						return Err(DecodeError);
					}
					value = Some(match field.integer()? {
						0 => false,
						1 => true,
						_ => return Err(DecodeError),
					});
				}
			}
			enabled = Some(value.unwrap_or(false));
		}
	}
	Ok(Settings {
		version: version.ok_or(DecodeError)?,
		enabled: enabled.unwrap_or(true),
		status_wire: status.into(),
	})
}

/// Replace only this preference; retain status, custom status and unknown status fields.
pub fn encode_patch(current: &Settings, enabled: bool) -> Result<String, DecodeError> {
	let mut status = Vec::new();
	for field in fields(&current.status_wire)? {
		if field.number != 3 {
			status.extend_from_slice(field.raw);
		}
	}
	integer_wrapper(3, u64::from(enabled), &mut status);
	if status.len() > MAX_STATUS_BYTES {
		return Err(DecodeError);
	}
	let mut patch = Vec::new();
	message(11, &status, &mut patch);
	Ok(STANDARD.encode(patch))
}

#[cfg(test)]
mod tests {
	use super::*;
	fn response(wire: &[u8]) -> Vec<u8> {
		serde_json::to_vec(&serde_json::json!({"settings":STANDARD.encode(wire)})).unwrap()
	}
	#[test]
	fn sharing_patch_preserves_status_unknown_fields_and_defaults() {
		let mut status = vec![10, 5, 10, 3, b'd', b'n', b'd'];
		message(2, &[10, 4, b'B', b'u', b's', b'y'], &mut status);
		message(55, b"future", &mut status);
		let preserved = status.clone();
		integer_wrapper(3, 0, &mut status);
		let mut wire = vec![10, 2, 24, 7];
		message(11, &status, &mut wire);
		let current = decode_response(&response(&wire)).unwrap();
		assert_eq!((current.version, current.enabled), (7, false));
		let patch = STANDARD
			.decode(encode_patch(&current, true).unwrap())
			.unwrap();
		let root = fields(&patch).unwrap();
		assert_eq!(root.len(), 1);
		assert_eq!(root[0].number, 11);
		assert!(root[0].message().unwrap().starts_with(&preserved));
		let mut saved = vec![10, 2, 24, 8];
		saved.extend_from_slice(&patch);
		assert!(decode_response(&response(&saved)).unwrap().enabled);
		assert!(decode_response(&response(&[10, 0])).unwrap().enabled);
		assert!(
			!decode_response(&response(&[10, 0, 90, 2, 26, 0]))
				.unwrap()
				.enabled
		);
		for invalid in [
			vec![],
			vec![10, 0, 10, 0],
			vec![10, 0, 90, 4, 26, 2, 8, 2],
			vec![10, 0, 90, 3, 26, 2, 8],
		] {
			assert!(decode_response(&response(&invalid)).is_err());
		}
		assert!(decode_response(&vec![b' '; MAX_SETTINGS_RESPONSE + 1]).is_err());
		let mut oversized = vec![10, 0];
		message(11, &vec![0; MAX_STATUS_BYTES + 1], &mut oversized);
		assert!(decode_response(&response(&oversized)).is_err());
		assert!(decode_response(br#"{"settings":"CgA=","out_of_date":true}"#).is_err());
	}
}
