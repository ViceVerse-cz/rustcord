//! Explicit archived-thread snapshots; member payloads are ignored, never enumerated into state.
use crate::{ChannelDto, Timestamp};
use model::{
	Id,
	archives::{Cursor, Kind, Page},
};
use serde::Deserialize;
pub const MAX_WIRE: usize = 512 * 1024;

#[derive(Deserialize)]
pub struct Reply {
	#[serde(deserialize_with = "crate::search::list::<_,_,25>")]
	threads: Vec<Thread>,
	has_more: bool,
}
#[derive(Deserialize)]
struct Thread {
	#[serde(flatten)]
	channel: ChannelDto,
	thread_metadata: Metadata,
}
#[derive(Deserialize)]
struct Metadata {
	archived: bool,
	archive_timestamp: Timestamp,
}

impl Reply {
	pub fn into_page(
		self,
		parent: Id,
		guild: Id,
		kind: Kind,
		before: Option<Cursor>,
	) -> Result<Page, &'static str> {
		let invalid = "Invalid archived-thread page";
		let mut threads = Vec::with_capacity(self.threads.len());
		let mut previous = None;
		for thread in self.threads {
			if !thread.thread_metadata.archived {
				return Err(invalid);
			}
			let cursor = match kind {
				Kind::Public | Kind::Private => {
					Cursor::Time(thread.thread_metadata.archive_timestamp.0)
				}
				Kind::JoinedPrivate => Cursor::Id(thread.channel.id),
			};
			let before_request = match (cursor, before) {
				(_, None) => true,
				(Cursor::Time(value), Some(Cursor::Time(before))) => value < before,
				(Cursor::Id(value), Some(Cursor::Id(before))) => value < before,
				_ => false,
			};
			let ordered = match (cursor, previous) {
				(_, None) => true,
				(Cursor::Time(value), Some(Cursor::Time(previous))) => value <= previous,
				(Cursor::Id(value), Some(Cursor::Id(previous))) => value < previous,
				_ => false,
			};
			if !before_request || !ordered {
				return Err(invalid);
			}
			previous = Some(cursor);
			threads.push(crate::threads::into_thread(thread.channel, guild).map_err(|_| invalid)?);
		}
		if self.has_more && previous.is_none() {
			return Err(invalid);
		}
		let page = Page {
			threads,
			next: if self.has_more { previous } else { None },
		};
		if !page.valid(parent, guild, kind, before) {
			return Err(invalid);
		}
		Ok(page)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;
	#[test]
	fn archives_validate_scope_kind_metadata_cursor_order_and_capacity() {
		let thread = |id: u64, kind: u8, stamp: &str| json!({"id":id.to_string(),"guild_id":"1","parent_id":"2","type":kind,"name":"Synthetic","thread_metadata":{"archived":true,"archive_timestamp":stamp}});
		let newer = thread(3, 11, "2026-09-10T12:00:00Z");
		let older = thread(9, 11, "2026-09-10T11:00:00.123456789Z");
		let decode = |threads, more| {
			crate::decode::<Reply>(
				&serde_json::to_vec(
					&json!({"threads":threads,"has_more":more,"members":[{"private":"ignored"}]}),
				)
				.unwrap(),
			)
		};
		let page = decode(vec![newer.clone(), older.clone()], true)
			.unwrap()
			.into_page(Id(2), Id(1), Kind::Public, None)
			.unwrap();
		assert_eq!(
			page.threads.iter().map(|t| t.id).collect::<Vec<_>>(),
			vec![Id(3), Id(9)]
		);
		let cursor = page.next.unwrap();
		let Cursor::Time(time) = cursor else { panic!() };
		assert_eq!(
			crate::pins::format_cursor(time).unwrap(),
			"2026-09-10T11:00:00.123456789Z"
		);
		assert!(
			decode(vec![older.clone()], true)
				.unwrap()
				.into_page(Id(2), Id(1), Kind::Public, Some(cursor))
				.is_err()
		);
		assert!(
			decode(vec![older.clone(), newer.clone()], false)
				.unwrap()
				.into_page(Id(2), Id(1), Kind::Public, None)
				.is_err()
		);
		assert!(
			decode(Vec::<serde_json::Value>::new(), true)
				.unwrap()
				.into_page(Id(2), Id(1), Kind::Public, None)
				.is_err()
		);
		assert!(
			decode(Vec::<serde_json::Value>::new(), false)
				.unwrap()
				.into_page(Id(2), Id(1), Kind::Public, Some(cursor))
				.unwrap()
				.next
				.is_none()
		);
		for (key, value) in [
			("guild_id", json!("7")),
			("parent_id", json!("7")),
			("type", json!(12)),
		] {
			let mut invalid = newer.clone();
			invalid[key] = value;
			assert!(
				decode(vec![invalid], false)
					.unwrap()
					.into_page(Id(2), Id(1), Kind::Public, None)
					.is_err()
			);
		}
		for metadata in [
			json!({"archived":false,"archive_timestamp":"2026-09-10T12:00:00Z"}),
			json!({"archived":true}),
			json!({"archived":true,"archive_timestamp":"bad"}),
		] {
			let mut invalid = newer.clone();
			invalid["thread_metadata"] = metadata;
			assert!(
				decode(vec![invalid], false)
					.and_then(|reply| reply
						.into_page(Id(2), Id(1), Kind::Public, None)
						.map_err(|_| crate::DecodeError))
					.is_err()
			);
		}
		assert!(decode(vec![newer.clone(); 26], false).is_err());
		assert!(
			decode(vec![newer.clone(); 2], false)
				.unwrap()
				.into_page(Id(2), Id(1), Kind::Public, None)
				.is_err()
		);
		let joined = vec![
			thread(9, 12, "2026-09-10T11:00:00Z"),
			thread(3, 12, "2026-09-10T12:00:00Z"),
		];
		let page = decode(joined.clone(), true)
			.unwrap()
			.into_page(Id(2), Id(1), Kind::JoinedPrivate, Some(Cursor::Id(Id(10))))
			.unwrap();
		assert_eq!(page.next, Some(Cursor::Id(Id(3))));
		assert!(!page.valid(Id(2), Id(1), Kind::JoinedPrivate, Some(Cursor::Id(Id(3)))));
		assert!(
			decode(joined, false)
				.unwrap()
				.into_page(Id(2), Id(1), Kind::Private, None)
				.is_err()
		);
		let mut excessive = decode(vec![newer], false)
			.unwrap()
			.into_page(Id(2), Id(1), Kind::Public, None)
			.unwrap();
		excessive.threads[0].name = "x".repeat(model::archives::MAX_BYTES + 1);
		assert!(!excessive.valid(Id(2), Id(1), Kind::Public, None));
	}
}
