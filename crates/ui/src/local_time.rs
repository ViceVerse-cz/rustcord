//! Converts UTC instants to the user's local zone for display.
//!
//! `time::UtcOffset::current_local_offset` refuses to run in a multithreaded process on Unix, so
//! the system zone is read once from `TZ` / `/etc/localtime` via `tz-rs`, which also yields the
//! correct DST offset for historical timestamps.
use std::sync::OnceLock;

fn zone() -> &'static tz::TimeZone {
	static ZONE: OnceLock<tz::TimeZone> = OnceLock::new();
	// Tests pin UTC so date-boundary assertions hold on every machine.
	ZONE.get_or_init(|| {
		if cfg!(test) {
			return tz::TimeZone::utc();
		}
		tz::TimeZone::local().unwrap_or_else(|_| tz::TimeZone::utc())
	})
}

/// Shifts `instant` to the local offset in effect at that moment; falls back to UTC.
pub fn local(instant: time::OffsetDateTime) -> time::OffsetDateTime {
	let seconds = zone()
		.find_local_time_type(instant.unix_timestamp())
		.map(|kind| kind.ut_offset())
		.unwrap_or(0);
	time::UtcOffset::from_whole_seconds(seconds)
		.map(|offset| instant.to_offset(offset))
		.unwrap_or(instant)
}

/// Current wall-clock time in the local zone.
pub fn now() -> time::OffsetDateTime {
	local(time::OffsetDateTime::now_utc())
}

#[cfg(test)]
mod tests {
	#[test]
	fn keeps_the_instant() {
		let utc = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
		assert_eq!(super::local(utc).unix_timestamp(), utc.unix_timestamp());
	}
}
