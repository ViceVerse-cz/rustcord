//! Display-only activity; never gates or changes transmitted audio.
// ponytail: a -45 dBFS level threshold detects sound, not speech; use VAD if noise lights it up.
pub(crate) fn hold(energy: f32, previous: u8) -> u8 {
	if energy.is_finite() && energy > 960.0 * 0.000_031_623 {
		10 // 200 ms at the transport's 20 ms cadence.
	} else {
		previous.saturating_sub(1)
	}
}
