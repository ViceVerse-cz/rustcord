//! Small Annex-B H.264 packetizer for a single Go Live video stream.
const MTU: usize = 1200;
const RTP_HEADER: usize = 12;
// XChaCha20-Poly1305 tag plus the rtpsize nonce trailer.
const TRANSPORT_OVERHEAD: usize = 20;
const MAX_PAYLOAD: usize = MTU - RTP_HEADER - TRANSPORT_OVERHEAD;
pub(crate) const MAX_FRAGMENTS: usize = 2048;
const MAX_DAVE_FRAME: usize = 2 * 1024 * 1024 + 64 * 1024;

pub(crate) struct Packet {
	pub header: [u8; RTP_HEADER],
	pub payload: Vec<u8>,
}

/// Packetize an already DAVE-encrypted Annex-B H.264 frame as RFC 6184 NAL/FU-A RTP.
pub(crate) fn packetize(
	frame: &[u8],
	sequence: &mut u16,
	timestamp: u32,
	ssrc: u32,
) -> Result<Vec<Packet>, &'static str> {
	if frame.len() > MAX_DAVE_FRAME {
		return Err("DAVE H264 frame exceeds the sharing limit");
	}
	let nalus = nalus(frame)?;
	let mut packets = Vec::new();
	for (index, nalu) in nalus.iter().enumerate() {
		if nalu.is_empty() {
			continue;
		}
		let last_nalu = index + 1 == nalus.len();
		if nalu.len() <= MAX_PAYLOAD {
			push(
				&mut packets,
				sequence,
				timestamp,
				ssrc,
				nalu.to_vec(),
				last_nalu,
			)?;
			continue;
		}
		if nalu.len() < 2 {
			return Err("Invalid H264 NAL unit");
		}
		let indicator = (nalu[0] & 0xe0) | 28;
		let kind = nalu[0] & 0x1f;
		let mut offset = 1;
		while offset < nalu.len() {
			let take = (nalu.len() - offset).min(MAX_PAYLOAD - 2);
			let mut payload = Vec::with_capacity(take + 2);
			payload.push(indicator);
			payload.push(
				kind | if offset == 1 { 0x80 } else { 0 }
					| if offset + take == nalu.len() { 0x40 } else { 0 },
			);
			payload.extend_from_slice(&nalu[offset..offset + take]);
			let last = last_nalu && offset + take == nalu.len();
			push(&mut packets, sequence, timestamp, ssrc, payload, last)?;
			offset += take;
		}
	}
	if packets.is_empty() {
		return Err("H264 frame has no NAL units");
	}
	Ok(packets)
}

pub(crate) fn validate_source(frame: &[u8]) -> Result<(), &'static str> {
	if frame.len() > 2 * 1024 * 1024 {
		return Err("H264 frame exceeds the sharing limit");
	}
	nalus(frame).map(|_| ())
}

fn push(
	packets: &mut Vec<Packet>,
	sequence: &mut u16,
	timestamp: u32,
	ssrc: u32,
	payload: Vec<u8>,
	marker: bool,
) -> Result<(), &'static str> {
	if packets.len() == MAX_FRAGMENTS {
		return Err("H264 frame exceeds fragment limit");
	}
	let mut header = [0; RTP_HEADER];
	header[0] = 0x80;
	header[1] = 101 | u8::from(marker) << 7;
	header[2..4].copy_from_slice(&sequence.to_be_bytes());
	header[4..8].copy_from_slice(&timestamp.to_be_bytes());
	header[8..12].copy_from_slice(&ssrc.to_be_bytes());
	*sequence = sequence.wrapping_add(1);
	packets.push(Packet { header, payload });
	Ok(())
}

