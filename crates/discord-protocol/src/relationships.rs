//! Unofficial normal-user relationship payloads; retain only ID and block status.
use model::Id;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Relationship {
	pub id: Id,
	#[serde(rename = "type")]
	pub kind: u8,
}
#[derive(Deserialize)]
pub struct Snapshot(
	#[serde(deserialize_with = "crate::read_state::entries")] pub Vec<Relationship>,
);
impl Snapshot {
	pub fn entries(self) -> Vec<(Id, bool)> {
		self.0.into_iter().map(|r| (r.id, r.kind == 2)).collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn relationships_decode_only_bounded_typed_account_state() {
		let rows: Snapshot = crate::decode(
			br#"[{"id":"1","type":2,"user":{"username":"ignored"}},{"id":"2","type":1}]"#,
		)
		.unwrap();
		assert_eq!(rows.entries(), vec![(Id(1), true), (Id(2), false)]);
		assert!(crate::decode::<Snapshot>(br#"[{"id":"1"}]"#).is_err());
		let large = format!("[{}]", vec![r#"{"id":"1","type":2}"#; 4001].join(","));
		assert!(crate::decode::<Snapshot>(large.as_bytes()).is_err());
	}
}
