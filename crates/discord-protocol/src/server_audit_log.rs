//! Documented audit-log responses projected to bounded, read-only display values.
use crate::{DecodeError, UserDto, permissions::List};
use model::{
	Id, Patch,
	server_audit_log::{self as m, Change, Detail, Entry, Page, Query},
};
use serde::Deserialize;
use serde_json::Value;
pub const MAX_WIRE: usize = 2 * 1024 * 1024;
#[derive(Deserialize)]
struct WireChange {
	key: String,
	#[serde(default)]
	old_value: Patch<Value>,
	#[serde(default)]
	new_value: Patch<Value>,
}
#[derive(Deserialize)]
struct WireEntry {
	id: Id,
	user_id: Option<Id>,
	target_id: Option<String>,
	action_type: u16,
	reason: Option<String>,
	#[serde(default)]
	changes: List<WireChange, { m::MAX_CHANGES }>,
	options: Option<WireOptions>,
}
#[derive(Deserialize)]
struct WireOptions {
	application_id: Option<Id>,
	auto_moderation_rule_name: Option<String>,
	auto_moderation_rule_trigger_type: Option<String>,
	channel_id: Option<Id>,
	count: Option<String>,
	delete_member_days: Option<String>,
	id: Option<Id>,
	members_removed: Option<String>,
	message_id: Option<Id>,
	role_name: Option<String>,
	#[serde(rename = "type")]
	kind: Option<String>,
	integration_type: Option<String>,
	status: Option<String>,
}
impl WireOptions {
	fn into_details(self) -> Vec<Detail> {
		[
			(
				"application_id",
				self.application_id.map(|id| id.to_string()),
			),
			("auto_moderation_rule_name", self.auto_moderation_rule_name),
			(
				"auto_moderation_rule_trigger_type",
				self.auto_moderation_rule_trigger_type,
			),
			("channel_id", self.channel_id.map(|id| id.to_string())),
			("count", self.count),
			("delete_member_days", self.delete_member_days),
			("id", self.id.map(|id| id.to_string())),
			("members_removed", self.members_removed),
			("message_id", self.message_id.map(|id| id.to_string())),
			("role_name", self.role_name),
			("type", self.kind),
			("integration_type", self.integration_type),
			("status", self.status),
		]
		.into_iter()
		.filter_map(|(key, value)| {
			value.map(|value| Detail {
				key: key.into(),
				value,
			})
		})
		.collect()
	}
}
impl WireEntry {
	fn checked(self) -> Result<Entry, DecodeError> {
		let entry = Entry {
			id: self.id,
			user_id: self.user_id,
			target_id: self.target_id,
			action_type: self.action_type,
			reason: self.reason,
			changes: self
				.changes
				.0
				.into_iter()
				.map(|change| {
					if change.key.len() > 128 {
						return Err(DecodeError);
					}
					Ok(Change {
						old: display_patch(
							&change.key,
							change.old_value,
							(50..=52).contains(&self.action_type),
						)?,
						new: display_patch(
							&change.key,
							change.new_value,
							(50..=52).contains(&self.action_type),
						)?,
						key: change.key,
					})
				})
				.collect::<Result<_, DecodeError>>()?,
			options: self
				.options
				.map_or_else(Vec::new, WireOptions::into_details),
		};
		if !entry.valid() {
			return Err(DecodeError);
		}
		Ok(entry)
	}
}
pub fn page(bytes: &[u8], guild: Id, query: &Query) -> Result<Page, DecodeError> {
	#[derive(Deserialize)]
	struct Response {
		audit_log_entries: List<WireEntry, { m::PAGE_SIZE }>,
		users: List<UserDto, 100>,
	}
	if bytes.len() > MAX_WIRE || guild.0 == 0 || !query.valid() {
		return Err(DecodeError);
	}
	let response: Response = crate::decode(bytes)?;
	if response.users.0.iter().any(|user| {
		user.username.len() > 512
			|| user
				.global_name
				.as_ref()
				.is_some_and(|name| name.len() > 512)
			|| user
				.avatar
				.as_ref()
				.is_some_and(|avatar| avatar.len() > 128)
			|| user.discriminator.len() > 4
	}) {
		return Err(DecodeError);
	}
	let has_more = response.audit_log_entries.0.len() == m::PAGE_SIZE;
	let page = Page {
		guild,
		entries: response
			.audit_log_entries
			.0
			.into_iter()
			.map(WireEntry::checked)
			.collect::<Result<_, _>>()?,
		users: response
			.users
			.0
			.into_iter()
			.map(UserDto::into_model)
			.collect(),
		has_more,
	};
	if !page.valid_response() || !page.matches_query(query) {
		return Err(DecodeError);
	}
	Ok(page)
}
fn display_patch(
	key: &str,
	value: Patch<Value>,
	webhook: bool,
) -> Result<Patch<String>, DecodeError> {
	Ok(match value {
		Patch::Absent => Patch::Absent,
		Patch::Null => Patch::Null,
		Patch::Value(value) => {
			let mut output = String::new();
			if sensitive(key) || (webhook && key.eq_ignore_ascii_case("url")) {
				output.push_str("[redacted]");
			} else {
				display_value(&value, &mut output, 0, webhook)?;
			}
			Patch::Value(output)
		}
	})
}
fn sensitive(key: &str) -> bool {
	matches!(
		key.to_ascii_lowercase().as_str(),
		"token"
			| "access_token"
			| "refresh_token"
			| "authorization"
			| "secret"
			| "client_secret"
			| "password"
			| "webhook_token"
	)
}
fn append(output: &mut String, text: &str) -> Result<(), DecodeError> {
	if output.len() + text.len() > m::MAX_VALUE_BYTES {
		return Err(DecodeError);
	}
	output.push_str(text);
	Ok(())
}
fn display_value(
	value: &Value,
	output: &mut String,
	depth: usize,
	webhook: bool,
) -> Result<(), DecodeError> {
	if depth > 4 {
		return Err(DecodeError);
	}
	match value {
		Value::Null => append(output, "null"),
		Value::Bool(value) => append(output, if *value { "true" } else { "false" }),
		Value::Number(value) => append(output, &value.to_string()),
		Value::String(value) => append(
			output,
			if value.contains("/webhooks/") {
				"[redacted]"
			} else {
				value
			},
		),
		Value::Array(values) => {
			if values.len() > 32 {
				return Err(DecodeError);
			}
			append(output, "[")?;
			for (index, value) in values.iter().enumerate() {
				if index > 0 {
					append(output, ", ")?;
				}
				display_value(value, output, depth + 1, webhook)?;
			}
			append(output, "]")
		}
		Value::Object(values) => {
			if values.len() > 32 {
				return Err(DecodeError);
			}
			append(output, "(")?;
			for (index, (key, value)) in values.iter().enumerate() {
				if key.len() > 128 {
					return Err(DecodeError);
				}
				if index > 0 {
					append(output, ", ")?;
				}
				append(output, key)?;
				append(output, ": ")?;
				if sensitive(key) || (webhook && key.eq_ignore_ascii_case("url")) {
					append(output, "[redacted]")?;
				} else {
					display_value(value, output, depth + 1, webhook)?;
				}
			}
			append(output, ")")
		}
	}
}
#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;
	#[test]
	fn audit_log_redacts_nested_credentials_without_hiding_ordinary_urls() {
		let value = Patch::Value(
			json!({"token":"SYNTHETIC_TOKEN","nested":{"authorization":"SYNTHETIC_AUTH","url":"https://discord.com/api/webhooks/1/SYNTHETIC_URL"},"avatar_url":"https://cdn.discordapp.com/avatars/1/example.png"}),
		);
		let Patch::Value(display) = display_patch("future", value, false).unwrap() else {
			panic!()
		};
		assert!(!display.contains("SYNTHETIC"));
		assert!(display.contains("https://cdn.discordapp.com/avatars/1/example.png"));
		assert_eq!(
			display_patch("url", Patch::Value(json!("SYNTHETIC_URL")), true).unwrap(),
			Patch::Value("[redacted]".into())
		);
	}
	#[test]
	fn audit_log_changes_preserve_absent_null_roles_and_filters() {
		let bytes=serde_json::to_vec(&json!({"audit_log_entries":[{"id":"100","user_id":"2","target_id":"invite-code","action_type":65000,"reason":"Reviewed by owner","changes":[{"key":"name","old_value":"old","new_value":null},{"key":"topic","new_value":"fresh"},{"key":"$add","new_value":[{"id":"3","name":"Member"}]},{"key":"token","new_value":"SYNTHETIC_SECRET"}],"options":{"channel_id":"4","count":"2"}}],"users":[{"id":"2","username":"Owner"}],"webhooks":[{"token":"SYNTHETIC_IGNORED_SECRET"}]})).unwrap();
		let query = Query {
			user: Some(Id(2)),
			action: Some(65000),
			before: Some(Id(101)),
		};
		let parsed = page(&bytes, Id(9), &query).unwrap();
		let entry = &parsed.entries[0];
		assert_eq!(entry.target_id.as_deref(), Some("invite-code"));
		assert_eq!(entry.changes[0].old, Patch::Value("old".into()));
		assert_eq!(entry.changes[0].new, Patch::Null);
		assert_eq!(entry.changes[1].old, Patch::Absent);
		assert_eq!(
			entry.changes[2].new,
			Patch::Value("[(id: 3, name: Member)]".into())
		);
		assert_eq!(entry.changes[3].new, Patch::Value("[redacted]".into()));
		assert_eq!(entry.options[0].value, "4");
		assert!(!parsed.has_more);
		for query in [
			Query {
				before: Some(Id(100)),
				..query.clone()
			},
			Query {
				user: Some(Id(3)),
				..query.clone()
			},
			Query {
				action: Some(1),
				..query
			},
		] {
			assert!(page(&bytes, Id(9), &query).is_err());
		}
	}
	#[test]
	fn audit_log_page_limits_order_and_budget() {
		let entry =
			|id: u64| json!({"id":id.to_string(),"user_id":null,"target_id":null,"action_type":1});
		let body = |entries: Vec<Value>| {
			serde_json::to_vec(&json!({"audit_log_entries":entries,"users":[]})).unwrap()
		};
		let parsed = page(
			&body((1..=50).rev().map(entry).collect()),
			Id(9),
			&Query::default(),
		)
		.unwrap();
		assert!(parsed.has_more);
		assert!(
			page(
				&body((1..=51).rev().map(entry).collect()),
				Id(9),
				&Query::default()
			)
			.is_err()
		);
		assert!(page(&body(vec![entry(2), entry(2)]), Id(9), &Query::default()).is_err());
		assert!(page(&body(vec![entry(1), entry(2)]), Id(9), &Query::default()).is_err());
		let mut oversized = entry(1);
		oversized["changes"] = json!([{"key":"name","new_value":"x".repeat(m::MAX_VALUE_BYTES+1)}]);
		assert!(page(&body(vec![oversized]), Id(9), &Query::default()).is_err());
		let mut deep = entry(1);
		deep["changes"] = json!([{"key":"future","new_value":[[[[[[1]]]]]]}]);
		assert!(page(&body(vec![deep]), Id(9), &Query::default()).is_err());
		assert!(page(&vec![b' '; MAX_WIRE + 1], Id(9), &Query::default()).is_err());
		let mut parsed = parsed;
		parsed.entries.reserve(m::MAX_BYTES);
		let result = model::server_admin::Result::AuditLog(parsed);
		assert!(result.bytes() > m::MAX_BYTES);
		assert!(!result.valid());
	}
}
