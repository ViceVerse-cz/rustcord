//! Pace callback batches into 20 ms packets without discarding normal speech.
use crate::Frame;
use std::sync::mpsc::Receiver;

#[derive(Default)]
pub(crate) struct CapturePacer {
	// One frame of lookahead absorbs callback/worker jitter. The input channel
	// remains capped at eight frames: at most nine frames / 34,560 PCM bytes total.
	pending: Option<Frame>,
}

impl CapturePacer {
	pub fn next(&mut self, input: &Receiver<Frame>, enabled: bool, stalled: bool) -> Option<Frame> {
		if !enabled || stalled {
			self.pending = None;
			for _ in 0..8 {
				if input.try_recv().is_err() {
					break;
				}
			}
			return None;
		}
		let frame = self.pending.take();
		self.pending = input.try_recv().ok();
		frame
	}
}
