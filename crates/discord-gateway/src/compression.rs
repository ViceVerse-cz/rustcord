use crate::{Failure, Frame, MAX_WIRE};
use flate2::{Decompress, FlushDecompress, Status};

pub(crate) struct Decoder {
	inflater: Decompress,
	pending: Vec<u8>,
}

impl Default for Decoder {
	fn default() -> Self {
		Self {
			inflater: Decompress::new(true),
			pending: Vec::new(),
		}
	}
}
impl Decoder {
	pub fn frame(&mut self, frame: Frame) -> Result<Option<Frame>, Failure> {
		let Frame::Binary(bytes) = frame else {
			if matches!(frame, Frame::Text(_)) && !self.pending.is_empty() {
				return Err(Failure::Protocol);
			}
			return Ok(Some(frame));
		};
		if bytes.len() > MAX_WIRE.saturating_sub(self.pending.len()) {
			return Err(Failure::Capacity);
		}
		self.pending.extend_from_slice(&bytes);
		if !self.pending.ends_with(&[0, 0, 255, 255]) {
			return Ok(None);
		}
		let mut output = Vec::new();
		let mut consumed = 0;
		loop {
			let mut chunk = [0; 16 * 1024];
			let before_in = self.inflater.total_in();
			let before_out = self.inflater.total_out();
			let status = self
				.inflater
				.decompress(&self.pending[consumed..], &mut chunk, FlushDecompress::Sync)
				.map_err(|_| Failure::Protocol)?;
			let read = (self.inflater.total_in() - before_in) as usize;
			let written = (self.inflater.total_out() - before_out) as usize;
			if status == Status::StreamEnd {
				return Err(Failure::Protocol);
			}
			if written > MAX_WIRE.saturating_sub(output.len()) {
				return Err(Failure::Capacity);
			}
			output.extend_from_slice(&chunk[..written]);
			consumed += read;
			if consumed == self.pending.len() && written < chunk.len() {
				break;
			}
			if read == 0 && written == 0 {
				return Err(Failure::Protocol);
			}
		}
		self.pending.clear();
		let text = String::from_utf8(output).map_err(|_| Failure::Protocol)?;
		Ok(Some(Frame::Text(text.into())))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::io::Write;

	#[test]
	fn split_payloads_share_the_dictionary_and_enforce_both_byte_limits() {
		let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
		let mut decoder = Decoder::default();
		let mut offset = 0;
		for text in [
			r#"{"op":10,"d":{"heartbeat_interval":1000}}"#,
			r#"{"op":11,"d":null}"#,
		] {
			encoder.write_all(text.as_bytes()).unwrap();
			encoder.flush().unwrap();
			let bytes = &encoder.get_ref()[offset..];
			let split = bytes.len() - 2;
			assert!(
				decoder
					.frame(Frame::Binary(bytes[..split].to_vec().into()))
					.unwrap()
					.is_none()
			);
			assert_eq!(
				decoder
					.frame(Frame::Binary(bytes[split..].to_vec().into()))
					.unwrap(),
				Some(Frame::Text(text.into()))
			);
			offset = encoder.get_ref().len();
		}
		assert_eq!(
			Decoder::default().frame(Frame::Binary(vec![0; MAX_WIRE + 1].into())),
			Err(Failure::Capacity)
		);
		let mut bomb = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
		bomb.write_all(&vec![b'x'; MAX_WIRE + 1]).unwrap();
		bomb.flush().unwrap();
		assert_eq!(
			Decoder::default().frame(Frame::Binary(bomb.get_ref().clone().into())),
			Err(Failure::Capacity)
		);
	}
}
