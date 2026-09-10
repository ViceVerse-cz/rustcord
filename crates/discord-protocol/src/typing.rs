//! Ephemeral typing identity only; optional guild/member payloads are discarded.
use crate::DecodeError;
use model::Id;
use serde::Deserialize;

pub const MAX_WIRE: usize = 16 * 1024;

#[derive(Deserialize)]
pub struct TypingStart {
	pub channel_id: Id,
	pub user_id: Id,
	/// Service timestamp in Unix seconds; freshness belongs to the active session.
	pub timestamp: u64,
}

pub fn decode(bytes: &[u8]) -> Result<TypingStart, DecodeError> {
	if bytes.len() > MAX_WIRE {
		return Err(DecodeError);
	}
	crate::decode(bytes)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn typing_retains_only_bounded_precise_identity_and_timestamp() {
		let wire = br#"{"channel_id":"18446744073709551615","user_id":"2","timestamp":18446744073709551615,"guild_id":"3","member":{"user":{"username":"discarded synthetic profile"},"roles":["4"]}}"#;
		let typing = decode(wire).unwrap();
		assert_eq!(
			(typing.channel_id, typing.user_id, typing.timestamp),
			(Id(u64::MAX), Id(2), u64::MAX)
		);
		let mut padded = wire.to_vec();
		padded.resize(MAX_WIRE, b' ');
		assert!(decode(&padded).is_ok());
		padded.push(b' ');
		assert!(decode(&padded).is_err());
		for invalid in [
			r#"{"channel_id":"0","user_id":"2","timestamp":1}"#,
			r#"{"channel_id":"1","user_id":"0","timestamp":1}"#,
			r#"{"channel_id":1,"user_id":"2","timestamp":1}"#,
			r#"{"channel_id":"1","user_id":"2"}"#,
			r#"{"channel_id":"1","user_id":"2","timestamp":null}"#,
			r#"{"channel_id":"1","user_id":"2","timestamp":-1}"#,
			r#"{"channel_id":"1","user_id":"2","timestamp":1.5}"#,
			r#"{"channel_id":"1","user_id":"2","timestamp":"1"}"#,
			r#"{"channel_id":"1","user_id":"2","timestamp":18446744073709551616}"#,
			"{",
		] {
			assert!(decode(invalid.as_bytes()).is_err());
		}
	}
}