fn nalus(frame: &[u8]) -> Result<Vec<&[u8]>, &'static str> {
	let mut starts = Vec::new();
	let mut at = 0;
	while at + 3 <= frame.len() {
		let size = if frame[at..].starts_with(&[0, 0, 0, 1]) {
			4
		} else if frame[at..].starts_with(&[0, 0, 1]) {
			3
		} else {
			at += 1;
			continue;
		};
		if starts.len() == MAX_FRAGMENTS {
			return Err("H264 frame exceeds fragment limit");
		}
		starts.push((at, size));
		at += size;
	}
	if starts.is_empty() || starts[0].0 != 0 {
		return Err("H264 frame is not Annex-B");
	}
	let mut out = Vec::with_capacity(starts.len());
	for (index, (start, size)) in starts.iter().enumerate() {
		let end = starts.get(index + 1).map_or(frame.len(), |next| next.0);
		if start + size >= end {
			return Err("H264 frame contains an empty NAL unit");
		}
		out.push(&frame[start + size..end]);
	}
	Ok(out)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::{crypto::Dave, test_mls::Delivery};

	fn depacketize(packets: &[Packet]) -> Vec<u8> {
		let mut frame = Vec::new();
		let mut fragmented = false;
		for packet in packets {
			if packet.payload[0] & 0x1f == 28 {
				if packet.payload[1] & 0x80 != 0 {
					assert!(!fragmented);
					frame.extend([0, 0, 0, 1]);
					frame.push((packet.payload[0] & 0xe0) | (packet.payload[1] & 0x1f));
					fragmented = true;
				} else {
					assert!(fragmented);
				}
				frame.extend_from_slice(&packet.payload[2..]);
				if packet.payload[1] & 0x40 != 0 {
					fragmented = false;
				}
			} else {
				assert!(!fragmented);
				frame.extend([0, 0, 0, 1]);
				frame.extend_from_slice(&packet.payload);
			}
		}
		assert!(!fragmented);
		frame
	}
	#[test]
	fn fragments_annex_b_h264_with_a_final_marker() {
		assert!(validate_source(&[0, 0, 0, 1, 0x65, 1, 0, 0, 1]).is_err());
		let mut frame = vec![0, 0, 0, 1, 0x65];
		frame.resize(frame.len() + MAX_PAYLOAD * 2, 7);
		frame.extend([0, 0, 1, 0x41, 9]);
		let mut sequence = 7;
		let packets = packetize(&frame, &mut sequence, 90_000, 11).unwrap();
		assert!(packets.len() > 2 && packets.len() <= MAX_FRAGMENTS);
		assert!(
			packets[..packets.len() - 1]
				.iter()
				.all(|packet| packet.header[1] & 0x80 == 0)
		);
		assert_eq!(packets.last().unwrap().header[1], 101 | 0x80);
		assert!(
			packets
				.iter()
				.all(
					|packet| packet.header.len() + packet.payload.len() + TRANSPORT_OVERHEAD <= MTU
				)
		);
		assert_eq!(packets[0].payload[0] & 0x1f, 28);
		assert_ne!(packets[0].payload[1] & 0x80, 0);
		let mut rebuilt = Vec::new();
		for packet in packets
			.iter()
			.take_while(|packet| packet.payload[0] & 0x1f == 28)
		{
			if packet.payload[1] & 0x80 != 0 {
				rebuilt.push((packet.payload[0] & 0xe0) | (packet.payload[1] & 0x1f));
			}
			rebuilt.extend_from_slice(&packet.payload[2..]);
		}
		assert_eq!(rebuilt, frame[4..5 + MAX_PAYLOAD * 2]);
	}
	#[test]
	fn dave_h264_ciphertext_packetizes_without_extra_start_codes() {
		let server = Delivery::new();
		let mut alice = Dave::new(1, Some(2), 3).unwrap();
		let mut bob = Dave::new(2, Some(1), 3).unwrap();
		alice.session.set_external_sender(&server.external).unwrap();
		bob.session.set_external_sender(&server.external).unwrap();
		let (commit, welcome) = server.add(&mut alice, &bob.key_package().unwrap());
		let mut committed = vec![0, 5];
		committed.extend(commit);
		let mut welcomed = vec![0, 5];
		welcomed.extend(welcome);
		assert_eq!(alice.group_changed(29, &committed).unwrap(), 5);
		assert_eq!(bob.group_changed(30, &welcomed).unwrap(), 5);
		alice.execute(5).unwrap();
		bob.execute(5).unwrap();

		let mut original = vec![0, 0, 0, 1, 0x67, 0x64, 0, 0x1f, 0xac, 0xd9, 0x40];
		original.extend([0, 0, 0, 1, 0x65, 0x88]);
		original.resize(original.len() + MAX_PAYLOAD * 2, 7);
		let encrypted = alice
			.session
			.encrypt(davey::MediaType::VIDEO, davey::Codec::H264, &original)
			.unwrap()
			.into_owned();
		assert_eq!(nalus(&encrypted).unwrap().len(), 2);
		let mut sequence = 0;
		let packets = packetize(&encrypted, &mut sequence, 90_000, 11).unwrap();
		let restored = depacketize(&packets);
		assert_eq!(restored, encrypted);
		assert_eq!(
			bob.session
				.decrypt(1, davey::MediaType::VIDEO, &restored)
				.unwrap(),
			original
		);
	}
}
